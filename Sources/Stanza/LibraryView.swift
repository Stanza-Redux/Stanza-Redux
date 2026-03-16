// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel
import SkipKit
#if !SKIP && canImport(ReadiumNavigator)
import ReadiumNavigator
import ReadiumShared
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
                                HStack {
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
                        Label({ Text("Add Book"), icon: Image("add", bundle: .module) })
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
            try await db.importBook(from: url)
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
            try await db.importBook(from: sampleURL)
            self.books = try db.allBooks()
            logger.info("Sample book imported successfully")
        } catch {
            logger.error("Failed to import sample book: \(error)")
            errorMessage = "Failed to import book: \(error.localizedDescription)"
        }
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
    @AppStorage("readerFontSize") var currentFontSize: Double = 1.0
    @AppStorage("animatePageTurns") var animatePageTurns: Bool = true
    @State var fontSizeApplied: Bool = false
    @Environment(\.dismiss) var dismiss

    #if !SKIP
    @State var navigator: EPUBNavigatorViewController? = nil
    @State var navigatorDelegate: ReaderLocationDelegate? = nil
    #endif
    #if SKIP
    @State var epubFragment: EpubNavigatorFragment? = nil
    @State var inputListenerAdded: Bool = false
    #endif

    var body: some View {
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
        .task {
            await loadBook()
        }
        .onDisappear {
            saveCurrentLocator()
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
            if currentFontSize != 1.0 {
                applyFontSize()
            }
            #endif
            if let db = database {
                try? db.markOpened(bookID: bookID)
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
            goBackward()
        } else if x > third * 2.0 {
            goForward()
        } else {
            showHUD = true
        }
    }

    // MARK: - Navigation

    func goForward() {
        let animated = animatePageTurns
        #if !SKIP
        if let nav = navigator {
            Task { await nav.goForward(options: animated ? .animated : .init()) }
        }
        #else
        if let fragment = epubFragment {
            Task { fragment.goForward(animated) }
        }
        #endif
    }

    func goBackward() {
        let animated = animatePageTurns
        #if !SKIP
        if let nav = navigator {
            Task { await nav.goBackward(options: animated ? .animated : .init()) }
        }
        #else
        if let fragment = epubFragment {
            Task { fragment.goBackward(animated) }
        }
        #endif
    }

    func navigateToTOCEntry(_ link: Lnk) {
        logger.info("Navigating to TOC entry: '\(link.title ?? "unknown")' href=\(link.href)")
        let animated = animatePageTurns
        #if !SKIP
        if let nav = navigator {
            Task { await nav.go(to: link.platformValue, options: animated ? .animated : .init()) }
        }
        #else
        if let fragment = epubFragment {
            Task { fragment.go(link.platformValue, animated) }
        }
        #endif
        showTOC = false
        showHUD = false
    }

    // MARK: - Font Size

    func adjustFontSize(increase: Bool) {
        if increase {
            currentFontSize = min(currentFontSize + 0.1, 3.0)
        } else {
            currentFontSize = max(currentFontSize - 0.1, 0.5)
        }
        logger.info("Reader font size changed to: \(Int(currentFontSize * 100))%")
        applyFontSize()
    }

    func applyFontSize() {
        #if !SKIP
        if let nav = navigator {
            let prefs = EPUBPreferences(fontSize: currentFontSize)
            nav.submitPreferences(prefs)
        }
        #else
        if let fragment = epubFragment {
            let prefs = org.readium.r2.navigator.epub.EpubPreferences(fontSize: currentFontSize)
            fragment.submitPreferences(prefs)
        }
        #endif
    }

    // MARK: - HUD Overlay

    @ViewBuilder func hudOverlay(publication: Pub) -> some View {
        if showHUD {
            VStack {
                // Top bar with close button
                HStack {
                    Spacer()
                    Button {
                        saveCurrentLocator()
                        dismiss()
                    } label: {
                        Image("cancel", bundle: .module)
                            .font(.system(size: 28))
                            .foregroundStyle(.white)
                            .background(Circle().fill(Color.black.opacity(0.5)))
                    }
                }
                .padding(.top, 54)
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

    // MARK: - Table of Contents

    func tocSheet(publication: Pub) -> some View {
        NavigationStack {
            List {
                ForEach(Array(publication.manifest.tableOfContents.enumerated()), id: \.offset) { index, link in
                    Button {
                        navigateToTOCEntry(link)
                    } label: {
                        Text(link.title ?? "Chapter \(index + 1)")
                    }
                    if !link.children.isEmpty {
                        ForEach(Array(link.children.enumerated()), id: \.offset) { childIndex, child in
                            Button {
                                navigateToTOCEntry(child)
                            } label: {
                                Text(child.title ?? "Section \(childIndex + 1)")
                                    .padding(.leading, 20)
                            }
                        }
                    }
                }
            }
            .navigationTitle("Table of Contents")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") {
                        showTOC = false
                    }
                }
            }
        }
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
            let fragmentManager = fragmentActivity.supportFragmentManager
            fragmentManager.fragmentFactory = fragmentFactory
            AndroidFragment<EpubNavigatorFragment>(
                onUpdate: { fragment in
                    self.epubFragment = fragment
                    if !self.inputListenerAdded {
                        self.inputListenerAdded = true
                        let listener = ReaderTapListener(tapHandler: { x, y in
                            let w = Double(fragment.view?.width ?? 1)
                            self.handleTap(x: x, width: w)
                        })
                        fragment.addInputListener(listener)
                    }
                    if !self.hasRestoredPosition {
                        self.hasRestoredPosition = true
                        if let savedLoc = self.initialLocator?.platformValue {
                            fragment.go(savedLoc, false)
                        }
                    }
                    if !self.fontSizeApplied {
                        self.fontSizeApplied = true
                        if self.currentFontSize != 1.0 {
                            self.applyFontSize()
                        }
                    }
                }
            )
        }
        #endif
    }
}
#endif
