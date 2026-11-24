// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! A collection contains one discoverable Stanza library. An independent library uses
//! another collection. Passwords live in the native keychain, identity beside the journal.
mod profile;
use crate::{App, res, storage::Library};
use day::prelude::*;
use day_persistence::{DbError, Sqlite, Value};
use day_sync::{Error, Identity, Replica, Space as SyncSpace, Store, assets};
use day_sync_webdav::{
    WebDavStore,
    webdav::{Authentication, Client, Endpoint},
};
use profile::{Field, LibraryProfile, PROFILE, rows};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::Arc,
};

type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Serialize, Deserialize)]
struct Account {
    endpoint: String,
    username: String,
    identity: Identity,
}
#[derive(Clone, Copy, Default, PartialEq)]
enum Phase {
    #[default]
    Idle,
    Working,
    Ready,
    Failed,
    Credentials,
    Capacity,
}
thread_local! {
    static LIBRARY: RefCell<Option<Library>> = const {RefCell::new(None)};
    static ACCOUNT: Signal<Option<Account>> = Signal::global(None);
    static STATUS: Signal<Phase> = Signal::global(Phase::Idle);
    static RETRY: std::cell::Cell<Option<std::time::Instant>> = const {std::cell::Cell::new(None)};
    static REPLICA: RefCell<Option<Replica>> = const {RefCell::new(None)};
    static RUNNING: std::cell::Cell<bool> = const {std::cell::Cell::new(false)};
}
static DOWNLOAD: async_lock::Mutex<()> = async_lock::Mutex::new(());
fn setting() -> Signal<Option<Account>> {
    ACCOUNT.with(|a| *a)
}
fn status() -> Signal<Phase> {
    STATUS.with(|s| *s)
}
fn library() -> Result<Library> {
    LIBRARY.with(|l| l.borrow().clone()).ok_or(Error::Integrity)
}
fn invalid() -> DbError {
    DbError::driver("stanza.sync.invalid_configuration")
}
fn io(e: std::io::Error) -> Error {
    DbError::driver(e.to_string()).into()
}
fn account(db: &Library) -> Result<Option<Account>> {
    Ok(db.db.try_with_connection(|c|{
        c.execute_batch("CREATE TABLE IF NOT EXISTS stanza_sync_account(id INTEGER PRIMARY KEY CHECK(id=1),config TEXT NOT NULL)")?;
        rows(c,"SELECT config FROM stanza_sync_account",&[])?.first().map(|r|serde_json::from_str(r[0].as_text()?).map_err(|_|invalid())).transpose()
    })?)
}
fn path(db: &Library) -> Result<PathBuf> {
    Ok(db.db.try_with_connection(|c| {
        let r = rows(
            c,
            "SELECT file FROM pragma_database_list WHERE name='main'",
            &[],
        )?;
        Ok(PathBuf::from(r.first().ok_or_else(invalid)?[0].as_text()?))
    })?)
}
fn client(url: &str, username: String, password: String) -> Result<WebDavStore> {
    let endpoint = Endpoint::loopback(url).map_err(|_| invalid())?;
    Ok(WebDavStore::new(
        Client::builder(endpoint)
            .authentication(Authentication::Basic { username, password })
            .build(),
    ))
}
fn credential(a: &Account) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "stanza.webdav.{}.{:x}",
        a.identity.actor(),
        Sha256::digest(a.username.as_bytes())
    )
}
fn store(a: &Account) -> Result<WebDavStore> {
    let secret = day_part_keychain::get(&credential(a)).map_err(|_| invalid())?;
    client(
        &a.endpoint,
        a.username.clone(),
        secret.as_str().ok_or_else(invalid)?.to_owned(),
    )
}
async fn descriptor(store: &dyn Store, existing: Option<&Identity>) -> Result<SyncSpace> {
    let dir = ["stanza-redux-v1".into()];
    let name = "library.json";
    if existing.is_none() {
        store.ensure_directory(&dir).await?;
    }
    let bytes = match store.get(&dir, name).await? {
        Some(bytes) => bytes,
        None if existing.is_none() => {
            let new = SyncSpace::new(PROFILE)?;
            let outcome = store.create(&dir, name, new.to_bytes()?).await;
            match store.get(&dir, name).await? {
                Some(bytes) => bytes,
                None => {
                    outcome?;
                    return Err(Error::Integrity);
                }
            }
        }
        None => return Err(Error::WrongSpace),
    };
    let space = SyncSpace::from_bytes(&bytes)?;
    if space.profile != PROFILE || existing.is_some_and(|i| i.space != space) {
        return Err(Error::WrongSpace);
    }
    Ok(space)
}
async fn connect(url: String, username: String, password: String) -> Result<()> {
    let db = library()?;
    let previous = account(&db)?;
    let dav = client(&url, username.clone(), password.clone())?;
    dav.check_writable_collection().await?;
    let endpoint = dav.client().endpoint().as_str().to_owned();
    if previous.as_ref().is_some_and(|a| a.endpoint != endpoint) {
        return Err(Error::WrongEndpoint);
    }
    let space = descriptor(&dav, previous.as_ref().map(|a| &a.identity)).await?;
    let identity = match &previous {
        Some(a) => a.identity.clone(),
        None => Identity::join(space)?,
    };
    let a = Account {
        identity,
        username,
        endpoint,
    };
    let encoded = serde_json::to_string(&a).map_err(|_| invalid())?;
    day_part_keychain::set(
        &credential(&a),
        &password.into(),
        day_part_keychain::Protection::WhenUnlocked,
    )
    .map_err(|_| invalid())?;
    db.db.try_with_connection(|c| {
        if previous.is_none() {
            c.execute(
                "INSERT INTO stanza_sync_account VALUES(1,?)",
                &[Value::Text(encoded)],
            )?;
        } else {
            c.execute(
                "UPDATE stanza_sync_account SET config=? WHERE id=1",
                &[Value::Text(encoded)],
            )?;
        }
        Ok(())
    })?;
    setting().set(Some(a));
    Ok(())
}
async fn replica(db: &Library, a: &Account) -> Result<Replica> {
    if let Some(replica) = REPLICA.with(|r| r.borrow().clone()) {
        return Ok(replica);
    }
    let replica = open(&path(db)?, &a.identity).await?;
    REPLICA.with(|r| *r.borrow_mut() = Some(replica.clone()));
    Ok(replica)
}
async fn open(path: &Path, identity: &Identity) -> Result<Replica> {
    Replica::open_application(
        path,
        Arc::new(identity.clone()),
        Arc::new(|_| Ok(())),
        Some(Arc::new(LibraryProfile)),
        |p| Ok(Library::with_driver(Sqlite::at(p))?.db),
    )
    .await
}

