# Local validation

## CI failure investigation — 2026-09-30

Investigated [run 36671873835](https://github.com/Stanza-Redux/Stanza-Redux/actions/runs/36671873835).
The OPDS fixture was never started by CI, causing catalog validation failures and subsequent
walkthrough failures. Two example programs also lacked the required SPDX header. Portable
macOS/Windows GTK and Qt jobs attempted packaging that Day does not implement.

Changes:

- Start and health-check the OPDS fixture through the shared workflow's new `script-setup` input;
  forward its port after Android/Harmony device boot and clean it up on exit. Cancellation
  fixture responses now permit browser CORS. Four Python fixture tests run in CI.
- Add the missing example license headers. Share the catalog loading indicator rather than
  declaring its ID twice. Day's route lint now recognizes formatted `.item_icon(...)` entries.
- Keep compilation and walkthroughs for portable GTK/Qt targets, but skip unsupported packaging
  and omit those targets from package-consumer matrices.
- Install WebView2 explicitly for Windows XAML. GTK resource responses now include the HTTP
  Content-Type header needed for script/style loading with `nosniff`. These platform fixes
  require Linux/Windows CI confirmation.
- Fix Day's relative Back routing: inactive tabs do not consume navigation; a reader cover and
  its internal stack take precedence over the covered library. Walkthroughs now clean up
  navigation state explicitly for the shared web session.

Local results:

- Full Chromium app walkthrough suite: **449/449 steps**, 39 screenshots.
- AppKit reader/settings walkthrough: **54/54 steps**.
- Updated book-info walkthrough: **53/53** on iPhone UIKit and Android MDC; the original
  **50/50** also passed on AppKit before the added navigation cleanup steps.
- Stanza Rust tests: **27 passed**; fixture tests: **4 passed**; App Fair lint: **6 rules passed**.
- Day mock navigation/UI suite: **212 passed**; route-lint regression and `cargo check` passed.
- Shared workflow tests: **68 passed**; `actionlint` passed. Webview Rust tests: **19 passed**.
- Full WebKit remains blocked by the previously recorded synchronous SQLite-worker stack
  overflow. No async persistence change was attempted.
- Harmony Rust compilation passed; HAP assembly still fails because the local SDK lacks
  `@ohos/hvigor-ohos-plugin`. No local Harmony emulator was launched.

These are local results, not a successful rerun of the hosted matrix. The shared workflow must
expose `script-setup` on `v1`, and CI must resolve the fixed Day/webview revisions, before the
app workflow can consume all fixes.


## EPUB resource-provider migration — 2026-09-29

- `stanza.load` receives metadata only. A provider mounts the generated reader asset directory
  and serves manifest-listed EPUB resources by path. Chapters navigate to actual document URLs;
  publisher-relative image, CSS, font and link paths are preserved. The base64/chunk bridge and
  blob URL rewriting have been removed.
- Native reads use a seekable Day file handle on workers; web retains compressed bytes and
  inflates requested entries on the WASM thread. One ZIP entry is cached. Immutable binary
  response bodies are shared with the provider.
- App unit tests: 27 pass, covering lazy loading, exact UTF-8 bytes, unknown/traversal paths,
  metadata-only serialization, and active-content removal without rewriting resource URLs.
- Browser reader tests: 6/6 pass in Chromium/WebKit, covering pagination, columns, restoration,
  script isolation, keyboard focus, untouched sections, cyclic CSS, rapid section replacement
  and absence of blob URLs.
- The 23-step `dayscript/reader-sections.yaml` passes on macOS AppKit, iOS UIKit, Android MDC
  and Chromium web DOM through the real provider. It changes chapters rapidly, reconfigures
  pagination, verifies the cover’s relative image URL, captures a chapter and closes the reader.
- The real WebKit app run is blocked before opening the library by a SQLite-worker
  `RangeError: Maximum call stack size exceeded`. The provider demo passes 37/37 in WebKit,
  and the standalone reader tests pass in both engines. Full-app web results above use Chromium.
- The provider demo has separate relative-resource coverage on AppKit, UIKit, MDC, GTK, Qt and
  web DOM. Linux and Windows adapters require their native CI runners. Harmony Rust compilation
  passes; local HAP packaging lacks `@ohos/hvigor-ohos-plugin`. No Harmony emulator was launched.

## Day persistence adoption — 2026-09-29

- Library navigation and full-text search: 102/102 dayscript steps pass on iOS UIKit,
  Android MDC, macOS AppKit and web DOM (Chromium). This includes chapter-only search,
  metadata search, safe handling of FTS punctuation, recents and author/genre folders.
- Catalog live-query mutations: 25/25 steps pass on those four targets. The scenario adds,
  validates, renames, reorders and removes a synthetic catalog. iOS ran all three scripts
  together (127/127); the other targets reran the catalog script after correcting its
  missing validation step.
- App unit tests: 23 pass, including migration of the previous SQLite schema, retained
  metadata/position/recency, relation updates, FTS chapter extraction and cascading deletion.
- The isolated Chromium OPFS test migrates a synthetic old database, searches migrated
  metadata, opens details and repeats after reload. Test profiles and origins are temporary;
  seeding refuses nonempty storage. `BROWSER=webkit` retains a diagnostic run, which currently
  fails to initialize the SQL worker on this host. It is not counted as passing.
- Day persistence/SQLite-worker tests pass, with new coverage for String/UUID FTS keys,
  indexed-key backfill, tokenizer migration, VACUUM/reopen, related FTS live queries,
  failed-save retry, failed-query retention, pre-commit query reads and worker batch migration.
  The native CI matrix enables the persistence list adapter on macOS, Linux and Windows;
  the SQLite worker’s native engine tests run on macOS/Linux (its C shim has no MSVC port). The web shim
  regressions (4 tests) also run in the web CI job. Remote CI was not triggered.
- HarmonyOS Rust compilation passes. HAP packaging remains blocked by the local SDK's
  missing `@ohos/hvigor-ohos-plugin`; no emulator was launched.
- Persistence remains synchronous, including the browser worker bridge. No asynchronous
  database API or background persistence scheduler was introduced.

## Document opening and catalog editor — 2026-09-29

- Builds pass: macOS AppKit/GTK/Qt, iOS UIKit, Android MDC, web DOM. HarmonyOS Rust compile-check
  passes; its ArkTS/HAP and runtime were not validated. Windows and Linux native activation
  still require their own hosts/CI.
- Native macOS Launch Services tests pass on AppKit, GTK and Qt: cold launch with an EPUB,
  warm activation, repeated open without duplicate windows, close and reopen.
- Native AppKit keyboard events verify Command-minus and Command-plus change reader text size,
  and Command-O opens the EPUB picker.
- iOS and Android native picker imports pass; Android ACTION_VIEW file activation passes.
  These checks do not cover every external content provider or sandbox grant combination.
- Chromium and WebKit browser file chooser and simulated launchQueue tests pass. The four
  reader rendering/keyboard regression tests also pass.
- iOS and Android catalog creation/validation, navigation, management, loading and cancellation
  flows pass. Chromium's full catalog editor/persistence test passes.
- WebKit's catalog editor test currently exposes an OPFS SQL-worker stall during a write;
  the UI reports the save error. It is not counted as a passing test. Private WebKit contexts
  also lack usable persistent storage; browser tests use persistent profiles.
- App unit tests: 20 pass, including catalog validation/editing/order, content identity and
  reading-position retention, and persistence of the sample-book deletion marker.
- Day document tests cover cold/reentrant delivery, scope disposal, bounded reads, manifest
  generation, safe registration removal, and escaping. Documentation builds and internal
  link checks pass.

The catalog editor uses Day's fullscreen cover presentation with a bounded-width form; it is
not a custom native macOS sheet. GTK's widget snapshot cannot capture its Cocoa webview overlay;
OS delivery and reader JavaScript assertions are used for its secondary-window tests.

## Web reader keyboard focus — 2026-09-29

- `tests/reader.mjs`: **4/4** tests pass in Chromium and WebKit. The new nested-iframe
  test failed in both engines before the fix, using actual browser key presses.
- Regression coverage: repeated forward/backward chapter crossings, Page Up/Down,
  normal/reduced motion, section links, settings relayout, and preserving focus in a
  host input while a chapter transition finishes.
- The rebuilt web-dom app passed 20 real arrow presses in Chromium and 18 in WebKit
  through the bundled Alice book, crossing three chapter boundaries forward and backward
  after one initial click. WebKit used a persistent context for the library’s OPFS storage.
- `cargo fmt --check` and the web-dom build pass. No Day/webview framework changes were needed.

The reader transfers focus synchronously from a chapter iframe to its stable shell before
replacing the chapter document or removing the outgoing iframe. Async completion does not
unconditionally refocus the reader, so it cannot interrupt typing in the surrounding app.

## Library navigation, cancellation and localization — 2026-09-29

- Android MDC: **244/244** DayScript steps across `library.yaml`, `catalog.yaml`,
  `browser.yaml`, `catalog-loading.yaml`, `book-info.yaml`, `catalog-cancellation.yaml`,
  and `catalog-thumbnails.yaml`.
- iOS UIKit and macOS AppKit: **210/210** steps each for library, catalog, browser,
  loading and book-info flows, plus **23/23** cancellation steps each through `day drive`.
- Stanza: **17 unit tests**. Day: **210 component/mock tests**, including a new regression
  for updating a loaded route title without rebuilding its destination.
- Chromium and WebKit: reader tests pass for pagination, page turns, settings, position
  restoration and publication-script isolation.
- HarmonyOS and web: library compile-checks pass. No HarmonyOS emulator was launched.

The library tests cover Recents, Titles, Authors, Genres, search, local metadata, opening a
book and changing language. Storage tests cover distinct author memberships (including commas
in names), genre filtering, history limits/order, persistence across reopen, position retention,
and removal of associated records. A worker-error test verifies that the UI's selected locale
is used at presentation time.

The cancellation fixture streams catalog and cover bodies slowly. Its server records a socket
disconnection for each transfer after Back, and exposes a success feed only after **both** were
cancelled. The test verifies that a loading destination replaces its parent immediately and
that the parent returns without another feed fetch. Run cancellation flows sequentially against
one fixture server; each flow resets its counters. Downloading a book still uses a separate
cancellable acquisition operation.

The iOS Xcode invocation with `-target Runner -arch arm64` stalled during destination discovery
on this host. The successful local build used `-scheme Runner -destination
'generic/platform=iOS Simulator' ARCHS=arm64`, then `day launch --skip-build`. No project or
signing settings were changed for that workaround.

All app-owned text and bundled resource references were audited. English and French resources
supply errors, progress, units, accessibility text and embedded reader chrome. Metadata from
publications and synthetic test fixtures remain separate from app-owned translations. Global
and project agent instructions now require generated localization/resource accessors.

Earlier validation follows; its shelf-specific descriptions and counts are historical.

## Async catalog thumbnails — 2026-09-29

The current iOS UIKit and Android MDC builds pass **158/158 Dayscript steps each**:
`catalog.yaml`, `browser.yaml`, `catalog-loading.yaml`, `book-info.yaml`, and
`catalog-thumbnails.yaml`. Both apps were rebuilt against the local Day checkout.
These runs include delayed, missing and corrupt covers, real nonzero image frames,
remote/local metadata, acquisition, native navigation, and reader settings.

`tests/catalog_server.py` provides `/many`, a 120-book feed with distinct colored covers
and staggered responses. Rapid native Android flings and repeated native iOS table scrolls
showed placeholders on reused rows, followed by the matching covers without further scrolling.
Screenshot pixel checks matched covers to seven visible book numbers on each platform.

The iOS failure was reproduced in both the native hierarchy (decoded UIImage, zero-size
UIImageView) and a Day mock regression. Fixed frames stopped measure invalidation before
it reached detached list-cell roots. Day now propagates placement dirt separately through
those boundaries; list and tree regressions cover conditional image insertion without
another native bind. UIKit and Android also clear an explicitly empty image source.
Stanza keeps one image view per cover, uses a bundled book skeleton, cancels prior requests,
and checks both request generation and URL before publishing/displaying results.

Validation: 15 Stanza unit tests, 209 Day component/mock tests, and 62 Day core tests passed
(one existing core test ignored). HarmonyOS Rust compile-check passed; no HarmonyOS emulator
was run. Rust formatting and Day's whitespace checks passed. Desktop runtime tests below
are historical and were not repeated for this mobile fix.

To run the mobile fixture suite, start `python3 tests/catalog_server.py`, then launch with
all five scripts listed above. The fixture uses port 18765; Android needs
`adb -s DEVICE reverse tcp:18765 tcp:18765`. Start with a fresh app process for the thumbnail
script's initial loading assertion, because covers are cached in memory for that process.

## Earlier navigation validation

The native catalog-stack revision passed `dayscript/catalog.yaml` and
`dayscript/browser.yaml`: **72/72 steps on each of the six local targets**.
The final iOS run needed one automatic retry after losing its simulator connection during
a native Back step; the retry completed all 72 steps. No matching Stanza crash report was found.

This includes nested feeds, facets, book details, search, acquisition, native mobile Back,
keyboard Back/Forward, catalog management and returning to the root list.

| Target | Result |
|---|---|
| macos-appkit | 72/72 |
| macos-gtk | 72/72 |
| macos-qt | 72/72 |
| ios-uikit | 72/72, iPhone simulator |
| android-mdc | 72/72, Android emulator |
| web-dom | 72/72, Chromium/Playwright |

The preceding reader/persistence revision passed the combined demo/catalog/browser suite
(93 steps per target). The unchanged 42-step reader demo also passed on Web during the
stack revision.

These checks cover sample import, rendered content and nonzero reader dimensions, animated page/chapter
navigation, hiding/showing reader controls, native reader settings, returning to the library,
reopening a saved cover with its reading position, Atom catalog search, EPUB acquisition, and
OPDS 2 browsing.
Catalog acquisition uses the local HTTP fixture; its log confirms `Stanza-Redux/1.4.4` on native
requests. A separate public probe followed Gutenberg's navigation feeds, downloaded *Pride and
Prejudice* (558,381 bytes), and parsed 16 spine items and 63 contents entries. Gutenberg and
Ebooks Gratuits root feeds also returned HTTP 200 through `day-part-http`.

Additional checks:

- Eleven Rust tests (including the persistence and migration additions): EPUB paths, bundled book parsing, font obfuscation, OPDS base URLs,
  groups/facets, search encoding, saved data migration, database reopen/delete/order behavior,
  read-only v1 parsing/path confinement/locator translation and gesture latching.
- Two browser rendering suites (Chromium and WebKit): embedded cover, chapter text, pagination,
  narrow/wide columns, margins, custom colors, recursive CSS imports, mouse edge/center taps,
  swipe cancellation/commit, chapter transitions, reduced motion, horizontal wheels, keyboard
  page turns and script isolation.
- Chromium and WebKit full-app input tests: real arrow keys and horizontal wheels navigate
  catalog history; selecting a root catalog pushes its feed; popped rows can be opened again;
  failed requests preserve the current page; text fields retain arrow editing; custom catalogs
  survive a page reload.
- Native AppKit CGEvent tests: Left key navigated back and horizontal wheel input navigated forward.
- A v1-format database and EPUB fixture imported automatically with chapter 2 / progression 0.4;
  a second launch did not duplicate it, and the legacy database checksum remained unchanged.
- Upgrade metadata verified: Apple bundle ID `org.appfair.app.Stanza-Redux`, Android package
  `org.appfair.app.Stanza_Redux`, version 1.4.5/build 24. The Day CLI regression test verifies
  that Apple launches honor platform-specific ID overrides.
- Day persistence regression suites: 7 SQLite tests and 9 wide-key tests passed.
- Native AppKit window capture confirms the reader title bar uses its system background.
- Earlier webview foundation checks: twelve Rust tests and five browser bridge tests.
- The webview component's AppKit demo: 24/24 dayscript steps.
- Rust formatting checks passed for the app, webview component and Day.
- Actual native GTK window capture confirmed the two-column reader is visible. GTK widget-only
  screenshots omit the Cocoa web view overlay; use a window capture for visual review.

## Other targets

All twelve platform/toolkit pairs are declared in `Day.toml`.

The new database, migration adapter and navigation passed HarmonyOS Rust compilation. HAP assembly fails in the installed SDK because
`@ohos/hvigor-ohos-plugin` is missing. No HarmonyOS emulator was launched.

The new common Windows GTK/Qt WebView2 host and Qt Rust bridge compile-check against
`x86_64-pc-windows-msvc` in a small harness. The Qt Windows hosting C++ passes a syntax check.
These are not full Windows app builds or runtime tests. Windows XAML, Windows GTK/Qt and Linux
GTK/Qt still need their native build/runtime validation. The Linux GTK evaluation API was checked
against the installed WebKitGTK Rust bindings.

See `README.md` for supported book/catalog formats, CORS/transport restrictions and build commands.
