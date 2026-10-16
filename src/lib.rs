// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Stanza Redux: native OPDS browsing and offline EPUB reading with Readium CSS.
use day::prelude::*;
use day_piece_settings::prelude::*;
use day_piece_webview::{JsHandle, LinkPolicy};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
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
// Gutenberg requires an identifiable client and a contact URL.
pub const USER_AGENT: &str = concat!(
    "Stanza-Redux/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Stanza-Redux/Stanza-Redux)"
);
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct Saved {
    id: String,
    title: String,
    author: String,
    #[serde(default)]
    cover: Option<String>,
}
pref_enum! {
    /// The reader typeface. The bridge takes its position in `ALL` (reader.js `FONTS`).
    enum ReaderFont {
        Serif = "serif",
        Sans = "sans",
        Mono = "mono",
        Montserrat = "montserrat",
        NotoSans = "noto-sans",
        NotoSerif = "noto-serif",
    }
}
pref_enum! {
    /// The reader's colors: three presets, or the custom paper and ink below.
    enum ReaderTheme {
        Light = "light",
        Sepia = "sepia",
        Dark = "dark",
        Custom = "custom",
    }
}
pref_enum! {
    /// Text columns on a wide window.
    enum Columns {
        One = "1",
        Two = "2",
    }
}
const DEFAULT_PAPER: &str = "#f4ead6";
const DEFAULT_INK: &str = "#30343b";
// The reader settings, one persisted value each (docs/settings.md in day). Every write lands in
// the platform store and in the signal the rows and the reader bridge read.
static FONT_SIZE: Pref<f64> = Pref::new("stanza.size", || 20.);
static LINE_HEIGHT: Pref<f64> = Pref::new("stanza.line", || 1.6);
static MARGINS: Pref<f64> = Pref::new("stanza.margin", || 24.);
static PARAGRAPH_SPACING: Pref<f64> = Pref::new("stanza.paragraph_spacing", || 0.6);
static FONT: Pref<ReaderFont> = Pref::new("stanza.font", || ReaderFont::Serif);
static THEME: Pref<ReaderTheme> = Pref::new("stanza.theme", || ReaderTheme::Sepia);
static COLUMNS: Pref<Columns> = Pref::new("stanza.columns", || Columns::One);
static PAPER: Pref<String> = Pref::new("stanza.paper", || DEFAULT_PAPER.into());
static INK: Pref<String> = Pref::new("stanza.ink", || DEFAULT_INK.into());
static JUSTIFIED: Pref<bool> = Pref::new("stanza.justified", || false);
static HYPHENATION: Pref<bool> = Pref::new("stanza.hyphenation", || true);
/// Hide the system status bar while a book is open (phones and tablets).
static HIDE_STATUS_BAR: Pref<bool> = Pref::new("stanza.hide_status_bar", || false);
/// A tap on the left edge turns forward, like the right edge, instead of back.
static LEFT_TAP_ADVANCES: Pref<bool> = Pref::new("stanza.left_tap_advances", || false);

thread_local! {
    /// The installed settings handle, for the reader's sheet (`App` stays `Copy`).
    static PREFS: RefCell<Option<Prefs>> = const { RefCell::new(None) };
}

fn position<T: PartialEq>(all: &[T], value: &T) -> usize {
    all.iter().position(|v| v == value).unwrap_or(0)
}

/// A `#rrggbb` setting as the packed integer the bridge's stylesheet takes.
fn rgb_u32(hex: &str, fallback: &str) -> u32 {
    let c = Color::parse(hex)
        .or_else(|| Color::parse(fallback))
        .unwrap_or(Color::BLACK);
    let q = |v: f64| (v.clamp(0., 1.) * 255.).round() as u32;
    q(c.r) << 16 | q(c.g) << 8 | q(c.b)
}

/// The preferences object the reader bridge takes (`stanza.load`, `stanza.configure`). Every
/// read is tracked, so a watcher over it runs again when any setting changes.
fn reader_config() -> serde_json::Value {
    serde_json::json!({
        "size": FONT_SIZE.get(),
        "line": LINE_HEIGHT.get(),
        "margin": MARGINS.get(),
        "paragraph_spacing": PARAGRAPH_SPACING.get(),
        "font": position(ReaderFont::ALL, &FONT.get()),
        "theme": position(ReaderTheme::ALL, &THEME.get()),
        "columns": position(Columns::ALL, &COLUMNS.get()) + 1,
        "paper": rgb_u32(&PAPER.get(), DEFAULT_PAPER),
        "ink": rgb_u32(&INK.get(), DEFAULT_INK),
        "justified": JUSTIFIED.get(),
        "hyphenation": HYPHENATION.get(),
        "hide_status_bar": HIDE_STATUS_BAR.get(),
        "left_tap_advances": LEFT_TAP_ADVANCES.get(),
    })
}