async fn metadata(replica: &Replica, store: &dyn Store) -> Result<()> {
    // Drain edits in a local-origin transaction before each import. An edit made between
    // drain and import is protected by the adapter's in-transaction before_import check.
    let mut races = 0;
    loop {
        replica.worker().write(|_| Ok(())).await?;
        let report = match replica.sync(store).await {
            Err(Error::Database(e))
                if e.kind == day_persistence::DbErrorKind::Busy && races < 3 =>
            {
                races += 1;
                continue;
            }
            other => other?,
        };
        if !report.uploads_deferred && !report.downloads_deferred && report.pending_deletions == 0 {
            return Ok(());
        }
    }
}
fn books_directory() -> Result<PathBuf> {
    Ok(day_part_filesystem::data_dir()
        .map_err(|_| invalid())?
        .join("day-fs/books"))
}
async fn upload_book(r: &Replica, dav: &dyn Store, book: &str, root: &Path) -> Result<()> {
    if !profile::book_id(book) {
        return Err(Error::InvalidWire);
    }
    let id = Value::Text(book.into());
    let selected = id.clone();
    let source = r
        .worker()
        .read(move |db| {
            db.try_with_connection(|c| {
                let query = "SELECT v.body,coalesce(m.json,'{}') FROM stanza_sync_uploads q
                     LEFT JOIN stanza_sync_values v ON v.book=q.book AND v.field='file'
                     LEFT JOIN book_metadata m ON m.book=q.book WHERE q.book=?";
                Ok(rows(c, query, &[selected])?.into_iter().next())
            })
        })
        .await?;
    let Some(source) = source else { return Ok(()) };
    let details = source[1].as_text()?.to_owned();
    let receipts = root.join(format!("{book}.upload.db"));
    let asset = match &source[0] {
        Value::Null => {
            let file = std::fs::File::open(root.join(format!("{book}.epub"))).map_err(io)?;
            let journal = assets::UploadJournal::open(&receipts)?;
            assets::upload_with_journal(dav, r.space(), file, Some(&journal)).await?
        }
        body => match serde_json::from_str(body.as_text()?).map_err(|_| Error::InvalidWire)? {
            Field::File { epub, .. } => epub,
            _ => return Err(Error::InvalidWire),
        },
    };
    if asset.sha256 != book {
        return Err(Error::Integrity);
    }
    if details.len() as u64 > crate::epub::RESOURCE_LIMIT {
        return Err(Error::Limit);
    }
    let metadata = assets::upload(dav, r.space(), details.as_bytes()).await?;
    let body = serde_json::to_string(&Field::File {
        epub: asset,
        metadata,
    })
    .map_err(|_| Error::InvalidWire)?;
    r.worker()
        .write(move |db| {
            db.try_with_connection(|c| {
                // A deletion or supplemental edit may have arrived while the network was busy.
                let publish = "INSERT INTO stanza_sync_pending
                           SELECT 'file',q.book,'',? FROM stanza_sync_uploads q
                           LEFT JOIN book_metadata m ON m.book=q.book
                           WHERE q.book=? AND coalesce(m.json,'{}')=?
                           ON CONFLICT DO UPDATE SET body=excluded.body";
                if c.execute(
                    publish,
                    &[Value::Text(body), id.clone(), Value::Text(details)],
                )? != 0
                {
                    c.execute("DELETE FROM stanza_sync_uploads WHERE book=?", &[id])?;
                }
                Ok(())
            })
        })
        .await?;
    // The durable publication and outbox no longer depend on this local transfer cache.
    let _ = std::fs::remove_file(receipts);
    Ok(())
}
/// Apply portable catalog metadata (including OPDS supplements) even on devices that
/// only download selected EPUBs. The receipt and the derived author/genre folders commit
/// together. A later metadata reference supersedes an in-flight download.
async fn descriptions(r: &Replica, dav: &dyn Store, scratch: &Path) -> Result<()> {
    let waiting = r.worker().read(|db| {
        let query = "SELECT v.book,v.body FROM stanza_sync_values v
                     JOIN books b ON b.id=v.book
                     LEFT JOIN stanza_sync_descriptions d ON d.book=v.book
                     WHERE v.field='file' AND d.hash IS NOT json_extract(v.body,'$.File.metadata.sha256')
                       AND NOT EXISTS(SELECT 1 FROM stanza_sync_uploads q WHERE q.book=v.book)";
        db.try_with_connection(|c| rows(c, query, &[]))
    }).await?;
    let mut failure = None;
    for row in waiting {
        let book = row[0].as_text()?.to_owned();
        let body = row[1].as_text()?.to_owned();
        if let Err(e) = fetch_description(r, dav, book, body, scratch).await {
            failure = Some(e);
        }
    }
    failure.map_or(Ok(()), Err)
}

