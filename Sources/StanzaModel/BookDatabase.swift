// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import Foundation
import OSLog
import SkipSQL
#if SKIP
import SkipSQLCore // needed for transpiled SkipSQL on Android
#endif

let dbLogger = Logger(subsystem: "Stanza", category: "BookDatabase")

/// Metadata for a book stored in the local library database.
public struct BookRecord: Identifiable, Hashable, SQLCodable {
    public var id: Int64
    static let id = SQLColumn(name: "ID", type: .long, primaryKey: true, autoincrement: true)

    /// The title of the book.
    public var title: String
    static let title = SQLColumn(name: "TITLE", type: .text, index: SQLIndex(name: "IDX_TITLE"))

    /// The author of the book.
    public var author: String
    static let author = SQLColumn(name: "AUTHOR", type: .text, index: SQLIndex(name: "IDX_AUTHOR"))

    /// Relative path to the book file under the documents directory (e.g. "Books/Alice.epub").
    /// Absolute paths must never be stored in the database — they break when the app is
    /// relocated or restored to a different device. Use `BookDatabase.absolutePath(for:)`
    /// to resolve to a full path at runtime.
    public var filePath: String
    static let filePath = SQLColumn(name: "FILE_PATH", type: .text)

    /// A unique identifier for the book (e.g. from the EPUB metadata).
    public var identifier: String?
    static let identifier = SQLColumn(name: "IDENTIFIER", type: .text)

    /// The total number of reading order items (chapters/sections) in the book.
    public var totalItems: Int64
    static let totalItems = SQLColumn(name: "TOTAL_ITEMS", type: .long)

    /// The index of the last reading order item the user viewed (0-based).
    public var currentItem: Int64
    static let currentItem = SQLColumn(name: "CURRENT_ITEM", type: .long)

    /// A fractional progress value from 0.0 to 1.0.
    public var progress: Double
    static let progress = SQLColumn(name: "PROGRESS", type: .real)

    /// The date the book was added to the library.
    public var dateAdded: Date
    static let dateAdded = SQLColumn(name: "DATE_ADDED", type: .real)

    /// The date the book was last opened.
    public var dateLastOpened: Date?
    static let dateLastOpened = SQLColumn(name: "DATE_LAST_OPENED", type: .real)

    /// JSON representation of the Readium Locator for the last reading position.
    public var locatorJSON: String?
    static let locatorJSON = SQLColumn(name: "LOCATOR_JSON", type: .text)

    /// Relative path to the extracted cover image file (e.g. "Books/Alice.jpg"), or nil if no cover.
    /// Absolute paths must never be stored in the database — they break when the app is
    /// relocated or restored to a different device. Use `BookDatabase.absolutePath(for:)`
    /// to resolve to a full path at runtime.
    public var coverImagePath: String?
    static let coverImagePath = SQLColumn(name: "COVER_IMAGE_PATH", type: .text)

    public static let table = SQLTable(name: "BOOK", columns: [
        id, title, author, filePath, identifier, totalItems, currentItem, progress, dateAdded, dateLastOpened, locatorJSON, coverImagePath
    ])

    public init(id: Int64 = 0, title: String, author: String, filePath: String, identifier: String? = nil, totalItems: Int64 = 0, currentItem: Int64 = 0, progress: Double = 0.0, dateAdded: Date = Date(), dateLastOpened: Date? = nil, locatorJSON: String? = nil, coverImagePath: String? = nil) {
        self.id = id
        self.title = title
        self.author = author
        self.filePath = filePath
        self.identifier = identifier
        self.totalItems = totalItems
        self.currentItem = currentItem
        self.progress = progress
        self.dateAdded = dateAdded
        self.dateLastOpened = dateLastOpened
        self.locatorJSON = locatorJSON
        self.coverImagePath = coverImagePath
    }

