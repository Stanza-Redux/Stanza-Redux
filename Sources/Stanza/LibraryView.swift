// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel
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
#endif

struct LibraryView: View {
    @State var books: [BookRecord] = []
    @State var database: BookDatabase? = nil
    @State var errorMessage: String? = nil
    @State var isImporting = false
    @State var searchText: String = ""

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
                        Task {
                            await importSampleBook()
                        }
                    } label: {
                        Label("Import Sample", systemImage: "plus")
                    }
                }
            }
            .task {
                initDatabase()
            }
        }
    }

    private func initDatabase() {
        guard database == nil else { return }
        do {
            let dir = URL.applicationSupportDirectory
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            let dbPath = dir.appendingPathComponent("library.sqlite").path
            let db = try BookDatabase(path: dbPath)
            self.database = db
            self.books = try db.allBooks()
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

    private func importSampleBook() async {
        guard let db = database else { return }
        guard let sampleURL = Bundle.module.url(forResource: "Alice", withExtension: "epub") else {
            logger.error("Sample book not found in bundle")
            return
        }
        do {
            try await db.importBook(from: sampleURL)
            self.books = try db.allBooks()
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
            do {
                try db.deleteBook(id: book.id)
                let fileURL = URL(fileURLWithPath: book.filePath)
                try? FileManager.default.removeItem(at: fileURL)
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
        do {
            guard var record = try db.book(id: bookID) else { return }
            record.title = editTitle
            record.author = editAuthor
            record.identifier = editIdentifier.isEmpty ? nil : editIdentifier
            try db.updateBook(record)
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
    @Environment(\.dismiss) var dismiss

    #if !SKIP
    @State var navigator: EPUBNavigatorViewController? = nil
    #endif

    var body: some View {
        Group {
            if let publication = viewModel?.publication {
                ZStack(alignment: .topLeading) {
                    readerViewContainer(publication: publication)
                    Button {
                        saveProgress()
                        dismiss()
                    } label: {
                        Label("Close", systemImage: "xmark.circle.fill")
                            .labelStyle(.iconOnly)
                            .font(.title)
                            .foregroundStyle(.secondary)
                    }
                    .padding()
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
            saveProgress()
        }
    }

    func loadBook() async {
        do {
            let bookURL = URL(fileURLWithPath: filePath)
            let publication = try await Pub.loadPublication(from: bookURL)
            self.viewModel = ReaderViewModel(publication: publication)
            #if !SKIP
            self.navigator = try EPUBNavigatorViewController(publication: publication.platformValue, initialLocation: locator?.platformValue, config: navConfig)
            #endif
            if let db = database {
                try? db.markOpened(bookID: bookID)
            }
        } catch {
            self.error = error
        }
    }

    func saveProgress() {
        guard let db = database, let vm = viewModel else { return }
        let totalItems = Int64(vm.publication.manifest.readingOrder.count)
        // Save current position — default to item 0 if no locator tracked
        let currentItem: Int64 = 0
        try? db.updateProgress(bookID: bookID, currentItem: currentItem, totalItems: totalItems)
    }

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
            let navigatorFactory = EpubNavigatorFactory(publication: publication.platformValue, configuration: navConfig)
            let fragmentFactory = navigatorFactory.createFragmentFactory(initialLocator: locator?.platformValue, listener: nil)
            guard let fragmentActivity = LocalContext.current.fragmentActivity else {
                fatalError("could not extract FragmentActivity from LocalContext.current")
            }
            let fragmentManager = fragmentActivity.supportFragmentManager
            fragmentManager.fragmentFactory = fragmentFactory
            AndroidFragment<EpubNavigatorFragment>(
                onUpdate: { fragment in
                    logger.info("LibraryReaderView: onUpdate: \(fragment)")
                }
            )
        }
        #endif
    }
}
#endif
