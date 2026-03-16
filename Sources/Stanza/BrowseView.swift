// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel

// MARK: - BrowseView

struct BrowseView: View {
    @State var catalogs: [CatalogRecord] = []
    @State var catalogDB: CatalogDatabase? = nil
    @State var bookDB: BookDatabase? = nil
    @State var showAddCatalog = false
    @State var errorMessage: String? = nil

    var body: some View {
        NavigationStack {
            Group {
                if catalogs.isEmpty && catalogDB != nil {
                    VStack(spacing: 16) {
                        Image("explore", bundle: .module)
                            .font(.system(size: 48))
                            .foregroundStyle(.secondary)
                        Text("No Catalogs")
                            .font(.title2)
                        Text("Add an OPDS catalog to start browsing books.")
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal)
                        Button("Add Catalog") {
                            showAddCatalog = true
                        }
                        .buttonStyle(.borderedProminent)
                    }
                } else if catalogDB == nil {
                    ProgressView("Loading...")
                } else {
                    List {
                        ForEach(catalogs) { catalog in
                            NavigationLink(value: catalog) {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(catalog.name)
                                        .font(.headline)
                                    if let desc = catalog.desc {
                                        Text(desc)
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                            .lineLimit(1)
                                    }
                                    Text(catalog.url)
                                        .font(.caption2)
                                        .foregroundStyle(.secondary).opacity(0.7)
                                        .lineLimit(1)
                                }
                                .padding(.vertical, 2)
                            }
                        }
                        .onDelete { indices in
                            deleteCatalogs(at: Array(indices))
                        }
                    }
                }
            }
            .navigationTitle("Browse")
            .toolbar {
                ToolbarItem {
                    Button {
                        showAddCatalog = true
                    } label: {
                        Label(title: { Text("Add Catalog") }, icon: { Image("add", bundle: .module) })
                    }
                }
            }
            .sheet(isPresented: $showAddCatalog) {
                AddCatalogView(catalogDB: catalogDB, onAdd: {
                    refreshCatalogs()
                })
            }
            .navigationDestination(for: CatalogRecord.self) { catalog in
                CatalogFeedView(
                    feedURL: catalog.url,
                    feedTitle: catalog.name,
                    bookDB: bookDB
                )
            }
            .task {
                initDatabases()
            }
        }
    }

    private func initDatabases() {
        guard catalogDB == nil else { return }
        logger.info("Initializing Browse databases")
        do {
            let dir = URL.applicationSupportDirectory
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

            let catalogPath = dir.appendingPathComponent("catalogs.sqlite").path
            let catDB = try CatalogDatabase(path: catalogPath)
            try catDB.seedDefaults()
            self.catalogDB = catDB

            let bookPath = dir.appendingPathComponent("library.sqlite").path
            self.bookDB = try BookDatabase(path: bookPath)

            self.catalogs = try catDB.allCatalogs()
            logger.info("Browse databases initialized with \(catalogs.count) catalogs")
        } catch {
            logger.error("Failed to open catalog database: \(error)")
            errorMessage = "Failed to open catalogs: \(error.localizedDescription)"
        }
    }

    private func refreshCatalogs() {
        guard let db = catalogDB else { return }
        do {
            self.catalogs = try db.allCatalogs()
        } catch {
            logger.error("Failed to refresh catalogs: \(error)")
        }
    }

    private func deleteCatalogs(at indices: [Int]) {
        guard let db = catalogDB else { return }
        for index in indices {
            let catalog = catalogs[index]
            logger.info("Deleting catalog: '\(catalog.name)' (id=\(catalog.id))")
            do {
                try db.deleteCatalog(id: catalog.id)
            } catch {
                logger.error("Failed to delete catalog: \(error)")
            }
        }
        refreshCatalogs()
    }
}

// MARK: - AddCatalogView

