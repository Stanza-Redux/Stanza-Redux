// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel
import SkipKit
#if !SKIP && canImport(ReadiumNavigator)
import ReadiumNavigator
import ReadiumShared
import UIKit
#endif
#if SKIP
import androidx.fragment.app.FragmentActivity
import androidx.fragment.compose.AndroidFragment
import androidx.compose.ui.platform.LocalContext
import org.readium.r2.navigator.epub.EpubNavigatorFactory
import org.readium.r2.navigator.epub.EpubNavigatorFragment
import org.readium.r2.navigator.input.InputListener
import org.readium.r2.navigator.input.TapEvent
import org.readium.r2.navigator.input.DragEvent
import org.readium.r2.navigator.input.KeyEvent
import org.readium.r2.shared.publication.services.cover
import org.readium.r2.navigator.preferences.Theme
import android.view.WindowInsets

/// Module-level reference to the current Activity, used for brightness and status bar control.
/// Set from the ComposeView context where LocalContext is available.
var currentAndroidActivity: android.app.Activity? = nil

class ReaderTapListener: InputListener {
    let tapHandler: (Double, Double) -> Void

    init(tapHandler: @escaping (Double, Double) -> Void) {
        self.tapHandler = tapHandler
    }

    override func onTap(_ event: TapEvent) -> Bool {
        tapHandler(Double(event.point.x), Double(event.point.y))
        return true
    }

    override func onDrag(_ event: DragEvent) -> Bool {
        return false
    }

    override func onKey(_ event: KeyEvent) -> Bool {
        return false
    }
}

class ReaderPaginationListener: EpubNavigatorFragment.PaginationListener {
    let pageChanged: (Int, Int, org.readium.r2.shared.publication.Locator) -> Void

    init(pageChanged: @escaping (Int, Int, org.readium.r2.shared.publication.Locator) -> Void) {
        self.pageChanged = pageChanged
    }

    override func onPageChanged(_ pageIndex: Int, _ totalPages: Int, _ locator: org.readium.r2.shared.publication.Locator) {
        pageChanged(pageIndex, totalPages, locator)
    }

    override func onPageLoaded() {
    }
}
#endif

/// Displays a book cover image, or a generic book icon if no cover is available.
struct BookCoverView: View {
    let coverImagePath: String?

    var body: some View {
        if let path = coverImagePath {
            AsyncImage(url: URL(fileURLWithPath: path)) { phase in
                if let image = phase.image {
                    image
                        .resizable()
                        .aspectRatio(contentMode: .fill)
                } else {
                    placeholderImage
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: 4))
        } else {
            placeholderImage
                .clipShape(RoundedRectangle(cornerRadius: 4))
        }
    }

    var placeholderImage: some View {
        Image("book_3", bundle: .module)
            .resizable()
            .aspectRatio(contentMode: .fit)
            .foregroundStyle(.secondary)
            .padding(6)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color.gray.opacity(0.15))
    }
}

/// A vertical brightness slider: drag up to brighten, down to dim.
struct BrightnessSlider: View {
    @Binding var value: Double

    var body: some View {
        GeometryReader { geo in
            let height = geo.size.height
            ZStack(alignment: .bottom) {
                Capsule()
                    .fill(Color.white.opacity(0.3))
                Capsule()
                    .fill(Color.yellow.opacity(0.8))
                    .frame(height: max(4.0, height * value))
            }
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { drag in
                        let newValue = 1.0 - (drag.location.y / height)
                        value = min(1.0, max(0.01, newValue))
                    }
            )
        }
    }
}

struct LibraryView: View {
    @State var books: [BookRecord] = []
    @State var database: BookDatabase? = nil
    @State var errorMessage: String? = nil
    @State var isImporting = false
    @State var searchText: String = ""
    @State var showDocumentPicker = false
    @State var pickedDocumentURL: URL? = nil
    @State var pickedFilename: String? = nil
    @State var pickedMimeType: String? = nil

    var filteredBooks: [BookRecord] {
        if searchText.isEmpty {
            return books
        }
        let query = searchText.lowercased()
        return books.filter { book in
            book.title.lowercased().contains(query) || book.author.lowercased().contains(query)
        }
    }

    var body: some View {
        NavigationStack {
            Group {
                if books.isEmpty {
                    VStack(spacing: 16) {
                        Text("No Books")
                            .font(.title2)
                        Text("Import a book to get started.")
                            .foregroundStyle(.secondary)
                        Button("Import Sample Book") {
                            Task {
                                await importSampleBook()
                            }
                        }
                    }
                } else {
                    List {
                        ForEach(filteredBooks) { book in
                            NavigationLink(value: book.id) {
                                HStack(spacing: 12) {
                                    BookCoverView(coverImagePath: book.coverImagePath)
                                        .frame(width: 50, height: 70)
                                    VStack(alignment: .leading) {
                                        Text(book.title)
                                            .font(.headline)
                                        if !book.author.isEmpty {
                                            Text(book.author)
                                                .font(.subheadline)
                                                .foregroundStyle(.secondary)
                                        }
                                    }
                                    Spacer()
                                    if book.progress > 0.0 {
                                        Text("\(Int(book.progress * 100))%")
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                    }
                                }
                            }
                        }
                        .onDelete { indices in
                            deleteBooks(at: Array(indices))
                        }
                    }
                    .searchable(text: $searchText, prompt: "Search books")
                    .navigationDestination(for: Int64.self) { bookID in
                        BookDetailView(bookID: bookID, database: database, onUpdate: { refreshBooks() })
                    }
                }
            }
            .navigationTitle("Library")
            .toolbar {
                ToolbarItem {
                    Button {
                        showDocumentPicker = true
                    } label: {
                        Label(title: { Text("Add Book") }, icon: { Image("add", bundle: .module) })
                    }
                }
            }
            .withDocumentPicker(
                isPresented: $showDocumentPicker,
                allowedContentTypes: [.epub],
                selectedDocumentURL: $pickedDocumentURL,
                selectedFilename: $pickedFilename,
                selectedFileMimeType: $pickedMimeType
            )
            .onChange(of: pickedDocumentURL) { oldURL, newURL in
                if var url = newURL {
                    if !url.absoluteString.hasPrefix("file:/") {
                        // FIXME: bug in withDocumentPicker URL: the url is sometimes just a path without a scheme, like /data/user/0/org.appfair.app.Stanza_Redux/cache/marcus-aurelius_meditations_george-long.epub
                        url = URL(fileURLWithPath: url.absoluteString)
                    }
                    pickedDocumentURL = nil
                    Task {
                        await importBookFromURL(url)
                    }
                }
            }
            .task {
                initDatabase()
            }
            .onAppear {
                refreshBooks()
            }
        }
    }

