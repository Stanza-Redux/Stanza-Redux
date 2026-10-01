# Stanza Redux

An EPUB reader built with Day and the App Fair template. The interface is available in English
and French. This checkout is local: no GitHub repository, release, or submission has been created.

## Use

```sh
day launch                         # default desktop toolkit
day launch -p ios-uikit
day launch -p android-mdc
day launch -p web-dom
```

Open **Library** to import an EPUB. An empty library receives *Alice in Wonderland* once on first launch; deleting it does not restore it. In **Catalogs**, open a catalog, search or follow its categories, select a book, and download its EPUB.
Downloaded books and reading positions remain available offline. **Settings** controls typeface,
font size, line height, paragraph spacing, justification, hyphenation, margins, preset themes,
custom paper/text colors, and one or two columns.
The library has native folders for Recents, Titles, Authors and Genres. Author and genre folders
contain searchable book lists. Recents shows the 50 most recently opened books, ordered by the
last successful open; importing alone does not add a book to this list. Phones use push navigation
above the bottom tabs. AppKit uses a sidebar, a library browser column and a persistent book-detail
pane. Local and catalog books share the same overview, with Open or Download as the primary action.
Opening a book presents the reader over the navigation.
Tap either edge or swipe horizontally to turn pages; tap the middle to show or hide the controls.
The toolbar offers Library, Contents, a bookmark toggle and Aa (reader settings).
Contents includes chapter search and a Bookmarks tab. Bookmarks are stored per book in the
Day database, ordered by reading position, and removed with the book. The list follows a
live query, including changes from another reader window.

The footer slider previews the position within the current chapter; releasing it moves to that
page. Chapter, bookmark and internal-link jumps offer a **Return to previous position** button.
Bookmarks use the reader's chapter/progression locator, so their page may shift when typography
or window dimensions change. They do not record an exact text selection.
 Page buttons and arrow/Page Up/Page Down
keys also work. Page turns slide; reduced-motion preferences disable the animation. Short or canceled
swipes return to the current page. Two columns require a viewport at least 640 points wide.
Desktop View → Full Screen uses the platform's full-screen window mode.

Day and `day-piece-webview` are git dependencies. To build against local checkouts, the ignored
`.cargo/config.toml` patches both git sources to their paths; a build without that file fetches
them from GitHub. Use `/opt/src/github/daybrite/day/target/debug/day` with these local checkouts.

## Opening EPUB documents

File → Open Book (⌘O / Ctrl+O) imports a local EPUB. Desktop file imports open a separate
reader window; mobile imports use the existing window. On macOS, Finder/Open With and dragging
an EPUB onto the app or Dock icon work with the app stopped or running. Other platforms use
their registered file handler. Opening identical contents under another filename reuses the
library entry, reading position, and existing reader window. The View menu supplies Increase /
Decrease Font Size (⌘+ / ⌘− on macOS; Ctrl on other desktops).

`Day.toml` declares `[[file_types]]`; `src/documents.rs` shares one import path between the
native picker and `day::on_open_files`. Reads are asynchronous and limited to 64 MiB. Day
handles native registration and temporary provider access. See Day's document guide for
installer/PWA requirements and untested-platform limitations.

Run `python3 tests/desktop-documents.py macos-appkit /path/to/day` after launching that build
to exercise cold/warm OS delivery, reader reuse, closing, and reopening. The script also accepts
`macos-gtk` and `macos-qt`. `tests/documents.mjs <local-web-url>` exercises the browser picker
and simulated launchQueue delivery; it does not test installation of a PWA file handler.

## Catalog covers

Catalog rows load covers asynchronously through `day-part-http`, with at most four requests
in flight. Native decoding and resizing run off the UI thread. Covers share a session cache
bounded to 64 entries and 16 MiB; missing/invalid covers are cached for that session too.
Rows display a bundled book skeleton until their own cover is ready. Reusing a row cancels
its prior task, replaces its previous image immediately, and rejects stale completions.
A cached cover is displayed synchronously. The same loader serves local covers and book overviews.

Catalog links push the destination immediately. Its content is empty while a centered loading
indicator is visible; the native Back button remains available. The destination owns the request,
including OpenSearch discovery. Popping it aborts the HTTP future/body. Hidden lists dispose their
row scopes and cancel cover requests; completed covers remain cached. Back restores the cached feed
without fetching it again. Late worker results cannot put a popped destination back on the stack.
Failures appear in the destination instead of a modal alert.

## Local storage and upgrades