/// Settings and About (docs/settings.md in day): Appearance and Language from Day's catalog, the
/// reader settings as three groups, the app's name and version from Day.toml. On the desktops
/// this adds the Settings… and About menu items and their windows; the Settings section shows
/// the same pages everywhere, and the reader's sheet two of them.
fn install_preferences() -> Prefs {
    let custom = Signal::global(THEME.get_untracked() == ReaderTheme::Custom);
    watch(
        move || THEME.get(),
        move |t, _| custom.set_if_changed(*t == ReaderTheme::Custom),
    );
    let decimal = |v: f64| day::format_decimal(v, 1);
    let whole = |v: f64| day::format_decimal(v, 0);
    let prefs = Preferences::new()
        // Wide enough for a slider, its value and a translated title on one line.
        .window_size(640., 760.)
        .about(
            About::from_manifest()
                .link(day::tr("day-about-website"), "https://stanza-redux.app")
                .link(
                    day::tr("day-about-source"),
                    "https://github.com/Stanza-Redux/Stanza-Redux",
                ),
        )
        .group(general_group(res::locales::ALL))
        .group(
            // `reader`, not `reading`: that segment is the reader cover's route (`ReaderRoute`).
            Group::new("reader", res::str::settings_reading())
                .icon(res::vectors::prefs_reading)
                .section(
                    Section::new()
                        .title(res::str::typography())
                        .row(
                            rows::choice(
                                &FONT,
                                res::str::font(),
                                [
                                    (ReaderFont::Serif, res::str::serif()),
                                    (ReaderFont::Sans, res::str::sans()),
                                    (ReaderFont::Mono, res::str::mono()),
                                    (ReaderFont::Montserrat, res::str::font_montserrat()),
                                    (ReaderFont::NotoSans, res::str::font_noto_sans()),
                                    (ReaderFont::NotoSerif, res::str::font_noto_serif()),
                                ],
                            )
                            .id("reader-font"),
                        )
                        .row(
                            rows::slider(&FONT_SIZE, res::str::font_size(), 12.0..=40.0)
                                .step(1.)
                                .value_text(whole)
                                .id("font-size"),
                        )
                        .row(
                            rows::slider(&LINE_HEIGHT, res::str::line_height(), 1.0..=2.4)
                                .step(0.1)
                                .value_text(decimal)
                                .id("line-height"),
                        )
                        .row(rows::toggle(&JUSTIFIED, res::str::justified()).id("reader-justified"))
                        .row(
                            rows::toggle(&HYPHENATION, res::str::hyphenation())
                                .id("reader-hyphenation"),
                        ),
                )
                .section(
                    Section::new()
                        .title(res::str::page_layout())
                        .row(
                            rows::slider(&MARGINS, res::str::margins(), 8.0..=64.0)
                                .step(2.)
                                .value_text(whole)
                                .id("margins"),
                        )
                        .row(
                            rows::slider(
                                &PARAGRAPH_SPACING,
                                res::str::paragraph_spacing(),
                                0.0..=2.0,
                            )
                            .step(0.1)
                            .value_text(decimal)
                            .id("paragraph-spacing"),
                        )
                        .row(
                            rows::choice(
                                &COLUMNS,
                                res::str::columns(),
                                [
                                    (Columns::One, res::str::column_one()),
                                    (Columns::Two, res::str::column_two()),
                                ],
                            )
                            .segmented()
                            .id("reader-columns"),
                        )
                        .footer(res::str::columns_hint()),
                ),
        )
        .group(
            Group::new("theme", res::str::settings_reader_theme())
                .icon(res::vectors::prefs_theme)
                .row(theme_row())
                .section(
                    Section::new()
                        .row(color_row(
                            &PAPER,
                            || res::str::paper_color().format(),
                            "paper-color",
                            custom,
                        ))
                        .row(color_row(
                            &INK,
                            || res::str::ink_color().format(),
                            "ink-color",
                            custom,
                        ))
                        .footer(res::str::custom_hint()),
                ),
        )
        .group(
            Group::new("controls", res::str::settings_controls())
                .icon(res::vectors::prefs_controls)
                // Only where the platform has a status bar to hide (phones and tablets).
                .row(
                    rows::toggle(&HIDE_STATUS_BAR, res::str::hide_status_bar())
                        .id("reader-hide-status-bar")
                        .requires(Cap::StatusBarHidden),
                )
                .row(
                    rows::toggle(&LEFT_TAP_ADVANCES, res::str::left_tap_advances())
                        .id("reader-left-tap-advances"),
                ),
        )
        .install();
    PREFS.with(|p| *p.borrow_mut() = Some(prefs.clone()));
    prefs
}