async fn fetch_description(
    r: &Replica,
    dav: &dyn Store,
    book: String,
    expected: String,
    scratch: &Path,
) -> Result<()> {
    let Field::File { metadata, .. } =
        serde_json::from_str(&expected).map_err(|_| Error::InvalidWire)?
    else {
        return Err(Error::InvalidWire);
    };
    std::fs::create_dir_all(scratch).map_err(io)?;
    let mut stage = tempfile::tempfile_in(scratch).map_err(io)?;
    assets::download(dav, r.space(), &metadata, &mut stage).await?;
    std::io::Seek::rewind(&mut stage).map_err(io)?;
    let description = serde_json::from_reader(stage).map_err(|_| Error::InvalidWire)?;
    r.worker()
        .write(move |db| {
            let lookup = "SELECT 1 FROM stanza_sync_values v JOIN books ON books.id=v.book
                      WHERE v.book=? AND v.field='file' AND v.body=?
                        AND NOT EXISTS(SELECT 1 FROM stanza_sync_uploads q WHERE q.book=v.book)";
            let current = db.try_with_connection(|c| {
                rows(
                    c,
                    lookup,
                    &[Value::Text(book.clone()), Value::Text(expected)],
                )
            })?;
            if current.is_empty() {
                return Ok(());
            }
            db.try_with_connection(|c| {
                c.execute_batch("UPDATE stanza_sync_control SET projecting=1")
            })?;
            Library { db: db.clone() }.save_metadata(&book, &description)?;
            db.try_with_connection(|c| {
                c.execute_batch("UPDATE stanza_sync_control SET projecting=0")?;
                let receipt = "INSERT INTO stanza_sync_descriptions VALUES(?,?)
                           ON CONFLICT(book) DO UPDATE SET hash=excluded.hash";
                c.execute(receipt, &[Value::Text(book), Value::Text(metadata.sha256)])?;
                Ok(())
            })
        })
        .await?;
    Ok(())
}
async fn materialize(
    db: &Library,
    dav: &dyn Store,
    space: &str,
    book: &str,
    root: &Path,
    repair: bool,
) -> Result<()> {
    if !profile::book_id(book) {
        return Err(Error::InvalidWire);
    }
    let _guard = DOWNLOAD.lock().await;
    let path = root.join(format!("{book}.epub"));
    if !repair && path.is_file() {
        return Ok(());
    }
    let asset = db.db.try_with_connection(|c| {
        let lookup =
            "SELECT body FROM stanza_sync_values JOIN books ON books.id=stanza_sync_values.book
                    WHERE book=? AND field='file'";
        let values = rows(c, lookup, &[Value::Text(book.into())])?;
        let value = values.first().ok_or_else(invalid)?[0].as_text()?;
        match serde_json::from_str(value) {
            Ok(Field::File { epub, .. }) => Ok(epub),
            _ => Err(invalid()),
        }
    })?;
    if asset.sha256 != book {
        return Err(Error::Integrity);
    }
    std::fs::create_dir_all(root).map_err(io)?;
    let partial = path.with_extension("part");
    let mut stage = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&partial)
        .map_err(io)?;
    stage.try_lock().map_err(|_| Error::Busy)?;
    assets::download(dav, space, &asset, &mut stage).await?;
    drop(stage);
    if db.try_book(book)?.is_none() {
        return Err(Error::WrongSpace);
    }
    std::fs::rename(partial, path).map_err(io)?;
    #[cfg(unix)]
    std::fs::File::open(root)
        .and_then(|dir| dir.sync_all())
        .map_err(io)?;
    Ok(())
}
pub(crate) async fn ensure_book(book: &str) -> Result<()> {
    if !profile::book_id(book) {
        return Err(Error::InvalidWire);
    }
    let root = books_directory()?;
    if setting().get_untracked().is_none() {
        return Ok(());
    }
    let a = setting().get_untracked().ok_or(Error::WrongSpace)?;
    let path = root.join(format!("{book}.epub"));
    let db = library()?;
    let repair = path.is_file();
    let local_verified = if repair {
        crate::covers::background(move || -> Result<String> {
            use sha2::{Digest, Sha256};
            let mut file = std::fs::File::open(path).map_err(io)?;
            let mut digest = Sha256::new();
            std::io::copy(&mut file, &mut digest).map_err(io)?;
            Ok(format!("{:x}", digest.finalize()))
        })
        .await
        .map_err(|_| Error::Integrity)??
            == book
    } else {
        false
    };
    if !local_verified {
        materialize(&db, &store(&a)?, a.identity.space(), book, &root, repair).await?;
    }
    if !local_verified
        || db
            .try_book(book)?
            .is_some_and(|b| b.indexed != crate::storage::SEARCH_INDEX_VERSION)
    {
        enrich(&db, book).await?;
    }
    Ok(())
}
/// Ask only at open, so synchronization cannot jump a reader who is already on a page.
pub(crate) async fn choose_position(book: &str) -> Result<()> {
    let Some(replica) = REPLICA.with(|r| r.borrow().clone()) else {
        return Ok(());
    };
    let key = day_sync::RecordKey {
        model: PROFILE.into(),
        key: book.into(),
        field: "position".into(),
    };
    let id = key.change::<Field>(None)?.id;
    let choices = replica.candidates(id.clone()).await?;
    if choices.len() < 2 {
        return Ok(());
    }
    let mut locations = std::collections::BTreeMap::new();
    for (i, candidate) in choices.iter().enumerate() {
        if let Some(v) = &candidate.value
            && let (
                _,
                Field::Position {
                    chapter,
                    millionths,
                },
            ) = day_sync::RecordKey::decode(&id, v)?
        {
            locations.entry((chapter, millionths)).or_insert(i);
        }
    }
    if locations.len() > 1 {
        let mut prompt = Alert::new(res::str::dav_choose_position());
        for ((chapter, millionths), i) in locations {
            prompt = prompt.button(
                res::str::dav_location(res::str::DavLocation {
                    chapter: i64::from(chapter) + 1,
                    percent: day::format_percent(f64::from(millionths) / 1e6, 1),
                }),
                i,
            );
        }
        if let Some(i) = prompt.cancel(res::str::cancel()).present().await {
            replica.choose_candidate(choices, i).await?;
            library()?.check_external()?;
        }
    }
    Ok(())
}
async fn enrich(db: &Library, id: &str) -> Result<()> {
    let mut book = crate::epub::open_saved(id)
        .await
        .map_err(|_| Error::Integrity)?;
    if let Some(cover) = &book.cover {
        let data = book.resource(cover).map_err(|_| Error::Integrity)?;
        let path = format!("books/{id}.cover");
        day_part_filesystem::write_future(&path, data.to_vec())
            .await
            .map_err(|_| Error::Integrity)?;
        if let Some(row) = db.db.try_get::<crate::storage::BookRecord>(id.to_owned())? {
            use crate::storage::BookRecordFields;
            row.cover().write(Some(path));
        }
    }
    if let Some(saved) = db.metadata(id) {
        book.metadata.supplement(&saved);
    }
    crate::storage::index_book(db, &book)
        .await
        .map_err(|_| Error::Integrity)
}
// Keep the metadata channel responsive during a slow EPUB transfer. Polling a pinned
// future by reference preserves its HTTP request and verified staging data between ticks.
async fn during_transfer<T>(
    job: impl std::future::Future<Output = Result<T>>,
    r: &Replica,
    dav: &dyn Store,
    db: &Library,
) -> Result<T> {
    let mut job = std::pin::pin!(job);
    loop {
        let completed = futures_lite::future::race(async { Some((&mut job).await) }, async {
            day::sleep(15_000).await;
            None
        })
        .await;
        if let Some(result) = completed {
            return result;
        }
        metadata(r, dav).await?;
        db.check_external()?;
    }
}
async fn pass() -> Result<()> {
    let db = library()?;
    let a = setting().get_untracked().ok_or(Error::WrongSpace)?;
    let r = replica(&db, &a).await?;
    let dav = store(&a)?;
    descriptor(&dav, Some(&a.identity)).await?;
    metadata(&r, &dav).await?;
    db.check_external()?;
    let ids = db.db.try_with_connection(|c| {
        rows(c, "SELECT book FROM stanza_sync_uploads ORDER BY book", &[])
    })?;
    let root = books_directory()?;
    let mut failure = None;
    for row in ids {
        if let Err(e) = during_transfer(
            upload_book(&r, &dav, row[0].as_text()?, &root),
            &r,
            &dav,
            &db,
        )
        .await
        {
            failure = Some(e)
        } else {
            metadata(&r, &dav).await?;
        }
    }
    if let Err(e) = descriptions(&r, &dav, &root).await {
        failure = Some(e);
    }
    db.check_external()?;
    if pinned() {
        for book in db.try_books()? {
            if !root.join(format!("{}.epub", book.id)).is_file()
                && let Err(error) = during_transfer(ensure_book(&book.id), &r, &dav, &db).await
            {
                failure = Some(error);
            }
        }
    }
    db.check_external()?;
    failure.map_or(Ok(()), Err)
}
fn pinned() -> bool {
    day::prefs::get("stanza.dav.offline").as_deref() == Some("1")
}
fn finish(result: Result<()>) {
    use day_sync::StoreErrorKind as Kind;
    let retry = if let Err(Error::Store(e)) = &result {
        e.retry_after
            .and_then(|delay| std::time::Instant::now().checked_add(delay))
    } else {
        None
    };
    RETRY.with(|cell| cell.set(retry));
    status().set(match &result {
        Ok(()) => Phase::Ready,
        Err(Error::Store(e)) if matches!(e.kind, Kind::Unauthorized | Kind::Forbidden) => {
            Phase::Credentials
        }
        Err(Error::Store(e)) if e.kind == Kind::InsufficientStorage => Phase::Capacity,
        Err(_) => Phase::Failed,
    });
    if let Err(Error::Database(error)) = &result {
        day::debug!(
            "Stanza WebDAV database ({:?}): {}",
            error.kind,
            error.message
        );
    }
    if let Err(error) = result {
        day::warn!("Stanza WebDAV sync: {error}");
    }
}
fn sync_now() {
    if status().get_untracked() == Phase::Working || setting().get_untracked().is_none() {
        return;
    }
    status().set(Phase::Working);
    day::task(async { finish(pass().await) });
}
pub(crate) fn start(app: App) {
    let Some(db) = app.database.get_untracked() else {
        return;
    };
    LIBRARY.with(|l| *l.borrow_mut() = Some(db.clone()));
    match account(&db) {
        Ok(a) => setting().set(a),
        Err(e) => {
            finish(Err(e));
            return;
        }
    }
    if RUNNING.with(|r| r.replace(true)) {
        return;
    }
    day::task(async {
        let mut delay = 30_000u32;
        loop {
            if !matches!(
                status().get_untracked(),
                Phase::Credentials | Phase::Capacity
            ) && RETRY.with(|r| r.get().is_none_or(|t| std::time::Instant::now() >= t))
            {
                sync_now();
            }
            day::sleep(delay).await;
            delay = if status().get_untracked() == Phase::Failed {
                delay.saturating_mul(2).min(900_000)
            } else {
                30_000
            };
        }
    });
}
pub(crate) fn settings() -> impl Piece {
    let account = setting();
    let current = account.get_untracked();
    let configured = move || account.get().is_some();
    let busy = || status().get() == Phase::Working;
    let pin = Signal::new(pinned());
    watch(
        move || pin.get(),
        move |on, _| {
            day::prefs::set("stanza.dav.offline", if *on { "1" } else { "0" });
        },
    );
    column((
        day_piece_webdav_settings::account_form(
            current
                .as_ref()
                .map(|a| a.endpoint.clone())
                .unwrap_or_default(),
            current.map(|a| a.username).unwrap_or_default(),
            configured,
            busy,
            day_piece_webdav_settings::Labels {
                endpoint: res::str::dav_endpoint(),
                username: res::str::dav_username(),
                password: res::str::dav_password(),
                connect: res::str::dav_connect(),
                update: res::str::dav_connect(),
            },
            |url, username, password| {
                status().set(Phase::Working);
                day::task(async move {
                    match connect(url, username, password).await {
                        Ok(()) => finish(pass().await),
                        Err(e) => finish(Err(e)),
                    }
                });
            },
        ),
        label(move || {
            match status().get() {
                Phase::Idle => res::str::dav_help(),
                Phase::Working => res::str::loading(),
                Phase::Ready => res::str::dav_ready(),
                Phase::Failed => res::str::dav_failed(),
                Phase::Credentials => res::str::dav_credentials(),
                Phase::Capacity => res::str::dav_capacity(),
            }
            .format()
        })
        .id(day::id!(dav_status)),
        when(configured, move || {
            column((
                button(res::str::dav_sync_now())
                    .enabled(move || !busy())
                    .action(sync_now)
                    .id(day::id!(dav_sync_now)),
                labeled(
                    res::str::dav_offline(),
                    toggle(pin).id(day::id!(dav_offline)),
                ),
            ))
        }),
    ))
    .spacing(Space::M)
    .grow(Dimension::Width)
}

#[cfg(test)]
mod tests;