    private func initDatabase() {
        guard database == nil else { return }
        logger.info("Initializing library database")
        do {
            let dir = URL.applicationSupportDirectory
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            let dbPath = dir.appendingPathComponent("library.sqlite").path
            let db = try BookDatabase(path: dbPath)
            self.database = db
            self.books = try db.allBooks()
            logger.info("Library initialized with \(books.count) books")
        } catch {
            logger.error("Failed to open database: \(error)")
            errorMessage = "Failed to open library: \(error.localizedDescription)"
        }
    }

    private func refreshBooks() {
        guard let db = database else { return }
        do {
            self.books = try db.allBooks()
        } catch {
            logger.error("Failed to refresh books: \(error)")
        }
    }

    private func importBookFromURL(_ url: URL) async {
        logger.info("importBookFromURL: \(url.absoluteString)")
        guard let db = database else { return }
        do {
            let record = try await db.importBook(from: url)
            await extractAndSaveCover(for: record)
            self.books = try db.allBooks()
        } catch {
            logger.error("Failed to import book: \(error)")
            errorMessage = "Failed to import book: \(error.localizedDescription)"
            #if SKIP
            android.util.Log.e("Stanza", "Error importing book", error as? Throwable)
            #endif
        }
    }

    private func importSampleBook() async {
        guard let db = database else { return }
        guard let sampleURL = Bundle.module.url(forResource: "Alice", withExtension: "epub") else {
            logger.error("Sample book not found in bundle")
            return
        }
        logger.info("Importing sample book from bundle")
        do {
            let record = try await db.importBook(from: sampleURL)
            await extractAndSaveCover(for: record)
            self.books = try db.allBooks()
            logger.info("Sample book imported successfully")
        } catch {
            logger.error("Failed to import sample book: \(error)")
            errorMessage = "Failed to import book: \(error.localizedDescription)"
        }
    }

    private func extractAndSaveCover(for record: BookRecord) async {
        guard let db = database else { return }
        let bookPath = BookDatabase.absolutePath(for: record.filePath)
        let bookURL = URL(fileURLWithPath: bookPath)
        let coverURL = bookURL.deletingPathExtension().appendingPathExtension("jpg")
        do {
            let pub = try await Pub.loadPublication(from: bookURL)
            let coverData = await extractCoverData(from: pub)
            if let data = coverData {
                try data.write(to: coverURL)
                let relativePath = BookDatabase.relativePath(for: coverURL.path)
                try db.setCoverImagePath(bookID: record.id, coverPath: relativePath)
                logger.info("Saved cover image for '\(record.title)'")
            }
        } catch {
            logger.warning("Failed to extract cover for '\(record.title)': \(error)")
        }
    }

    private func extractCoverData(from pub: Pub) async -> Data? {
        #if !SKIP && canImport(ReadiumNavigator)
        switch await pub.platformValue.cover() {
        case .success(let image):
            guard let image = image else { return nil }
            return image.jpegData(compressionQuality: 0.85)
        case .failure:
            return nil
        }
        #elseif SKIP
        let bitmap: android.graphics.Bitmap? = pub.platformValue.cover()
        guard let bitmap = bitmap else { return nil }
        let stream = java.io.ByteArrayOutputStream()
        bitmap.compress(android.graphics.Bitmap.CompressFormat.JPEG, 85, stream)
        return Data(platformValue: stream.toByteArray())
        #else
        return nil
        #endif
    }

    private func deleteBooks(at indices: [Int]) {
        guard let db = database else { return }
        let booksToDelete = filteredBooks
        for index in indices {
            let book = booksToDelete[index]
            logger.info("Deleting book: '\(book.title)' (id=\(book.id)) at \(book.filePath)")
            do {
                try db.deleteBook(id: book.id)
                let fileURL = URL(fileURLWithPath: book.filePath)
                try? FileManager.default.removeItem(at: fileURL)
                if let coverPath = book.coverImagePath {
                    try? FileManager.default.removeItem(at: URL(fileURLWithPath: coverPath))
                }
                logger.debug("Book file removed: \(book.filePath)")
            } catch {
                logger.error("Failed to delete book: \(error)")
            }
        }
        refreshBooks()
    }
}

struct BookDetailView: View {
    let bookID: Int64
    let database: BookDatabase?
    var onUpdate: (() -> Void)? = nil
    @State var book: BookRecord? = nil
    @State var isEditing = false
    @State var showReader = false