    public init(row: SQLRow, context: SQLContext) throws {
        self.id = try Self.id.longValueRequired(in: row)
        self.title = try Self.title.textValueRequired(in: row)
        self.author = try Self.author.textValueRequired(in: row)
        self.filePath = try Self.filePath.textValueRequired(in: row)
        self.identifier = Self.identifier.textValue(in: row)
        self.totalItems = try Self.totalItems.longValueRequired(in: row)
        self.currentItem = try Self.currentItem.longValueRequired(in: row)
        self.progress = try Self.progress.realValueRequired(in: row)
        self.dateAdded = try Self.dateAdded.dateValueRequired(in: row)
        self.dateLastOpened = Self.dateLastOpened.dateValue(in: row)
        self.locatorJSON = Self.locatorJSON.textValue(in: row)
        self.coverImagePath = Self.coverImagePath.textValue(in: row)
    }

    public func encode(row: inout SQLRow) throws {
        row[Self.id] = SQLValue(self.id)
        row[Self.title] = SQLValue(self.title)
        row[Self.author] = SQLValue(self.author)
        row[Self.filePath] = SQLValue(self.filePath)
        row[Self.identifier] = SQLValue(self.identifier)
        row[Self.totalItems] = SQLValue(self.totalItems)
        row[Self.currentItem] = SQLValue(self.currentItem)
        row[Self.progress] = SQLValue(self.progress)
        row[Self.dateAdded] = SQLValue(self.dateAdded.timeIntervalSince1970)
        row[Self.dateLastOpened] = SQLValue(self.dateLastOpened?.timeIntervalSince1970)
        row[Self.locatorJSON] = SQLValue(self.locatorJSON)
        row[Self.coverImagePath] = SQLValue(self.coverImagePath)
    }
}

/// A bookmark stored for a specific book.
public struct BookmarkRecord: Identifiable, Hashable, SQLCodable {
    public var id: Int64
    static let id = SQLColumn(name: "ID", type: .long, primaryKey: true, autoincrement: true)

    /// The ID of the book this bookmark belongs to.
    public var bookID: Int64
    static let bookID = SQLColumn(name: "BOOK_ID", type: .long, index: SQLIndex(name: "IDX_BOOKMARK_BOOK_ID"))

    /// JSON representation of the Readium Locator for this bookmark.
    public var locatorJSON: String
    static let locatorJSON = SQLColumn(name: "LOCATOR_JSON", type: .text)

    /// Human-readable progress indication (e.g. "42%").
    public var progressLabel: String
    static let progressLabel = SQLColumn(name: "PROGRESS_LABEL", type: .text)

    /// A small excerpt of the text on the page with the bookmark.
    public var excerpt: String
    static let excerpt = SQLColumn(name: "EXCERPT", type: .text)

    /// The name of the chapter for the bookmark (if available).
    public var chapter: String
    static let chapter = SQLColumn(name: "CHAPTER", type: .text)

    /// User notes for the bookmark.
    public var notes: String
    static let notes = SQLColumn(name: "NOTES", type: .text)

    /// Display order for user reordering.
    public var sortOrder: Int64
    static let sortOrder = SQLColumn(name: "SORT_ORDER", type: .long)

    /// The date the bookmark was created.
    public var dateCreated: Date
    static let dateCreated = SQLColumn(name: "DATE_CREATED", type: .real)

    public static let table = SQLTable(name: "BOOKMARK", columns: [
        id, bookID, locatorJSON, progressLabel, excerpt, chapter, notes, sortOrder, dateCreated
    ])

    public init(id: Int64 = 0, bookID: Int64, locatorJSON: String, progressLabel: String = "", excerpt: String = "", chapter: String = "", notes: String = "", sortOrder: Int64 = 0, dateCreated: Date = Date()) {
        self.id = id
        self.bookID = bookID
        self.locatorJSON = locatorJSON
        self.progressLabel = progressLabel
        self.excerpt = excerpt
        self.chapter = chapter
        self.notes = notes
        self.sortOrder = sortOrder
        self.dateCreated = dateCreated
    }

