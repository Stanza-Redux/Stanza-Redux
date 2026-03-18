// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel
import SkipKit
#if !SKIP
import ReadiumShared
#else
import org.readium.r2.shared.publication.services.cover
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
//            .accessibilityLabel("Book placeholder")
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
    @State var selectedBook: BookRecord? = nil
    @State var bookForDetail: BookRecord? = nil
    @State var bookToDelete: BookRecord? = nil
    @Environment(StanzaSettings.self) var settings: StanzaSettings

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
                            .accessibilityIdentifier("noBooksTitle")
                        Text("Import a book to get started.")
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("noBooksMessage")
                        Button("Import Sample Book") {
                            Task {
                                await importSampleBook()
                            }
                        }
                        .accessibilityIdentifier("importSampleBookButton")
                    }
                } else {
                    booksListView()
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
                    .accessibilityIdentifier("addBookButton")
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

    func booksListView() -> some View {
        List {
            ForEach(filteredBooks) { book in
                Button {
                    logger.info("Opening book: \(book.filePath)")
                    selectedBook = book
                } label: {
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
                .buttonStyle(.plain)
                .contextMenu {
                    Button {
                        selectedBook = book
                    } label: {
                        Label("Open Book", image: "newsstand")
                    }
                    Button {
                        bookForDetail = book
                    } label: {
                        Label("Show Book Info", systemImage: "info.circle")
                    }
                    Button(role: .destructive) {
                        bookToDelete = book
                    } label: {
                        Label("Delete", systemImage: "trash")
                    }
                }
            }
            .onDelete { indices in
                let booksToDelete = filteredBooks
                if let index = indices.first {
                    bookToDelete = booksToDelete[index]
                }
            }
        }
        .accessibilityIdentifier("bookList") // this crashes the build for some reason
        .searchable(text: $searchText, prompt: "Search books")
        .fullScreenCover(item: $selectedBook, onDismiss: { refreshBooks() }) { book in
            ReaderView(bookID: book.id, filePath: book.filePath, database: database)
        }
        .sheet(item: $bookForDetail) { book in
            NavigationStack {
                BookDetailView(bookID: book.id, database: database, onUpdate: { refreshBooks() })
            }
        }
        .alert("Delete Book", isPresented: Binding(get: { bookToDelete != nil }, set: { if !$0 { bookToDelete = nil } })) {
            Button("Delete", role: .destructive) {
                if let book = bookToDelete {
                    deleteBook(book)
                    bookToDelete = nil
                }
            }
            Button("Cancel", role: .cancel) {
                bookToDelete = nil
            }
        } message: {
            if let book = bookToDelete {
                Text("Are you sure you want to delete \"\(book.title)\"? This cannot be undone.")
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

            // Restore previously open book if the app was exited while reading
            let savedBookID = settings.lastOpenBookID
            if savedBookID != Int64(0) {
                // Clear immediately so a crash during open won't loop on next launch
                settings.lastOpenBookID = 0
                if let book = try? db.book(id: savedBookID) {
                    logger.info("Restoring previously open book: '\(book.title)' (id=\(savedBookID))")
                    self.selectedBook = book
                }
            }
        } catch {
            logger.error("Failed to open database: \(error)")
            errorMessage = "Failed to open library: \(error.localizedDescription)"
        }
    }

    private func refreshBooks() {
        guard let db = database else { return }
        do {
            let allBooks = try db.allBooks()
            withAnimation {
                self.books = allBooks
            }
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
        #if !SKIP
        switch await pub.platformValue.cover() {
        case .success(let image):
            guard let image = image else { return nil }
            return image.jpegData(compressionQuality: 0.85)
        case .failure:
            return nil
        }
        #else
        let bitmap: android.graphics.Bitmap? = pub.platformValue.cover()
        guard let bitmap = bitmap else { return nil }
        let stream = java.io.ByteArrayOutputStream()
        bitmap.compress(android.graphics.Bitmap.CompressFormat.JPEG, 85, stream)
        return Data(platformValue: stream.toByteArray())
        #endif
    }

    private func deleteBook(_ book: BookRecord) {
        guard let db = database else { return }
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
        refreshBooks()
    }
}