struct AddCatalogView: View {
    let catalogDB: CatalogDatabase?
    let onAdd: () -> Void
    @Environment(\.dismiss) var dismiss
    @State var customURL: String = ""
    @State var customName: String = ""
    @State var errorMessage: String? = nil

    struct RecommendedCatalog: Identifiable {
        let id: String
        let name: String
        let url: String
        let desc: String
    }

    let recommended: [RecommendedCatalog] = [
        //RecommendedCatalog(id: "feedbooks", name: "Feedbooks", url: "https://catalog.feedbooks.com/catalog/index.atom", desc: "Public domain and original ebooks"),
        RecommendedCatalog(id: "gutenberg", name: "Project Gutenberg", url: "https://m.gutenberg.org/ebooks.opds/", desc: "Over 70,000 free ebooks"),
        RecommendedCatalog(id: "archive", name: "Internet Archive", url: "https://bookserver.archive.org/catalog/", desc: "Open library of digital books"),
        RecommendedCatalog(id: "ebooksgratuits", name: "Ebooks Gratuits", url: "https://www.ebooksgratuits.com/opds/", desc: "Free French-language ebooks"),
    ]

    var body: some View {
        NavigationStack {
            Form {
                Section("Recommended Catalogs") {
                    ForEach(recommended) { catalog in
                        Button {
                            addCatalog(name: catalog.name, url: catalog.url, desc: catalog.desc)
                        } label: {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(catalog.name)
                                    .font(.headline)
                                    .foregroundStyle(.primary)
                                Text(catalog.desc)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                Text(catalog.url)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary).opacity(0.7)
                            }
                            .padding(.vertical, 2)
                        }
                    }
                }

                Section("Custom Catalog") {
                    TextField("Catalog Name", text: $customName)
                    TextField("OPDS Feed URL", text: $customURL)
                        #if !SKIP
                        .textInputAutocapitalization(.never)
                        .keyboardType(.URL)
                        #endif
                    Button("Add Custom Catalog") {
                        let name = customName.isEmpty ? customURL : customName
                        addCatalog(name: name, url: customURL, desc: nil)
                    }
                    .disabled(customURL.isEmpty)
                }

                if let error = errorMessage {
                    Section {
                        Text(error)
                            .foregroundStyle(.red)
                    }
                }
            }
            .navigationTitle("Add Catalog")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }

    private func addCatalog(name: String, url: String, desc: String?) {
        guard let db = catalogDB else { return }
        logger.info("Adding catalog: '\(name)' url=\(url)")
        do {
            // Check for duplicate URL
            let existing = try db.allCatalogs()
            if existing.contains(where: { $0.url == url }) {
                logger.warning("Duplicate catalog URL: \(url)")
                errorMessage = "This catalog has already been added."
                return
            }
            let sortOrder = Int64(existing.count)
            try db.addCatalog(CatalogRecord(name: name, url: url, desc: desc, sortOrder: sortOrder))
            logger.info("Catalog added successfully")
            onAdd()
            dismiss()
        } catch {
            logger.error("Failed to add catalog: \(error)")
            errorMessage = "Failed to add catalog: \(error.localizedDescription)"
        }
    }
}

// MARK: - CatalogFeedView

struct CatalogFeedView: View {
    let feedURL: String
    let feedTitle: String
    let bookDB: BookDatabase?
    @State var feedContent: OPDSFeedContent? = nil
    @State var isLoading = true
    @State var errorMessage: String? = nil
    @State var searchText: String = ""
    @State var searchTemplate: String? = nil
    @State var isSearching = false
    @State var searchResults: OPDSFeedContent? = nil
    @State var additionalPublications: [OPDSPubEntry] = []
    @State var isLoadingMore = false
    @State var nextPageURL: String? = nil

    var displayedContent: OPDSFeedContent? {
        if isSearching && searchResults != nil {
            return searchResults
        }
        return feedContent
    }

    var allPublications: [OPDSPubEntry] {
        let base = displayedContent?.publications ?? []
        return base + additionalPublications
    }

