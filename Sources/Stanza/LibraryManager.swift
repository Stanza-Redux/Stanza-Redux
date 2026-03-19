// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import Foundation
import SwiftUI
import OSLog
import Observation
import StanzaModel
#if !SKIP
import ReadiumShared
#else
import org.readium.r2.shared.publication.services.cover
#endif

/// Manages the book library: database access, importing, cover extraction, and deletion.
/// Shared via the SwiftUI environment so that both LibraryView and BrowseView use the same instance.
@Observable public class LibraryManager {
    /// All books in the library, kept in sync with the database.
    public var books: [BookRecord] = []

    /// User-facing error message from the last failed operation, or nil.
    public var errorMessage: String? = nil

    /// The underlying book database, available after `initialize()`.
    public private(set) var database: BookDatabase? = nil

    private let libraryLogger = Logger(subsystem: "Stanza", category: "LibraryManager")

    public init() {}

    // MARK: - Initialization

    /// Opens (or creates) the library database and loads all books.
    /// Safe to call multiple times; subsequent calls are no-ops.
    public func initialize() {
        guard database == nil else { return }
        libraryLogger.info("Initializing library database")
        do {
            let dir = URL.applicationSupportDirectory
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            let dbPath = dir.appendingPathComponent("library.sqlite").path
            let db = try BookDatabase(path: dbPath)
            self.database = db
            self.books = try db.allBooks()
            libraryLogger.info("Library initialized with \(self.books.count) books")
        } catch {
            libraryLogger.error("Failed to open database: \(error)")
            errorMessage = "Failed to open library: \(error.localizedDescription)"
        }
    }

    // MARK: - Refresh

    /// Reloads the book list from the database.
    public func refreshBooks() {
        guard let db = database else { return }
        do {
            self.books = try db.allBooks()
        } catch {
            libraryLogger.error("Failed to refresh books: \(error)")
        }
    }

    // MARK: - Lookup

    /// Returns the book record matching the given identifier, or nil if not in the library.
    public func book(withIdentifier identifier: String) -> BookRecord? {
        return books.first(where: { $0.identifier == identifier })
    }

    // MARK: - Import

    /// Imports a book from a local file URL (e.g. document picker or bundle resource).
    /// Copies the file into the Books directory, extracts metadata and cover, and updates the database.
    @discardableResult
    public func importBook(from url: URL) async -> BookRecord? {
        libraryLogger.info("importBook: \(url.absoluteString)")
        guard let db = database else {
            errorMessage = "Library not available"
            return nil
        }
        do {
            let record = try await db.importBook(from: url)
            await extractAndSaveCover(for: record)
            refreshBooks()
            return record
        } catch {
            libraryLogger.error("Failed to import book: \(error)")
            errorMessage = "Failed to import book: \(error.localizedDescription)"
            #if SKIP
            android.util.Log.e("Stanza", "Error importing book", error as? Throwable)
            #endif
            return nil
        }
    }

    /// Imports a book that was already downloaded to a local path (e.g. from an OPDS catalog).
    /// Uses the provided metadata from the catalog entry when available.
    @discardableResult
    public func importDownloadedBook(from fileURL: URL, title: String, authors: [String], identifier: String?) async -> BookRecord? {
        libraryLogger.info("importDownloadedBook: '\(title)' from \(fileURL.path)")
        guard let db = database else {
            errorMessage = "Library not available"
            return nil
        }
        do {
            let pub = try await Pub.loadPublication(from: fileURL)
            let metadata = pub.metadata
            let bookTitle = metadata.title ?? title
            let bookAuthor = authors.isEmpty ? (metadata.identifier ?? "") : authors.joined(separator: ", ")
            let totalItems = Int64(pub.manifest.readingOrder.count)
            let relativePath = BookDatabase.relativePath(for: fileURL.path)

            let record = BookRecord(
                title: bookTitle,
                author: bookAuthor,
                filePath: relativePath,
                identifier: identifier ?? metadata.identifier,
                totalItems: totalItems
            )
            let savedRecord = try db.addBook(record)
            libraryLogger.info("Book imported to library: '\(bookTitle)' (id=\(savedRecord.id))")
            await extractAndSaveCover(for: savedRecord)
            refreshBooks()
            return savedRecord
        } catch {
            libraryLogger.error("Failed to import downloaded book: \(error)")
            errorMessage = "Import failed: \(error.localizedDescription)"
            #if SKIP
            android.util.Log.e("Stanza", "Error importing downloaded book", error as? Throwable)
            #endif
            return nil
        }
    }

    /// Imports the bundled sample book (Alice.epub).
    @discardableResult
    public func importSampleBook() async -> BookRecord? {
        guard let sampleURL = Bundle.module.url(forResource: "Alice", withExtension: "epub") else {
            libraryLogger.error("Sample book not found in bundle")
            return nil
        }
        libraryLogger.info("Importing sample book from bundle")
        return await importBook(from: sampleURL)
    }

