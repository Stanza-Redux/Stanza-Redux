// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later
import XCTest
import Foundation
@testable import StanzaModel

@available(macOS 14, *)
final class BookDatabaseTests: XCTestCase {

    func testAddAndFetchBook() throws {
        let db = try BookDatabase(path: nil)
        defer { db.close() }

        let record = BookRecord(title: "Test Book", author: "Author A", filePath: "/tmp/test.epub", identifier: "id-1", totalItems: 10)
        let inserted = try db.addBook(record)

        XCTAssertTrue(inserted.id > 0, "inserted record should have a positive ID")
        XCTAssertEqual("Test Book", inserted.title)
        XCTAssertEqual("Author A", inserted.author)
        XCTAssertEqual("/tmp/test.epub", inserted.filePath)
        XCTAssertEqual("id-1", inserted.identifier)
        XCTAssertEqual(10, inserted.totalItems)
        XCTAssertEqual(0, inserted.currentItem)
        XCTAssertEqual(0.0, inserted.progress)
        XCTAssertNil(inserted.dateLastOpened)

        let fetched = try db.book(id: inserted.id)
        XCTAssertNotNil(fetched)
        XCTAssertEqual(inserted.id, fetched?.id)
        XCTAssertEqual("Test Book", fetched?.title)
    }

    func testAllBooksOrderedByDateAdded() throws {
        let db = try BookDatabase(path: nil)
        defer { db.close() }

        let earlier = Date(timeIntervalSince1970: 1000)
        let later = Date(timeIntervalSince1970: 2000)

        try db.addBook(BookRecord(title: "Old Book", author: "A", filePath: "/a.epub", dateAdded: earlier))
        try db.addBook(BookRecord(title: "New Book", author: "B", filePath: "/b.epub", dateAdded: later))

        let books = try db.allBooks()
        XCTAssertEqual(2, books.count)
        // Most recent first
        XCTAssertEqual("New Book", books[0].title)
        XCTAssertEqual("Old Book", books[1].title)
    }

    func testUpdateBook() throws {
        let db = try BookDatabase(path: nil)
        defer { db.close() }

        var record = try db.addBook(BookRecord(title: "Original", author: "Author", filePath: "/test.epub"))
        record.title = "Updated Title"
        record.author = "New Author"
        try db.updateBook(record)

        let fetched = try db.book(id: record.id)
        XCTAssertEqual("Updated Title", fetched?.title)
        XCTAssertEqual("New Author", fetched?.author)
    }

    func testUpdateProgress() throws {
        let db = try BookDatabase(path: nil)
        defer { db.close() }

        let record = try db.addBook(BookRecord(title: "Book", author: "Auth", filePath: "/book.epub", totalItems: 20))

        try db.updateProgress(bookID: record.id, currentItem: 5, totalItems: 20)

        let fetched = try db.book(id: record.id)
        XCTAssertNotNil(fetched)
        XCTAssertEqual(5, fetched?.currentItem)
        XCTAssertEqual(20, fetched?.totalItems)
        XCTAssertEqual(0.25, fetched?.progress ?? -1.0, accuracy: 0.001)
        XCTAssertNotNil(fetched?.dateLastOpened)
    }

    func testMarkOpened() throws {
        let db = try BookDatabase(path: nil)
        defer { db.close() }

        let record = try db.addBook(BookRecord(title: "Book", author: "Auth", filePath: "/book.epub"))
        XCTAssertNil(record.dateLastOpened)

        try db.markOpened(bookID: record.id)

        let fetched = try db.book(id: record.id)
        XCTAssertNotNil(fetched?.dateLastOpened)
    }

    func testDeleteBook() throws {
        let db = try BookDatabase(path: nil)
        defer { db.close() }

        let record = try db.addBook(BookRecord(title: "To Delete", author: "Auth", filePath: "/del.epub"))
        XCTAssertEqual(1, try db.count())

        try db.deleteBook(id: record.id)
        XCTAssertEqual(0, try db.count())
        XCTAssertNil(try db.book(id: record.id))
    }

    func testCount() throws {
        let db = try BookDatabase(path: nil)
        defer { db.close() }

        XCTAssertEqual(0, try db.count())
        try db.addBook(BookRecord(title: "A", author: "A", filePath: "/a.epub"))
        XCTAssertEqual(1, try db.count())
        try db.addBook(BookRecord(title: "B", author: "B", filePath: "/b.epub"))
        XCTAssertEqual(2, try db.count())
    }

    func testImportBook() async throws {
        let db = try BookDatabase(path: nil)
        defer { db.close() }

        let epubURL = try XCTUnwrap(Bundle.module.url(forResource: "Alice", withExtension: "epub"))
        let record = try await db.importBook(from: epubURL)

        XCTAssertEqual("Alice's Adventures in Wonderland", record.title)
        XCTAssertEqual("http://www.gutenberg.org/11", record.identifier)
        XCTAssertEqual(15, record.totalItems)
        XCTAssertEqual(0, record.currentItem)
        XCTAssertEqual(0.0, record.progress)

        XCTAssertEqual(1, try db.count())
    }

    func testSearchBooks() throws {
        let db = try BookDatabase(path: nil)
        defer { db.close() }

        try db.addBook(BookRecord(title: "Alice in Wonderland", author: "Lewis Carroll", filePath: "/alice.epub"))
        try db.addBook(BookRecord(title: "Through the Looking Glass", author: "Lewis Carroll", filePath: "/looking.epub"))
        try db.addBook(BookRecord(title: "Moby Dick", author: "Herman Melville", filePath: "/moby.epub"))

        // Search by title
        let aliceResults = try db.searchBooks(query: "Alice")
        XCTAssertEqual(1, aliceResults.count)
        XCTAssertEqual("Alice in Wonderland", aliceResults[0].title)

        // Search by author
        let carrollResults = try db.searchBooks(query: "Carroll")
        XCTAssertEqual(2, carrollResults.count)

        // Search with no match
        let noResults = try db.searchBooks(query: "Gatsby")
        XCTAssertEqual(0, noResults.count)

        // Case-insensitive search
        let lowerResults = try db.searchBooks(query: "alice")
        XCTAssertEqual(1, lowerResults.count)
    }

    func testProgressCalculation() throws {
        let db = try BookDatabase(path: nil)
        defer { db.close() }

        let record = try db.addBook(BookRecord(title: "Book", author: "Auth", filePath: "/b.epub", totalItems: 10))

        // Progress at halfway
        try db.updateProgress(bookID: record.id, currentItem: 5, totalItems: 10)
        var fetched = try XCTUnwrap(db.book(id: record.id))
        XCTAssertEqual(0.5, fetched.progress, accuracy: 0.001)

        // Progress at end
        try db.updateProgress(bookID: record.id, currentItem: 10, totalItems: 10)
        fetched = try XCTUnwrap(db.book(id: record.id))
        XCTAssertEqual(1.0, fetched.progress, accuracy: 0.001)

        // Zero total items should give 0 progress
        try db.updateProgress(bookID: record.id, currentItem: 0, totalItems: 0)
        fetched = try XCTUnwrap(db.book(id: record.id))
        XCTAssertEqual(0.0, fetched.progress, accuracy: 0.001)
    }
}
