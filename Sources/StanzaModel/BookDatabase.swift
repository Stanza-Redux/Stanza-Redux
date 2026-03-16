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

    /// The file path where the book is stored on disk.
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

    public static let table = SQLTable(name: "BOOK", columns: [
        id, title, author, filePath, identifier, totalItems, currentItem, progress, dateAdded, dateLastOpened, locatorJSON
    ])

    public init(id: Int64 = 0, title: String, author: String, filePath: String, identifier: String? = nil, totalItems: Int64 = 0, currentItem: Int64 = 0, progress: Double = 0.0, dateAdded: Date = Date(), dateLastOpened: Date? = nil, locatorJSON: String? = nil) {
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
    }
}

/// Manages the local book library database.
public class BookDatabase {
    private let context: SQLContext

    /// The documents directory used as the base for relative file paths.
    private static var documentsPath: String {
        URL.documentsDirectory.path
    }

    /// Converts an absolute file path to a path relative to the documents directory.
    /// If the path is not under the documents directory, it is returned as-is.
    public static func relativePath(for absolutePath: String) -> String {
        let docs = documentsPath
        if absolutePath.hasPrefix(docs) {
            var relative = String(absolutePath.dropFirst(docs.count))
            // Remove leading slash if present
            if relative.hasPrefix("/") {
                relative = String(relative.dropFirst())
            }
            return relative
        }
        return absolutePath
    }

    /// Resolves a stored (relative) file path to an absolute path under the documents directory.
    /// If the path is already absolute, it is returned as-is.
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
            context.userVersion = 3
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

    /// Returns all books, ordered by most recently added first.
    /// File paths are resolved to absolute paths.
    public func allBooks() throws -> [BookRecord] {
        var books = try context.fetchAll(BookRecord.self, orderBy: BookRecord.dateAdded, order: .descending)
        for i in books.indices {
            books[i].filePath = BookDatabase.absolutePath(for: books[i].filePath)
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
        try context.update(stored)
    }

    /// Deletes a book record from the database.
    public func deleteBook(id: Int64) throws {
        dbLogger.info("Deleting book id=\(id)")
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
        try context.update(record)
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
        try context.update(record)
    }

    /// Marks a book as having been opened now.
    public func markOpened(bookID: Int64) throws {
        dbLogger.info("Marking book id=\(bookID) as opened")
        guard var record = try book(id: bookID) else {
            dbLogger.warning("markOpened: book id=\(bookID) not found")
            return
        }
        record.dateLastOpened = Date()
        try context.update(record)
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

    /// Closes the database connection.
    public func close() {
        dbLogger.info("Closing book database")
        try? context.close()
    }
}