fn theme_row() -> Row {
    // A menu, like Typeface: four segments crowd a phone's row and wrap the title.
    rows::choice(
        &THEME,
        res::str::theme(),
        [
            (ReaderTheme::Light, res::str::light()),
            (ReaderTheme::Sepia, res::str::sepia()),
            (ReaderTheme::Dark, res::str::dark()),
            (ReaderTheme::Custom, res::str::custom()),
        ],
    )
    .id("reader-theme")
}

/// A color well bound to a `#rrggbb` setting. The well is offered under the Custom theme; a
/// dimmed swatch stands in otherwise.
fn color_row(
    pref: &'static Pref<String>,
    title: fn() -> String,
    id: &'static str,
    custom: Signal<bool>,
) -> Row {
    rows::custom(move || {
        let parse = |hex: &str| Color::parse(hex).map(|c| Color::rgb(c.r, c.g, c.b));
        let fallback = parse(&pref.default()).unwrap_or(Color::BLACK);
        let color = Signal::new(parse(&pref.get_untracked()).unwrap_or(fallback));
        // Two-way: a pick writes the setting; a write from elsewhere (a reset) moves the well.
        watch(
            move || color.get(),
            move |c, _| {
                let hex = Color::rgb(c.r, c.g, c.b).to_hex_string();
                if pref.get_untracked() != hex {
                    pref.set(hex);
                }
            },
        );
        watch(
            move || pref.get(),
            move |hex, _| {
                if let Some(c) = parse(hex) {
                    color.set_if_changed(c);
                }
            },
        );
        row((
            label(title).grow_w(),
            when(
                move || custom.get(),
                move || day_piece_colorpicker::color_picker(color).id(id),
            )
            .otherwise(move || {
                column(())
                    .frame(28., 20.)
                    .background(move || color.get())
                    .corner_radius(6.)
                    .opacity(0.4)
            }),
        ))
        .spacing(12.)
        .height(48.)
    })
    .enabled(custom)
}