    var body: some View {
        Group {
            if let book = book {
                List {
                    Section("Book Info") {
                        HStack {
                            Text("Title")
                            Spacer()
                            Text(book.title).foregroundStyle(.secondary)
                        }
                        if !book.author.isEmpty {
                            HStack {
                                Text("Author")
                                Spacer()
                                Text(book.author).foregroundStyle(.secondary)
                            }
                        }
                        if let identifier = book.identifier {
                            HStack {
                                Text("Identifier")
                                Spacer()
                                Text(identifier).foregroundStyle(.secondary)
                            }
                        }
                    }
                    Section("Progress") {
                        HStack {
                            Text("Chapters")
                            Spacer()
                            Text("\(book.currentItem)/\(book.totalItems)").foregroundStyle(.secondary)
                        }
                        ProgressView(value: book.progress)
                        HStack {
                            Text("Progress")
                            Spacer()
                            Text("\(Int(book.progress * 100))%").foregroundStyle(.secondary)
                        }
                        if let dateOpened = book.dateLastOpened {
                            HStack {
                                Text("Last Opened")
                                Spacer()
                                Text(dateOpened.description).foregroundStyle(.secondary)
                            }
                        }
                    }
                    Section("File") {
                        Text(book.filePath)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Section {
                        #if SKIP || canImport(ReadiumNavigator)
                        Button("Open Book") {
                            showReader = true
                        }
                        #endif
                    }
                }
                .navigationTitle(book.title)
                .toolbar {
                    ToolbarItem(placement: .automatic) {
                        Button("Edit") {
                            isEditing = true
                        }
                    }
                }
                .sheet(isPresented: $isEditing) {
                    BookEditView(book: book, database: database) { updatedBook in
                        self.book = updatedBook
                        onUpdate?()
                    }
                }
                #if SKIP || canImport(ReadiumNavigator)
                .fullScreenCover(isPresented: $showReader) {
                    LibraryReaderView(bookID: bookID, filePath: book.filePath, database: database)
                }
                #endif
            } else {
                Text("Book not found")
            }
        }
        .task {
            do {
                self.book = try database?.book(id: bookID)
            } catch {
                logger.error("Failed to load book: \(error)")
            }
        }
    }
}

struct BookEditView: View {
    @State var editTitle: String
    @State var editAuthor: String
    @State var editIdentifier: String
    let bookID: Int64
    let database: BookDatabase?
    let onSave: (BookRecord) -> Void
    @Environment(\.dismiss) var dismiss

    init(book: BookRecord, database: BookDatabase?, onSave: @escaping (BookRecord) -> Void) {
        self._editTitle = State(initialValue: book.title)
        self._editAuthor = State(initialValue: book.author)
        self._editIdentifier = State(initialValue: book.identifier ?? "")
        self.bookID = book.id
        self.database = database
        self.onSave = onSave
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Metadata") {
                    TextField("Title", text: $editTitle)
                    TextField("Author", text: $editAuthor)
                    TextField("Identifier", text: $editIdentifier)
                }
            }
            .navigationTitle("Edit Book")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { saveChanges() }
                }
            }
        }
    }

    private func saveChanges() {
        guard let db = database else { return }
        logger.info("Saving book edits for id=\(bookID): title='\(editTitle)', author='\(editAuthor)'")
        do {
            guard var record = try db.book(id: bookID) else { return }
            record.title = editTitle
            record.author = editAuthor
            record.identifier = editIdentifier.isEmpty ? nil : editIdentifier
            try db.updateBook(record)
            logger.info("Book edits saved successfully")
            onSave(record)
            dismiss()
        } catch {
            logger.error("Failed to save book: \(error)")
        }
    }
}

#if SKIP || canImport(ReadiumNavigator)
struct LibraryReaderView: View {
    let bookID: Int64
    let filePath: String
    let database: BookDatabase?
    @State var viewModel: ReaderViewModel? = nil
    @State var error: Error? = nil
    @State var locator: Loc? = nil
    @State var initialLocator: Loc? = nil
    @State var hasRestoredPosition: Bool = false
    @State var showHUD: Bool = false
    @State var showTOC: Bool = false
    @State var bookmarks: [BookmarkRecord] = []
    @State var isCurrentPageBookmarked: Bool = false
    @Environment(StanzaSettings.self) var settings: StanzaSettings
    @Environment(\.colorScheme) var colorScheme
    @State var initialPrefsApplied: Bool = false
    @State var screenBrightness: Double = 0.5
    @State var originalBrightness: Double = 0.5
    @Environment(\.dismiss) var dismiss

    #if !SKIP
    @State var navigator: EPUBNavigatorViewController? = nil
    @State var navigatorDelegate: ReaderLocationDelegate? = nil
    #endif
    #if SKIP
    @State var navigator: EpubNavigatorFragment? = nil
    @State var inputListenerAdded: Bool = false
    #endif

    var body: some View {
        readerBody
        .onChange(of: settings.fontSize) { applyPreferences() }
        .onChange(of: settings.fontFamily) { applyPreferences() }
        .onChange(of: settings.columnCount) { applyPreferences() }
        .onChange(of: settings.fit) { applyPreferences() }
        .onChange(of: settings.hyphens) { applyPreferences() }
        .onChange(of: settings.lineHeight) { applyPreferences() }
        .onChange(of: settings.pageMargins) { applyPreferences() }
        .onChange(of: settings.paragraphSpacing) { applyPreferences() }
        .onChange(of: settings.publisherStyles) { applyPreferences() }
        .onChange(of: settings.textAlign) { applyPreferences() }
        .onChange(of: settings.textNormalization) { applyPreferences() }
        .onChange(of: settings.wordSpacing) { applyPreferences() }
        .onChange(of: settings.appearance) { applyPreferences() }
        .onChange(of: colorScheme) { applyPreferences() }
        .onChange(of: screenBrightness) { applyScreenBrightness(screenBrightness) }
        #if SKIP
        .onChange(of: showHUD) { updateAndroidStatusBar() }
        #endif
    }

