// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Stanza Redux: native OPDS browsing and offline EPUB reading with Readium CSS.
use day::prelude::*;
use day_piece_webview::{JsHandle, LinkPolicy};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
mod browser;
mod covers;
pub mod error;
use error::AppError;
mod catalog_editor;
mod documents;
pub mod epub;
mod legacy;
mod library;
mod metadata;
pub mod opds;
mod reader_resources;
mod storage;
day::resources!();
day::routes! { enum ReaderRoute { Reading => "reading" } }
day::day_start!(options: window(), root);
pub fn window() -> day::WindowOptions {
    day::WindowOptions {
        locales: Some((res::locales::DEFAULT, res::locales::CATALOG)),
        title_fn: Some(|| res::str::app_title().format()),
        size: Size::new(1100., 800.),
        ..Default::default()
    }
}
pub fn sample_book() -> Result<Vec<u8>, AppError> {
    #[cfg(not(test))]
    {
        day::resource(res::assets::alice_epub)
            .map(|r| r.to_vec())
            .ok_or_else(|| AppError::new(res::str::sample_unavailable))
    }
    #[cfg(test)]
    {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("resource/assets")
                .join(res::assets::alice_epub.as_str()),
        )
        .map_err(|e| AppError::detail(res::str::sample_unavailable, e))
    }
}
async fn bundled_sample() -> Result<Vec<u8>, AppError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        sample_book()
    }
    #[cfg(target_arch = "wasm32")]
    {
        // Web bundles deploy generated AssetName paths under the host's assets/data directory.
        let response = day_part_http::Client::builder()
            .build()
            .send_future(day_part_http::Request::get(format!(
                "assets/data/{}",
                res::assets::alice_epub.as_str()
            )))
            .await
            .map_err(|e| AppError::detail(res::str::sample_unavailable, e))?;
        if response.status() != 200 {
            return Err(AppError::new(res::str::sample_unavailable));
        }
        let mut body = response.into_body();
        let mut bytes = Vec::new();
        while let Some(chunk) = body.next().await {
            bytes.extend(chunk.map_err(|e| AppError::detail(res::str::sample_unavailable, e))?);
        }
        Ok(bytes)
    }
}
// Match the legacy application's actual header, including its current release version.
pub const USER_AGENT: &str = "Stanza-Redux/1.4.4";
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct Saved {
    id: String,
    title: String,
    author: String,
    #[serde(default)]
    cover: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    size: f64,
    line: f64,
    margin: f64,
    theme: usize,
    font: usize,
    columns: usize,
    paper: u32,
    ink: u32,
    justified: bool,
    hyphenation: bool,
    paragraph_spacing: f64,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            size: 20.,
            line: 1.6,
            margin: 24.,
            theme: 1,
            font: 0,
            columns: 1,
            paper: 0xf4ead6,
            ink: 0x30343b,
            justified: false,
            hyphenation: true,
            paragraph_spacing: 0.6,
        }
    }
}
// Mark nonempty libraries too: deleting the last book must not recreate the sample.
async fn seed_library(db: &storage::Library) -> Result<(), AppError> {
    if db
        .imported("starter-book-v1")
        .map_err(|e| AppError::from(e.to_string()))?
    {
        return Ok(());
    }
    if db
        .try_books()
        .map_err(|e| AppError::from(e.to_string()))?
        .is_empty()
    {
        let bytes = bundled_sample().await?;
        let (bytes, book) = covers::background(move || {
            let book = epub::parse(&bytes);
            (bytes, book)
        })
        .await
        .map_err(AppError::from)?;
        // Another import may have completed while the bundled book was being parsed.
        if db
            .try_books()
            .map_err(|e| AppError::from(e.to_string()))?
            .is_empty()
        {
            storage::store_book(db, &book?, bytes).await?;
        }
    }
    db.mark_imported("starter-book-v1")
        .map_err(|e| AppError::from(e.to_string()))
}