Books, reading positions, catalog names/order and import markers use **day-persistence** through
Day's `persistence` feature and `ModelContainer`. There is no package named
`day-part-persistence` in the current Day workspace. The database is `stanza-library.sqlite`.
`books` holds EPUB SHA-256 identities, reading positions and last-opened timestamps;
`catalogs` holds subscriptions and ordering. `book_metadata` uses Day's JSON codec.
`library_folders` and Day's generated `library_memberships` join represent authors and genres.
`book_contents` holds chapter text for full-text indexing; `imports` records completed imports.
Authors remain separate creator values; commas in names are not separators.

Library, recents, author/genre and saved catalog lists bind directly to Day live queries.
Search uses Day FTS5 across title, author, genres, description and chapter text; chapter bodies
are queried through a relationship and are not loaded into book-list rows. Search terms are
literal, case/diacritic-insensitive tokens, combined with AND within each indexed document.
Recents applies the search before its limit of 50 books. Existing EPUBs are indexed once per
search-index version. EPUB parsing can run off the UI thread; all database work is synchronous.

A staged migration preserves the old schema's metadata, positions and reading history.
Typed relationships cascade book deletion to metadata, chapter text and memberships; shared
author/genre folders survive while referenced. Imports stage book, metadata, relationships and
chapter text in one database save. Assets are written first and may leave replaceable orphan
files after interruption. Preferences and the isolated v1 importer retain their separate roles.

Save errors retain pending edits for retry; imports and catalog commands use checked reads so
storage failure cannot be mistaken for an empty database. Library/catalog navigation checks
for commits from other processes. Reader windows in one process share the same container.
Appearance preferences remain in `day::prefs`.

Native databases live in `Sqlite::app_data_dir()` under `day-db/`. When the host sets
`DAY_DATA_DIR`, the database is `$DAY_DATA_DIR/day-db/stanza-library.sqlite`; otherwise Day uses
its platform data directory. For local unsandboxed macOS runs this is
`~/Library/Application Support/day/day-db/stanza-library.sqlite`.
EPUBs and extracted covers remain separate files in the corresponding `day-fs/books/` directory.
Back up both directories with the app closed; copying a live SQLite file alone can omit its WAL.

Web uses Day's SQLite worker and OPFS, scoped to the browser origin. It requires cross-origin
isolation (COOP/COEP headers, supplied by `day launch`) and an available OPFS store. A storage
failure is reported; the app does not silently substitute a temporary in-memory library.
Another tab can hold the store's exclusive lock. A different localhost port is a different origin.

The first launch copies the earlier Day prototype's JSON library/catalog preferences into SQLite.
Those preferences are retained for recovery. New library writes and positions go only to SQLite.
Removing every catalog leaves an empty catalog list; defaults are seeded only during initial setup.

### Stanza Redux v1 import

The Apple bundle ID remains `org.appfair.app.Stanza-Redux`; Android keeps
`org.appfair.app.Stanza_Redux`. Other targets use `org.appfair.app.StanzaRedux`. The local successor version is 1.4.5, build 24
(v1 was 1.4.4, build 23).
Installing over v1 also requires the original signing identity and an acceptable version/build
number; matching an ID does not bypass platform signing or downgrade checks.

`src/legacy.rs` alone understands the old `BOOK` table and Readium locator JSON. At startup it
looks for `library.sqlite` in the existing Apple Application Support directory or Android files
directory and copies EPUBs from the old Documents/Books (Android: files/Books) directory.
On macOS it also checks the old app's standard sandbox container. An exported old container can
be inspected with `STANZA_LEGACY_DIR=/path/to/container day launch`; this is a recovery/test option,
not needed for an in-place mobile upgrade.

The importer opens the old database read-only, rejects paths outside its documents directory,
parses each EPUB with the new reader, preserves its saved title/author, regenerates its cover,
and translates the Readium href/chapter and within-chapter progression. The new schema stores
no Readium payloads or old file paths. Whole-book percentages are not mistaken for chapter
progress; an absent locator falls back to the saved chapter start.

A book, its position, and its completion marker commit together. Failed books are reported and
retried on the next launch; successful imports are not repeated. Content hashes prevent duplicate
EPUBs, and an existing new-library reading position wins. The old database and book files are
never moved or deleted. Old bookmarks, reader settings and v1 catalog configuration are retained
in the old store but are not imported by this adapter.

## Catalog navigation

Catalogs use Day's `nav_stack` on every platform. The root is a native catalog list;
selecting a catalog pushes its feed, selecting a subcatalog or facet pushes another feed,
and selecting a book pushes its details. Native Back buttons/gestures pop one level, all the
way back to the catalog list. Each page owns its feed snapshot; returning
to a parent does not refetch it or replace it with the child's content. Search results and
pagination are stack destinations too. Failed requests leave the stack intact and show a
dismissible native alert.