    var readerBody: some View {
        Group {
            if let publication = viewModel?.publication {
                readerViewContainer(publication: publication)
                    .overlay {
                        hudOverlay(publication: publication)
                    }
                    .sheet(isPresented: $showTOC) {
                        tocSheet(publication: publication)
                    }
            } else if let error = error {
                VStack {
                    Text("Error: \(String(describing: error))")
                    Button("Dismiss") { dismiss() }
                }
            } else {
                ProgressView("Loading...")
            }
        }
        .preferredColorScheme(settings.appearance == "dark" ? .dark : settings.appearance == "light" ? .light : nil)
        .background(colorScheme == .dark ? Color.black : Color.white)
        #if !SKIP
        .statusBarHidden(settings.hideStatusBarInReader && !showHUD)
        #endif
        .task {
            await loadBook()
        }
        .onAppear {
            initBrightness()
        }
        .onDisappear {
            saveCurrentLocator()
            restoreBrightness()
        }
    }

    func loadBook() async {
        logger.info("Opening book id=\(bookID) from \(filePath)")
        do {
            // Load saved locator from database
            if let db = database, let record = try? db.book(id: bookID),
               let json = record.locatorJSON {
                let savedLoc = Loc.fromJSON(json)
                self.locator = savedLoc
                self.initialLocator = savedLoc
                logger.debug("Restored reading position: progress=\(savedLoc?.totalProgression ?? 0.0)")
            }

            let bookURL = URL(fileURLWithPath: filePath)
            let publication = try await Pub.loadPublication(from: bookURL)
            logger.info("Publication loaded: '\(publication.metadata.title ?? "Unknown")' with \(publication.manifest.readingOrder.count) chapters")
            self.viewModel = ReaderViewModel(publication: publication)
            #if !SKIP
            self.navigator = try EPUBNavigatorViewController(publication: publication.platformValue, initialLocation: locator?.platformValue, config: navConfig)
            let delegate = ReaderLocationDelegate { loc in
                self.locator = loc
                self.persistLocator(loc)
            }
            delegate.onTap = { point, viewSize in
                self.handleTap(x: Double(point.x), width: Double(viewSize.width))
            }
            self.navigatorDelegate = delegate
            self.navigator?.delegate = delegate
            applyPreferences()
            #endif
            if let db = database {
                try? db.markOpened(bookID: bookID)
                self.bookmarks = (try? db.bookmarks(forBookID: bookID)) ?? []
            }
        } catch {
            logger.error("Failed to open book id=\(bookID): \(error)")
            self.error = error
        }
    }

    func persistLocator(_ loc: Loc) {
        guard let db = database else { return }
        guard let json = loc.jsonString else { return }
        let progress = loc.totalProgression ?? 0.0
        logger.debug("Persisting reading position for book id=\(bookID): progress=\(progress)")
        try? db.saveReadingPosition(bookID: bookID, locatorJSON: json, progress: progress)
        updateBookmarkState()
    }

    func saveCurrentLocator() {
        logger.info("Saving current locator for book id=\(bookID)")
        #if !SKIP
        if let nav = navigator, let platformLoc = nav.currentLocation {
            let loc = Loc(platformValue: platformLoc)
            persistLocator(loc)
        }
        #endif
        if let loc = locator {
            persistLocator(loc)
        }
    }

    // MARK: - Tap Handling

    func handleTap(x: Double, width: Double) {
        let third = width / 3.0
        if showHUD {
            showHUD = false
        } else if x < third {
            if settings.leftTapAdvances { goForward() } else { goBackward() }
        } else if x > third * 2.0 {
            goForward()
        } else {
            showHUD = true
        }
    }

    // MARK: - Navigation

    func goForward() {
        let animated = settings.animatePageTurns
        #if !SKIP
        if let nav = navigator {
            Task { await nav.goForward(options: animated ? .animated : .none) }
        }
        #else
        if let nav = navigator {
            Task { nav.goForward(animated) }
        }
        #endif
    }

    func goBackward() {
        let animated = settings.animatePageTurns
        #if !SKIP
        if let nav = navigator {
            Task { await nav.goBackward(options: animated ? .animated : .none) }
        }
        #else
        if let nav = navigator {
            Task { nav.goBackward(animated) }
        }
        #endif
    }

    func navigateToTOCEntry(_ link: Lnk) {
        logger.info("Navigating to TOC entry: '\(link.title ?? "unknown")' href=\(link.href)")
        let animated = settings.animatePageTurns
        #if !SKIP
        if let nav = navigator {
            Task { await nav.go(to: link.platformValue, options: animated ? .animated : .none) }
        }
        #else
        if let nav = navigator {
            Task { nav.go(link.platformValue, animated) }
        }
        #endif
        showTOC = false
        showHUD = false
    }

    // MARK: - Preferences

    /// Returns `true` if any reading preference differs from its default value.
    func hasNonDefaultPreferences() -> Bool {
        let s = settings
        return s.fontSize != 1.0
            || !s.fontFamily.isEmpty
            || !s.columnCount.isEmpty
            || !s.fit.isEmpty
            || !s.hyphens.isEmpty
            || s.lineHeight > 0.0
            || s.pageMargins > 0.0
            || s.paragraphSpacing > 0.0
            || !s.publisherStyles.isEmpty
            || !s.textAlign.isEmpty
            || !s.textNormalization.isEmpty
            || s.wordSpacing > 0.0
    }

    func adjustFontSize(increase: Bool) {
        if increase {
            settings.fontSize = min(settings.fontSize + 0.1, 3.0)
        } else {
            settings.fontSize = max(settings.fontSize - 0.1, 0.5)
        }
        logger.info("Reader font size changed to: \(Int(settings.fontSize * 100))%")
        applyPreferences()
    }

