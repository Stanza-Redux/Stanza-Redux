# Stanza Redux

A free EPUB reader built with Day and the App Fair template. Version 2 replaces the earlier
Stanza app while keeping its Apple and Android store identifiers.

The interface and store listing support Arabic, Chinese (Simplified), English, French, German,
Hindi, Indonesian, Italian, Japanese, Korean, Portuguese (Brazil), Russian, and Spanish.

## Use

```sh
day launch                         # default desktop toolkit
day launch -p ios-uikit
day launch -p android-material
day launch -p web-dom
```

Open **Library** to import an EPUB. An empty library receives *Alice in Wonderland* once on first launch; deleting it does not restore it. In **Catalogs**, open a catalog, search or follow its categories, select a book, and download its EPUB.
Downloaded books and reading positions remain available offline. **Reader preferences** include:
General (appearance, language), Reading (typeface, text size, line spacing, justification,
hyphenation, margins, paragraph spacing, columns), Reader theme (Light, Sepia, Dark, or custom
paper and text colors) and Controls (status bar, left-edge tap). On the desktops it is the app
menu's Settings… window; every platform also lists it as a section. The reader's Aa sheet shows
the Reading and Reader theme pages. **About** shows the version and build from `Day.toml`.
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

## Synchronizing a library

On each native device, open **Settings → Synchronization**, select or enter the WebDAV
server URL, and choose the same **App folder**. Enter your username and a password or app
password. For Nextcloud, copy the WebDAV address from Files settings. To use an existing
collection URL directly, leave **App folder** empty. **Verify and save**
checks authentication, listing, conditional writes, readback, and deletion in a disposable
child folder. Stanza stores the password in the operating system's keychain.