On mobile the stack fills the Catalogs tab: the native header supplies Back and the page title,
and lists extend behind iOS’s floating tab bar or down to Android’s bottom navigation bar.
Manage and Search are native toolbar actions. Add OPDS Catalog lives in the mobile overflow menu and the desktop Catalogs menu (also a web toolbar action). The shared add/edit form validates the feed asynchronously, shows its metadata, and prefills an editable title. Editing is available from the source row context menu and Manage. Management is a pushed page with native Back/Done.

Forward restores popped pages; following a new link clears that forward history. Cached
stack pages live only for the current session and cannot be opened through arbitrary deep links.

**Manage** edits, removes, or reorders HTTP(S) catalogs. Changes are saved
immediately. Left/Right arrows navigate history after browsing; text fields keep their editing
keys. Horizontal trackpad/wheel gestures navigate history on AppKit, GTK, Qt, Web and Android
pointer devices; UIKit uses its two-finger pan recognizer. One gesture produces one navigation,
including its inertial tail. XAML/ArkUI retain native Back; native container wheel gestures are not available there. In the reader, arrows and
horizontal wheel gestures turn pages through the webview on supported web engines. When a
focused chapter is replaced, keyboard focus moves to the persistent reader shell so page-turn
keys continue working across chapter boundaries, including inside the web app’s nested iframe.

## Implementation

| Source | Responsibility |
|---|---|
| `src/lib.rs` | Native UI, HTTP tasks, acquisition, library, preferences and reader bridge |
| `src/storage.rs` | Current SQLite schema, explicit saves and EPUB asset storage |
| `src/legacy.rs` | Read-only v1 discovery and import adapter |
| `src/browser.rs` | Native catalog stack, feed lists and catalog management |
| `src/opds.rs` | OPDS 1 Atom, OPDS 2 JSON, categories, groups, facets, pagination and search |
| `src/epub.rs` | EPUB ZIP/package/spine/contents parsing and embedded resources |
| `resource/assets/reader/reader.js` | Reflow, pagination, links, gestures and reading-position reports |
| `resource/assets/reader/readium-css/` | Vendored Readium CSS and its font resources |
| `resource/locales/{en,fr}/` | Interface translations |
| `dayscript/` and `tests/` | App walkthrough, local OPDS fixture and browser rendering tests |

Book rendering follows the web-view approach used by Readium. There is no Readium SDK dependency.
The CSS comes from the local Readium Swift toolkit assets, with its BSD license preserved in
`resource/assets/reader/READIUM-LICENSE.txt`; bundled fonts retain their licenses.
`tools/prepare_reader_css.py` regenerates the embedded styles after CSS changes.

Catalogs, covers and acquisitions use `day-part-http`. Native requests send the legacy
`Stanza-Redux/1.4.4` User-Agent. The browser controls request headers and may replace that value.
The reader starts with a metadata-only manifest (identity, title, spine and contents).
`day-piece-webview::ResourceProvider` serves each requested XHTML section, stylesheet, image
and font directly from the EPUB archive. The engine resolves relative URLs, CSS imports and
nested resources; publication URLs are not rewritten into blobs. The reader shell shares the
provider origin through `ResourceProvider::with_site(res::assets::reader, ...)`.
Section changes abandon obsolete browser loads. Already-running synchronous reads can finish,
but do not update the new chapter. Native callbacks run off the UI thread; web callbacks run on
the WASM thread. The host caches at most one inflated ZIP entry and shares binary response
buffers without another application-side copy. There is no base64/chunk transport.

Native library readers use `day_part_fs::open_read` and seek directly in the stored ZIP. Web
currently retains the compressed EPUB buffer because Day's OPFS file API exposes whole-file
reads; individual entries are still inflated on demand. Imports and full-text indexing must read
all chapter text once, independently of the reader's section-loading lifecycle. The browser test
exporter deliberately expands a small fixture; it is not the application's load path.

Book scripts are removed, the document
runs in an iframe, and a content policy denies all script sources and remote book resources.
The iframe permits scripts at the sandbox layer because WebKit otherwise blocks even the trusted
parent's event listeners; EPUB script elements/event attributes are removed and `script-src 'none'`
is enforced separately, in both response headers and a document meta policy. The rendering tests exercise both the sanitizer and the script policy.
The trusted reader shell exchanges JSON with the native UI through `day-piece-webview`.

## Scope and platform limits

- Public OPDS 1/2 feeds and direct EPUB acquisition are supported. Account/login flows, DRM,
  loan fulfillment, payment, PDF/comic formats, fixed-layout EPUB, media overlays and speech
  are not implemented. IDPF and Adobe embedded-font obfuscation are supported.