    /// Determines whether the effective appearance is dark.
    func effectiveIsDark() -> Bool {
        if settings.appearance == "dark" { return true }
        if settings.appearance == "light" { return false }
        return colorScheme == .dark
    }

    func applyPreferences() {
        let s = settings
        let isDark = effectiveIsDark()

        // Map string settings to typed optionals
        let hyphensVal: Bool? = s.hyphens == "true" ? true : s.hyphens == "false" ? false : nil
        let lineHeightVal: Double? = s.lineHeight > 0.0 ? s.lineHeight : nil
        let pageMarginsVal: Double? = s.pageMargins > 0.0 ? s.pageMargins : nil
        let paragraphSpacingVal: Double? = s.paragraphSpacing > 0.0 ? s.paragraphSpacing : nil
        let publisherStylesVal: Bool? = s.publisherStyles == "true" ? true : s.publisherStyles == "false" ? false : nil
        let textNormalizationVal: Bool? = s.textNormalization == "true" ? true : s.textNormalization == "false" ? false : nil
        let wordSpacingVal: Double? = s.wordSpacing > 0.0 ? s.wordSpacing : nil

        #if !SKIP
        if let nav = navigator {
            let fontFamilyVal: ReadiumNavigator.FontFamily? = s.fontFamily.isEmpty ? nil : ReadiumNavigator.FontFamily(rawValue: s.fontFamily)
            let columnCountVal = s.columnCount.isEmpty ? nil : ReadiumNavigator.ColumnCount(rawValue: s.columnCount)
            let fitVal = s.fit.isEmpty ? nil : ReadiumNavigator.Fit(rawValue: s.fit)
            let textAlignVal = s.textAlign.isEmpty ? nil : ReadiumNavigator.TextAlignment(rawValue: s.textAlign)
            let themeVal: ReadiumNavigator.Theme = isDark ? .dark : .light
            let prefs = EPUBPreferences(
                columnCount: columnCountVal,
                fit: fitVal,
                fontFamily: fontFamilyVal,
                fontSize: s.fontSize,
                hyphens: hyphensVal,
                lineHeight: lineHeightVal,
                pageMargins: pageMarginsVal,
                paragraphSpacing: paragraphSpacingVal,
                publisherStyles: publisherStylesVal,
                textAlign: textAlignVal,
                textNormalization: textNormalizationVal,
                theme: themeVal,
                wordSpacing: wordSpacingVal
            )
            nav.submitPreferences(prefs)
        }
        #else
        if let nav = navigator {
            let fontFamilyVal: org.readium.r2.navigator.preferences.FontFamily? = s.fontFamily.isEmpty ? nil : org.readium.r2.navigator.preferences.FontFamily(s.fontFamily)
            let themeVal: org.readium.r2.navigator.preferences.Theme = isDark ? Theme.DARK : Theme.LIGHT
            let prefs = org.readium.r2.navigator.epub.EpubPreferences(
                fontFamily: fontFamilyVal,
                fontSize: s.fontSize,
                hyphens: hyphensVal,
                lineHeight: lineHeightVal,
                pageMargins: pageMarginsVal,
                paragraphSpacing: paragraphSpacingVal,
                publisherStyles: publisherStylesVal,
                textNormalization: textNormalizationVal,
                theme: themeVal,
                wordSpacing: wordSpacingVal
            )
            nav.submitPreferences(prefs)
        }
        #endif
    }

    // MARK: - Brightness

    func initBrightness() {
        #if !SKIP
        let current = Double(UIScreen.main.brightness)
        screenBrightness = current
        originalBrightness = current
        #endif
    }

    func restoreBrightness() {
        #if !SKIP
        UIScreen.main.brightness = CGFloat(originalBrightness)
        #else
        if let activity = currentAndroidActivity {
            activity.runOnUiThread {
                let lp = activity.window.attributes
                lp.screenBrightness = Float(-1.0) // restore system default
                activity.window.attributes = lp
            }
            // Restore status bar
            activity.window.insetsController?.show(android.view.WindowInsets.Type.statusBars())
        }
        currentAndroidActivity = nil
        #endif
    }

    func applyScreenBrightness(_ value: Double) {
        #if !SKIP
        UIScreen.main.brightness = CGFloat(value)
        #else
        if let activity = currentAndroidActivity {
            activity.runOnUiThread {
                let lp = activity.window.attributes
                lp.screenBrightness = Float(value)
                activity.window.attributes = lp
            }
        }
        #endif
    }

    // MARK: - Status Bar (Android)

    #if SKIP
    func updateAndroidStatusBar() {
        guard settings.hideStatusBarInReader else { return }
        if let activity = currentAndroidActivity {
            let controller = activity.window.insetsController
            if showHUD {
                controller?.show(android.view.WindowInsets.Type.statusBars())
            } else {
                controller?.hide(android.view.WindowInsets.Type.statusBars())
            }
        }
    }
    #endif

    // MARK: - Bookmarks

    func refreshBookmarks() {
        guard let db = database else { return }
        do {
            self.bookmarks = try db.bookmarks(forBookID: bookID)
            updateBookmarkState()
        } catch {
            logger.error("Failed to refresh bookmarks: \(error)")
        }
    }

    func updateBookmarkState() {
        guard let loc = locator, let json = loc.jsonString else {
            isCurrentPageBookmarked = false
            return
        }
        // Check if current locator matches any bookmark by comparing locator JSON
        var found = false
        for bookmark in bookmarks {
            if bookmark.locatorJSON == json {
                found = true
            }
        }
        isCurrentPageBookmarked = found
    }

