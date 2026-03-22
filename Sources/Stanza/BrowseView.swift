// Copyright 2026 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import SkipKit
import StanzaModel

// MARK: - BrowseView

struct BrowseView: View {
    @Environment(LibraryManager.self) var library: LibraryManager
    @Environment(ErrorManager.self) var errorManager: ErrorManager
    @State var catalogs: [CatalogRecord] = []
    @State var catalogDB: CatalogDatabase? = nil
    @State var showAddCatalog = false

    var body: some View {
        NavigationStack {
            Group {
                if catalogs.isEmpty && catalogDB != nil {
                    VStack(spacing: 16) {
                        Image("explore", bundle: .module)
                            .font(.system(size: 48))
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("noCatalogsIcon")
                            .accessibilityLabel("No catalogs")
                        Text("No Catalogs")
                            .font(.title2)
                            .accessibilityIdentifier("noCatalogsTitle")
                        Text("Add an OPDS catalog to start browsing books.")
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal)
                            .accessibilityIdentifier("noCatalogsMessage")
                        Button("Add Catalog") {
                            showAddCatalog = true
                        }
                        .buttonStyle(.borderedProminent)
                        .accessibilityIdentifier("addCatalogEmptyButton")
                    }
                } else if catalogDB == nil {
                    ProgressView("Loading...")
                        .accessibilityIdentifier("browseLoadingIndicator")
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
                            .accessibilityIdentifier("catalogRow_\(catalog.id)")
                        }
                        .onDelete { indices in
                            deleteCatalogs(at: Array(indices))
                        }
                    }
                    .accessibilityIdentifier("catalogList")
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
                    .accessibilityIdentifier("addCatalogButton")
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
                    feedTitle: catalog.name
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
        library.initialize()
        do {
            let dir = URL.applicationSupportDirectory
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

            let catalogPath = dir.appendingPathComponent("catalogs.sqlite").path
            let catDB = try CatalogDatabase(path: catalogPath)
            try catDB.seedDefaults()
            self.catalogDB = catDB

            self.catalogs = try catDB.allCatalogs()
            logger.info("Browse databases initialized with \(catalogs.count) catalogs")
        } catch {
            logger.error("Failed to open catalog database: \(error)")
            errorManager.errorOccurred(info: AppErrorInfo(title: "Catalog Error", message: "Failed to open catalogs.", error: error))
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
    @Environment(ErrorManager.self) var errorManager: ErrorManager
    @State var customURL: String = ""
    @State var customName: String = ""
    @State var errorMessage: String? = nil

    let recommended = DefaultCatalog.all

    var body: some View {
        NavigationStack {
            Form {
                Section("Recommended Catalogs") {
                    ForEach(recommended) { catalog in
                        Button {
                            addCatalog(name: catalog.name, url: catalog.url, desc: catalog.description)
                        } label: {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(catalog.name)
                                    .font(.headline)
                                    .foregroundStyle(.primary)
                                Text(catalog.description)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                Text(catalog.url)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary).opacity(0.7)
                            }
                            .padding(.vertical, 2)
                        }
                        .accessibilityIdentifier("recommendedCatalog_\(catalog.id)")
                    }
                }

                Section("Custom Catalog") {
                    TextField("Catalog Name", text: $customName)
                        .accessibilityIdentifier("customCatalogNameField")
                    TextField("OPDS Feed URL", text: $customURL)
                        .accessibilityIdentifier("customCatalogURLField")
                        #if !SKIP
                        .textInputAutocapitalization(.never)
                        .keyboardType(.URL)
                        #endif
                    Button("Add Custom Catalog") {
                        let name = customName.isEmpty ? customURL : customName
                        addCatalog(name: name, url: customURL, desc: nil)
                    }
                    .disabled(customURL.isEmpty)
                    .accessibilityIdentifier("addCustomCatalogButton")
                }

                if let error = errorMessage {
                    Section {
                        Text(error)
                            .foregroundStyle(.red)
                            .accessibilityIdentifier("addCatalogError")
                    }
                }
            }
            .navigationTitle("Add Catalog")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .accessibilityIdentifier("addCatalogCancelButton")
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
            errorManager.errorOccurred(info: AppErrorInfo(title: "Catalog Error", message: "Failed to add catalog.", error: error))
        }
    }
}

