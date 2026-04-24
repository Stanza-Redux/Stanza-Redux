# Stanza Redux

Stanza Redux is a cross-platform ebook reader for iOS and Android, built with [Skip](https://skip.dev) and powered by the [Readium SDK](https://readium.org/development/readium-sdk-overview/). A single Swift codebase powers a platform-native app that utilizes SwiftUI on iOS and Jetpack Compose on Android, while each platform uses its own native Readium toolkit for EPUB parsing, rendering, and navigation.

<div align="center">
  <a href="https://play.google.com/store/apps/details?id=org.appfair.app.Stanza_Redix" style="display: inline-block;"><img src="https://appfair.org/assets/badges/google-play-store.svg" alt="Download on the Google Play Store" style="height: 60px; vertical-align: middle; object-fit: contain;" /></a>
  <a href="https://apps.apple.com/us/app/stanza-redux/id1639831676" style="display: inline-block;"><img src="https://appfair.org/assets/badges/apple-app-store.svg" alt="Download on the Apple App Store" style="height: 60px; vertical-align: middle; object-fit: contain;" /></a>
</div>


## Architecture

Stanza Redux demonstrates how Skip can bridge platform-specific native libraries from a shared codebase. The app is organized into two Swift Package Manager modules:

- **`Stanza`** — The UI layer (SwiftUI views), built in `Sources/Stanza/`
- **`StanzaModel`** — The data layer (database, OPDS parsing, settings), built in `Sources/StanzaModel/`

### Readium Integration

The Readium SDK is published as two independent toolkits:

- [**readium/swift-toolkit**](https://github.com/readium/swift-toolkit) — Used on iOS via Swift Package Manager
- [**readium/kotlin-toolkit**](https://github.com/readium/kotlin-toolkit) — Used on Android via Gradle dependencies

Stanza Redux uses `#if SKIP` / `#if !SKIP` conditional compilation to call the appropriate platform SDK while sharing all UI and business logic. Platform-specific types are abstracted behind thin wrappers defined in `StanzaModel`:

| Wrapper | iOS Type | Android Type |
|---------|----------|--------------|
| `Pub` | `ReadiumShared.Publication` | `org.readium.r2.shared.publication.Publication` |
| `Loc` | `ReadiumShared.Locator` | `org.readium.r2.shared.publication.Locator` |
| `Lnk` | `ReadiumShared.Link` | `org.readium.r2.shared.publication.Link` |
| `Man` | `ReadiumShared.Manifest` | `org.readium.r2.shared.publication.Manifest` |

These wrappers expose cross-platform properties (title, href, progression, etc.) that the UI layer consumes without needing to know which platform SDK is providing the data.

The EPUB navigator is embedded differently on each platform:

- **iOS**: `EPUBNavigatorViewController` is wrapped in a `UIViewControllerRepresentable` and hosted in a SwiftUI view
- **Android**: `EpubNavigatorFragment` is embedded via Jetpack Compose's `AndroidFragment` composable within a `ComposeView`

### Data Flow

```
StanzaApp (entry point)
  └── ContentView (tab bar)
        ├── LibraryView (book management)
        ├── BrowseView (OPDS catalogs)
        └── SettingsView (preferences)
              └── AdvancedSettingsView
```

Shared state is managed through the SwiftUI environment:

- **`StanzaSettings`** — `@Observable` class persisting user preferences to `UserDefaults`
- **`LibraryManager`** — `@Observable` class managing the book database and file operations
- **`ErrorManager`** — `@Observable` class providing centralized error alert presentation

## Library

The Library tab is the main screen of the app. It displays all imported books with cover art, titles, authors, and reading progress.

<img height="500" alt="Screenshot 2026-03-23 at 17 52 36" src="https://github.com/user-attachments/assets/06ff9e5a-2b14-4cfb-b4e0-c28aa2b7f88d" />
<img height="500" alt="Screenshot 2026-03-23 at 17 53 26" src="https://github.com/user-attachments/assets/d891307f-4e1c-475e-9427-c6c8d982c489" />


### Features

- **Import books** from the device's file system using the system document picker
- **Cover art extraction** — automatically extracts and caches cover images from EPUB files
- **Reading progress** — displays percentage complete for each book
- **Search** — filter the library by title or author
- **Book management** — long-press context menu to view details, edit metadata, or delete books
- **Resume reading** — tap a book to open it in the reader; the app remembers your last reading position
- **Sample book** — a bundled copy of *Alice's Adventures in Wonderland* can be imported to try the reader immediately

<!-- TODO: Screenshot of Library empty state with Import Sample Book button -->

### Book Detail

Each book has a detail view showing metadata (title, author, identifier), reading progress, chapter count, and file path. An edit mode allows modifying the title, author, and identifier.

<!-- TODO: Screenshot of Book Detail view -->

## Reader

The reader presents EPUB content in a paginated view with customizable typography and an overlay HUD for navigation controls.

<img height="500" alt="Stanza_Reader_Android" src="https://github.com/user-attachments/assets/e05784a0-69e4-4805-8ca6-a2deb490c8d3" />
<img height="500" alt="Stanza_Reader_iOS" src="https://github.com/user-attachments/assets/14c60926-1508-4a7d-b543-e957e242ae63" />

### Navigation

- **Tap zones** — tap the left or right third of the screen to go backward or forward; tap the center to toggle the HUD
- **Animated page turns** — smooth page transition animations on both platforms
- **Table of Contents** — hierarchical chapter navigation with the current chapter highlighted
- **Bookmarks** — add, view, edit notes on, and navigate to bookmarks
- **Reading position persistence** — your position is saved on every page turn and restored on next launch

### HUD Controls

The heads-up display provides:

- **Progress bar** with chapter title and percentage
- **Font size** controls (increase/decrease buttons)
- **Font picker** — horizontal scrolling panel with font previews
- **Spacing controls** — cycle through presets for line height, character spacing, word spacing, and page margins
- **Table of Contents** and **Bookmark** buttons

<!-- TODO: Screenshot of Reader HUD with extended font/spacing panel open -->

### Themes and Appearance

- **Light, Dark, and Sepia** reading themes
- **System appearance** follows the device setting
- **Status bar hiding** for immersive reading (iOS)

## Catalogs

The Catalogs tab (enabled via Advanced Settings) allows browsing and downloading books from [OPDS](https://opds.io/) catalog feeds.

<img height="500" alt="Stanza_Browse_Android" src="https://github.com/user-attachments/assets/691e249a-d4ed-41c7-abb1-a4ec66a59c99" />
<img height="500" alt="Stanza_Browse_iOS" src="https://github.com/user-attachments/assets/0450099f-78a2-4a9a-9268-c2e8bb58f807" />

### Features

- **Pre-configured catalogs** — Standard Ebooks, Project Gutenberg, and Ebooks Gratuits
- **Custom catalogs** — add any OPDS feed URL
- **Catalog browsing** — navigate categories, groups, and facets
- **Search** — search within catalogs that support OpenSearch
- **Book detail** — view cover art, author, summary, and available download formats
- **Multiple format support** — when a book is available in multiple EPUB variants (e.g., "Recommended compatible epub", "Advanced epub"), a menu lets you choose which to download
- **Direct download** — download and import books directly into the library
- **About This Catalog** — displays feed metadata including icon, description, total book count, and informational links

<!-- TODO: Screenshot of Catalog book detail with multiple download formats -->

### OPDS Implementation

The OPDS service (`OPDSService`) handles:

- Parsing both OPDS 1 (Atom/XML) and OPDS 2 (JSON) feeds via the Readium OPDS parsers
- Extracting navigation images from feed entries using raw XML parsing with [SkipXML](https://skip.dev/docs/modules/skip-xml/)
- Promoting Atom `rel="enclosure"` entries to proper publication entries for feeds that don't use standard OPDS acquisition links
- OpenSearch template resolution for catalog search
- HTML-to-Markdown conversion of book summaries using `HTMLMarkdown`

## Settings

The Settings tab provides controls for reading preferences, text layout, and spacing.

<img height="500" alt="Stanza_Settings_Android" src="https://github.com/user-attachments/assets/b8735a4b-e219-47b6-9187-29608bf0476c" />
<img height="500" alt="Stanza_Settings_iOS" src="https://github.com/user-attachments/assets/4bc6a3a1-aadc-4184-ba06-6faa700bda8c" />

### Reading Preferences

- **Appearance** — System, Light, or Dark mode
- **Sepia Theme** — warm-toned reading theme
- **Font** — choose from system fonts and bundled custom fonts (Montserrat, Noto Serif, Noto Sans)
- **Font Size** — adjustable from 50% to 300%
- **Animate Page Turns** — toggle animated transitions
- **Left Tap Advances** — swap the left-tap direction
- **Hide Status Bar in Reader** — immersive reading mode
- **Open Web Pages in Embedded Browser** — use SFSafariViewController (iOS) or Chrome Custom Tabs (Android)

### Text Layout

- Columns (auto, one, two)
- Content fit (auto, page, width)
- Hyphenation, text alignment, text normalization
- Publisher styles toggle

### Spacing

- Line height, page margins, paragraph spacing, word spacing (all with slider controls)

### Advanced Settings

- **Enable Catalogs** — show/hide the Catalogs tab

<!-- TODO: Screenshot of Advanced Settings -->

## Error Handling

The app uses a centralized `ErrorManager` that provides consistent error alerts across all screens. When an error occurs anywhere in the app, `errorManager.errorOccurred(info:)` is called with structured `ErrorInfo` containing a title, message, underlying error, and optional help URL. The error manager logs the error and presents an alert with "OK" to dismiss and "Help" to search the project's issue tracker.

The `ErrorManager` is safe to call from any thread — it dispatches to the main actor internally.

## Building

This project is both a stand-alone Swift Package Manager module,
as well as an Xcode project that builds and translates the project
into a Kotlin Gradle project for Android using the skipstone plugin.

## Testing

The module can be tested using the standard `swift test` command
or by running the test target for the macOS destination in Xcode,
which will run the Swift tests as well as the transpiled
Kotlin JUnit tests in the Robolectric Android simulation environment.

Parity testing can be performed with `skip test`,
which will output a table of the test results for both platforms.

## Running

Xcode and Android Studio must be downloaded and installed in order to
run the app in the iOS simulator / Android emulator.
An Android emulator must already be running, which can be launched from
Android Studio's Device Manager.

To run both the Swift and Kotlin apps simultaneously,
launch the StanzaApp target from Xcode.
A build phases runs the "Launch Android APK" script that
will deploy the transpiled app a running Android emulator or connected device.
Logging output for the iOS app can be viewed in the Xcode console, and in
Android Studio's logcat tab for the transpiled Kotlin app.

## License

This software is licensed under the [GNU General Public License v2.0 or later](https://spdx.org/licenses/GPL-2.0-or-later.html).