    public init(row: SQLRow, context: SQLContext) throws {
        self.id = try Self.id.longValueRequired(in: row)
        self.bookID = try Self.bookID.longValueRequired(in: row)
        self.locatorJSON = try Self.locatorJSON.textValueRequired(in: row)
        self.progressLabel = try Self.progressLabel.textValueRequired(in: row)
        self.excerpt = try Self.excerpt.textValueRequired(in: row)
        self.chapter = try Self.chapter.textValueRequired(in: row)
        self.notes = try Self.notes.textValueRequired(in: row)
        self.sortOrder = try Self.sortOrder.longValueRequired(in: row)
        self.dateCreated = try Self.dateCreated.dateValueRequired(in: row)
    }

    public func encode(row: inout SQLRow) throws {
        row[Self.id] = SQLValue(self.id)
        row[Self.bookID] = SQLValue(self.bookID)
        row[Self.locatorJSON] = SQLValue(self.locatorJSON)
        row[Self.progressLabel] = SQLValue(self.progressLabel)
        row[Self.excerpt] = SQLValue(self.excerpt)
        row[Self.chapter] = SQLValue(self.chapter)
        row[Self.notes] = SQLValue(self.notes)
        row[Self.sortOrder] = SQLValue(self.sortOrder)
        row[Self.dateCreated] = SQLValue(self.dateCreated.timeIntervalSince1970)
    }
}

/// Manages the local book library database.
public class BookDatabase {
    private let context: SQLContext

    /// Strips the documents directory prefix from an absolute path, returning a relative path
    /// like `Books/Alice.epub`.
    ///
    /// Only relative paths should be stored in the database — absolute paths break when the
    /// app is relocated or restored to a different device.
    public static func relativePath(for absolutePath: String) -> String {
        let docsPath = URL.documentsDirectory.path
        if absolutePath.hasPrefix(docsPath + "/") {
            return String(absolutePath.dropFirst(docsPath.count + 1))
        }
        if absolutePath.hasPrefix(docsPath) {
            return String(absolutePath.dropFirst(docsPath.count))
        }
        // Path is not under the documents directory — this should not normally happen.
        // Return it as-is but log a warning so callers can investigate.
        dbLogger.warning("relativePath: path is not under documents directory: \(absolutePath)")
        return absolutePath
    }

    /// Resolves a stored relative path to an absolute path under the documents directory.
    /// If the path is already absolute, returns it as-is.
    public static func absolutePath(for storedPath: String) -> String {
        if storedPath.hasPrefix("/") {
            return storedPath
        }
        return URL.documentsDirectory.appendingPathComponent(storedPath).path
    }

    /// Opens or creates the book database at the given path.
    /// Pass `nil` for an in-memory database (useful for testing).
    public init(path: String? = nil) throws {
        dbLogger.info("Opening book database at: \(path ?? ":memory:")")
        if let path = path {
            let dir = URL(fileURLWithPath: path).deletingLastPathComponent()
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            self.context = try SQLContext(path: path, flags: [.create, .readWrite], configuration: SQLiteConfiguration.platform)
        } else {
            self.context = try SQLContext(path: ":memory:", configuration: SQLiteConfiguration.platform)
        }

        self.context.trace { sql in
            dbLogger.info("SQL: \(sql)")
        }

        try createOrMigrateSchema()
    }