- Reflowable EPUB 2/3 books may contain publisher CSS. Unusual scripting-dependent layouts,
  vertical writing and complex right-to-left layouts need further testing.
- Catalogs are limited to 8 MiB, covers to 4 MiB and downloaded EPUBs to 64 MiB. The archive
  parser also limits declared expanded resources to 256 MiB total and 32 MiB per resource.
- Web resource views require HTTPS or localhost and service workers; `day build` stages the
  piece’s scoped worker automatically. The reader namespace lasts only while the app is open.
- Web catalogs must allow CORS. HTTPS pages cannot fetch HTTP catalogs. No proxy is installed.
  Android release builds require HTTPS; debug builds permit HTTP only to localhost for tests.
  Other native transport restrictions also apply.
- All twelve Day targets are declared. macOS AppKit/GTK/Qt, iOS UIKit, Android MDC and Web DOM
  can be exercised locally. Windows and Linux hosts need validation on those systems.
  Windows GTK/Qt require WebView2 Runtime; Linux GTK requires WebKitGTK, and Qt uses Qt WebEngine.
- HarmonyOS Rust compilation is checked locally. HAP packaging currently stops at a missing
  `@ohos/hvigor-ohos-plugin` in the installed SDK; emulator testing belongs in CI.

The unused template `src/pages/` sources remain in the checkout and are not compiled.

## Tests

See [VALIDATION.md](VALIDATION.md) for the results and remaining platform checks.

```sh
cargo test --lib
python3 tests/catalog_server.py
# In another terminal; Android also needs: adb reverse tcp:18765 tcp:18765
day launch -p macos-appkit --script dayscript/demo.yaml --script dayscript/catalog.yaml --script dayscript/browser.yaml
```

CI uses `script-setup: source tests/ci-fixture.sh` in the shared Day workflow. It starts the
fixture after device boot, waits for its health check, forwards port 18765 for Android/Harmony,
and stops the server on exit. The `script-setup` input must be available on the shared workflow's
`v1` reference before this workflow can run. To use the same setup locally, from the project root:

```sh
DAY_SCRIPT_TARGET=android-mdc ANDROID_SERIAL=emulator-5554 bash -c '
  source tests/ci-fixture.sh
  day launch -p android-mdc --script dayscript/book-info.yaml
'
python3 -m unittest discover -s tests -p 'test_*.py' -v
```

The fixture binds loopback only. Stop an existing server on port 18765 before using this wrapper;
it refuses to reuse another process's server. Web CI runs all scripts in one browser session to
preserve OPFS storage, so walkthroughs return to the library/catalog root before finishing.

For persistence regression coverage, run `dayscript/library.yaml`, `dayscript/persistence.yaml`
and `dayscript/catalog-persistence.yaml` on each local target. The catalog test needs the fixture
server above. `NODE_PATH=/path/to/node_modules node tests/persistence-browser.cjs <local-web-url>`
checks an old SQLite schema through real Chromium OPFS storage and reloads. It uses fresh
temporary profiles/origins and refuses to overwrite existing storage. `BROWSER=webkit` runs
the same diagnostic in WebKit; the synchronous worker still fails to initialize on this host.

Repeat the Day scripts for each local target. The fixture serves Atom and JSON feeds, OpenSearch,
and a real EPUB; it records request User-Agents. The scripts check reader geometry and content,
turn pages, change appearance, search a catalog, acquire a book, follow nested facets and manage
custom catalogs. `tests/browser-input.mjs <local-web-url>` checks real browser keys/wheels, field
isolation and catalog persistence after a reload. Use `--locale fr` for French.
For automated Web screenshots configure the Playwright driver shown by `day web driver`.

Browser layout and script-isolation tests use a local Playwright installation:

```sh
cargo run --quiet --example export_book -- resource/assets/Alice.epub > /tmp/stanza-book.json
PLAYWRIGHT_ROOT=/path/containing/node_modules node --test tests/reader.mjs
```

The public-feed probe performs read-only requests through the same HTTP package:

```sh
cargo run --example probe_catalog -- --acquire https://www.gutenberg.org/ebooks.opds/
```

## License

AGPL-3.0-only with the App Fair Distribution Exception. See `LICENSE.txt` and
`LICENSE-EXCEPTIONS.txt`. Readium CSS and font license notices are retained separately.

## Localization and resources

App-owned strings use generated `res::str` accessors in English and French, including errors,
progress, units, accessibility text and reader controls. Worker errors carry a message constructor
and diagnostic details; presentation formats the message on the UI thread. The embedded reader
receives its translations from Rust. Publication metadata is displayed as supplied by the book or
catalog. Bundled assets use generated `res::assets` and `res::vectors` constants. The project and
global agent instructions record these requirements.