/// The reader's settings sheet: the Reading and Reader theme groups, one above the other in one
/// scroll.
fn reader_settings_sheet() -> AnyPiece {
    PREFS.with(|p| match &*p.borrow() {
        Some(prefs) => scroll(
            column((prefs.group_body("reader"), prefs.group_body("theme")))
                .spacing(0.)
                .grow_w(),
        )
        .grow()
        .any(),
        None => column(()).any(),
    })
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
    toc: Signal<usize>,
}
/// A JSON value the earlier Day prototype left in `day::prefs`, for the one-time library import
/// (`storage::Library::import_preferences`).
fn saved<T: serde::de::DeserializeOwned + Default>(key: &str) -> T {
    day::prefs::get(key)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn defaults() -> Vec<opds::Link> {
    [
        (
            res::str::catalog_standard().format(),
            "https://standardebooks.org/feeds/opds",
        ),
        (
            res::str::catalog_gutenberg().format(),
            "https://www.gutenberg.org/ebooks/search.opds/",
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
                        let prefs = reader_config();
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
fn http_client() -> day_part_http::Client {
    day_part_http::Client::builder()
        .user_agent(USER_AGENT)
        .timeout_total(std::time::Duration::from_secs(90))
        .build()
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
    let r = http_client()
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
    let prefs = install_preferences();
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
            // creators; a comma in a person's name is never treated as a delimiter. The
            // current index version already carries the current parser's metadata, so a book
            // that names no author is not parsed and indexed again on every launch.
            let books = match db.try_books() {
                Ok(books) => books,
                Err(e) => {
                    a.error(e.to_string());
                    return;
                }
            };
            for book in books {
                if db
                    .book(&book.id)
                    .is_some_and(|b| b.indexed == storage::SEARCH_INDEX_VERSION)
                {
                    continue;
                }
                if let Ok(mut parsed) = epub::open_saved(&book.id).await {
                    if let Some(old) = db.metadata(&book.id) {
                        parsed.metadata.supplement(&old);
                    }
                    if let Err(e) = storage::index_book(&db, &parsed).await {
                        a.error(e);
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
    // A change is pushed to the book live, from the half sheet or a menu command. A wide window's
    // settings unmount the web view instead, and closing them reloads the book with the current
    // settings.
    watch(reader_config, move |p, _| {
        if a.ready.get_untracked()
            && a.reader_open.get_untracked().is_some()
            && !(a.reader_settings.get_untracked() && !settings_over_book(a))
        {
            a.eval(format!("stanza.configure({p})"))
        }
    });
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
                        .enabled(FONT_SIZE.get() < 40.)
                        .action(|| FONT_SIZE.set((FONT_SIZE.get_untracked() + 1.).min(40.))),
                    menu_item(res::str::decrease_font().format())
                        .id("decrease-font")
                        .key("-")
                        .enabled(FONT_SIZE.get() > 12.)
                        .action(|| FONT_SIZE.set((FONT_SIZE.get_untracked() - 1.).max(12.))),
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
        // Settings is a section on every platform; on the desktops the Settings… window is a
        // second way to the same pages.
        .items(
            || vec![String::from("settings")],
            |key: &String| item(key.clone(), res::str::settings()).icon(res::vectors::tab_settings),
        )
        .destination(move |_: &String| prefs.page());
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
    let summary = |p: &opds::Publication| {
        let mut metadata = p.metadata.clone();
        metadata.description = p.description.clone();
        metadata
    };
    let initial = summary(&publication);
    // Feeds often list summaries; the complete entry adds the description, subjects and links.
    let shown = Signal::new(publication.detail.is_empty().then(|| initial.clone()));
    let current = Signal::new(publication.clone());
    let epub = Signal::new(publication.epub().cloned());
    let checking = Signal::new(false);
    let task = day::task(async move {
        let detail = current.get_untracked().detail;
        if !detail.is_empty() {
            if let Ok(feed) = browser::load_feed(detail).await {
                if let Some(full) = feed.entries.into_iter().next() {
                    current.update(|p| p.supplement(full));
                }
            }
            shown.set(Some(summary(&current.get_untracked())));
        }
        let p = current.get_untracked();
        if let Some(link) = p.epub() {
            epub.set(Some(link.clone()));
        } else if let Some(item) = p.archive_item() {
            checking.set(true);
            if let Ok((files, _)) =
                fetch_bytes(&opds::archive_files_url(&item), 1 << 20, None).await
            {
                epub.set(opds::archive_epub(&item, &files));
            }
            checking.set(false);
        }
    });
    Scope::current().on_cleanup(move || task.abort());
    let cover_url = publication.cover.clone();
    book_overview(
        publication.title,
        publication.author,
        "publication-title",
        covers::view(move || cover_url.clone(), 160., 220.),
        shown,
        column((
            button(res::str::download())
                .prominent()
                .enabled(move || epub.get().is_some() && !a.busy.get())
                .action(move || {
                    if let Some(link) = epub.get_untracked() {
                        let mut metadata = shown.get_untracked().unwrap_or_else(|| initial.clone());
                        metadata.acquisition = link.url.clone();
                        a.acquire(link.url, metadata);
                    }
                })
                .id("download-book")
                .height(50.)
                .grow_w(),
            label(move || {
                if epub.get().is_some() {
                    res::str::epub_only().format()
                } else if checking.get() || shown.get().is_none() {
                    res::str::epub_checking().format()
                } else {
                    match current.get().unavailable() {
                        opds::Unavailable::Account => res::str::unavailable_account(),
                        opds::Unavailable::Pdf => res::str::unavailable_pdf(),
                        opds::Unavailable::Audio => res::str::unavailable_audio(),
                        opds::Unavailable::Other => res::str::unavailable_other(),
                    }
                    .format()
                }
            })
            .font(Font::Footnote)
            .secondary()
            .id("publication-availability"),
            when(
                move || {
                    epub.get().is_none()
                        && !checking.get()
                        && shown.get().is_some()
                        && !current.get().web.is_empty()
                },
                move || {
                    button(res::str::view_on_web())
                        .bordered()
                        .action(move || open_link(&current.get_untracked().web))
                        .id("publication-web")
                },
            ),
            column((each(
                items(
                    move || {
                        current
                            .get()
                            .browse
                            .into_iter()
                            .enumerate()
                            .collect::<Vec<_>>()
                    },
                    |(i, l): &(usize, opds::Link)| format!("{i}:{}", l.url),
                ),
                move |link| {
                    let title = move || {
                        let l = link.get().1;
                        if l.rel == "author" {
                            res::str::more_by(l.title).format()
                        } else {
                            l.title
                        }
                    };
                    row((
                        label(title).grow(),
                        vector(res::vectors::chevron_forward)
                            .frame(12., 24.)
                            .decorative(),
                    ))
                    .spacing(12.)
                    .padding(Insets::symmetric(0., 10.))
                    .on_tap(move || {
                        let l = link.get().1;
                        a.browse_named(l.url, false, title())
                    })
                    .id_of(move || format!("publication-browse-{}", link.get().0))
                },
            ),))
            .align(HAlign::Leading),
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
/// Whether the reader's settings open as a half sheet over the book, so each change shows on the
/// page as it is made: in a compact (phone-width) main window. A wider window gives them the
/// whole reader instead, as does a desktop reader window.
fn settings_over_book(a: App) -> bool {
    !a.document_window && day::size_class().is_some_and(|c| c.width == WidthClass::Compact)
}

/// The half sheet's open state: open while the reader's settings are and the window is compact.
/// Pulling the sheet down (or Back) closes the settings.
#[derive(Clone, Copy)]
struct SettingsSheet(App);

impl Binding<Option<String>> for SettingsSheet {
    fn read(&self) -> Option<String> {
        (self.0.reader_settings.get() && settings_over_book(self.0)).then(|| "settings".into())
    }
    fn peek(&self) -> Option<String> {
        (self.0.reader_settings.get_untracked() && untrack(|| settings_over_book(self.0)))
            .then(|| "settings".into())
    }
    fn write(&self, v: Option<String>) {
        self.0.reader_settings.set(v.is_some());
    }
}

/// The reader's settings under their title and Done button, `top` and `bottom` points from the
/// edges. Only the title row takes side padding: each platform's groups bring their own margins.
fn reader_settings_panel(a: App, top: f64, bottom: f64) -> impl Piece {
    column((
        row((
            label(res::str::settings()).font(Font::Title),
            spacer(),
            button(res::str::done())
                .action(move || a.reader_settings.set(false))
                .id("reader-settings-done"),
        ))
        .padding(Insets::symmetric(20., 0.)),
        reader_settings_sheet(),
    ))
    .spacing(16.)
    .padding(Insets {
        top,
        bottom,
        ..Insets::default()
    })
}

fn reader(a: App) -> impl Piece {
    zstack((
        // In a wide window the settings take the reader's place, and closing them reloads the
        // book; under the half sheet the web view stays, and changes reach it live.
        when(
            move || a.reader_settings.get() && !settings_over_book(a),
            move || {
                reader_settings_panel(a, day::safe_area().top + 16., day::safe_area().bottom + 16.)
            },
        )
        .otherwise(move || {
            a.ready.set(false);
            reader_resources::view(a).js(a.js).id("book-webview")
        }),
        cover(SettingsSheet(a), move |_: &String| {
            reader_settings_panel(a, 20., day::safe_area().bottom)
        })
        .half_sheet()
        .unrouted(),
    ))
    .status_bar_hidden(|| HIDE_STATUS_BAR.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_books_without_covers_deserialize() {
        let book: Saved =
            serde_json::from_str(r#"{"id":"sample","title":"A","author":"B"}"#).unwrap();
        assert!(book.cover.is_none());
    }

    #[test]
    fn reader_colors_pack_as_the_bridge_reads_them() {
        assert_eq!(rgb_u32(DEFAULT_PAPER, DEFAULT_INK), 0xf4ead6);
        assert_eq!(rgb_u32("not a color", DEFAULT_INK), 0x30343b);
        assert_eq!(rgb_u32("", ""), 0);
    }

    #[test]
    fn every_bundled_locale_is_offered() {
        let tags: Vec<&str> = res::locales::ALL.iter().map(|(tag, _)| *tag).collect();
        assert_eq!(tags.len(), 13);
        for tag in ["en", "ar", "pt-BR", "zh-CN"] {
            assert!(tags.contains(&tag), "{tag} missing from res::locales::ALL");
        }
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