    func toggleBookmark() {
        guard let db = database else { return }
        guard let loc = locator, let json = loc.jsonString else {
            logger.warning("Cannot bookmark: no current locator")
            return
        }

        if isCurrentPageBookmarked {
            // Remove the bookmark matching the current locator
            for bookmark in bookmarks {
                if bookmark.locatorJSON == json {
                    logger.info("Removing bookmark id=\(bookmark.id)")
                    do {
                        try db.deleteBookmark(id: bookmark.id)
                    } catch {
                        logger.error("Failed to delete bookmark: \(error)")
                    }
                }
            }
        } else {
            // Add a new bookmark
            let prog = loc.totalProgression ?? 0.0
            let progressLabel = "\(Int(prog * 100))%"
            let chapter = loc.title ?? ""
            var excerpt = ""
            if let highlight = loc.textHighlight {
                excerpt = highlight
            } else if let before = loc.textBefore {
                excerpt = before
            }
            // Truncate excerpt to 200 characters
            if excerpt.count > 200 {
                excerpt = String(excerpt.prefix(200))
            }
            let sortOrder = Int64(bookmarks.count)
            let record = BookmarkRecord(
                bookID: bookID,
                locatorJSON: json,
                progressLabel: progressLabel,
                excerpt: excerpt,
                chapter: chapter,
                sortOrder: sortOrder
            )
            logger.info("Adding bookmark at \(progressLabel) chapter='\(chapter)'")
            do {
                try db.addBookmark(record)
            } catch {
                logger.error("Failed to add bookmark: \(error)")
            }
        }
        refreshBookmarks()
    }

    func navigateToBookmark(_ bookmark: BookmarkRecord) {
        guard let loc = Loc.fromJSON(bookmark.locatorJSON) else {
            logger.error("Invalid bookmark locator JSON")
            return
        }
        logger.info("Navigating to bookmark id=\(bookmark.id): \(bookmark.progressLabel)")
        let animated = settings.animatePageTurns
        #if !SKIP
        if let nav = navigator {
            Task { await nav.go(to: loc.platformValue, options: animated ? .animated : .none) }
        }
        #else
        if let nav = navigator {
            Task { nav.go(loc.platformValue, animated) }
        }
        #endif
        showTOC = false
        showHUD = false
    }

    // MARK: - HUD Overlay

    @ViewBuilder func hudOverlay(publication: Pub) -> some View {
        if showHUD {
            ZStack {
                // Left column: close button + brightness slider
                HStack {
                    VStack(spacing: 12) {
                        // Close button aligned with brightness slider
                        Button {
                            saveCurrentLocator()
                            dismiss()
                        } label: {
                            Image("cancel", bundle: .module)
                                .font(.system(size: 28))
                                .foregroundStyle(.white)
                                .background(Circle().fill(Color.black.opacity(0.5)))
                        }

                        Image(systemName: "sun.max.fill")
                            .font(.caption)
                            .foregroundStyle(.white)
                        BrightnessSlider(value: $screenBrightness)
                        Image(systemName: "sun.min")
                            .font(.caption)
                            .foregroundStyle(.white)
                    }
                    .frame(width: 36)
                    .padding(.top, 16)
                    .padding(.bottom, 160)
                    .padding(.leading, 10)
                    Spacer()
                }

                // Main HUD content
                VStack {
                    // Top bar with bookmark button (right-aligned)
                    HStack {
                        Spacer()
                        Button {
                            toggleBookmark()
                        } label: {
                            Image(isCurrentPageBookmarked ? "bookmark_filled" : "bookmark", bundle: .module)
                                .font(.system(size: 28))
                                .foregroundStyle(.white)
                                .background(Circle().fill(Color.black.opacity(0.5)))
                        }
                    }
                    .padding(.top, 16)
                    .padding(.horizontal, 16)

                    Spacer()

                    // Bottom controls
                    VStack(spacing: 16) {
                        // Progress indicator
                        HStack {
                            Text(locator?.title ?? "")
                                .font(.caption)
                                .foregroundStyle(.white)
                                .lineLimit(1)
                            Spacer()
                            let prog = locator?.totalProgression ?? 0.0
                            Text("\(Int(prog * 100))%")
                                .font(.caption)
                                .foregroundStyle(.white)
                        }
                        .padding(.horizontal, 16)

                        ProgressView(value: locator?.totalProgression ?? 0.0)
                            .tint(.white)
                            .padding(.horizontal, 16)

                        // Font size and TOC controls
                        HStack(spacing: 32) {
                            Button {
                                adjustFontSize(increase: false)
                            } label: {
                                Image("remove_circle", bundle: .module)
                                    .font(.title2)
                                    .foregroundStyle(.white)
                            }

                            Text("Aa")
                                .font(.headline)
                                .foregroundStyle(.white)

                            Button {
                                adjustFontSize(increase: true)
                            } label: {
                                Image("add_circle", bundle: .module)
                                    .font(.title2)
                                    .foregroundStyle(.white)
                            }

                            Spacer()

                            Button {
                                showTOC = true
                            } label: {
                                Image("toc", bundle: .module)
                                    .font(.title2)
                                    .foregroundStyle(.white)
                            }
                        }
                        .padding(.horizontal, 24)
                        .padding(.bottom, 40)
                    }
                    .padding(.top, 12)
                    .background(Color.black.opacity(0.7))
                }
            }
        }
    }

    // MARK: - Table of Contents & Bookmarks Sheet

    func tocSheet(publication: Pub) -> some View {
        TOCAndBookmarksSheet(
            publication: publication,
            bookmarks: bookmarks,
            currentLocator: locator,
            database: database,
            bookID: bookID,
            onNavigateToTOC: { link in
                navigateToTOCEntry(link)
            },
            onNavigateToBookmark: { bookmark in
                navigateToBookmark(bookmark)
            },
            onBookmarksChanged: {
                refreshBookmarks()
            },
            onDismiss: {
                showTOC = false
            }
        )
    }

    // MARK: - Reader Container