#[derive(Clone, Copy)]
struct App {
    document_window: bool,
    database: Signal<Option<storage::Library>>,
    section: Signal<String>,
    reader_open: Signal<Option<ReaderRoute>>,
    reader_settings: Signal<bool>,
    language: Signal<usize>,
    library_query: Signal<String>,
    url: Signal<String>,
    catalog_path: Signal<Vec<browser::CatalogPage>>,
    catalog_forward: Signal<Vec<browser::CatalogPage>>,
    catalog_serial: Signal<u64>,
    catalog_focus: Signal<bool>,
    catalog_editor: Signal<Option<catalog_editor::EditorRoute>>,
    library_path: Signal<Vec<library::Page>>,
    library_selection: Signal<Option<Saved>>,
    book: Signal<Option<Arc<epub::Book>>>,
    status: Signal<String>,
    busy: Signal<bool>,
    task: Signal<Option<day::TaskHandle>>,
    js: JsHandle,
    ready: Signal<bool>,
    position: Signal<String>,
    prefs: Signal<Preferences>,
    toc: Signal<usize>,
}
fn saved<T: serde::de::DeserializeOwned + Default>(key: &str) -> T {
    day::prefs::get(key)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn persist<T: Serialize>(key: &str, value: &T) {
    if let Ok(s) = serde_json::to_string(value) {
        day::prefs::set(key, &s);
    }
}
fn defaults() -> Vec<opds::Link> {
    [
        (
            res::str::catalog_standard().format(),
            "https://standardebooks.org/feeds/opds",
        ),
        (
            res::str::catalog_gutenberg().format(),
            "https://www.gutenberg.org/ebooks.opds/",
        ),
        (
            res::str::catalog_ebooks_gratuits().format(),
            "https://www.ebooksgratuits.com/opds/",
        ),
    ]
    .into_iter()
    .map(|(title, url)| opds::Link {
        title: title.into(),
        url: url.into(),
        ..Default::default()
    })
    .collect()
}
impl App {
    fn catalogs(self) -> Result<Vec<opds::Link>, AppError> {
        self.database
            .get_untracked()
            .ok_or_else(|| AppError::new(res::str::database_unavailable))?
            .try_catalogs()
            .map_err(|e| AppError::from(e.to_string()))
    }
    fn cancel(self) {
        if let Some(t) = self.task.get_untracked() {
            t.abort()
        }
        self.task.set(None);
        self.busy.set(false);
    }
    fn error(self, error: impl Into<AppError>) {
        let error = error.into();
        eprintln!("{error}");
        let e = error.localized();
        self.status.set(res::str::error_status(e.clone()).format());
        self.busy.set(false);
        if self.reader_open.get_untracked().is_none() {
            day::task(async move {
                alert(res::str::error())
                    .message(e)
                    .button(res::str::done(), ())
                    .present()
                    .await;
            });
        }
    }
    fn eval(self, code: String) {
        day::task(async move {
            if let Err(e) = self.js.eval(code).await {
                self.error(e.to_string())
            }
        });
    }
    fn browse(self, url: String, root: bool) {
        self.browse_named(url, root, res::str::loading().format());
    }
    fn acquire(self, url: String, metadata: metadata::Metadata) {
        self.cancel();
        self.busy.set(true);
        let task = day::task(async move {
            match fetch(&url, 64 * 1024 * 1024, self).await {
                Ok((bytes, _)) => self.install_metadata(bytes, metadata).await,
                Err(e) => self.error(e),
            }
        });
        self.task.set(Some(task));
    }
    async fn install_metadata(self, bytes: Vec<u8>, metadata: metadata::Metadata) {
        self.install_document(bytes, metadata, false).await
    }
    async fn install_document(self, bytes: Vec<u8>, metadata: metadata::Metadata, separate: bool) {
        let Some(db) = self.database.get_untracked() else {
            self.error(AppError::new(res::str::database_unavailable));
            return;
        };
        let parsed = covers::background(move || {
            let parsed = epub::parse(&bytes);
            (bytes, parsed)
        })
        .await;
        let (bytes, parsed) = match parsed {
            Ok(v) => v,
            Err(e) => {
                self.error(e);
                return;
            }
        };
        match parsed {
            Ok(mut b) => {
                let existing = match db.try_book(&b.id) {
                    Ok(existing) => existing,
                    Err(e) => {
                        self.error(e.to_string());
                        return;
                    }
                };
                if existing.is_some() {
                    if let Some(existing) = db.metadata(&b.id) {
                        b.metadata = existing;
                    }
                    self.busy.set(false);
                    if separate {
                        documents::reader_window(self, b);
                    } else {
                        self.open(b);
                    }
                    return;
                }
                b.metadata.supplement(&metadata);
                match storage::store_book(&db, &b, bytes).await {
                    Ok(()) => {
                        self.busy.set(false);
                        if separate {
                            documents::reader_window(self, b);
                        } else {
                            self.open(b);
                        }
                    }
                    Err(e) => self.error(e),
                }
            }
            Err(e) => self.error(e),
        }
    }
    fn open(self, b: epub::Book) {
        if let Some(db) = self.database.get_untracked() {
            if let Err(e) = db.opened(&b.id, now_ms() as i64) {
                self.error(e.to_string());
                return;
            }
        }
        self.book.set(None);
        self.ready.set(false);
        self.toc.set(0);
        self.position.set(String::new());
        self.book.set(Some(Arc::new(b)));
        self.reader_settings.set(false);
        self.reader_open.set(Some(ReaderRoute::Reading));
        self.busy.set(false);
        self.status.set(String::new());
    }
    fn load(self, id: String) {
        self.cancel();
        self.busy.set(true);
        day::task(async move {
            match epub::open_saved(&id).await {
                Ok(book) => self.open(book),
                Err(error) => self.error(error),
            }
        });
    }
    fn on_link(self, u: &str) -> LinkPolicy {
        let Ok(u) = url::Url::parse(u) else {
            return LinkPolicy::Ignore;
        };
        if u.scheme() != "stanza" {
            return LinkPolicy::Ignore;
        }
        let data = u
            .query_pairs()
            .find(|(k, _)| k == "data")
            .and_then(|(_, v)| serde_json::from_str::<serde_json::Value>(&v).ok())
            .unwrap_or_default();
        match u.host_str() {
            Some("ready") => {
                if !self.ready.get_untracked() {
                    if let Some(b) = self.book.get_untracked() {
                        let prefs = serde_json::to_string(&self.prefs.get_untracked()).unwrap();
                        let pos = self
                            .database
                            .get_untracked()
                            .and_then(|db| db.book(&b.id))
                            .map(|b| {
                                serde_json::json!({"chapter":b.chapter,"progress":b.progress})
                                    .to_string()
                            })
                            .unwrap_or_else(|| "{}".into());
                        let json = serde_json::to_string(&*b).unwrap();
                        self.ready.set(true);
                        let labels = serde_json::json!({
                            "close":res::str::back_library().format(), "contents":res::str::contents().format(),
                            "appearance":res::str::settings().format(), "previous":res::str::previous().format(),
                            "next":res::str::next().format(), "chapter":res::str::chapter().format(),
                            "done":res::str::done().format(), "hint":res::str::reader_gestures().format(),
                            "appTitle":res::str::app_title().format(), "appearanceGlyph":res::str::appearance_glyph().format(),
                            "position":res::str::position_text("{chapter}", "{chapters}", "{page}", "{pages}").format(),
                            "bookmarkAdd":res::str::bookmark_add().format(),
                            "bookmarkRemove":res::str::bookmark_remove().format(),
                            "bookmarks":res::str::bookmarks().format(),
                            "bookmarksEmpty":res::str::bookmarks_empty().format(),
                            "chapterPosition":res::str::chapter_position().format(),
                            "returnPosition":res::str::return_position().format(),
                            "contentsSearch":res::str::contents_search().format(),
                            "contentsEmpty":res::str::contents_empty().format(),
                            "chapterPercent":res::str::chapter_percent("{percent}").format(),
                            "locale":day::locale().get_untracked(),
                        });
                        let bookmarks = self
                            .database
                            .get_untracked()
                            .map(|db| db.bookmark_query(&b.id).try_collect())
                            .transpose();
                        match bookmarks {
                            Ok(rows) => self.eval(format!(
                                "stanza.load({json},{prefs},{pos},{labels},{})",
                                serde_json::to_string(&rows.unwrap_or_default()).unwrap()
                            )),
                            Err(error) => self.error(error.to_string()),
                        }
                    }
                }
            }
            Some("bookmark") => {
                if let (Some(book), Some(db)) =
                    (self.book.get_untracked(), self.database.get_untracked())
                {
                    let result = if let Some(id) = data["remove"].as_str() {
                        db.remove_bookmark(&book.id, id)
                    } else if let (Some(chapter), Some(progress)) =
                        (data["chapter"].as_u64(), data["progress"].as_f64())
                    {
                        if let Some(entry) = book.chapters.get(chapter as usize) {
                            db.add_bookmark(&book.id, chapter as u32, progress, entry.title.clone())
                        } else {
                            return LinkPolicy::Ignore;
                        }
                    } else {
                        return LinkPolicy::Ignore;
                    };
                    if let Err(error) = result {
                        self.error(error.to_string());
                    }
                }
            }
            Some("position") => {
                if let Some(b) = self.book.get_untracked() {
                    if let Some(db) = self.database.get_untracked() {
                        if let Err(e) = db.position(
                            &b.id,
                            data["chapter"].as_u64().unwrap_or(0) as u32,
                            data["progress"].as_f64().unwrap_or(0.),
                        ) {
                            self.error(e.to_string())
                        }
                    }
                    let c = data["chapter"].as_u64().unwrap_or(0) + 1;
                    let p = data["page"].as_u64().unwrap_or(0) + 1;
                    let n = data["pages"].as_u64().unwrap_or(1);
                    self.position.set(
                        res::str::position_text(
                            c as i64,
                            b.chapters.len() as i64,
                            p as i64,
                            n as i64,
                        )
                        .format(),
                    );
                }
            }
            Some("close") => {
                if self.document_window {
                    if let Some(book) = self.book.get_untracked() {
                        if let Some(window) = day::window_by_key(&format!("reader-{}", book.id)) {
                            window.close();
                        }
                    }
                    return LinkPolicy::Ignore;
                }
                self.reader_open.set(None);
                self.section.set("library".into());
            }
            Some("settings") => self.reader_settings.set(true),
            Some("error") => self.error(AppError::detail(res::str::epub_chapter_missing, data)),
            Some("link") => {
                if let Some(link) = data["url"].as_str() {
                    if link.starts_with("https://") || link.starts_with("http://") {
                        open_link(link)
                    }
                }
            }
            _ => {}
        }
        LinkPolicy::Ignore
    }
}
async fn fetch(url: &str, limit: usize, app: App) -> Result<(Vec<u8>, String), AppError> {
    fetch_bytes(url, limit, Some(app)).await
}
async fn fetch_bytes(
    url: &str,
    limit: usize,
    app: Option<App>,
) -> Result<(Vec<u8>, String), AppError> {
    let url = opds::resolve(url, "")?;
    let client = day_part_http::Client::builder()
        .user_agent(USER_AGENT)
        .timeout_total(std::time::Duration::from_secs(90))
        .build();
    let r = client
        .send_future(day_part_http::Request::get(url).header(
            "Accept",
            "application/atom+xml, application/opds+json, application/epub+zip, */*",
        ))
        .await
        .map_err(|e| AppError::detail(res::str::network_failed, e))?;
    if !(200..300).contains(&r.status()) {
        return Err(AppError::detail(
            res::str::network_failed,
            format!("HTTP {} · {}", r.status(), r.url()),
        ));
    }
    if r.expected_length().is_some_and(|n| n > limit as u64) {
        return Err(AppError::new(res::str::download_limit));
    }
    let final_url = r.url().to_owned();
    let total = r.expected_length();
    let mut body = r.into_body();
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|e| AppError::detail(res::str::network_failed, e))?;
        if bytes.len() + chunk.len() > limit {
            return Err(AppError::new(res::str::download_limit));
        }
        bytes.extend(chunk);
        if let Some(app) = app {
            let received = day::format_decimal(bytes.len() as f64 / 1048576., 1);
            app.status.set(match total {
                Some(n) => res::str::download_progress_total(
                    received,
                    day::format_decimal(n as f64 / 1048576., 1),
                )
                .format(),
                None => res::str::download_progress(received).format(),
            });
        }
    }
    Ok((bytes, final_url))
}
pub fn root() -> impl Piece {
    let lang = Signal::new(if day::locale().get_untracked().starts_with("fr") {
        1
    } else {
        0
    });
    watch(
        move || lang.get(),
        move |i, _| {
            set_locale(if *i == 1 { "fr" } else { "en" });
        },
    );
    let prefs: Preferences = saved("stanza.preferences");
    let database = storage::Library::open().and_then(|db| {
        db.import_preferences()?;
        Ok(db)
    });
    let database_error = database
        .as_ref()
        .err()
        .map(|e| {
            day::error!("Cannot open library database: {e}");
            res::str::database_unavailable().format()
        })
        .unwrap_or_default();
    let database = database.ok();
    let a = App {
        document_window: false,
        database: Signal::new(database),
        section: Signal::new("library".into()),
        reader_open: Signal::new(None),
        reader_settings: Signal::new(false),
        language: lang,
        library_query: Signal::new(String::new()),
        url: Signal::new(String::new()),
        catalog_path: Signal::new(Vec::new()),
        catalog_forward: Signal::new(Vec::new()),
        catalog_serial: Signal::new(0),
        catalog_focus: Signal::new(false),
        catalog_editor: Signal::new(None),
        library_path: Signal::new(Vec::new()),
        library_selection: Signal::new(None),
        book: Signal::new(None),
        status: Signal::new(database_error),
        busy: Signal::new(false),
        task: Signal::new(None),
        js: JsHandle::new(),
        ready: Signal::new(false),
        position: Signal::new(String::new()),
        prefs: Signal::new(prefs),
        toc: Signal::new(0),
    };
    if let Some(db) = a.database.get_untracked() {
        let errors = db.error();
        watch(
            move || errors.get(),
            move |error, _| {
                if let Some(error) = error {
                    a.error(AppError::detail(res::str::database_unavailable, error));
                }
            },
        );
        // Check when returning to library/catalog navigation; readers in this process
        // already share the container, while other processes can write the same library.
        watch(
            move || (a.section.get(), a.reader_open.get().is_some()),
            move |_, _| {
                if let Err(error) = db.check_external() {
                    a.error(error.to_string());
                }
            },
        );
    }
    documents::register(a);
    browser::observe_path(a);
    if let Some(db) = a.database.get_untracked() {
        day::task(async move {
            let errors = legacy::migrate(db.clone()).await;
            if let Err(error) = seed_library(&db).await {
                a.error(error);
            }

            // Enrich older Day libraries in the background. Authors are parsed as separate
            // creators; a comma in a person's name is never treated as a delimiter.
            let books = match db.try_books() {
                Ok(books) => books,
                Err(e) => {
                    a.error(e.to_string());
                    return;
                }
            };
            for book in books {
                let old = db.metadata(&book.id);
                if old.as_ref().is_some_and(|m| !m.authors.is_empty())
                    && db
                        .book(&book.id)
                        .is_some_and(|b| b.indexed == storage::SEARCH_INDEX_VERSION)
                {
                    continue;
                }
                if let Ok(mut parsed) = epub::open_saved(&book.id).await {
                    if let Some(old) = old {
                        parsed.metadata.supplement(&old);
                    }
                    if let Err(e) = db.index_book(&parsed) {
                        a.error(e.to_string());
                    }
                }
            }

            if !errors.is_empty() {
                a.error(AppError::detail(
                    res::str::legacy_import_failed,
                    errors.join("\n"),
                ))
            }
        });
    }
    watch(
        move || a.prefs.get(),
        move |p, _| {
            persist("stanza.preferences", p);
            if a.ready.get_untracked()
                && a.reader_open.get_untracked().is_some()
                && !a.reader_settings.get_untracked()
            {
                a.eval(format!(
                    "stanza.configure({})",
                    serde_json::to_string(p).unwrap()
                ))
            }
        },
    );
    let open_book = documents::open_command(a);
    app_menu_reactive(move || {
        vec![
            sub_menu(
                res::str::file_menu().format(),
                vec![
                    open_book.menu_item(),
                    menu_separator(),
                    menu_role(MenuRole::CloseWindow),
                ],
            )
            .bar_role(MenuBarRole::File),
            sub_menu(
                res::str::catalogs().format(),
                vec![
                    menu_item(res::str::add_catalog().format())
                        .id("add-opds-catalog")
                        .icon(Symbol::Add)
                        .enabled(a.reader_open.get().is_none() && a.catalog_editor.get().is_none())
                        .action(move || {
                            a.catalog_editor.set(Some(catalog_editor::EditorRoute::Add))
                        }),
                    menu_item(res::str::manage_catalogs().format())
                        .id("manage-catalogs-menu")
                        .icon(Symbol::Settings)
                        .enabled(a.reader_open.get().is_none() && a.catalog_editor.get().is_none())
                        .action(move || {
                            a.section.set("catalogs".into());
                            a.manage_catalogs()
                        }),
                ],
            ),
            sub_menu(
                res::str::view_menu().format(),
                vec![
                    menu_item(res::str::increase_font().format())
                        .id("increase-font")
                        .key("+")
                        .enabled(a.prefs.get().size < 40.)
                        .action(move || a.prefs.update(|p| p.size = (p.size + 1.).min(40.))),
                    menu_item(res::str::decrease_font().format())
                        .id("decrease-font")
                        .key("-")
                        .enabled(a.prefs.get().size > 12.)
                        .action(move || a.prefs.update(|p| p.size = (p.size - 1.).max(12.))),
                    menu_separator(),
                    menu_role(MenuRole::Fullscreen),
                ],
            )
            .bar_role(MenuBarRole::View),
        ]
    });
    let style = if cfg!(any(
        target_os = "ios",
        target_os = "android",
        target_env = "ohos"
    )) {
        NavStyle::Tabs
    } else if cfg!(target_arch = "wasm32") {
        NavStyle::Automatic
    } else {
        NavStyle::Sidebar
    };
    let navigation = nav(a.section)
        .title(res::str::app_title())
        .style(style)
        .item_icon(
            "library",
            res::str::library(),
            res::vectors::tab_library,
            move || {
                #[cfg(feature = "appkit")]
                {
                    library::desktop_detail(a).any()
                }
                #[cfg(not(feature = "appkit"))]
                {
                    library::view(a).any()
                }
            },
        )
        .item_icon(
            "catalogs",
            res::str::catalogs(),
            res::vectors::tab_catalogs,
            move || browser::view(a),
        )
        .item_icon(
            "settings",
            res::str::settings(),
            res::vectors::tab_settings,
            move || page(a, settings_page(a)),
        );
    #[cfg(feature = "appkit")]
    let navigation = navigation
        .content_list(move || library::view(a))
        .content_list_width(340.)
        .content_list_for(|key| key == "library");
    zstack((
        navigation.id("navigation"),
        cover(a.reader_open, move |_: &ReaderRoute| reader(a)),
        cover(
            a.catalog_editor,
            move |route: &catalog_editor::EditorRoute| catalog_editor::view(a, route.clone()),
        )
        .unrouted(),
    ))
}
fn page(a: App, body: impl Piece) -> impl Piece {
    column((
        body.grow(),
        when(
            move || !a.status.get().is_empty() || a.busy.get(),
            move || {
                row((
                    label(move || a.status.get())
                        .font(Font::Footnote)
                        .id("status"),
                    when(
                        move || a.busy.get(),
                        move || {
                            button(res::str::cancel())
                                .action(move || a.cancel())
                                .id("cancel")
                        },
                    ),
                ))
                .spacing(8.)
            },
        ),
    ))
    .spacing(12.)
    .padding(20.)
}
fn settings_page(a: App) -> impl Piece {
    when(
        move || a.reader_open.get().is_none(),
        move || {
            column((
                label(res::str::settings()).font(Font::LargeTitle).bold(),
                labeled(
                    res::str::language(),
                    picker(
                        [
                            res::str::language_english().format(),
                            res::str::language_french().format(),
                        ],
                        a.language,
                    )
                    .id("language"),
                ),
                settings(a),
            ))
            .spacing(16.)
            .align(HAlign::Leading)
        },
    )
}