    // MARK: - Delete

    /// Deletes a book from the database and removes its files from disk.
    public func deleteBook(_ book: BookRecord) {
        guard let db = database else { return }
        libraryLogger.info("Deleting book: '\(book.title)' (id=\(book.id)) at \(book.filePath)")
        do {
            try db.deleteBook(id: book.id)
            let absolutePath = BookDatabase.absolutePath(for: book.filePath)
            try? FileManager.default.removeItem(at: URL(fileURLWithPath: absolutePath))
            if let coverPath = book.coverImagePath {
                let absoluteCoverPath = BookDatabase.absolutePath(for: coverPath)
                try? FileManager.default.removeItem(at: URL(fileURLWithPath: absoluteCoverPath))
            }
            libraryLogger.debug("Book file removed: \(book.filePath)")
        } catch {
            libraryLogger.error("Failed to delete book: \(error)")
        }
        refreshBooks()
    }

    // MARK: - Cover Extraction

    private func extractAndSaveCover(for record: BookRecord) async {
        guard let db = database else {
            libraryLogger.warning("Cover extraction: no database available for '\(record.title)'")
            return
        }
        let bookPath = BookDatabase.absolutePath(for: record.filePath)
        let bookURL = URL(fileURLWithPath: bookPath)
        let coverURL = bookURL.deletingPathExtension().appendingPathExtension("jpg")
        libraryLogger.info("Cover extraction: loading publication from \(bookPath)")
        libraryLogger.info("Cover extraction: cover will be saved to \(coverURL.path)")
        do {
            let pub = try await Pub.loadPublication(from: bookURL)
            libraryLogger.info("Cover extraction: publication loaded, attempting cover extraction")
            let coverData = await extractCoverData(from: pub, title: record.title)
            if let data = coverData {
                libraryLogger.info("Cover extraction: got \(data.count) bytes of cover data for '\(record.title)'")
                try data.write(to: coverURL)
                let relativePath = BookDatabase.relativePath(for: coverURL.path)
                libraryLogger.info("Cover extraction: saved to '\(relativePath)', updating database")
                try db.setCoverImagePath(bookID: record.id, coverPath: relativePath)
                libraryLogger.info("Cover extraction: successfully saved cover for '\(record.title)'")
            } else {
                libraryLogger.warning("Cover extraction: no cover data returned for '\(record.title)'")
            }
        } catch {
            libraryLogger.error("Cover extraction: failed for '\(record.title)': \(error)")
            #if SKIP
            android.util.Log.e("Stanza", "Cover extraction error", error as? Throwable)
            #endif
        }
    }

    private func extractCoverData(from pub: Pub, title: String) async -> Data? {
        #if !SKIP
        libraryLogger.info("Cover extraction [iOS]: calling publication.cover() for '\(title)'")
        let result = await pub.platformValue.cover()
        switch result {
        case .success(let image):
            if let image = image {
                libraryLogger.info("Cover extraction [iOS]: got image \(Int(image.size.width))x\(Int(image.size.height)) for '\(title)'")
                guard let jpegData = image.jpegData(compressionQuality: 0.85) else {
                    libraryLogger.error("Cover extraction [iOS]: jpegData() returned nil for '\(title)'")
                    return nil
                }
                return jpegData
            } else {
                libraryLogger.warning("Cover extraction [iOS]: cover() returned success but image is nil for '\(title)' — publication may not have a cover resource")
                return nil
            }
        case .failure(let error):
            libraryLogger.error("Cover extraction [iOS]: cover() returned failure for '\(title)': \(error)")
            return nil
        }
        #else
        libraryLogger.info("Cover extraction [Android]: calling publication.cover() for '\(title)'")
        do {
            let bitmap: android.graphics.Bitmap? = pub.platformValue.cover()
            if let bitmap = bitmap {
                libraryLogger.info("Cover extraction [Android]: got bitmap \(bitmap.width)x\(bitmap.height) for '\(title)'")
                let stream = java.io.ByteArrayOutputStream()
                bitmap.compress(android.graphics.Bitmap.CompressFormat.JPEG, 85, stream)
                let bytes = stream.toByteArray()
                return Data(platformValue: bytes)
            } else {
                libraryLogger.warning("Cover extraction [Android]: cover() returned null for '\(title)' — publication may not have a cover resource")
                return nil
            }
        } catch {
            libraryLogger.error("Cover extraction [Android]: cover() threw exception for '\(title)': \(error)")
            android.util.Log.e("Stanza", "Cover extraction error for '\(title)'", error as? Throwable)
            return nil
        }
        #endif
    }
}