    func readerViewContainer(publication: Pub) -> some View {
        #if !SKIP
        ReaderViewContainer(
            viewModel: viewModel!,
            viewControllerWrapper: ReaderViewControllerWrapper(
                viewController: ReaderViewController(
                    viewModel: viewModel!,
                    navigator: navigator!
                )
            )
        )
        #else
        ComposeView { context in
            let savedLocator = initialLocator?.platformValue
            let navigatorFactory = EpubNavigatorFactory(publication: publication.platformValue, configuration: navConfig)
            let paginationListener = ReaderPaginationListener(pageChanged: { pageIndex, totalPages, platformLocator in
                let loc = Loc(platformValue: platformLocator)
                self.locator = loc
                self.persistLocator(loc)
            })
            let fragmentFactory = navigatorFactory.createFragmentFactory(initialLocator: savedLocator, listener: nil, paginationListener: paginationListener)
            guard let fragmentActivity = LocalContext.current.fragmentActivity else {
                fatalError("could not extract FragmentActivity from LocalContext.current")
            }

            // Capture activity for brightness and status bar control
            if currentAndroidActivity == nil {
                currentAndroidActivity = fragmentActivity
                let wb = Double(fragmentActivity.window.attributes.screenBrightness)
                if wb >= 0.0 {
                    self.screenBrightness = wb
                    self.originalBrightness = wb
                }
                if self.settings.hideStatusBarInReader {
                    fragmentActivity.window.insetsController?.hide(android.view.WindowInsets.Type.statusBars())
                }
            }

            let fragmentManager = fragmentActivity.supportFragmentManager
            fragmentManager.fragmentFactory = fragmentFactory
            AndroidFragment<EpubNavigatorFragment>(
                onUpdate: { nav in
                    self.navigator = nav
                    if !self.inputListenerAdded {
                        self.inputListenerAdded = true
                        let listener = ReaderTapListener(tapHandler: { x, y in
                            let w = Double(nav.view?.width ?? 1)
                            self.handleTap(x: x, width: w)
                        })
                        nav.addInputListener(listener)
                    }
                    if !self.hasRestoredPosition {
                        self.hasRestoredPosition = true
                        if let savedLoc = self.initialLocator?.platformValue {
                            nav.go(savedLoc, false)
                        }
                    }
                    if !self.initialPrefsApplied {
                        self.initialPrefsApplied = true
                        self.applyPreferences()
                    }
                }
            )
        }
        #endif
    }
}

// MARK: - TOC & Bookmarks Sheet

enum TOCTab: String, CaseIterable {
    case contents = "Contents"
    case bookmarks = "Bookmarks"
}

struct TOCAndBookmarksSheet: View {
    let publication: Pub
    @State var bookmarks: [BookmarkRecord]
    let currentLocator: Loc?
    let database: BookDatabase?
    let bookID: Int64
    let onNavigateToTOC: (Lnk) -> Void
    let onNavigateToBookmark: (BookmarkRecord) -> Void
    let onBookmarksChanged: () -> Void
    let onDismiss: () -> Void
    @State var selectedTab: TOCTab = .contents
    @State var editingBookmark: BookmarkRecord? = nil
    @State var showEditSheet: Bool = false