/// Remote and downloaded books share the same presentation. The caller supplies
/// the available metadata, cover source and primary action.
fn book_overview(
    title: String,
    author: String,
    title_id: &'static str,
    cover: impl Piece,
    metadata: Signal<Option<metadata::Metadata>>,
    action: impl Piece,
) -> impl Piece {
    column((
        cover,
        label(title).font(Font::Title).bold().id(title_id),
        label(author).secondary(),
        label(move || metadata.get().map(|m| m.subtitle).unwrap_or_default())
            .font(Font::Footnote)
            .secondary(),
        action,
        when(move || metadata.get().is_none(), || spinner()),
        when(
            move || metadata.get().is_some(),
            move || metadata::panel(metadata.get_untracked().unwrap_or_default()),
        ),
    ))
    .spacing(16.)
    .padding(24.)
}
fn details(a: App, publication: opds::Publication) -> impl Piece {
    let cover_url = publication.cover.clone();
    let mut metadata = publication.metadata.clone();
    metadata.description = publication.description.clone();
    let download_url = publication.epub().map(|l| l.url.clone());
    metadata.acquisition = download_url.clone().unwrap_or_default();
    let available = download_url.is_some();
    let shown = Signal::new(Some(metadata.clone()));
    book_overview(
        publication.title,
        publication.author,
        "publication-title",
        covers::view(move || cover_url.clone(), 160., 220.),
        shown,
        column((
            button(res::str::download())
                .prominent()
                .enabled(move || available && !a.busy.get())
                .action(move || {
                    if let Some(url) = &download_url {
                        a.acquire(url.clone(), metadata.clone());
                    }
                })
                .id("download-book")
                .height(50.)
                .grow_w(),
            label(res::str::epub_only())
                .font(Font::Footnote)
                .secondary(),
        ))
        .spacing(8.),
    )
}
fn local_details(a: App, book: Saved) -> impl Piece {
    let initial = a
        .database
        .get_untracked()
        .and_then(|db| db.metadata(&book.id));
    let metadata = Signal::new(initial);
    let id = book.id.clone();
    if metadata.get_untracked().is_none() {
        let task = day::task(async move {
            // Enrich old entries without changing their reading position or last-opened date.
            if let Ok(bytes) = day_part_fs::read_future(&format!("books/{id}.epub")).await {
                if let Ok(Ok(parsed)) = covers::background(move || epub::parse(&bytes)).await {
                    if let Some(db) = a.database.get_untracked() {
                        if let Err(e) = db.save_metadata(&id, &parsed.metadata) {
                            a.error(e.to_string());
                        }
                    }
                    metadata.set(Some(parsed.metadata));
                    return;
                }
            }
            metadata.set(Some(metadata::Metadata::default()));
        });
        Scope::current().on_cleanup(move || task.abort());
    }
    let source = book
        .cover
        .map(|p| format!("library:{p}"))
        .unwrap_or_default();
    let id = book.id;
    book_overview(
        book.title,
        book.author,
        "local-book-title",
        covers::view(move || source.clone(), 160., 220.),
        metadata,
        button(res::str::open_book())
            .prominent()
            .enabled(move || !a.busy.get())
            .action(move || a.load(id.clone()))
            .id("open-local-book")
            .height(50.)
            .grow_w(),
    )
}
fn reader(a: App) -> impl Piece {
    when(
        move || a.reader_settings.get(),
        move || {
            column((
                row((
                    label(res::str::settings()).font(Font::Title),
                    spacer(),
                    button(res::str::done())
                        .action(move || a.reader_settings.set(false))
                        .id("reader-settings-done"),
                )),
                settings(a).grow(),
            ))
            .spacing(16.)
            .padding(Insets {
                top: day::safe_area().top + 16.,
                bottom: day::safe_area().bottom + 16.,
                leading: 20.,
                trailing: 20.,
            })
        },
    )
    .otherwise(move || {
        a.ready.set(false);
        reader_resources::view(a).js(a.js).id("book-webview")
    })
}