    var body: some View {
        Group {
            if isLoading && feedContent == nil {
                VStack(spacing: 12) {
                    ProgressView()
                    Text("Loading catalog...")
                        .foregroundStyle(.secondary)
                }
            } else if let error = errorMessage, feedContent == nil {
                VStack(spacing: 16) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.system(size: 48))
                        .foregroundStyle(.secondary)
                    Text("Failed to Load")
                        .font(.title3)
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal)
                    Button("Retry") {
                        Task { await loadFeed() }
                    }
                    .buttonStyle(.bordered)
                }
            } else {
                feedList
            }
        }
        .navigationTitle(isSearching ? "Search Results" : feedTitle)
        .searchable(text: $searchText, prompt: "Search catalog")
        .onSubmit(of: .search) {
            Task { await performSearch() }
        }
        .onChange(of: searchText) { oldValue, newValue in
            if newValue.isEmpty && isSearching {
                isSearching = false
                searchResults = nil
            }
        }
        .navigationDestination(for: FeedLink.self) { link in
            CatalogFeedView(
                feedURL: link.href,
                feedTitle: link.title,
                bookDB: bookDB
            )
        }
        .navigationDestination(for: PubLink.self) { pubLink in
            OPDSBookDetailView(entry: pubLink.entry, bookDB: bookDB)
        }
        .task {
            await loadFeed()
        }
    }

    @ViewBuilder var feedList: some View {
        List {
            // Navigation links
            if let content = displayedContent, !content.navigation.isEmpty {
                Section("Categories") {
                    ForEach(Array(content.navigation.enumerated()), id: \.offset) { index, nav in
                        if !nav.title.isEmpty {
                            NavigationLink(value: FeedLink(href: nav.href, title: nav.title)) {
                                Label {
                                    Text(nav.title)
                                } icon: {
                                    Image(systemName: "folder")
                                }
                            }
                        }
                    }
                }
            }

            // Groups
            if let content = displayedContent, !content.groups.isEmpty {
                ForEach(Array(content.groups.enumerated()), id: \.offset) { index, group in
                    Section {
                        // Group publications
                        ForEach(Array(group.publications.enumerated()), id: \.offset) { pubIndex, pub in
                            NavigationLink(value: PubLink(entry: pub)) {
                                PublicationRow(entry: pub)
                            }
                        }
                        // Group navigation
                        ForEach(Array(group.navigation.enumerated()), id: \.offset) { navIndex, nav in
                            if !nav.title.isEmpty {
                                NavigationLink(value: FeedLink(href: nav.href, title: nav.title)) {
                                    Label {
                                        Text(nav.title)
                                    } icon: {
                                        Image(systemName: "folder")
                                    }
                                }
                            }
                        }
                        // "More" link for groups
                        if let moreURL = group.moreURL {
                            NavigationLink(value: FeedLink(href: moreURL, title: group.title)) {
                                Text("See All")
                                    .foregroundStyle(Color.accentColor)
                            }
                        }
                    } header: {
                        Text(group.title)
                    }
                }
            }

            // Facets
            if let content = displayedContent, !content.facets.isEmpty {
                ForEach(Array(content.facets.enumerated()), id: \.offset) { index, facet in
                    Section(facet.title) {
                        ForEach(Array(facet.links.enumerated()), id: \.offset) { linkIndex, link in
                            NavigationLink(value: FeedLink(href: link.href, title: link.title)) {
                                Text(link.title)
                            }
                        }
                    }
                }
            }

            // Publications
            if !allPublications.isEmpty {
                Section(displayedContent?.groups.isEmpty == true ? "Books" : "All Books") {
                    ForEach(Array(allPublications.enumerated()), id: \.offset) { index, pub in
                        NavigationLink(value: PubLink(entry: pub)) {
                            PublicationRow(entry: pub)
                        }
                        .onAppear {
                            if index == allPublications.count - 1 {
                                Task { await loadNextPage() }
                            }
                        }
                    }
                    if isLoadingMore {
                        HStack {
                            Spacer()
                            ProgressView()
                            Spacer()
                        }
                    }
                }
            }
        }
    }

    // MARK: - Data Loading

    func loadFeed() async {
        logger.info("Loading feed: \(feedURL)")
        guard let url = URL(string: feedURL) else {
            logger.error("Invalid feed URL: \(feedURL)")
            errorMessage = "Invalid feed URL"
            isLoading = false
            return
        }
        isLoading = true
        errorMessage = nil
        do {
            let content = try await OPDSService.fetchFeed(url: url)
            self.feedContent = content
            self.nextPageURL = content.nextPageURL
            logger.info("Feed loaded: '\(content.title)' — \(content.publications.count) pubs, \(content.navigation.count) nav, \(content.groups.count) groups")

            // Resolve search template if available
            if let searchHref = content.searchURL {
                if searchHref.contains("{searchTerms}") {
                    logger.debug("Search template found inline: \(searchHref)")
                    self.searchTemplate = searchHref
                } else {
                    // Fetch the OpenSearch template
                    logger.debug("Fetching OpenSearch template from: \(searchHref)")
                    do {
                        let template = try await OPDSService.fetchSearchTemplate(searchLinkHref: searchHref)
                        logger.debug("Search template resolved: \(template)")
                        self.searchTemplate = template
                    } catch {
                        logger.info("No search template available: \(error)")
                    }
                }
            }
        } catch {
            errorMessage = error.localizedDescription
            logger.error("Failed to load feed: \(error)")
        }
        isLoading = false
    }

    func performSearch() async {
        guard !searchText.isEmpty else { return }
        guard let template = searchTemplate else {
            logger.warning("Search attempted but no search template available")
            return
        }
        logger.info("Performing search: '\(searchText)'")
        isSearching = true
        additionalPublications = []
        do {
            let results = try await OPDSService.fetchSearchResults(searchURL: template, query: searchText)
            logger.info("Search returned \(results.publications.count) results")
            self.searchResults = results
            self.nextPageURL = results.nextPageURL
        } catch {
            logger.error("Search failed: \(error)")
        }
    }

    func loadNextPage() async {
        guard let nextURL = nextPageURL, !isLoadingMore else { return }
        guard let url = URL(string: nextURL) else { return }
        logger.info("Loading next page: \(nextURL)")
        isLoadingMore = true
        do {
            let content = try await OPDSService.fetchFeed(url: url)
            logger.info("Next page loaded: \(content.publications.count) additional publications")
            self.additionalPublications += content.publications
            self.nextPageURL = content.nextPageURL
        } catch {
            logger.error("Failed to load next page: \(error)")
        }
        isLoadingMore = false
    }
}

