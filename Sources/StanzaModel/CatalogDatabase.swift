// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import Foundation
import OSLog
import SkipSQL
#if SKIP
import SkipSQLCore // needed for transpiled SkipSQL on Android
#endif

let catalogLogger = Logger(subsystem: "Stanza", category: "CatalogDatabase")

/// Metadata for an OPDS catalog feed stored in the local database.
public struct CatalogRecord: Identifiable, Hashable, SQLCodable {
    public var id: Int64
    static let id = SQLColumn(name: "ID", type: .long, primaryKey: true, autoincrement: true)

    /// The display name of the catalog.
    public var name: String
    static let name = SQLColumn(name: "NAME", type: .text, index: SQLIndex(name: "IDX_CATALOG_NAME"))

    /// The URL of the OPDS feed.
    public var url: String
    static let url = SQLColumn(name: "URL", type: .text)

    /// An optional icon name or emoji for the catalog.
    public var icon: String?
    static let icon = SQLColumn(name: "ICON", type: .text)

    /// An optional description of the catalog.
    public var desc: String?
    static let desc = SQLColumn(name: "DESCRIPTION", type: .text)

    /// The date the catalog was added.
    public var dateCreated: Date
    static let dateCreated = SQLColumn(name: "DATE_CREATED", type: .real)

    /// Sort order for display.
    public var sortOrder: Int64
    static let sortOrder = SQLColumn(name: "SORT_ORDER", type: .long)

    public static let table = SQLTable(name: "CATALOG", columns: [
        id, name, url, icon, desc, dateCreated, sortOrder
    ])

    public init(id: Int64 = 0, name: String, url: String, icon: String? = nil, desc: String? = nil, dateCreated: Date = Date(), sortOrder: Int64 = 0) {
        self.id = id
        self.name = name
        self.url = url
        self.icon = icon
        self.desc = desc
        self.dateCreated = dateCreated
        self.sortOrder = sortOrder
    }

    public init(row: SQLRow, context: SQLContext) throws {
        self.id = try Self.id.longValueRequired(in: row)
        self.name = try Self.name.textValueRequired(in: row)
        self.url = try Self.url.textValueRequired(in: row)
        self.icon = Self.icon.textValue(in: row)
        self.desc = Self.desc.textValue(in: row)
        self.dateCreated = try Self.dateCreated.dateValueRequired(in: row)
        self.sortOrder = try Self.sortOrder.longValueRequired(in: row)
    }

    public func encode(row: inout SQLRow) throws {
        row[Self.id] = SQLValue(self.id)
        row[Self.name] = SQLValue(self.name)
        row[Self.url] = SQLValue(self.url)
        row[Self.icon] = SQLValue(self.icon)
        row[Self.desc] = SQLValue(self.desc)
        row[Self.dateCreated] = SQLValue(self.dateCreated.timeIntervalSince1970)
        row[Self.sortOrder] = SQLValue(self.sortOrder)
    }
}

/// Manages the local catalog database.
public class CatalogDatabase {
    private let context: SQLContext

    /// Opens or creates the catalog database at the given path.
    /// Pass `nil` for an in-memory database (useful for testing).
    public init(path: String? = nil) throws {
        catalogLogger.info("Opening catalog database at: \(path ?? ":memory:")")
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
        catalogLogger.info("Catalog database schema version: \(self.context.userVersion)")
        if context.userVersion < 1 {
            catalogLogger.info("Creating CATALOG table (schema v1)")
            for ddl in CatalogRecord.table.createTableSQL(columns: [
                CatalogRecord.id, CatalogRecord.name, CatalogRecord.url,
                CatalogRecord.icon, CatalogRecord.desc, CatalogRecord.dateCreated,
                CatalogRecord.sortOrder
            ]) {
                catalogLogger.debug("SQL: \(String(describing: ddl))")
                try context.exec(ddl)
            }
            context.userVersion = 1
        }
    }

    // MARK: - CRUD operations

    @discardableResult
    public func addCatalog(_ record: CatalogRecord) throws -> CatalogRecord {
        catalogLogger.info("Adding catalog: '\(record.name)' url=\(record.url)")
        let result = try context.insert(record)
        catalogLogger.info("Added catalog with id: \(result.id)")
        return result
    }

    public func allCatalogs() throws -> [CatalogRecord] {
        let catalogs = try context.fetchAll(CatalogRecord.self, orderBy: CatalogRecord.sortOrder, order: .ascending)
        catalogLogger.debug("Fetched \(catalogs.count) catalogs")
        return catalogs
    }

    public func catalog(id: Int64) throws -> CatalogRecord? {
        let result = try context.fetch(CatalogRecord.self, primaryKeys: [SQLValue(id)])
        catalogLogger.debug("Fetched catalog id=\(id): \(result != nil ? result!.name : "not found")")
        return result
    }

    public func updateCatalog(_ record: CatalogRecord) throws {
        catalogLogger.info("Updating catalog id=\(record.id): '\(record.name)'")
        try context.update(record)
    }

    public func deleteCatalog(id: Int64) throws {
        catalogLogger.info("Deleting catalog id=\(id)")
        try context.delete(CatalogRecord.self, where: CatalogRecord.id.equals(SQLValue(id)))
    }

    public func count() throws -> Int64 {
        let n = try context.count(CatalogRecord.self)
        catalogLogger.debug("Catalog count: \(n)")
        return n
    }

    /// Seeds the database with default recommended catalogs if it is empty.
    public func seedDefaults() throws {
        guard try count() == Int64(0) else {
            catalogLogger.debug("Catalog database already seeded, skipping defaults")
            return
        }
        catalogLogger.info("Seeding default catalogs")
        let defaults: [(String, String, String?, String?)] = [
            ("Project Gutenberg", "https://www.gutenberg.org/ebooks/search.opds/", nil, "Over 70,000 free ebooks"),
            ("Internet Archive", "https://bookserver.archive.org/catalog/", nil, "Open library of digital books"),
            ("Ebooks Gratuits", "https://www.ebooksgratuits.com/opds/", nil, "Free French-language ebooks"),
        ]
        for (index, entry) in defaults.enumerated() {
            try addCatalog(CatalogRecord(
                name: entry.0,
                url: entry.1,
                icon: entry.2,
                desc: entry.3,
                sortOrder: Int64(index)
            ))
        }
    }

    public func close() {
        catalogLogger.info("Closing catalog database")
        try? context.close()
    }
}
