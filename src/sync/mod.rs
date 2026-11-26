// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! A collection contains one discoverable Stanza library. An independent library uses
//! another collection. Passwords live in the native keychain, identity beside the journal.
mod deletion;
mod diagnostics;
mod profile;
mod progress;
mod schedule;
use crate::{App, res, storage::Library};
use day::prelude::*;
use day_persistence::{DbError, Sqlite, Value};
use day_sync::{Error, Identity, Replica, Space as SyncSpace, Store, assets};
use day_sync_webdav::{
    WebDavStore,
    webdav::{Authentication, Client, Endpoint, Policy},
};
use diagnostics::{Failure, Stage};
use profile::{Field, LibraryProfile, PROFILE, rows};
use progress::{Direction, Work};
pub(crate) use schedule::{before_open, changed, flush};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::Arc,
};

type Result<T> = std::result::Result<T, Error>;
type Attempt = std::result::Result<(), Failure>;
#[derive(Clone, Serialize, Deserialize)]
struct Account {
    #[serde(default)]
    grouped: bool,
    #[serde(default)]
    folder: Option<String>,
    endpoint: String,
    username: String,
    identity: Identity,
}
#[derive(Clone, Default, PartialEq)]
enum Phase {
    #[default]
    Idle,
    Working,
    Ready,
    Failed(Failure),
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
static METADATA: async_lock::Mutex<()> = async_lock::Mutex::new(());
static OPEN: async_lock::Mutex<()> = async_lock::Mutex::new(());
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
fn client(
    url: &str,
    username: String,
    password: String,
    policy: Policy,
) -> std::result::Result<WebDavStore, day_sync_webdav::webdav::Error> {
    let endpoint = Endpoint::loopback(url)?;
    Ok(WebDavStore::new(
        Client::builder(endpoint)
            .policy(policy)
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
fn store(a: &Account) -> std::result::Result<WebDavStore, Failure> {
    let secret = day_part_keychain::get(&credential(a)).map_err(|_| Failure::keychain())?;
    let dav = client(
        &a.endpoint,
        a.username.clone(),
        secret.as_str().ok_or_else(Failure::keychain)?.to_owned(),
        Policy::default(),
    )
    .map_err(|e| Failure::dav(Stage::Address, &e))?;
    layout(dav, a.grouped).map_err(|e| Failure::sync(Stage::Library, &e))
}
fn layout(store: WebDavStore, grouped: bool) -> Result<WebDavStore> {
    if grouped {
        Ok(store.with_private_directory(".sync", "books")?)
    } else {
        Ok(store)
    }
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
async fn connect(url: String, folder: String, username: String, password: String) -> Attempt {
    let local = |e: Error| Failure::sync(Stage::Account, &e);
    let db = library().map_err(local)?;
    let previous = account(&db).map_err(local)?;
    // The explicit account check exchanges a probe and the library descriptor only.
    // The subsequent library pass opens its own client with the usual media policy.
    let policy = Policy {
        allow_expensive: true,
        allow_constrained: true,
        ..Policy::default()
    };
    let dav = client(&url, username.clone(), password.clone(), policy)
        .map_err(|e| Failure::dav(Stage::Address, &e))?;
    let endpoint = dav.client().endpoint().as_str().to_owned();
    if previous.as_ref().is_some_and(|a| a.endpoint != endpoint) {
        return Err(local(Error::WrongEndpoint));
    }
    dav.client()
        .check_app_folder(previous.is_none() && !folder.is_empty())
        .await
        .map_err(|failure| Failure::dav(Stage::Probe(failure.step), &failure.source))?;
    progress::set(Work::Opening);
    let grouped = match &previous {
        Some(account) => account.grouped,
        None => dav
            .get(&["stanza-redux-v1".into()], "library.json")
            .await
            .map_err(|e| Failure::sync(Stage::Library, &e.into()))?
            .is_none(),
    };
    let dav = layout(dav, grouped).map_err(local)?;
    let space = descriptor(&dav, previous.as_ref().map(|a| &a.identity))
        .await
        .map_err(|e| Failure::sync(Stage::Library, &e))?;
    let identity = match &previous {
        Some(a) => a.identity.clone(),
        None => Identity::join(space).map_err(local)?,
    };
    let a = Account {
        grouped,
        folder: Some(folder),
        identity,
        username,
        endpoint,
    };
    let encoded = serde_json::to_string(&a).map_err(|_| local(invalid().into()))?;
    day_part_keychain::set(
        &credential(&a),
        &password.into(),
        day_part_keychain::Protection::WhenUnlocked,
    )
    .map_err(|_| Failure::keychain())?;
    db.db
        .try_with_connection(|c| {
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
        })
        .map_err(|e| local(e.into()))?;
    setting().set(Some(a));
    Ok(())
}
async fn replica(db: &Library, a: &Account) -> Result<Replica> {
    let _guard = OPEN.lock().await;
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
    let _guard = METADATA.lock().await;
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
            deletion::after_exchange(replica, store).await?;
            return Ok(());
        }
    }
}
pub(crate) async fn offers_server_delete(book: &str) -> Result<bool> {
    deletion::available(book).await
}
pub(crate) async fn delete_book(book: &str, server: bool) -> Result<()> {
    deletion::remove(book, server).await
}
fn books_directory() -> Result<PathBuf> {
    crate::shelf::root().map_err(io)
}
/// A case-sensitive server must assign names a case-insensitive device can also store.
async fn claim_name(dav: &dyn Store, name: String, book: &str) -> Result<Option<String>> {
    use sha2::{Digest, Sha256};
    fn folded(name: &str) -> String {
        let nfc = icu_normalizer::ComposingNormalizer::new_nfc();
        let normalized = nfc.normalize(name);
        nfc.normalize(&icu_casemap::CaseMapper::new().fold_string(&normalized))
            .into_owned()
    }
    if !profile::book_id(book) || !crate::shelf::valid(&name) {
        return Err(Error::InvalidWire);
    }
    let key = folded(&name);
    let filename = format!("{:x}.json", Sha256::digest(key.as_bytes()));
    let dir = ["stanza-redux-v1".into(), "filenames".into()];
    dav.ensure_directory(&dir).await?;
    let encoded = serde_json::to_vec(&(book, &name)).map_err(|_| Error::InvalidWire)?;
    let creation = dav.create(&dir, &filename, encoded).await;
    let Some(bytes) = dav.get(&dir, &filename).await? else {
        creation?;
        return Err(Error::Integrity);
    };
    let (owner, spelling): (String, String) =
        serde_json::from_slice(&bytes).map_err(|_| Error::InvalidWire)?;
    if !profile::book_id(&owner) || !crate::shelf::valid(&spelling) || folded(&spelling) != key {
        return Err(Error::Integrity);
    }
    Ok((owner == book).then_some(spelling))
}
async fn upload_book(r: &Replica, dav: &dyn Store, book: &str, root: &Path) -> Result<()> {
    if !profile::book_id(book) {
        return Err(Error::InvalidWire);
    }
    let gate = deletion::file_operation(r, book);
    let _guard = gate.lock().await;
    let id = Value::Text(book.into());
    let selected = id.clone();
    let source = r
        .worker()
        .read(move |db| {
            db.try_with_connection(|c| {
                let query = "SELECT v.body,coalesce(m.json,'{}'),b.title,
                            EXISTS(SELECT 1 FROM stanza_sync_uploads q WHERE q.book=b.id),
                            EXISTS(SELECT 1 FROM stanza_sync_file_repairs f WHERE f.book=b.id) FROM books b
                     LEFT JOIN stanza_sync_values v ON v.book=b.id AND v.field='file'
                     LEFT JOIN book_metadata m ON m.book=b.id WHERE b.id=?";
                Ok(rows(c, query, &[selected])?.into_iter().next())
            })
        })
        .await?;
    let Some(source) = source else { return Ok(()) };
    let details = source[1].as_text()?.to_owned();
    let previous = match &source[0] {
        Value::Null => None,
        body => match serde_json::from_str(body.as_text()?).map_err(|_| Error::InvalidWire)? {
            Field::File { epub, metadata } if epub.sha256 == book => Some((epub, metadata)),
            _ => return Err(Error::InvalidWire),
        },
    };
    let (previous_epub, previous_metadata) = previous.unzip();
    let asset = match previous_epub {
        Some(ref asset) if asset.file.is_some() && source[4] == Value::Int(0) => asset.clone(),
        legacy => {
            let path = crate::shelf::path(root, book).map_err(io)?;
            let local = path.is_file();
            let stage;
            let source_path = if local {
                &path
            } else {
                let asset = legacy.as_ref().ok_or(Error::Integrity)?;
                stage = {
                    let mut temp =
                        tempfile::NamedTempFile::new_in(day::app_temp_dir()).map_err(io)?;
                    assets::download(dav, r.space(), asset, temp.as_file_mut()).await?;
                    temp
                };
                stage.path()
            };
            // An interrupted deletion invalidates reuse of the old reference. Verify
            // or republish its bytes before clearing the durable recovery marker.
            let title = source[2].as_text()?;
            let preferred = legacy
                .as_ref()
                .and_then(|asset| asset.file.as_ref())
                .map_or_else(|| crate::shelf::name(title, book), |file| file.name.clone());
            let total = std::fs::metadata(source_path).map_err(io)?.len();
            let mut published = None;
            for candidate in crate::shelf::candidates(&preferred, book) {
                let Some(name) = claim_name(dav, candidate, book).await? else {
                    continue;
                };
                let file = assets::AssetFile {
                    directory: "books".into(),
                    name: name.clone(),
                };
                if let Some(asset) = assets::upload_file(dav, file, source_path, book, |bytes| {
                    progress::transferred(Direction::Upload, book, bytes, total)
                })
                .await?
                {
                    if local && path != root.join(&name) {
                        crate::shelf::adopt(root, book, &name).map_err(io)?;
                    }
                    published = Some(asset);
                    break;
                }
            }
            published.ok_or(Error::Integrity)?
        }
    };
    let queued = source[3].clone();
    let mut stage = tempfile::NamedTempFile::new_in(day::app_temp_dir()).map_err(io)?;
    use sha2::{Digest, Sha256};
    let local_hash = format!("{:x}", Sha256::digest(details.as_bytes()));
    let hash = if let Some(reference) = previous_metadata
        .filter(|reference| queued == Value::Int(0) && reference.sha256 != local_hash)
    {
        // Preserve remote-only metadata. An identical local copy can republish a missing
        // remote object without fetching the deleted chunks or changing its contents.
        if reference.bytes > crate::epub::RESOURCE_LIMIT {
            return Err(Error::Limit);
        }
        assets::download(dav, r.space(), &reference, stage.as_file_mut()).await?;
        reference.sha256
    } else {
        use std::io::Write as _;
        if details.len() as u64 > crate::epub::RESOURCE_LIMIT {
            return Err(Error::Limit);
        }
        stage.write_all(details.as_bytes()).map_err(io)?;
        local_hash
    };
    let metadata = assets::upload_file(
        dav,
        assets::AssetFile {
            directory: "stanza-redux-v1".into(),
            name: format!("{hash}.json"),
        },
        stage.path(),
        &hash,
        |_| {},
    )
    .await?
    .ok_or(Error::Integrity)?;
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
                           SELECT 'file',b.id,'',? FROM books b
                           LEFT JOIN book_metadata m ON m.book=b.id
                           LEFT JOIN stanza_sync_values v ON v.book=b.id AND v.field='file'
                           WHERE b.id=? AND coalesce(m.json,'{}')=? AND v.body IS ?
                           AND EXISTS(SELECT 1 FROM stanza_sync_uploads q WHERE q.book=b.id)=?
                           ON CONFLICT DO UPDATE SET body=excluded.body";
                if c.execute(
                    publish,
                    &[
                        Value::Text(body),
                        id.clone(),
                        Value::Text(details),
                        source[0].clone(),
                        queued,
                    ],
                )? != 0
                {
                    c.execute(
                        "DELETE FROM stanza_sync_uploads WHERE book=?",
                        std::slice::from_ref(&id),
                    )?;
                    c.execute("DELETE FROM stanza_sync_file_repairs WHERE book=?", &[id])?;
                }
                Ok(())
            })
        })
        .await?;

    Ok(())
}
/// Apply portable catalog metadata (including OPDS supplements) even on devices that
/// only download selected EPUBs. The receipt and the derived author/genre folders commit
/// together. A later metadata reference supersedes an in-flight download.
#[derive(Default)]
struct DescriptionProgress {
    active: std::collections::BTreeMap<usize, String>,
    completed: usize,
    total: usize,
}
impl DescriptionProgress {
    fn show(&self) {
        if let Some(title) = self.active.values().next() {
            progress::set(Work::Descriptions {
                title: title.clone(),
                index: self.completed,
                total: self.total,
            });
        }
    }
}
async fn description_job(
    job: Option<(usize, &[Value])>,
    r: &Replica,
    dav: &dyn Store,
    scratch: &Path,
    progress: &RefCell<DescriptionProgress>,
) -> Result<()> {
    let Some((index, row)) = job else {
        return Ok(());
    };
    let book = row[0].as_text()?.to_owned();
    let expected = row[1].as_text()?.to_owned();
    let title = row[2].as_text()?.to_owned();
    let local = match &row[3] {
        Value::Null => None,
        value => Some(value.as_text()?.to_owned()),
    };
    {
        let mut state = progress.borrow_mut();
        state.active.insert(index, title);
        state.show();
    }
    let result = fetch_description(r, dav, book, expected, local, scratch).await;
    {
        let mut state = progress.borrow_mut();
        state.active.remove(&index);
        state.completed += 1;
        state.show();
    }
    result
}
async fn descriptions(r: &Replica, dav: &dyn Store, scratch: &Path) -> Result<()> {
    let waiting = r.worker().read(|db| {
        let query = "SELECT v.book,v.body,b.title,m.json FROM stanza_sync_values v
                     JOIN books b ON b.id=v.book
                     LEFT JOIN book_metadata m ON m.book=v.book
                     LEFT JOIN stanza_sync_descriptions d ON d.book=v.book
                     WHERE v.field='file' AND d.hash IS NOT json_extract(v.body,'$.File.metadata.sha256')
                       AND NOT EXISTS(SELECT 1 FROM stanza_sync_uploads q WHERE q.book=v.book)
                     ORDER BY b.title,b.id";
        db.try_with_connection(|c| rows(c, query, &[]))
    }).await?;
    let state = RefCell::new(DescriptionProgress {
        total: waiting.len(),
        ..Default::default()
    });
    let mut failure = None;
    // Match the DAV client's two-request budget. Database application stays serialized
    // by the replica worker; each result rechecks its reference before committing.
    for (batch, pair) in waiting.chunks(2).enumerate() {
        let (first, second) = futures_lite::future::zip(
            description_job(Some((batch * 2, &pair[0])), r, dav, scratch, &state),
            description_job(
                pair.get(1).map(|row| (batch * 2 + 1, row.as_slice())),
                r,
                dav,
                scratch,
                &state,
            ),
        )
        .await;
        for result in [first, second] {
            if let Err(error) = result {
                failure = Some(error);
            }
        }
    }
    failure.map_or(Ok(()), Err)
}