// MARK: - Navigation Value Types

struct FeedLink: Hashable {
    let href: String
    let title: String
}

struct PubLink: Hashable {
    let entry: OPDSPubEntry

    static func == (lhs: PubLink, rhs: PubLink) -> Bool {
        lhs.entry.id == rhs.entry.id
    }

    func hash(into hasher: inout Hasher) {
        hasher.combine(entry.id)
    }
}

// MARK: - PublicationRow

struct PublicationRow: View {
    let entry: OPDSPubEntry

    var body: some View {
        HStack(spacing: 12) {
            if let imageURL = entry.thumbnailURL ?? entry.imageURL, let url = URL(string: imageURL) {
                AsyncImage(url: url) { phase in
                    switch phase {
                    case .success(let image):
                        image
                            .resizable()
                            .aspectRatio(contentMode: .fill)
                    case .failure:
                        bookPlaceholder
                    default:
                        ProgressView()
                    }
                }
                .frame(width: 50, height: 70)
                .clipShape(RoundedRectangle(cornerRadius: 4))
            } else {
                bookPlaceholder
                    .frame(width: 50, height: 70)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(entry.title)
                    .font(.headline)
                    .lineLimit(2)
                if !entry.authors.isEmpty {
                    Text(entry.authors.joined(separator: ", "))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                if let summary = entry.summary {
                    Text(summary)
                        .font(.caption)
                        .foregroundStyle(.secondary).opacity(0.7)
                        .lineLimit(2)
                }
            }
        }
        .padding(.vertical, 4)
    }

    var bookPlaceholder: some View {
        RoundedRectangle(cornerRadius: 4)
            .fill(Color.secondary.opacity(0.2))
            .overlay {
                Image(systemName: "book")
                    .foregroundStyle(.secondary)
            }
    }
}

// MARK: - OPDSBookDetailView

struct OPDSBookDetailView: View {
    let entry: OPDSPubEntry
    let bookDB: BookDatabase?
    @State var isDownloading = false
    @State var downloadProgress = 0.0
    @State var downloadComplete = false
    @State var downloadedBookID: Int64? = nil
    @State var errorMessage: String? = nil
    @State var showReader = false