    /// The title of the chapter the reader is currently in.
    var currentChapterTitle: String? {
        currentLocator?.title
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Picker("View", selection: $selectedTab) {
                    ForEach(TOCTab.allCases, id: \.self) { tab in
                        Text(tab.rawValue).tag(tab)
                    }
                }
                .pickerStyle(.segmented)
                .padding(.horizontal)
                .padding(.vertical, 8)

                if selectedTab == .contents {
                    tocList
                } else {
                    bookmarksList
                }
            }
            .navigationTitle(selectedTab == .contents ? "Table of Contents" : "Bookmarks")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") {
                        onDismiss()
                    }
                }
            }
            .sheet(isPresented: $showEditSheet) {
                if let bookmark = editingBookmark {
                    BookmarkEditSheet(bookmark: bookmark, database: database) { updated in
                        refreshBookmarks()
                        onBookmarksChanged()
                    }
                }
            }
        }
    }

    /// Whether the given TOC link matches the current reading position.
    func isCurrentChapter(_ link: Lnk) -> Bool {
        guard let current = currentChapterTitle, let linkTitle = link.title else { return false }
        return current == linkTitle
    }

    @ViewBuilder var tocList: some View {
        List {
            ForEach(Array(publication.manifest.tableOfContents.enumerated()), id: \.offset) { index, link in
                Button {
                    onNavigateToTOC(link)
                } label: {
                    Text(link.title ?? "Chapter \(index + 1)")
                        .foregroundStyle(.primary)
                }
                .buttonStyle(.plain)
                .listRowBackground(isCurrentChapter(link) ? Color.accentColor.opacity(0.12) : nil)
                if !link.children.isEmpty {
                    ForEach(Array(link.children.enumerated()), id: \.offset) { childIndex, child in
                        Button {
                            onNavigateToTOC(child)
                        } label: {
                            Text(child.title ?? "Section \(childIndex + 1)")
                                .foregroundStyle(.primary)
                                .padding(.leading, 20)
                        }
                        .buttonStyle(.plain)
                        .listRowBackground(isCurrentChapter(child) ? Color.accentColor.opacity(0.12) : nil)
                    }
                }
            }
        }
    }

    @ViewBuilder var bookmarksList: some View {
        if bookmarks.isEmpty {
            VStack(spacing: 12) {
                Image("bookmark", bundle: .module)
                    .font(.system(size: 48))
                    .foregroundStyle(.secondary)
                Text("No Bookmarks")
                    .font(.title3)
                    .foregroundStyle(.secondary)
                Text("Tap the bookmark icon while reading to add one.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)
            }
            .frame(maxHeight: .infinity)
        } else {
            List {
                ForEach(Array(bookmarks.enumerated()), id: \.offset) { index, bookmark in
                    Button {
                        onNavigateToBookmark(bookmark)
                    } label: {
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(bookmark.chapter.isEmpty ? "Bookmark" : bookmark.chapter)
                                    .font(.headline)
                                    .foregroundStyle(.primary)
                                    .lineLimit(1)
                                Spacer()
                                Text(bookmark.progressLabel)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            if !bookmark.excerpt.isEmpty {
                                Text(bookmark.excerpt)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(2)
                            }
                            if !bookmark.notes.isEmpty {
                                Text(bookmark.notes)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                        }
                        .padding(.vertical, 2)
                    }
                    .buttonStyle(.plain)
                    // 'fun contextMenu(menuItems: () -> View): View' is deprecated. This API is not yet available in Skip. Consider placing it within a #if !SKIP block. You can file an issue against the owning library at https://github.com/skiptools, or see the library README for information on adding support.
                    #if !SKIP
                    .contextMenu {
                        Button {
                            shareBookmark(bookmark)
                        } label: {
                            Label("Share", systemImage: "square.and.arrow.up")
                        }
                        Button {
                            editingBookmark = bookmark
                            showEditSheet = true
                        } label: {
                            Label("Edit Notes", systemImage: "pencil")
                        }
                        Button(role: .destructive) {
                            deleteBookmark(bookmark)
                        } label: {
                            Label("Delete", systemImage: "trash")
                        }
                    }
                    #endif
                }
                .onDelete { indices in
                    let sorted = Array(indices).sorted(by: >)
                    for index in sorted {
                        let bookmark = bookmarks[index]
                        deleteBookmark(bookmark)
                    }
                }
                .onMove { source, destination in
                    moveBookmarks(from: source, to: destination)
                }
            }
        }
    }

    func refreshBookmarks() {
        guard let db = database else { return }
        do {
            self.bookmarks = try db.bookmarks(forBookID: bookID)
        } catch {
            logger.error("Failed to refresh bookmarks: \(error)")
        }
    }

    func deleteBookmark(_ bookmark: BookmarkRecord) {
        guard let db = database else { return }
        do {
            try db.deleteBookmark(id: bookmark.id)
            refreshBookmarks()
            onBookmarksChanged()
        } catch {
            logger.error("Failed to delete bookmark: \(error)")
        }
    }

    func moveBookmarks(from source: IndexSet, to destination: Int) {
        guard let db = database else { return }
        var reordered = bookmarks
        reordered.move(fromOffsets: source, toOffset: destination)
        for i in reordered.indices {
            var bm = reordered[i]
            bm.sortOrder = Int64(i)
            do {
                try db.updateBookmark(bm)
            } catch {
                logger.error("Failed to reorder bookmark: \(error)")
            }
        }
        refreshBookmarks()
        onBookmarksChanged()
    }

    func shareBookmark(_ bookmark: BookmarkRecord) {
        var text = ""
        if !bookmark.chapter.isEmpty {
            text += bookmark.chapter + "\n"
        }
        text += "Progress: " + bookmark.progressLabel + "\n"
        if !bookmark.excerpt.isEmpty {
            text += "\"\(bookmark.excerpt)\"\n"
        }
        if !bookmark.notes.isEmpty {
            text += "Notes: " + bookmark.notes + "\n"
        }

        #if !SKIP
        let activityVC = UIActivityViewController(activityItems: [text], applicationActivities: nil)
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
           let rootVC = windowScene.windows.first?.rootViewController {
            var topVC = rootVC
            while let presented = topVC.presentedViewController {
                topVC = presented
            }
            topVC.present(activityVC, animated: true)
        }
        #else
        let context = ProcessInfo.processInfo.androidContext
        let intent = android.content.Intent(android.content.Intent.ACTION_SEND)
        intent.setType("text/plain")
        intent.putExtra(android.content.Intent.EXTRA_TEXT, text)
        let chooser = android.content.Intent.createChooser(intent, "Share Bookmark")
        chooser.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(chooser)
        #endif
    }
}

struct BookmarkEditSheet: View {
    let bookmark: BookmarkRecord
    let database: BookDatabase?
    let onSave: (BookmarkRecord) -> Void
    @State var editNotes: String
    @Environment(\.dismiss) var dismiss

    init(bookmark: BookmarkRecord, database: BookDatabase?, onSave: @escaping (BookmarkRecord) -> Void) {
        self.bookmark = bookmark
        self.database = database
        self.onSave = onSave
        self._editNotes = State(initialValue: bookmark.notes)
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Bookmark Info") {
                    if !bookmark.chapter.isEmpty {
                        HStack {
                            Text("Chapter")
                            Spacer()
                            Text(bookmark.chapter)
                                .foregroundStyle(.secondary)
                        }
                    }
                    HStack {
                        Text("Progress")
                        Spacer()
                        Text(bookmark.progressLabel)
                            .foregroundStyle(.secondary)
                    }
                    if !bookmark.excerpt.isEmpty {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Excerpt")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Text(bookmark.excerpt)
                                .font(.body)
                        }
                    }
                }
                Section("Notes") {
                    TextField("Add notes...", text: $editNotes)
                }
            }
            .navigationTitle("Edit Bookmark")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        saveNotes()
                    }
                }
            }
        }
    }

    func saveNotes() {
        guard let db = database else { return }
        var updated = bookmark
        updated.notes = editNotes
        do {
            try db.updateBookmark(updated)
            logger.info("Updated bookmark notes for id=\(bookmark.id)")
            onSave(updated)
            dismiss()
        } catch {
            logger.error("Failed to save bookmark notes: \(error)")
        }
    }
}
#endif
