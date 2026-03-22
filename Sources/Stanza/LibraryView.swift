// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel
import SkipKit

/// Displays a book cover image, or a generic book icon if no cover is available.
struct BookCoverView: View {
    let coverImagePath: String?

    var body: some View {
        if let path = coverImagePath {
            let absolutePath = BookDatabase.absolutePath(for: path)
            AsyncImage(url: URL(fileURLWithPath: absolutePath)) { phase in
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
    @Environment(LibraryManager.self) var library: LibraryManager
    @Environment(StanzaSettings.self) var settings: StanzaSettings
    @State var searchText: String = ""
    @State var showDocumentPicker = false
    @State var pickedDocumentURL: URL? = nil
    @State var pickedFilename: String? = nil
    @State var pickedMimeType: String? = nil
    @State var selectedBook: BookRecord? = nil
    @State var bookForDetail: BookRecord? = nil
    @State var bookToDelete: BookRecord? = nil
    @State var showDeleteConfirmation: Bool = false

    var filteredBooks: [BookRecord] {
        if searchText.isEmpty {
            return library.books
        }
        let query = searchText.lowercased()
        return library.books.filter { book in
            book.title.lowercased().contains(query) || book.author.lowercased().contains(query)
        }
    }

    var body: some View {
        NavigationStack {
            Group {
                if library.books.isEmpty {
                    VStack(spacing: 16) {
                        Text("No Books")
                            .font(.title2)
                            .accessibilityIdentifier("noBooksTitle")
                        Text("Import a book to get started.")
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("noBooksMessage")
                        Button("Import Sample Book") {
                            Task {
                                await library.importSampleBook()
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
                    logger.info("Importing book: \(url.absoluteString)")
                    if !url.absoluteString.hasPrefix("file:/") {
                        // FIXME: bug in withDocumentPicker URL: the url is sometimes just a path without a scheme, like /data/user/0/org.appfair.app.Stanza_Redux/cache/marcus-aurelius_meditations_george-long.epub
                        url = URL(fileURLWithPath: url.absoluteString)
                    }
                    pickedDocumentURL = nil
                    Task {
                        // Re-acquire security-scoped access for the picked file.
                        // The DocumentPicker releases access immediately after setting the
                        // URL binding, but we need it to persist through the async import:
                        // Failed to import book: Error Domain=NSCocoaErrorDomain Code=257 "The file “Demolished Man, The - Alfred Bester.epub” couldn’t be opened because you don’t have permission to view it." UserInfo={NSFilePath=/private/var/mobile/Library/Mobile Documents/com~apple~CloudDocs/Stanza Redux….epub, NSUnderlyingError=0x1168249f0 {Error Domain=NSPOSIXErrorDomain Code=1 "Operation not permitted"}}
                        #if !SKIP
                        let accessing = url.startAccessingSecurityScopedResource()
                        defer { if accessing { url.stopAccessingSecurityScopedResource() } }
                        #endif
                        await library.importBook(from: url)
                        logger.info("Done importing book: \(url.absoluteString)")
                    }
                }
            }
            .task {
                library.initialize()
                restoreLastOpenBook()
            }
            .onAppear {
                library.refreshBooks()
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
                #if !SKIP // needed to make the entire area tappable on iOS
                .contentShape(Rectangle())
                #endif
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
                        showDeleteConfirmation = true
                    } label: {
                        Label("Delete", systemImage: "trash")
                    }
                }
            }
            .onDelete { indices in
                let booksToDelete = filteredBooks
                if let index = indices.first {
                    bookToDelete = booksToDelete[index]
                    showDeleteConfirmation = true
                }
            }
        }
        .accessibilityIdentifier("bookList") // this crashes the build for some reason
        .searchable(text: $searchText, prompt: "Search library")
        .fullScreenCover(item: $selectedBook, onDismiss: { library.refreshBooks() }) { book in
            ReaderView(bookID: book.id, filePath: book.filePath, database: library.database)
        }
        .sheet(item: $bookForDetail) { book in
            NavigationStack {
                BookDetailView(bookID: book.id, database: library.database, onUpdate: { library.refreshBooks() })
            }
        }
        .alert("Delete Book", isPresented: $showDeleteConfirmation) {
            Button("Delete", role: .destructive) {
                if let book = bookToDelete {
                    withAnimation {
                        library.deleteBook(book)
                    }
                }
                bookToDelete = nil
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

    /// Restores the last open book if the app was exited while reading.
    private func restoreLastOpenBook() {
        let savedBookID = settings.lastOpenBookID
        if savedBookID != Int64(0) {
            // Clear immediately so a crash during open won't loop on next launch
            settings.lastOpenBookID = 0
            if let db = library.database, let book = try? db.book(id: savedBookID) {
                logger.info("Restoring previously open book: '\(book.title)' (id=\(savedBookID))")
                self.selectedBook = book
            }
        }
    }
}