    private func createOrMigrateSchema() throws {
        dbLogger.info("Book database schema version: \(self.context.userVersion)")
        if context.userVersion < 1 {
            dbLogger.info("Creating BOOK table (schema v1)")
            for ddl in BookRecord.table.createTableSQL(columns: [
                BookRecord.id, BookRecord.title, BookRecord.author, BookRecord.filePath,
                BookRecord.identifier, BookRecord.totalItems, BookRecord.currentItem,
                BookRecord.progress, BookRecord.dateAdded, BookRecord.dateLastOpened
            ]) {
                dbLogger.debug("SQL: \(String(describing: ddl))")
                try context.exec(ddl)
            }
            context.userVersion = 1
        }
        if context.userVersion < 2 {
            dbLogger.info("Migrating BOOK table to schema v2 (adding LOCATOR_JSON)")
            let ddl = SQLExpression("ALTER TABLE BOOK ADD COLUMN LOCATOR_JSON TEXT")
            dbLogger.debug("SQL: \(String(describing: ddl))")
            try context.exec(ddl)
            context.userVersion = 2
        }
        if context.userVersion < 3 {
            dbLogger.info("Migrating BOOK table to schema v3 (converting absolute paths to relative)")
            let books = try context.fetchAll(BookRecord.self)
            for var book in books {
                let oldPath = book.filePath
                let newPath = BookDatabase.relativePath(for: oldPath)
                if newPath != oldPath {
                    dbLogger.debug("Migrating path: '\(oldPath)' -> '\(newPath)'")
                    book.filePath = newPath
                    try context.update(book)
                }
            }
            context.userVersion = 3
        }
        if context.userVersion < 4 {
            dbLogger.info("Creating BOOKMARK table (schema v4)")
            for ddl in BookmarkRecord.table.createTableSQL(columns: [
                BookmarkRecord.id, BookmarkRecord.bookID, BookmarkRecord.locatorJSON,
                BookmarkRecord.progressLabel, BookmarkRecord.excerpt, BookmarkRecord.chapter,
                BookmarkRecord.notes, BookmarkRecord.sortOrder, BookmarkRecord.dateCreated
            ]) {
                dbLogger.debug("SQL: \(String(describing: ddl))")
                try context.exec(ddl)
            }
            context.userVersion = 4
        }
        if context.userVersion < 5 {
            dbLogger.info("Migrating BOOK table to schema v5 (adding COVER_IMAGE_PATH)")
            let ddl = SQLExpression("ALTER TABLE BOOK ADD COLUMN COVER_IMAGE_PATH TEXT")
            dbLogger.debug("SQL: \(String(describing: ddl))")
            try context.exec(ddl)
            context.userVersion = 5
        }
    }

    // MARK: - CRUD operations

    /// Adds a new book record to the database and returns it with its assigned ID.
    @discardableResult
    public func addBook(_ record: BookRecord) throws -> BookRecord {
        dbLogger.info("Adding book: '\(record.title)' by '\(record.author)' at \(record.filePath)")
        let result = try context.insert(record)
        dbLogger.info("Added book with id: \(result.id)")
        return result
    }

    /// Returns all books, ordered by the most recent activity (last opened or
    /// date added) first so that new and recently-read books appear at the top.
    /// File paths are resolved to absolute paths.
    public func allBooks() throws -> [BookRecord] {
        var books = try context.fetchAll(BookRecord.self)
        for i in books.indices {
            books[i].filePath = BookDatabase.absolutePath(for: books[i].filePath)
            if let cover = books[i].coverImagePath {
                books[i].coverImagePath = BookDatabase.absolutePath(for: cover)
            }
        }
        books.sort { a, b in
            let aDate = max(a.dateAdded, a.dateLastOpened ?? a.dateAdded)
            let bDate = max(b.dateAdded, b.dateLastOpened ?? b.dateAdded)
            return aDate > bDate
        }
        dbLogger.debug("Fetched \(books.count) books")
        return books
    }

    /// Fetches a single book by its database ID, or `nil` if not found.
    /// The file path is resolved to an absolute path.
    public func book(id: Int64) throws -> BookRecord? {
        var result = try context.fetch(BookRecord.self, primaryKeys: [SQLValue(id)])
        if result != nil {
            result!.filePath = BookDatabase.absolutePath(for: result!.filePath)
            if let cover = result!.coverImagePath {
                result!.coverImagePath = BookDatabase.absolutePath(for: cover)
            }
        }
        dbLogger.debug("Fetched book id=\(id): \(result != nil ? result!.title : "not found")")
        return result
    }