    var body: some View {
        ScrollView {
            VStack(spacing: 20) {
                // Cover image
                if let imageURL = entry.imageURL ?? entry.thumbnailURL, let url = URL(string: imageURL) {
                    AsyncImage(url: url) { phase in
                        switch phase {
                        case .success(let image):
                            image
                                .resizable()
                                .aspectRatio(contentMode: .fit)
                        case .failure:
                            coverPlaceholder
                        default:
                            ProgressView()
                                .frame(height: 200)
                        }
                    }
                    .frame(maxHeight: 300)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                    .shadow(radius: 4)
                } else {
                    coverPlaceholder
                }

                // Title and author
                VStack(spacing: 8) {
                    Text(entry.title)
                        .font(.title2)
                        .fontWeight(.bold)
                        .multilineTextAlignment(.center)

                    if !entry.authors.isEmpty {
                        Text(entry.authors.joined(separator: ", "))
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                }

                // Summary
                if let summary = entry.summary, !summary.isEmpty {
                    Text(summary)
                        .font(.body)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal)
                }

                Divider()
                    .padding(.horizontal)

                // Download / Open actions
                VStack(spacing: 12) {
                    if downloadComplete {
                        Label(title: { Text("Downloaded") }, icon: { Image("checkmark.circle.fill", bundle: .module) })
                            .foregroundStyle(.green)
                            .font(.headline)

                        #if SKIP || canImport(ReadiumNavigator)
                        if downloadedBookID != nil {
                            Button {
                                showReader = true
                            } label: {
                                Label(title: { Text("Open Book") }, icon: { Image("book", bundle: .module) })
                                    .frame(maxWidth: .infinity)
                            }
                            .buttonStyle(.borderedProminent)
                            .padding(.horizontal)
                        }
                        #endif
                    } else if isDownloading {
                        VStack(spacing: 8) {
                            ProgressView(value: downloadProgress)
                                .padding(.horizontal)
                            Text("Downloading... \(Int(downloadProgress * 100))%")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Button("Cancel") {
                                isDownloading = false
                            }
                            .foregroundStyle(.red)
                        }
                    } else if entry.acquisitionURL != nil {
                        Button {
                            Task { await downloadBook() }
                        } label: {
                            Label(title: { Text("Download Book") }, icon: { Image("arrow.down.circle", bundle: .module) })
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.borderedProminent)
                        .padding(.horizontal)
                    } else {
                        Text("No download available")
                            .foregroundStyle(.secondary)
                    }
                }

                if let error = errorMessage {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .padding(.horizontal)
                }

                // Metadata
                VStack(alignment: .leading, spacing: 8) {
                    if let acqType = entry.acquisitionType {
                        HStack {
                            Text("Format")
                                .foregroundStyle(.secondary)
                            Spacer()
                            Text(formatDisplayName(acqType))
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                .padding(.horizontal)

                Spacer(minLength: 40)
            }
            .padding(.top)
        }
        .navigationTitle(entry.title)
        #if !SKIP
        .navigationBarTitleDisplayMode(.inline)
        #endif
        #if SKIP || canImport(ReadiumNavigator)
        .fullScreenCover(isPresented: $showReader) {
            if let bookID = downloadedBookID, let bookDB = bookDB {
                if let book = try? bookDB.book(id: bookID) {
                    LibraryReaderView(bookID: bookID, filePath: book.filePath, database: bookDB)
                }
            }
        }
        #endif
    }

    var coverPlaceholder: some View {
        RoundedRectangle(cornerRadius: 8)
            .fill(Color.secondary.opacity(0.15))
            .frame(width: 180, height: 260)
            .overlay {
                VStack(spacing: 8) {
                    Image(systemName: "book")
                        .font(.system(size: 40))
                        .foregroundStyle(.secondary)
                    Text(entry.title)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 8)
                        .lineLimit(3)
                }
            }
    }

    func downloadBook() async {
        guard let urlString = entry.acquisitionURL, let url = URL(string: urlString) else {
            logger.error("No download URL for book: '\(entry.title)'")
            errorMessage = "No download URL available"
            return
        }
        guard let db = bookDB else {
            logger.error("Library database not available for download")
            errorMessage = "Library not available"
            return
        }

        logger.info("Starting download: '\(entry.title)' from \(urlString)")
        isDownloading = true
        downloadProgress = 0.0
        errorMessage = nil

        do {
            let booksDir = URL.documentsDirectory.appendingPathComponent("Books")
            logger.info("booksDir: \(booksDir)")

            try FileManager.default.createDirectory(at: booksDir, withIntermediateDirectories: true)

            // Generate a filename ensuring .epub extension so Readium can detect the format
            var filename = entry.title
                .replacingOccurrences(of: "/", with: "-")
                .replacingOccurrences(of: ":", with: "-")
                .replacingOccurrences(of: "?", with: "-")
                .replacingOccurrences(of: " ", with: "_")
            if filename.isEmpty {
                filename = url.lastPathComponent
            }
            if !filename.hasSuffix(".epub") {
                filename += ".epub"
            }
            logger.info("destinationURL for filename: \(filename)")
            let destinationURL = booksDir.appendingPathComponent(filename)
            logger.info("destinationURL: \(destinationURL)")

            try await OPDSService.downloadBook(from: url, to: destinationURL) { progress in
                self.downloadProgress = progress
            }

            guard isDownloading else {
                logger.info("Download cancelled for: '\(entry.title)'")
                return
            }

            // Import into library
            logger.info("Download complete, importing: '\(entry.title)'")
            let pub = try await Pub.loadPublication(from: destinationURL)
            let metadata = pub.metadata
            let bookTitle = metadata.title ?? entry.title
            let bookAuthor = entry.authors.joined(separator: ", ")
            let totalItems = Int64(pub.manifest.readingOrder.count)

            let record = BookRecord(
                title: bookTitle,
                author: bookAuthor,
                filePath: BookDatabase.relativePath(for: destinationURL.path),
                identifier: metadata.identifier,
                totalItems: totalItems
            )
            let savedRecord = try db.addBook(record)
            logger.info("Book imported to library: '\(bookTitle)' (id=\(savedRecord.id))")
            self.downloadedBookID = savedRecord.id
            self.downloadComplete = true
            self.isDownloading = false
        } catch {
            self.isDownloading = false
            errorMessage = "Download failed: \(error.localizedDescription)"
            logger.error("Download failed: \(error)")
            #if SKIP
            android.util.Log.e("Stanza", "Error downloading book", error as? Throwable)
            #endif
        }
    }

    func formatDisplayName(_ mimeType: String) -> String {
        if mimeType.contains("epub") { return "EPUB" }
        if mimeType.contains("pdf") { return "PDF" }
        if mimeType.contains("mobi") { return "MOBI" }
        return mimeType
    }
}