async fn fetch_description(
    r: &Replica,
    dav: &dyn Store,
    book: String,
    expected: String,
    local: Option<String>,
    scratch: &Path,
) -> Result<()> {
    let Field::File { metadata, .. } =
        serde_json::from_str(&expected).map_err(|_| Error::InvalidWire)?
    else {
        return Err(Error::InvalidWire);
    };
    use sha2::{Digest, Sha256};
    metadata.validate()?;
    if metadata.bytes > crate::epub::RESOURCE_LIMIT {
        return Err(Error::Limit);
    }
    let local = local.filter(|json| {
        json.len() as u64 == metadata.bytes
            && format!("{:x}", Sha256::digest(json.as_bytes())) == metadata.sha256
    });
    let description = if let Some(json) = local {
        serde_json::from_str(&json).map_err(|_| Error::InvalidWire)?
    } else {
        std::fs::create_dir_all(scratch).map_err(io)?;
        let mut stage = tempfile::tempfile_in(scratch).map_err(io)?;
        assets::download(dav, r.space(), &metadata, &mut stage).await?;
        std::io::Seek::rewind(&mut stage).map_err(io)?;
        serde_json::from_reader(stage).map_err(|_| Error::InvalidWire)?
    };
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
    let path = crate::shelf::path(root, book).map_err(io)?;
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
    let progress = |bytes| progress::transferred(Direction::Download, book, bytes, asset.bytes);
    let ready = if asset.file.is_some() {
        let mut temp = tempfile::NamedTempFile::new_in(root).map_err(io)?;
        assets::download_with_progress(dav, space, &asset, temp.as_file_mut(), progress).await?;
        temp
    } else {
        // Only the chunk format can resume a previous verified prefix.
        let partial = root.join(format!("{book}.part"));
        let mut stage = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&partial)
            .map_err(io)?;
        stage.try_lock().map_err(|_| Error::Busy)?;
        assets::download_with_progress(dav, space, &asset, &mut stage, progress).await?;
        tempfile::NamedTempFile::from_parts(
            stage,
            tempfile::TempPath::try_from_path(partial).map_err(io)?,
        )
    };
    let record = db.try_book(book)?.ok_or(Error::WrongSpace)?;
    let preferred = asset
        .file
        .map_or_else(|| crate::shelf::name(&record.title, book), |f| f.name);
    crate::shelf::install(root, book, &preferred, ready).map_err(io)?;
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
    let path = crate::shelf::path(&root, book).map_err(io)?;
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
        materialize(
            &db,
            &store(&a).map_err(|_| invalid())?,
            a.identity.space(),
            book,
            &root,
            repair,
        )
        .await?;
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
async fn pass() -> Attempt {
    progress::set(Work::Opening);
    let local = |e: Error| Failure::sync(Stage::Sync, &e);
    let db = library().map_err(local)?;
    let a = setting()
        .get_untracked()
        .ok_or_else(|| local(Error::WrongSpace))?;
    let r = replica(&db, &a).await.map_err(local)?;
    let dav = store(&a)?;
    dav.client().check_collection().await.map_err(|error| {
        Failure::dav(
            Stage::Probe(day_sync_webdav::webdav::ConnectionStep::OpenCollection),
            &error,
        )
    })?;
    descriptor(&dav, Some(&a.identity))
        .await
        .map_err(|e| Failure::sync(Stage::Library, &e))?;
    sync_library(db, r, dav).await.map_err(local)
}
/// Another device may have assigned the canonical filename while this copy was offline.
async fn reconcile_names(db: &Library, root: &Path) -> Result<()> {
    let local = db.db.try_with_connection(|c| {
        rows(
            c,
            "SELECT book,json_extract(body,'$.File.epub.file.name') AS name
         FROM stanza_sync_values JOIN books ON books.id=book
         WHERE field='file' AND name IS NOT NULL ORDER BY length(name) DESC",
            &[],
        )
    })?;
    let root = root.to_owned();
    crate::covers::background(move || -> Result<()> {
        for row in local {
            let id = row[0].as_text()?;
            let preferred = row[1].as_text()?;
            let current = crate::shelf::path(&root, id).map_err(io)?;
            if current.file_name().and_then(|n| n.to_str()) != Some(preferred) && current.is_file()
            {
                crate::shelf::adopt(&root, id, preferred).map_err(io)?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|_| Error::Integrity)?
}
async fn sync_library(db: Library, r: Replica, dav: WebDavStore) -> Result<()> {
    progress::set(Work::Metadata);
    metadata(&r, &dav).await?;
    db.check_external()?;
    let ids = db.db.try_with_connection(|c| {
        rows(c, "SELECT b.id,b.title FROM books b LEFT JOIN stanza_sync_uploads q ON q.book=b.id
            LEFT JOIN stanza_sync_values v ON v.book=b.id AND v.field='file'
            LEFT JOIN stanza_sync_file_repairs f ON f.book=b.id
            WHERE q.book IS NOT NULL OR f.book IS NOT NULL OR (v.body IS NOT NULL AND json_type(v.body,'$.File.epub.file') IS NULL)
            ORDER BY b.title,b.id", &[])
    })?;
    let root = books_directory()?;
    let mut failure = None;
    let total = ids.len();
    for (index, row) in ids.into_iter().enumerate() {
        progress::set(Work::Transfer {
            direction: Direction::Upload,
            book: row[0].as_text()?.into(),
            title: row[1].as_text()?.into(),
            index,
            total,
            bytes: 0,
            total_bytes: 0,
        });
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
    reconcile_names(&db, &root).await?;
    if pinned() {
        let books: Vec<_> = db
            .try_books()?
            .into_iter()
            .filter(|book| !crate::shelf::path(&root, &book.id).is_ok_and(|path| path.is_file()))
            .collect();
        let total = books.len();
        for (index, book) in books.into_iter().enumerate() {
            progress::set(Work::Transfer {
                direction: Direction::Download,
                book: book.id.clone(),
                title: book.title,
                index,
                total,
                bytes: 0,
                total_bytes: 0,
            });
            if let Err(error) = during_transfer(ensure_book(&book.id), &r, &dav, &db).await {
                failure = Some(error);
            }
        }
    }
    progress::set(Work::Finishing);
    db.check_external()?;
    failure.map_or(Ok(()), Err)
}
fn pinned() -> bool {
    day::prefs::get("stanza.dav.offline").as_deref() == Some("1")
}
fn finish(result: Attempt) {
    let delay = result
        .as_ref()
        .err()
        .and_then(|failure| failure.retry_after)
        .and_then(|delay| std::time::Instant::now().checked_add(delay));
    RETRY.with(|cell| cell.set(delay));
    status().set(match result {
        Ok(()) => Phase::Ready,
        Err(failure) => Phase::Failed(failure),
    });
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
            finish(Err(Failure::sync(Stage::Account, &e)));
            return;
        }
    }
    if RUNNING.with(|r| r.replace(true)) {
        return;
    }
    schedule::start();
    day::task(async {
        let mut delay = 30_000u32;
        loop {
            if !matches!(
                status().get_untracked(),
                Phase::Failed(failure) if failure.pauses_retry()
            ) && RETRY.with(|r| r.get().is_none_or(|t| std::time::Instant::now() >= t))
            {
                sync_now();
            }
            day::sleep(delay).await;
            delay = if matches!(status().get_untracked(), Phase::Failed(_)) {
                delay.saturating_mul(2).min(900_000)
            } else {
                30_000
            };
        }
    });
}
pub(crate) fn settings_group() -> day_piece_settings::Group {
    use day_piece_settings::{Group, Section, rows};
    let account = setting();
    let configured = move || account.get().is_some();
    let busy = || status().get() == Phase::Working;
    let pin = Signal::new(pinned());
    watch(
        move || pin.get(),
        move |on, _| {
            day::prefs::set("stanza.dav.offline", if *on { "1" } else { "0" });
        },
    );
    let mut group = Group::new("webdav", res::str::dav_title())
        .icon(res::vectors::prefs_sync)
        .section(Section::new().row(rows::custom(sync_status)));
    for section in day_piece_webdav_settings::account_sections(
        move || {
            let current = account.get();
            day_piece_webdav_settings::AccountFields {
                endpoint: current
                    .as_ref()
                    .map(|a| a.endpoint.clone())
                    .unwrap_or_default(),
                saved_folder: current.as_ref().and_then(|a| a.folder.clone()),
                default_folder: "Stanza-Redux",
                username: current.map(|a| a.username).unwrap_or_default(),
            }
        },
        move || {
            account
                .get_untracked()
                .and_then(|account| {
                    day_part_keychain::get(&credential(&account))
                        .ok()
                        .and_then(|secret| secret.as_str().map(str::to_owned))
                })
                .unwrap_or_default()
        },
        configured,
        busy,
        day_piece_webdav_settings::SectionLabels {
            server_section: res::str::dav_server_section(),
            account_section: res::str::dav_account_section(),
            help: res::str::dav_help(),
            fields: day_piece_webdav_settings::Labels {
                endpoint: res::str::dav_base_url(),
                folder: res::str::dav_app_folder(),
                invalid_address: res::str::dav_invalid_address(),
                username: res::str::dav_username(),
                password: res::str::dav_password(),
                connect: res::str::dav_connect(),
                update: res::str::dav_connect(),
            },
        },
        |url, folder, username, password| {
            progress::set(Work::Checking);
            status().set(Phase::Working);
            day::task(async move {
                match connect(url, folder, username, password).await {
                    Ok(()) => finish(pass().await),
                    Err(e) => finish(Err(e)),
                }
            });
        },
    ) {
        group = group.section(section);
    }
    group.section(
        Section::new()
            .title(res::str::dav_title())
            .row(
                rows::action(res::str::dav_sync_now(), sync_now)
                    .enabled(move || configured() && !busy())
                    .id(day::id!(dav_sync_now)),
            )
            .row(rows::custom(move || {
                labeled(
                    res::str::dav_offline(),
                    toggle(pin).id(day::id!(dav_offline)),
                )
            })),
    )
}
fn sync_status() -> impl Piece {
    let busy = || status().get() == Phase::Working;
    column((
        label(move || {
            match status().get() {
                Phase::Idle => res::str::dav_not_connected(),
                Phase::Working => progress::current().get().message(),
                Phase::Ready => res::str::dav_ready(),
                Phase::Failed(failure) => failure.summary(),
            }
            .format()
        })
        .max_lines(move || u32::from(busy()))
        .grow(Dimension::Width)
        .id(day::id!(dav_status)),
        when(
            move || busy() && progress::current().get().fraction().is_some(),
            move || {
                column((
                    day::prelude::progress(move || {
                        progress::current().get().fraction().unwrap_or(0.)
                    })
                    .a11y(|a| a.role(Role::Meter).label(res::str::dav_title()))
                    .id(day::id!(dav_progress))
                    .grow(Dimension::Width),
                    label(move || progress::current().get().detail())
                        .font(Font::Caption)
                        .secondary()
                        .id(day::id!(dav_progress_detail)),
                ))
                .spacing(Space::S)
                .align(HAlign::Leading)
                .grow(Dimension::Width)
            },
        ),
        when(
            move || busy() && progress::current().get().fraction().is_none(),
            spinner,
        ),
        day_piece_webdav_settings::failure_details(res::str::dav_more_info(), || {
            match status().get() {
                Phase::Failed(failure) => Some(failure.detail()),
                _ => None,
            }
        }),
    ))
    .spacing(Space::S)
    .align(HAlign::Leading)
    .grow(Dimension::Width)
}

#[cfg(test)]
mod tests;