// MARK: - CatalogFeedView

struct CatalogFeedView: View {
    let feedURL: String
    let feedTitle: String
    @Environment(LibraryManager.self) var library: LibraryManager
    @Environment(ErrorManager.self) var errorManager: ErrorManager
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
                        .accessibilityIdentifier("feedLoadingSpinner")
                    Text("Loading catalog...")
                        .foregroundStyle(.secondary)
                }
            } else if let error = errorMessage, feedContent == nil {
                VStack(spacing: 16) {
                    Image("warning", bundle: .module)
                        .font(.system(size: 48))
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("feedErrorIcon")
                        .accessibilityLabel("Error")
                    Text("Failed to Load")
                        .font(.title3)
                        .accessibilityIdentifier("feedErrorTitle")
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal)
                        .accessibilityIdentifier("feedErrorMessage")
                    Button("Retry") {
                        Task { await loadFeed() }
                    }
                    .buttonStyle(.bordered)
                    .accessibilityIdentifier("feedRetryButton")
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
                feedTitle: link.title
            )
        }
        .navigationDestination(for: PubLink.self) { pubLink in
            OPDSBookDetailView(entry: pubLink.entry)
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
                    ForEach(content.navigation) { nav in
                        if !nav.title.isEmpty {
                            NavigationLink(value: FeedLink(href: nav.href, title: nav.title)) {
                                Label {
                                    Text(nav.title)
                                } icon: {
                                    Image("folder", bundle: .module)
                                }
                            }
                        }
                    }
                }
            }

            // Groups
            if let content = displayedContent, !content.groups.isEmpty {
                ForEach(content.groups) { group in
                    Section {
                        // Group publications
                        ForEach(group.publications) { pub in
                            NavigationLink(value: PubLink(entry: pub)) {
                                PublicationRow(entry: pub)
                            }
                        }
                        // Group navigation
                        ForEach(group.navigation) { nav in
                            if !nav.title.isEmpty {
                                NavigationLink(value: FeedLink(href: nav.href, title: nav.title)) {
                                    Label {
                                        Text(nav.title)
                                    } icon: {
                                        Image("folder", bundle: .module)
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
                ForEach(content.facets) { facet in
                    Section(facet.title) {
                        ForEach(facet.links) { link in
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
                    ForEach(allPublications) { pub in
                        NavigationLink(value: PubLink(entry: pub)) {
                            PublicationRow(entry: pub)
                        }
                        .onAppear {
                            if pub.id == allPublications.last?.id {
                                Task { await loadNextPage() }
                            }
                        }
                    }
                    if isLoadingMore {
                        HStack {
                            Spacer()
                            ProgressView()
                                .accessibilityIdentifier("loadMoreSpinner")
                            Spacer()
                        }
                    }
                }
            }

            // Catalog info section
            if let content = displayedContent {
                let hasInfo = content.subtitle != nil || content.iconURL != nil || !content.infoLinks.isEmpty || content.totalResults != nil
                if hasInfo {
                    Section("About This Catalog") {
                        if let iconURLString = content.iconURL, let iconURL = URL(string: iconURLString) {
                            HStack {
                                Spacer()
                                AsyncImage(url: iconURL) { phase in
                                    switch phase {
                                    case .success(let image):
                                        image
                                            .resizable()
                                            .aspectRatio(contentMode: .fit)
                                    default:
                                        EmptyView()
                                    }
                                }
                                .frame(maxWidth: 120, maxHeight: 120)
                                .clipShape(RoundedRectangle(cornerRadius: 8))
                                .accessibilityIdentifier("catalogIcon")
                                .accessibilityLabel("\(content.title) icon")
                                Spacer()
                            }
                        }
                        if let subtitle = content.subtitle, !subtitle.isEmpty {
                            Text(subtitle)
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("catalogSubtitle")
                        }
                        if let total = content.totalResults {
                            HStack {
                                Text("Total Books")
                                    .foregroundStyle(.secondary)
                                Spacer()
                                Text("\(total)")
                                    .foregroundStyle(.secondary)
                            }
                            .accessibilityIdentifier("catalogTotalResults")
                        }
                        ForEach(content.infoLinks) { link in
                            if let url = URL(string: link.href) {
                                Link(destination: url) {
                                    HStack {
                                        Text(link.title)
                                        Spacer()
                                        Image("open_in_new", bundle: .module)
                                            .foregroundStyle(.secondary)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        .accessibilityIdentifier("feedList")
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
            errorManager.errorOccurred(info: AppErrorInfo(title: "Feed Error", message: "Failed to load catalog feed.", error: error))
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
                .accessibilityIdentifier("pubRowCover")
                .accessibilityLabel("\(entry.title) cover")
            } else {
                bookPlaceholder
                    .frame(width: 50, height: 70)
                    .accessibilityIdentifier("pubRowPlaceholder")
                    .accessibilityLabel("\(entry.title) placeholder")
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(entry.title)
                    .font(.headline)
                    .lineLimit(2)
                    .accessibilityIdentifier("pubRowTitle")
                if !entry.authors.isEmpty {
                    Text(entry.authors.joined(separator: ", "))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .accessibilityIdentifier("pubRowAuthor")
                }
                if let summary = entry.summary {
                    textFromOPDSSummary(summary)
                        .font(.caption)
                        .foregroundStyle(.secondary).opacity(0.7)
                        .lineLimit(2)
                        .accessibilityIdentifier("pubRowSummary")
                }
            }
        }
        .padding(.vertical, 4)
    }

    var bookPlaceholder: some View {
        RoundedRectangle(cornerRadius: 4)
            .fill(Color.secondary.opacity(0.2))
            .overlay {
                Image("menu_book", bundle: .module)
                    .foregroundStyle(.secondary)
                    .accessibilityLabel("Book placeholder")
            }
    }
}

// MARK: - OPDSBookDetailView

struct OPDSBookDetailView: View {
    let entry: OPDSPubEntry
    @Environment(LibraryManager.self) var library: LibraryManager
    @Environment(ErrorManager.self) var errorManager: ErrorManager
    @State var downloader: FileDownloader? = nil
    @State var downloadedBookID: Int64? = nil
    @State var importError: String? = nil
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
                                .accessibilityIdentifier("bookDetailCoverLoading")
                        }
                    }
                    .frame(maxHeight: 300)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                    .shadow(radius: 4)
                    .accessibilityIdentifier("bookDetailCover")
                    .accessibilityLabel("\(entry.title) cover image")
                } else {
                    coverPlaceholder
                        .accessibilityIdentifier("bookDetailCoverPlaceholder")
                }

                // Title and author
                VStack(spacing: 8) {
                    Text(entry.title)
                        .font(.title2)
                        .fontWeight(.bold)
                        .multilineTextAlignment(.center)
                        .accessibilityIdentifier("bookDetailTitle")

                    if !entry.authors.isEmpty {
                        Text(entry.authors.joined(separator: ", "))
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("bookDetailAuthor")
                    }
                }

                // Summary
                if let summary = entry.summary, !summary.isEmpty {
                    textFromOPDSSummary(summary)
                        .font(.body)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal)
                        .accessibilityIdentifier("bookDetailSummary")
                }

                if let error = importError {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .padding(.horizontal)
                        .accessibilityIdentifier("bookDetailImportError")
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
                        .accessibilityIdentifier("bookDetailFormat")
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
        .toolbar {
            ToolbarItem(placement: .automatic) {
                toolbarDownloadButton
            }
        }
        .fullScreenCover(isPresented: $showReader) {
            if let bookID = downloadedBookID, let db = library.database {
                if let book = try? db.book(id: bookID) {
                    ReaderView(bookID: bookID, filePath: book.filePath, database: db)
                }
            }
        }
        .onAppear {
            // Check if the book is already in the library
            if downloadedBookID == nil {
                if let existing = library.book(withIdentifier: entry.id) {
                    downloadedBookID = existing.id
                }
            }
        }
    }

    var coverPlaceholder: some View {
        RoundedRectangle(cornerRadius: 8)
            .fill(Color.secondary.opacity(0.15))
            .frame(width: 180, height: 260)
            .overlay {
                VStack(spacing: 8) {
                    Image("menu_book", bundle: .module)
                        .font(.system(size: 40))
                        .foregroundStyle(.secondary)
                        .accessibilityLabel("Book placeholder")
                    Text(entry.title)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 8)
                        .lineLimit(3)
                }
            }
    }

    @ViewBuilder var toolbarDownloadButton: some View {
        if downloadedBookID != nil {
            // Already in library — show Read button
            Button {
                showReader = true
            } label: {
                Text("Read")
                    .fontWeight(.bold)
            }
            .buttonStyle(.borderedProminent)
            .accessibilityIdentifier("bookDetailReadButton")
        } else if let dl = downloader {
            // Download in progress — show circular progress; tap cancels
            CircularDownloadProgressView(downloader: dl, onCompleted: {
                importDownloadedBook()
            })
        } else if entry.acquisitionURL != nil {
            // Not yet downloaded — show Get button
            Button {
                startDownload()
            } label: {
                Text("Get")
                    .fontWeight(.bold)
            }
            .buttonStyle(.borderedProminent)
            //.buttonBorderShape(.capsule)
            .accessibilityIdentifier("bookDetailGetButton")
        }
    }

    func startDownload() {
        guard let urlString = entry.acquisitionURL, let url = URL(string: urlString) else {
            logger.error("No download URL for book: '\(entry.title)'")
            importError = "No download URL available"
            errorManager.errorOccurred(info: AppErrorInfo(title: "Download Failed", message: "No download URL available for this book."))
            return
        }

        let booksDir = URL.documentsDirectory.appendingPathComponent("Books")
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
        let destinationURL = booksDir.appendingPathComponent(filename)

        logger.info("Starting download: '\(entry.title)' from \(urlString) to \(destinationURL.path)")
        let dl = FileDownloader(sourceURL: url, destinationURL: destinationURL, displayName: entry.title)
        self.downloader = dl
        dl.start()
    }

    func importDownloadedBook() {
        guard let dl = downloader else { return }

        let destinationURL = dl.destinationURL
        logger.info("Download complete, importing: '\(entry.title)' from \(destinationURL.path)")

        Task {
            if let record = await library.importDownloadedBook(
                from: destinationURL,
                title: entry.title,
                authors: entry.authors,
                identifier: entry.id
            ) {
                self.downloadedBookID = record.id
            } else {
                importError = "Import failed"
            }
        }
    }

    func formatDisplayName(_ mimeType: String) -> String {
        if mimeType.contains("epub") { return "EPUB" }
        if mimeType.contains("pdf") { return "PDF" }
        if mimeType.contains("mobi") { return "MOBI" }
        return mimeType
    }
}

// if this turns out to be a bottleneck, we could add some caching
private let textFromOPDSSummaryCache = SkipKit.Cache<String, AttributedString>()

public extension View {

    /// Try to parse the light HTML permitted in OPDS summaries and return a Text with the contents.
    func textFromOPDSSummary(_ summary: String) -> Text {
        Text(HTMLMarkdown(summary, options: [.noLinks]).attributedStringFromHTMLString())
    }
}