    /// Updates an existing book record in the database.
    /// The file path is converted to a relative path before storage.
    public func updateBook(_ record: BookRecord) throws {
        dbLogger.info("Updating book id=\(record.id): '\(record.title)'")
        var stored = record
        stored.filePath = BookDatabase.relativePath(for: record.filePath)
        if let cover = stored.coverImagePath {
            stored.coverImagePath = BookDatabase.relativePath(for: cover)
        }
        try context.update(stored)
    }

    /// Deletes a book record and its associated bookmarks from the database.
    public func deleteBook(id: Int64) throws {
        dbLogger.info("Deleting book id=\(id) and its bookmarks")
        try context.delete(BookmarkRecord.self, where: BookmarkRecord.bookID.equals(SQLValue(id)))
        try context.delete(BookRecord.self, where: BookRecord.id.equals(SQLValue(id)))
    }

    /// Returns the number of books in the database.
    public func count() throws -> Int64 {
        let n = try context.count(BookRecord.self)
        dbLogger.debug("Book count: \(n)")
        return n
    }

    // MARK: - Search

    /// Searches books by title or author. Case-insensitive substring match.
    public func searchBooks(query: String) throws -> [BookRecord] {
        dbLogger.info("Searching books for: '\(query)'")
        let pattern = SQLValue("%" + query + "%")
        let predicate = BookRecord.title.like(pattern).or(BookRecord.author.like(pattern))
        var results = try context.fetchAll(BookRecord.self, where: predicate, orderBy: BookRecord.dateAdded, order: .descending)
        for i in results.indices {
            results[i].filePath = BookDatabase.absolutePath(for: results[i].filePath)
            if let cover = results[i].coverImagePath {
                results[i].coverImagePath = BookDatabase.absolutePath(for: cover)
            }
        }
        dbLogger.debug("Search returned \(results.count) results")
        return results
    }

    // MARK: - Progress tracking

    /// Updates the reading progress for a book.
    public func updateProgress(bookID: Int64, currentItem: Int64, totalItems: Int64) throws {
        dbLogger.debug("Updating progress for book id=\(bookID): item \(currentItem)/\(totalItems)")
        guard var record = try book(id: bookID) else {
            dbLogger.warning("updateProgress: book id=\(bookID) not found")
            return
        }
        record.currentItem = currentItem
        record.totalItems = totalItems
        record.progress = totalItems > 0 ? Double(currentItem) / Double(totalItems) : 0.0
        record.dateLastOpened = Date()
        try updateBook(record)
    }

    /// Saves the reading position for a book as a serialized Readium Locator JSON string,
    /// along with the overall progress (0.0 to 1.0).
    public func saveReadingPosition(bookID: Int64, locatorJSON: String, progress: Double) throws {
        dbLogger.debug("Saving reading position for book id=\(bookID): progress=\(progress)")
        guard var record = try book(id: bookID) else {
            dbLogger.warning("saveReadingPosition: book id=\(bookID) not found")
            return
        }
        record.locatorJSON = locatorJSON
        record.progress = progress
        record.dateLastOpened = Date()
        try updateBook(record)
    }

    /// Marks a book as having been opened now.
    public func markOpened(bookID: Int64) throws {
        dbLogger.info("Marking book id=\(bookID) as opened")
        guard var record = try book(id: bookID) else {
            dbLogger.warning("markOpened: book id=\(bookID) not found")
            return
        }
        record.dateLastOpened = Date()
        try updateBook(record)
    }

    // MARK: - Import