fn setting_slider(
    title: String,
    value: Signal<f64>,
    range: std::ops::RangeInclusive<f64>,
    step: f64,
    id: &'static str,
) -> impl Piece {
    column((
        row((
            label(title),
            spacer(),
            label(move || day::format_decimal(value.get(), 1))
                .font(Font::Footnote)
                .secondary()
                .tabular(),
        )),
        slider(value)
            .range(range)
            .step(step)
            .id(id)
            .grow_w()
            .height(32.),
    ))
    .spacing(6.)
    .padding(2.)
}
fn settings(a: App) -> impl Piece {
    let p = a.prefs.get_untracked();
    let size = Signal::new(p.size);
    let margin = Signal::new(p.margin);
    let line = Signal::new(p.line);
    let theme = Signal::new(p.theme);
    let font = Signal::new(p.font);
    let columns = Signal::new(p.columns.saturating_sub(1));
    let paper = Signal::new(Color::hex(p.paper));
    let ink = Signal::new(Color::hex(p.ink));
    let justified = Signal::new(p.justified);
    let hyphenation = Signal::new(p.hyphenation);
    let paragraph = Signal::new(p.paragraph_spacing);
    watch(
        move || {
            (
                size.get(),
                margin.get(),
                line.get(),
                theme.get(),
                font.get(),
                columns.get(),
                paper.get(),
                ink.get(),
                justified.get(),
                hyphenation.get(),
                paragraph.get(),
            )
        },
        move |&(
            size,
            margin,
            line,
            theme,
            font,
            columns,
            paper,
            ink,
            justified,
            hyphenation,
            paragraph_spacing,
        ),
              _| {
            let rgb = |c: Color| {
                let q = |v: f64| (v.clamp(0., 1.) * 255.).round() as u32;
                q(c.r) << 16 | q(c.g) << 8 | q(c.b)
            };
            a.prefs.set(Preferences {
                size,
                margin,
                line,
                theme,
                font,
                columns: columns + 1,
                paper: rgb(paper),
                ink: rgb(ink),
                justified,
                hyphenation,
                paragraph_spacing,
            });
        },
    );
    // Menu commands and other reader windows share preferences. Keep the visible form in
    // sync without writing an unchanged value back into the preferences watcher.
    watch(
        move || a.prefs.get(),
        move |p, _| {
            macro_rules! sync {
                ($signal:ident, $value:expr) => {
                    if $signal.get_untracked() != $value {
                        $signal.set($value);
                    }
                };
            }
            sync!(size, p.size);
            sync!(margin, p.margin);
            sync!(line, p.line);
            sync!(theme, p.theme);
            sync!(font, p.font);
            sync!(columns, p.columns.saturating_sub(1));
            sync!(paper, Color::hex(p.paper));
            sync!(ink, Color::hex(p.ink));
            sync!(justified, p.justified);
            sync!(hyphenation, p.hyphenation);
            sync!(paragraph, p.paragraph_spacing);
        },
    );
    scroll(
        form((
            section((
                column((
                    label(res::str::font()),
                    picker(
                        [
                            res::str::serif().format(),
                            res::str::sans().format(),
                            res::str::mono().format(),
                        ],
                        font,
                    )
                    .options_reactive(|| {
                        vec![
                            res::str::serif().format(),
                            res::str::sans().format(),
                            res::str::mono().format(),
                        ]
                    })
                    .id("reader-font")
                    .grow_w(),
                ))
                .spacing(8.)
                .align(HAlign::Leading),
                setting_slider(
                    res::str::font_size().format(),
                    size,
                    12.0..=40.0,
                    1.,
                    "font-size",
                ),
                setting_slider(
                    res::str::line_height().format(),
                    line,
                    1.0..=2.4,
                    0.1,
                    "line-height",
                ),
                row((
                    label(res::str::justified()).grow_w(),
                    toggle(justified).id("reader-justified"),
                ))
                .spacing(12.)
                .height(48.),
                row((
                    label(res::str::hyphenation()).grow_w(),
                    toggle(hyphenation).id("reader-hyphenation"),
                ))
                .spacing(12.)
                .height(48.),
            ))
            .title(res::str::typography()),
            section((
                column((
                    label(res::str::theme()),
                    picker(
                        [
                            res::str::light().format(),
                            res::str::sepia().format(),
                            res::str::dark().format(),
                            res::str::custom().format(),
                        ],
                        theme,
                    )
                    .options_reactive(|| {
                        vec![
                            res::str::light().format(),
                            res::str::sepia().format(),
                            res::str::dark().format(),
                            res::str::custom().format(),
                        ]
                    })
                    .style(if cfg!(feature = "mdc") {
                        PickerStyle::Menu
                    } else {
                        PickerStyle::Segmented
                    })
                    .id("reader-theme")
                    .grow_w(),
                ))
                .spacing(10.)
                .align(HAlign::Leading),
                when(
                    move || theme.get() == 3,
                    move || {
                        column((
                            row((
                                label(res::str::paper_color()).grow_w(),
                                day_piece_colorpicker::color_picker(paper).id("paper-color"),
                            ))
                            .spacing(12.)
                            .height(48.),
                            row((
                                label(res::str::ink_color()).grow_w(),
                                day_piece_colorpicker::color_picker(ink).id("ink-color"),
                            ))
                            .spacing(12.)
                            .height(48.),
                        ))
                        .spacing(10.)
                    },
                ),
            ))
            .title(res::str::appearance()),
            section((
                setting_slider(
                    res::str::margins().format(),
                    margin,
                    8.0..=64.0,
                    2.,
                    "margins",
                ),
                setting_slider(
                    res::str::paragraph_spacing().format(),
                    paragraph,
                    0.0..=2.0,
                    0.1,
                    "paragraph-spacing",
                ),
                row((
                    label(res::str::columns()).grow_w(),
                    picker(
                        [
                            res::str::column_one().format(),
                            res::str::column_two().format(),
                        ],
                        columns,
                    )
                    .segmented()
                    .id("reader-columns"),
                ))
                .spacing(12.)
                .height(44.),
                label(res::str::columns_hint())
                    .font(Font::Footnote)
                    .secondary(),
            ))
            .title(res::str::page_layout()),
        ))
        .padding(2.),
    )
    .id("reader-preferences")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_library_and_reader_preferences_migrate() {
        let book: Saved =
            serde_json::from_str(r#"{"id":"sample","title":"A","author":"B"}"#).unwrap();
        assert!(book.cover.is_none());
        let prefs: Preferences = serde_json::from_str(r#"{"size":24,"theme":2}"#).unwrap();
        assert_eq!(prefs.size, 24.);
        assert_eq!(prefs.columns, 1);
        assert_eq!(prefs.paper, Preferences::default().paper);
    }
}

fn now_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        day_dom::now_epoch_ms()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}