The native settings page separates server details, credentials, and synchronization status.
During a transfer it names the current book, shows its place in the upload or download
queue, and advances a progress bar with transfer data. Completion waits for verification
and indexing. Existing local EPUBs with hash filenames are migrated to readable titles
at startup, even while offline. The migration verifies their hashes and writes filename
receipts before removing the old paths; book IDs and reading state stay unchanged. Connection checks, metadata exchange, and finalization have explicit status messages. The Google Material Symbols Sync icon is
from [Google Fonts](https://fonts.google.com/icons?selected=Material+Symbols+Outlined:sync);
its Apache 2.0 license is bundled in `resource/assets/material-symbols-LICENSE.txt`.

Existing libraries merge on enrollment. Identical EPUB bytes identify the same edition.
Titles, author and genre details, bookmarks, recency, and reading positions synchronize while
the app is running. Reading-position changes request a metadata exchange after one second;
closing a book or changing app activation requests it immediately. Active apps poll metadata
every ten seconds, independently of bulk EPUB transfers. Opening a book checks for a newer
position with a five-second limit so offline reading remains available. Metadata already
matching its server hash is reused; other descriptions download two at a time. Open a title to fetch the EPUB or turn on **Keep all books offline**.
After a download, reading works without a connection. A deliberate move backward synchronizes.
When devices record conflicting reading positions offline, reopening the book offers the
available locations. Removing a book from the library propagates to the other devices.

Open `books/` in the app’s WebDAV folder to browse or download ordinary EPUBs. Native
devices use the same title-based filenames in `day-fs/books/`. For example, a book titled
*Pride and Prejudice* appears as `books/Pride and Prejudice.epub`. A conflicting edition
gets a short content-ID suffix. Devices agree on the spelling even when the server
distinguishes names that differ only in capitalization or Unicode normalization. The file identity stays stable for bookmarks and progress.

New libraries keep replication records and metadata in `.sync/`. WebDAV clients that hide
dot-prefixed folders show only the browsable `books/` folder. Existing libraries keep
their journal location and publish readable EPUBs on sync, including books held only
in the former chunk store. Update each participating device before syncing the new layout.
Keep the old sync directories for recovery. Once a book's EPUB and library record have been
confirmed on the server, Remove offers **This device only**, **This device and server**, and
**Cancel**. Local-only removal stays hidden on that device until explicitly re-imported and
leaves other libraries intact. Server removal publishes the shared deletion before removing
the verified remote EPUB; interrupted removals retry. Books not yet synchronized keep the
ordinary local confirmation. Files retained by older versions are not purged retroactively.
Use Stanza to import books and make catalog changes.
Other WebDAV clients can open or copy an EPUB directly. Moving or editing the server copy
will break its stored reference. Interrupted whole-file transfers retry from the beginning.

The WebDAV account can read the files and catalog. A fresh device can download the catalog
and resume without copying an existing SQLite file or device identity. The web edition uses
its browser-local library.

### Testing synchronization

`cargo test --lib sync::tests` runs the two-library recovery and checkpoint tests.
The live version runs the same two-library scenario with real HTTP. Create a disposable
collection named `stanza-fixture-<unique suffix>` on a local WebDAV server at
`127.0.0.1`. Set `STANZA_TEST_DAV` to its full URL and provide its test account via
`STANZA_TEST_USER` and `STANZA_TEST_PASSWORD`. Run
`cargo test --lib dav_library_roundtrip -- --ignored`. Delete the disposable collection
afterward. The test expects an empty collection for each run.

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
to exercise cold/warm OS delivery, reader reuse, closing, and reopening.
`tests/documents.mjs <local-web-url>` exercises the browser picker
and simulated launchQueue delivery; it does not test installation of a PWA file handler.

## Catalog discovery

Catalog sources show a favicon and, below the name, the published description of a default catalog or the hostname of one the reader added. Icons come from each catalog's own origin
(no third-party favicon service), share the bounded cover cache, and fall back to bundled
artwork when unavailable (distinct motifs for the built-in sources, a globe for custom catalogs).
The built-in sources currently deny cross-origin favicon fetches, so web uses this fallback. Feed pages honor Atom/OPDS 2 icon links and show the published
summary, result counts, and navigation descriptions. Empty catalogs offer guidance;
**Refresh** and **Try again** replace the current destination, so Back still returns to its parent.
The root offers Add when the saved catalog list is empty.

New libraries use Project Gutenberg's documented public entry point,
`https://www.gutenberg.org/ebooks/search.opds/`. Existing saved subscriptions remain intact.
Gutenberg's [feed documentation](https://www.gutenberg.org/ebooks/offline_catalogs.html)
says the XML service is expected to retire in 2027 in favor of a JSON OPDS 2 service. Its
preview, `https://opds-test.pglaf.org/opds/`, was exercised on 2026-10-14 (shelves, bookshelves,
subjects, search, facets, paging, publication detail and acquisition) and can be added through
**Add**; it is not a default until Gutenberg publishes it. Native requests include the contact
URL required by Gutenberg's [OPDS terms](https://www.gutenberg.org/policy/terms_of_use.html#opds-feed);
browsing and pagination remain user initiated. Web access still depends on each resource's
CORS policy.

The Internet Archive's OPDS 2 catalog, `https://archive.org/services/opds`, browses and searches
the same way. Most of its books are loans, PDFs or audiobooks, which Stanza explains on the
book page and links to the archive's web page. For an open-access scan, the feed lists only a
PDF; Stanza looks up the item's files through the archive's metadata API and offers the scan's
EPUB when there is one. The older `bookserver.archive.org` catalog did not respond on 2026-10-14.

Reader chrome respects safe-area insets and dynamic viewport height, shows the current chapter,
and follows the interface locale's direction. Pagination measures the toolbar and footer
(including their safe areas) so text and images stay clear of controls, even after rotation
or a change in control height. Hiding/showing controls preserves pagination. Home/End seek to the beginning/end of the current
chapter and preserve **Return to previous position**. Text-entry and control keys retain their
normal behavior.

## Default catalogs

The catalogs a new library starts with are not compiled in. They are the list in
`website/public/catalogs.json`, published at <https://stanza-redux.app/catalogs.json> and
fetched at every launch, so the list can change without an app release. The format is
described at the top of `src/default_catalogs.rs`: each catalog has a stable `id`, a `url`,
a `name` and `description` that are either one string or a map from language tags to strings,
and optional `locales`, which list the catalog only for readers with a matching language
anywhere in their device's preferred languages.

`resource/assets/catalogs.json` is the same file, bundled for a first launch with no network;
a test fails when the two differ. The app keeps the last list it fetched. Catalogs a reader
adds, renames, removes or reorders are theirs, and no published list changes them.

To try a list before publishing it, preview the website (below) and build the app against it.
Android needs `adb reverse tcp:4323 tcp:4323` first:

```sh
STANZA_CATALOGS_URL=http://127.0.0.1:4323/catalogs.json day launch -t ios-uikit --flavor dev
```

## Catalog covers

Catalog rows load covers asynchronously through `day-part-http`, with at most four requests
in flight. Native decoding and resizing run off the UI thread. Covers share a session cache
bounded to 64 entries and 16 MiB; missing/invalid covers are cached for that session too.
Rows display a bundled book skeleton until their own cover is ready. Reusing a row cancels
its prior task, replaces its previous image immediately, and rejects stale completions.
A cached cover is displayed synchronously. The same loader serves local covers and book overviews.

Catalog links push the destination immediately. Its content is empty while a centered loading
indicator is visible; the native Back button remains available. The destination owns the request,
including OpenSearch discovery. Popping it aborts the HTTP future/body and its cover requests;
completed covers remain cached. A page stays built while pages are pushed above it, so Back
shows the same list at the same scroll position, without fetching or rebuilding it. Late worker
results cannot put a popped destination back on the stack.
Failures appear in the destination instead of a modal alert.

## Local storage and upgrades

Books, reading positions, catalog names/order and import markers use **day-persistence** through
Day's `persistence` feature and `ModelContainer`. There is no package named
`day-part-persistence` in the current Day workspace. The database is `stanza-library.db`.
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
Settings are typed preferences (`day-piece-settings`), one key per setting in the platform's
own store; the reader reads them as signals.

Native databases live in the app's data directory, `Sqlite::app_data_dir()`
(`day::storage::data_dir()`). When the host sets `DAY_DATA_DIR`, the database is
`$DAY_DATA_DIR/stanza-library.db`; otherwise Day uses the platform's folder for the app. For
local unsandboxed macOS runs this is
`~/Library/Application Support/org.appfair.app.StanzaRedux/stanza-library.db`, and inside the App Sandbox the same
path within the app's container.
EPUBs and extracted covers remain separate files in `day-fs/books/` in that directory.
Back up the whole directory with the app closed; copying a live SQLite file alone can omit its WAL.
Builds before 2026-10 kept the database at `day/day-db/stanza-library.sqlite` and the books in
`day/day-fs/books/`, in a `day/` folder every Day app shared. On first launch the app moves both
into its own directory, after folding the database's WAL into the file; if the database cannot be
moved, the app opens it where it is and tries again on the next launch. The web build keeps its
OPFS file's original name, `stanza-library.sqlite`.

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
selecting a catalog pushes its feed, selecting a subcatalog pushes another feed,
and selecting a book pushes its details. Native Back buttons/gestures pop one level, all the
way back to the catalog list. Each page owns its feed snapshot; returning
to a parent does not refetch it or replace it with the child's content. Search results are
stack destinations too; further result pages join the current list. Failed requests leave the stack intact and show an
inline error with **Try again**.

On mobile the stack fills the Catalogs tab: the native header supplies Back and the page title,
and lists extend behind iOS’s floating tab bar or down to Android’s bottom navigation bar.
Manage and Search are native toolbar actions. Add OPDS Catalog lives in the desktop Catalogs menu and, on mobile and web, in the catalog list's toolbar; pages inside a catalog do not offer it. The shared add/edit form validates the feed asynchronously, shows its metadata, and prefills an editable title. Editing is available from the source row context menu and Manage. Management is a pushed page with native Back/Done.

A grouped OPDS 2 feed, such as Gutenberg's front page, is a page of shelves: each group shows
its title, size and a horizontally scrolling row of covers, and **See all** opens the group's
own feed. The feed's navigation links sit above the shelves as buttons. Other feeds are lists
with item counts beside navigation links. Facets (sort order, language, subject) live in the
**Sort and filter** toolbar pull-down, one submenu per facet group with the current choice
checked; choosing one replaces the current page, so Back still returns to its parent. The page
header names any non-default choice. Pages are named after the link that opened them, since
catalogs often title every feed alike. Search accepts OpenSearch descriptions and templates
as well as the RFC 6570 templates OPDS 2 catalogs publish (`{?query,title,author}`); results
are titled with the query. Further result pages load into the same list as it nears its end.

A book page fetches the catalog's complete entry when the feed lists only a summary, adding
the description, subjects and publisher. Its author, subject and "readers also downloaded"
links open their feeds.

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
| `src/opds.rs` | OPDS 1 Atom, OPDS 2 JSON, categories, groups, facets, pagination, search templates and archive.org EPUB lookup |
| `src/epub.rs` | EPUB ZIP/package/spine/contents parsing and embedded resources |
| `resource/assets/reader/reader.js` | Reflow, pagination, links, gestures and reading-position reports |
| `resource/assets/reader/readium-css/` | Vendored Readium CSS and its font resources |
| `resource/assets/reader/fonts/` | Bundled reader typefaces (Montserrat, Noto Sans, Noto Serif) and their OFL licenses |
| `resource/locales/{en,fr}/` | Interface translations |
| `dayscript/` and `tests/` | App walkthrough, local OPDS fixture and browser rendering tests |

Book rendering follows the web-view approach used by Readium. There is no Readium SDK dependency.
The CSS comes from the local Readium Swift toolkit assets, with its BSD license preserved in
`resource/assets/reader/READIUM-LICENSE.txt`; bundled fonts retain their licenses.
`tools/prepare_reader_css.py` regenerates the embedded styles after CSS changes.

Catalogs, covers and acquisitions use `day-part-http`. Native requests send `Stanza-Redux/<package version>` with the project contact URL in their User-Agent. The browser controls request headers and may replace that value.
The reader starts with a metadata-only manifest (identity, title, spine and contents).
`day-piece-webview::ResourceProvider` serves each requested XHTML section, stylesheet, image
and font directly from the EPUB archive. The engine resolves relative URLs, CSS imports and
nested resources; publication URLs are not rewritten into blobs. The reader shell shares the
provider origin through `ResourceProvider::with_site(res::assets::reader, ...)`.
Section changes abandon obsolete browser loads. Already-running synchronous reads can finish,
but do not update the new chapter. Native callbacks run off the UI thread; web callbacks run on
the WASM thread. The host caches at most one inflated ZIP entry and shares binary response
buffers without another application-side copy. There is no base64/chunk transport.

Native library readers use `day_part_filesystem::open_read` and seek directly in the stored ZIP. Web
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
- All twelve Day targets are declared. macOS AppKit/GTK/Qt, iOS UIKit, Android Material Components and Web DOM
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
DAY_SCRIPT_TARGET=android-material ANDROID_SERIAL=emulator-5554 bash -c '
  source tests/ci-fixture.sh
  day launch -p android-material --script dayscript/book-info.yaml
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

Stanza Redux is licensed under the [GNU General Public License, version 2 or later](LICENSE.txt). The bundled Readium CSS
and fonts retain their own license notices.

## Localization and resources

App-owned strings use generated `res::str` accessors in all 13 locale catalogs, including errors,
progress, units, accessibility text and reader controls. Worker errors carry a message constructor
and diagnostic details; presentation formats the message on the UI thread. The embedded reader
receives its translations from Rust. Publication metadata is displayed as supplied by the book or
catalog. Bundled assets use generated `res::assets` and `res::vectors` constants. The project and
global agent instructions record these requirements.

## Version 2 store preparation

`Cargo.toml` declares version 2.0.0 and `Day.toml` build 25. The v1 checkout records
1.4.4 / build 23; the intervening Day prototype used build 24. Both the base and
App Fair builds retain `org.appfair.app.Stanza-Redux` on Apple and
`org.appfair.app.Stanza_Redux` on Android. Existing listing IDs live in `[store]`.

`store/storefront.toml` supplies each language’s description, subtitle, Play short
description, keywords, release notes, promotional text, reviewer instructions, and URLs.
Day validates store length limits and maps language tags to Apple/Google spellings.

The store gallery uses eight named captures from `dayscript/store-walkthrough.yaml`.
It downloads three real books from the loopback OPDS fixture, exercises page turns,
and captures light, dark, and sepia pages, contents, settings, catalog details, and
the populated library. It waits for cover images before capturing. Test books and
their source hashes are documented in `tests/fixtures/books/README.md`; the extra
EPUBs are test fixtures, not shipped app assets.

```sh
# Bash, from the project root. Requires a booted simulator/emulator for mobile.
source tests/ci-fixture.sh
day launch -p macos-appkit --script dayscript/store-walkthrough.yaml
day launch -p ios-uikit --script dayscript/store-walkthrough.yaml
# On Android, set ANDROID_SERIAL to the device you selected.
adb -s "$ANDROID_SERIAL" reverse tcp:18765 tcp:18765
day launch -p android-material --android-device "$ANDROID_SERIAL" --script dayscript/store-walkthrough.yaml
```

Use a temporary `DAY_DATA_DIR` for a clean desktop capture library; do not delete
a personal library to prepare screenshots. If port 18765 is occupied, set
`STANZA_FIXTURE_PORT` and replace the port in a temporary copy of the walkthrough.

CI captures every locale in light and dark themes. iPhone and iPad profiles come
from the shared workflow; Android uses a 9:16 phone and a landscape tablet.
Regression scripts run once per device using `scripts-once`, avoiding a full
regression rerun for every marketing image. Captures are selected explicitly in
the storefront for each target, so diagnostic screenshots do not enter listings.

```sh
day lint
python3 -m unittest discover -s tests -p 'test_*.py'
cargo test --lib
day store stage -p ios-uikit
day store stage -p android-material
# Generate the website data and start Astro using a local daysite checkout:
DAYSITE_CONFIG="$PWD/website/site.toml" DAYSITE_THEME=/path/to/appsite \
  node /path/to/daysite/scripts/install-customization.mjs
DAYSITE_CONFIG="$PWD/website/site.toml" DAYSITE_THEME=/path/to/appsite DAY_BIN=day \
  node /path/to/daysite/scripts/preview.mjs
```

These commands stage local artifacts only. Store submission, signing credentials,
and publication remain separate from preparing this checkout. Review the generated
listings and screenshots before submitting the release.

## Website theme

`website/site.toml` selects [App Fair's appsite theme](https://github.com/appfair/appsite)
at `main`. The shared Day workflow fetches the theme and uses its Astro configuration,
App Fair header/footer, and journal pages automatically on qualifying website deployments.
The app keeps its own URL, accent color, platform choices, localized listings, screenshots,
and downloads. Project customization can be added in `website/daysite.config.mjs`;
`website/theme.css` is applied after the theme's styles.

This requires the published customization-capable daysite renderer, the theme repository,
and the updated `daybrite/actions` `v1` workflow. No separate website deployment workflow is
needed. For local previews, clone and install daysite, clone appsite, and set `DAYSITE_THEME`
to the appsite checkout's absolute path before installing customization dependencies or
running the preview. See Day's [app website guide](https://daybrite.dev/docs/websites)
for setup, customization, GitHub Pages, and custom domains.

## Remote catalog and cover restrictions

`website/public/censorship.json` publishes at `https://stanza-redux.app/censorship.json`.
The app fetches it **once per process launch**, not periodically or on foreground activation.
It starts with the last valid cached document, or `resource/assets/censorship.json` on a
first offline launch. A successful launch fetch updates the current UI, including cached
covers, catalog lists/shelves, and an open reader's cover resources. Downloaded EPUB files,
text, bookmarks, reading positions, and local library entries are never removed by these rules.

The published and bundled defaults are identical. Edit both when preparing an app release;
a website-only policy update needs only the published file. Deploying the website makes the
update available on the next launch of each connected app. Invalid JSON, unsupported schemas,
HTTP errors, oversized documents, and network failures retain the last valid policy.
Deleting the remote file does **not** disable censorship: publish `"enabled": false`, disable
individual rules, or publish an empty `rules` array. An accepted document replaces the prior
one completely, including when it deactivates restrictions. Offline devices retain their cached
policy until a later successful launch fetch.

The defaults restore *The Golden Ass* (case 5-1708000040527), cover *Confessions of Zeno*
(the title Google confirmed in case 3-9772000041706), and retain *The Chink in the Armour*
as a precaution. Google explicitly did not cite or clear the latter. These are exact edition
matches, not title matching or a general content filter. Cover replacement alone is not a
certification that an unrestricted catalog complies with Families requirements.

The version-one document contains `enabled`, a `messages` map, and `rules`. Every message
maps locale tags to plain text; `en` is required as the remote document's fallback. The shipped
messages cover all 13 app languages. The app uses its selected language, then device languages,
then the supplied English entry. Messages are limited to 240 characters and never interpreted
as HTML, JavaScript, or Fluent syntax. For example, a rule can use:

```json
{
  "id": "google-zeno",
  "enabled": true,
  "storefronts": ["google-play"],
  "match": {
    "identifiers": ["https://standardebooks.org/ebooks/italo-svevo/confessions-of-zeno/beryl-de-zoete"],
    "url_prefixes": ["https://standardebooks.org/images/covers/italo-svevo_confessions-of-zeno_beryl-de-zoete/"]
  },
  "cover_message": "google-cover",
  "hide_from_catalogs": false
}
```

- `cover_message` references a key in `messages`, such as `google-cover` or `apple-cover`.
  It replaces cover pixels with a placeholder carrying that localized message. Customize the
  wording in JSON without rebuilding the app. Omit it for a hide-only rule.
- `hide_from_catalogs: true` removes the matching publication from catalog results, search
  results, shelves, and subsequent pages. Existing local books remain available. It can be
  combined with `cover_message` to also replace covers of previously downloaded copies.
- `match.identifiers` uses exact publisher identifiers (including URNs), publication/detail
  links, and acquisition URLs. `url_prefixes` matches URL origin and directory path; prefixes
  must end in `/` and cannot cover an entire host. Titles and author names are never matched.
- `enabled` can disable the whole document or one rule. Only enabled rules for the build's
  storefront apply. If multiple cover rules match, the first supplies the message; any
  matching hide rule hides the catalog entry.
- `case` and `evidence` are optional administrative notes, never displayed to readers.

`STANZA_STOREFRONT` is a build-time choice: `google-play` (Android default),
`apple-app-store` (iOS default), or `unrestricted` (other platforms by default). Set
`STANZA_STOREFRONT=apple-app-store` for Mac App Store builds; set `unrestricted` for
Android distributions outside Google Play. No Apple rule is active in the initial registry.
Unrestricted builds do not fetch or enforce the registry. There is no reviewer-specific
behavior or user switch. Unknown storefront values fail the build.

For a local fixture server, compile with `STANZA_CENSORSHIP_URL=http://127.0.0.1:PORT/censorship.json`.
Production endpoints require HTTPS; only loopback HTTP is accepted for development. Fetches
request cache revalidation and accept at most 256 KiB. Test all build modes with:

```sh
STANZA_STOREFRONT=google-play cargo test --lib
STANZA_STOREFRONT=apple-app-store cargo test --lib
STANZA_STOREFRONT=unrestricted cargo test --lib
```

## Release dependency policy

Tagged App Fair releases use external Rust dependencies from crates.io and a checked-in
`Cargo.lock`. First-party crates within this repository remain workspace/path dependencies.
The tag workflow selects `day-deps: release`: it checks the lock with `cargo metadata --locked`,
rejects Git/local source overrides, preserves every resolved version, and installs the published
Day CLI matching the locked framework. It never updates or regenerates a release lock.
Development/main and scheduled compatibility builds may resolve different versions.

Prepare the release lock in a clean checkout without machine-local `.cargo` patches, run the
app's tests against that crates.io graph, and include the validated lock in the release commit
before tagging. Missing published dependencies must be released first; a Git fallback is not
permitted. After local patched development, regenerate/validate the crates.io lock in isolation
so the release does not inherit machine-local sources. App Fair submission rebuilds use the
same pinned lock and CLI. Committing or pushing still requires the maintainer's permission.