    /// Imports a book from the given file URL by loading its publication metadata.
    /// The file is copied into the app's documents directory for persistent storage.
    /// Returns the newly created `BookRecord`.
    @discardableResult
    public func importBook(from sourceURL: URL) async throws -> BookRecord {
        dbLogger.info("Importing book from: \(sourceURL.absoluteString)")
        let booksDir = URL.documentsDirectory.appendingPathComponent("Books")
        try FileManager.default.createDirectory(at: booksDir, withIntermediateDirectories: true)
        let destinationURL = booksDir.appendingPathComponent(sourceURL.lastPathComponent)

        if FileManager.default.fileExists(atPath: destinationURL.path) {
            dbLogger.debug("Removing existing file at: \(destinationURL.path)")
            try FileManager.default.removeItem(at: destinationURL)
        }
        dbLogger.debug("Copying book to: \(destinationURL.path)")
        try FileManager.default.copyItem(at: sourceURL, to: destinationURL)

        dbLogger.debug("Loading publication metadata")
        let pub = try await Pub.loadPublication(from: destinationURL)
        let metadata = pub.metadata
        let bookTitle = metadata.title ?? sourceURL.deletingPathExtension().lastPathComponent
        let bookAuthor = metadata.identifier ?? ""
        let totalItems = Int64(pub.manifest.readingOrder.count)
        dbLogger.info("Imported book: '\(bookTitle)' with \(totalItems) reading order items")

        let relativePath = BookDatabase.relativePath(for: destinationURL.path)
        dbLogger.debug("Storing relative path: '\(relativePath)'")
        let record = BookRecord(
            title: bookTitle,
            author: bookAuthor,
            filePath: relativePath,
            identifier: metadata.identifier,
            totalItems: totalItems
        )
        return try addBook(record)
    }

    /// Updates the cover image path for a book.
    /// The `coverPath` must be a relative path (e.g. "Books/Alice.jpg"). Absolute paths
    /// must never be stored in the database — use `BookDatabase.relativePath(for:)` first.
    public func setCoverImagePath(bookID: Int64, coverPath: String) throws {
        if coverPath.hasPrefix("/") {
            dbLogger.warning("setCoverImagePath called with absolute path — this should be relative: \(coverPath)")
        }
        guard var record = try context.fetch(BookRecord.self, primaryKeys: [SQLValue(bookID)]) else { return }
        record.coverImagePath = coverPath
        try context.update(record)
        dbLogger.info("Updated cover image path for book id=\(bookID): '\(coverPath)'")
    }

    // MARK: - Bookmarks

    /// Adds a bookmark and returns it with its assigned ID.
    @discardableResult
    public func addBookmark(_ record: BookmarkRecord) throws -> BookmarkRecord {
        dbLogger.info("Adding bookmark for book id=\(record.bookID): chapter='\(record.chapter)'")
        let result = try context.insert(record)
        dbLogger.info("Added bookmark with id: \(result.id)")
        return result
    }

    /// Returns all bookmarks for a given book, ordered by sort order then date created.
    public func bookmarks(forBookID bookID: Int64) throws -> [BookmarkRecord] {
        let predicate = BookmarkRecord.bookID.equals(SQLValue(bookID))
        let results = try context.fetchAll(BookmarkRecord.self, where: predicate, orderBy: BookmarkRecord.sortOrder, order: .ascending)
        dbLogger.debug("Fetched \(results.count) bookmarks for book id=\(bookID)")
        return results
    }

    /// Updates an existing bookmark record.
    public func updateBookmark(_ record: BookmarkRecord) throws {
        dbLogger.info("Updating bookmark id=\(record.id)")
        try context.update(record)
    }

    /// Deletes a bookmark by its ID.
    public func deleteBookmark(id: Int64) throws {
        dbLogger.info("Deleting bookmark id=\(id)")
        try context.delete(BookmarkRecord.self, where: BookmarkRecord.id.equals(SQLValue(id)))
    }

    /// Closes the database connection.
    public func close() {
        dbLogger.info("Closing book database")
        try? context.close()
    }
}
