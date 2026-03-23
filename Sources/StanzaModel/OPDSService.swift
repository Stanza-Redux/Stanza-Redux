// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import Foundation
import OSLog
import SkipXML
#if !SKIP
import ReadiumOPDS
import ReadiumShared
#else
import org.readium.r2.opds.OPDS1Parser
import org.readium.r2.opds.OPDS2Parser
import org.readium.r2.shared.opds.Feed
import org.readium.r2.shared.opds.Group
import org.readium.r2.shared.opds.Facet
import org.readium.r2.shared.publication.Link
import org.readium.r2.shared.publication.Publication
import org.readium.r2.shared.publication.Contributor
import org.readium.r2.shared.publication.Metadata
import org.readium.r2.shared.util.AbsoluteUrl
import org.readium.r2.shared.util.Try
import org.readium.r2.shared.util.Url
import org.readium.r2.shared.util.getOrElse
import org.readium.r2.shared.util.toUrl
import org.readium.r2.shared.util.toAbsoluteUrl
import org.readium.r2.shared.util.http.HttpRequest
import org.readium.r2.shared.util.http.fetchWithDecoder
#endif

let opdsLogger = Logger(subsystem: "Stanza", category: "OPDSService")

/// Represents a navigation link in an OPDS feed (e.g., a subcategory).
public final class OPDSNavLink: Identifiable {
    public let id: String
    public let title: String
    public let href: String
    public let rel: String?

    public init(title: String, href: String, rel: String? = nil) {
        self.id = href + ":" + title
        self.title = title
        self.href = href
        self.rel = rel
    }
}

/// Represents a publication entry in an OPDS feed.
public final class OPDSPubEntry: Identifiable {
    public let id: String
    public let title: String
    public let authors: [String]
    public let summary: String?
    public let imageURL: String?
    public let thumbnailURL: String?
    public let acquisitionURL: String?
    public let acquisitionType: String?

    public init(id: String, title: String, authors: [String] = [], summary: String? = nil, imageURL: String? = nil, thumbnailURL: String? = nil, acquisitionURL: String? = nil, acquisitionType: String? = nil) {
        self.id = id
        self.title = title
        self.authors = authors
        self.summary = summary
        self.imageURL = imageURL
        self.thumbnailURL = thumbnailURL
        self.acquisitionURL = acquisitionURL
        self.acquisitionType = acquisitionType
    }
}

/// Represents a group of entries in an OPDS feed.
public final class OPDSGroupEntry: Identifiable {
    public let id: String
    public let title: String
    public let publications: [OPDSPubEntry]
    public let navigation: [OPDSNavLink]
    public let moreURL: String?

    public init(title: String, publications: [OPDSPubEntry] = [], navigation: [OPDSNavLink] = [], moreURL: String? = nil) {
        self.id = "group:" + title
        self.title = title
        self.publications = publications
        self.navigation = navigation
        self.moreURL = moreURL
    }
}

/// Represents a facet group in an OPDS feed.
public final class OPDSFacetEntry: Identifiable {
    public let id: String
    public let title: String
    public let links: [OPDSNavLink]

    public init(title: String, links: [OPDSNavLink] = []) {
        self.id = "facet:" + title
        self.title = title
        self.links = links
    }
}

/// The parsed content of an OPDS feed.
public final class OPDSFeedContent {
    public let title: String
    public let subtitle: String?
    public let iconURL: String?
    public let infoLinks: [OPDSNavLink]
    public let navigation: [OPDSNavLink]
    public let publications: [OPDSPubEntry]
    public let groups: [OPDSGroupEntry]
    public let facets: [OPDSFacetEntry]
    public let searchURL: String?
    public let nextPageURL: String?
    public let totalResults: Int?

    public init(title: String, subtitle: String? = nil, iconURL: String? = nil, infoLinks: [OPDSNavLink] = [], navigation: [OPDSNavLink] = [], publications: [OPDSPubEntry] = [], groups: [OPDSGroupEntry] = [], facets: [OPDSFacetEntry] = [], searchURL: String? = nil, nextPageURL: String? = nil, totalResults: Int? = nil) {
        self.title = title
        self.subtitle = subtitle
        self.iconURL = iconURL
        self.infoLinks = infoLinks
        self.navigation = navigation
        self.publications = publications
        self.groups = groups
        self.facets = facets
        self.searchURL = searchURL
        self.nextPageURL = nextPageURL
        self.totalResults = totalResults
    }
}

/// Cross-platform OPDS feed fetching and parsing service.
public final class OPDSService {

    /// Fetches and parses an OPDS feed from the given URL.
    public static func fetchFeed(url: URL) async throws -> OPDSFeedContent {
        opdsLogger.info("Fetching OPDS feed: \(url.absoluteString)")

        #if !SKIP
        // Fetch the data ourselves so we can set our User-Agent header,
        // then parse with the Readium OPDS parsers directly.
        var request = URLRequest(url: url)
        request.setValue(stanzaUserAgent, forHTTPHeaderField: "User-Agent")
        let (data, response) = try await URLSession.shared.data(for: request)

        var parseData: ParseData? = nil
        parseData = try? OPDS1Parser.parse(xmlData: data, url: url, response: response)
        if parseData == nil {
            parseData = try? OPDS2Parser.parse(jsonData: data, url: url, response: response)
        }
        guard let feed = parseData?.feed else {
            opdsLogger.error("No feed content from \(url.absoluteString)")
            throw OPDSServiceError.noFeedContent
        }
        let content = convertFeed(feed, baseURL: url)
        opdsLogger.info("Parsed feed '\(content.title)': \(content.publications.count) publications, \(content.navigation.count) nav links, \(content.groups.count) groups")
        return content
        #else
        // On Android, fetch data via java.net.URL and parse with Readium OPDS parsers
        let javaUrl = java.net.URL(url.absoluteString)
        let connection = javaUrl.openConnection() as! java.net.HttpURLConnection
        connection.requestMethod = "GET"
        connection.setRequestProperty("User-Agent", stanzaUserAgent)
        let inputStream = connection.inputStream
        let data = inputStream.readBytes()
        inputStream.close()
        connection.disconnect()

        let absoluteUrl = url.kotlin().toURL().toAbsoluteUrl()!

        // Try OPDS 1 (XML) first, then OPDS 2 (JSON)
        var feed: Feed? = nil
        do {
            opdsLogger.debug("Trying OPDS 1 (XML) parser")
            let parseData = OPDS1Parser.parse(data, absoluteUrl)
            feed = parseData.feed
            opdsLogger.debug("OPDS 1 parse succeeded")
        } catch {
            opdsLogger.debug("OPDS 1 parse failed, trying OPDS 2: \(error)")
        }

        if feed == nil {
            do {
                opdsLogger.debug("Trying OPDS 2 (JSON) parser")
                let parseData = OPDS2Parser.parse(data, absoluteUrl)
                feed = parseData.feed
                opdsLogger.debug("OPDS 2 parse succeeded")
            } catch {
                opdsLogger.error("Both OPDS 1 and OPDS 2 parsing failed for \(url.absoluteString)")
                throw OPDSServiceError.parseFailed
            }
        }

        guard let parsedFeed = feed else {
            opdsLogger.error("No feed content from \(url.absoluteString)")
            throw OPDSServiceError.noFeedContent
        }
        let content = convertFeedKotlin(parsedFeed, baseURL: url)
        opdsLogger.info("Parsed feed '\(content.title)': \(content.publications.count) publications, \(content.navigation.count) nav links, \(content.groups.count) groups")
        return content
        #endif
    }

    /// Resolves a search URL template with a query, then fetches and parses the results.
    public static func fetchSearchResults(searchURL: String, query: String) async throws -> OPDSFeedContent {
        opdsLogger.info("Searching OPDS catalog for: '\(query)' using template: \(searchURL)")
        let encoded = query.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? query
        let resolved = searchURL.replacingOccurrences(of: "{searchTerms}", with: encoded)
        opdsLogger.debug("Resolved search URL: \(resolved)")
        guard let url = URL(string: resolved) else {
            opdsLogger.error("Invalid search URL: \(resolved)")
            throw OPDSServiceError.invalidURL
        }
        return try await fetchFeed(url: url)
    }

    /// Fetches the OpenSearch template for a feed that has a search link.
    /// Fetches and parses an OpenSearch description document to extract the search URL template.
    ///
    /// The OpenSearch XML format contains `<Url>` elements with a `template` attribute.
    /// We look for the first `<Url>` whose `type` attribute indicates an OPDS/Atom feed,
    /// falling back to any `<Url>` element with a `template` attribute.
    public static func fetchSearchTemplate(searchLinkHref: String) async throws -> String {
        opdsLogger.info("Fetching OpenSearch template from: \(searchLinkHref)")
        guard let url = URL(string: searchLinkHref) else {
            opdsLogger.error("Invalid search link URL: \(searchLinkHref)")
            throw OPDSServiceError.invalidURL
        }

        let data: Data
        #if !SKIP
        var request = URLRequest(url: url)
        request.setValue(stanzaUserAgent, forHTTPHeaderField: "User-Agent")
        let (responseData, _) = try await URLSession.shared.data(for: request)
        data = responseData
        #else
        let javaUrl = java.net.URL(url.absoluteString)
        let connection = javaUrl.openConnection() as! java.net.HttpURLConnection
        connection.setRequestProperty("User-Agent", stanzaUserAgent)
        let inputStream = connection.inputStream
        let bytes = inputStream.readBytes()
        inputStream.close()
        connection.disconnect()
        data = Data(platformValue: bytes)
        #endif

        return try parseSearchTemplate(from: data)
    }

    /// Parses an OpenSearch description document and extracts the URL template.
    private static func parseSearchTemplate(from data: Data) throws -> String {
        let doc: SkipXML.XMLNode
        do {
            doc = try XMLNode.parse(data: data)
        } catch {
            opdsLogger.error("Failed to parse OpenSearch XML: \(error)")
            throw OPDSServiceError.parseFailed
        }

        // The document root is typically <OpenSearchDescription>.
        // Look for <Url> elements which have a "template" attribute.
        let root = doc.elementChildren.first ?? doc
        let urlElements = root.descendants(named: "Url")

        // Prefer a <Url> element whose type matches OPDS/Atom
        let opdsTypes = ["application/atom+xml", "application/opds+json", "application/xml", "text/xml"]
        for urlElement in urlElements {
            if let template = urlElement.attributes["template"],
               let type = urlElement.attributes["type"],
               opdsTypes.contains(where: { type.contains($0) }) {
                opdsLogger.debug("Found OPDS search template: \(template)")
                return template
            }
        }

        // Fall back to any <Url> with a template attribute
        for urlElement in urlElements {
            if let template = urlElement.attributes["template"] {
                opdsLogger.debug("Found search template (fallback): \(template)")
                return template
            }
        }

        // Last resort: check for a template attribute on any descendant
        func findTemplate(in node: SkipXML.XMLNode) -> String? {
            if let template = node.attributes["template"] {
                return template
            }
            for child in node.elementChildren {
                if let found = findTemplate(in: child) {
                    return found
                }
            }
            return nil
        }

        if let template = findTemplate(in: root) {
            opdsLogger.debug("Found search template (deep search): \(template)")
            return template
        }

        opdsLogger.error("No search template found in OpenSearch document")
        throw OPDSServiceError.parseFailed
    }

    // MARK: - iOS Feed Conversion

    #if !SKIP
    private static func convertFeed(_ feed: Feed, baseURL: URL) -> OPDSFeedContent {
        let navigation = feed.navigation.map { link in
            OPDSNavLink(
                title: link.title ?? "",
                href: resolveHref(link.href, base: baseURL),
                rel: link.rels.first?.string
            )
        }

        let publications = feed.publications.map { pub in
            convertPublication(pub, baseURL: baseURL)
        }

        let groups = feed.groups.map { group in
            let groupPubs = group.publications.map { pub in
                convertPublication(pub, baseURL: baseURL)
            }
            let groupNav = group.navigation.map { link in
                OPDSNavLink(
                    title: link.title ?? "",
                    href: resolveHref(link.href, base: baseURL),
                    rel: link.rels.first?.string
                )
            }
            let selfLink = group.links.firstWithRel(.self)
            return OPDSGroupEntry(
                title: group.metadata.title,
                publications: groupPubs,
                navigation: groupNav,
                moreURL: selfLink.map { resolveHref($0.href, base: baseURL) }
            )
        }

        let facets = feed.facets.map { facet in
            let links = facet.links.map { link in
                OPDSNavLink(
                    title: link.title ?? "",
                    href: resolveHref(link.href, base: baseURL),
                    rel: link.rels.first?.string
                )
            }
            return OPDSFacetEntry(title: facet.metadata.title, links: links)
        }

        let searchURL = feed.links.firstWithRel(.search)?.href
        let nextPageURL = feed.links.firstWithRel(.next).map { resolveHref($0.href, base: baseURL) }

        // Extract feed icon from links
        let iconLink = feed.links.first(where: { $0.rels.contains(where: { $0.string.contains("opds-spec.org/image") }) })
            ?? feed.links.first(where: { $0.mediaType?.string.hasPrefix("image/") == true })
        let iconURL = iconLink.map { resolveHref($0.href, base: baseURL) }

        // Extract informational links (alternate, about, related)
        let infoRels: Set<String> = ["alternate", "about", "related", "http://opds-spec.org/shelf", "help", "terms-of-service", "privacy-policy"]
        let infoLinks = feed.links.compactMap { link -> OPDSNavLink? in
            guard let rel = link.rels.first?.string, infoRels.contains(rel) else { return nil }
            let title = link.title ?? rel.replacingOccurrences(of: "-", with: " ").capitalized
            return OPDSNavLink(title: title, href: resolveHref(link.href, base: baseURL), rel: rel)
        }

        // OpdsMetadata in Readium Kotlin does not have a description property
        let subtitle: String? = nil // feed.metadata.description

        return OPDSFeedContent(
            title: feed.metadata.title,
            subtitle: subtitle,
            iconURL: iconURL,
            infoLinks: infoLinks,
            navigation: navigation,
            publications: publications,
            groups: groups,
            facets: facets,
            searchURL: searchURL,
            nextPageURL: nextPageURL,
            totalResults: feed.metadata.numberOfItem
        )
    }

    private static func convertPublication(_ pub: Publication, baseURL: URL) -> OPDSPubEntry {
        let title = pub.metadata.localizedTitle?.string ?? "Untitled"
        let authors = pub.metadata.authors.map { $0.name }
        let summary = pub.metadata.description
        let identifier = pub.metadata.identifier ?? title

        let images = pub.manifest.subcollections["images"]?.first?.links ?? []
        let thumbnailLink = images.first(where: { $0.rels.contains(.opdsImageThumbnail) }) ?? images.first
        let imageLink = images.first(where: { $0.rels.contains(.opdsImage) }) ?? thumbnailLink

        // Prefer EPUB acquisition link, fall back to any acquisition link
        let allAcquisitionLinks = pub.manifest.links.filter { link in
            link.rels.contains(where: { $0.string.contains("http://opds-spec.org/acquisition") })
        }
        let acquisitionLink = allAcquisitionLinks.first(where: { $0.mediaType?.string.contains("epub") == true })
            ?? allAcquisitionLinks.first

        return OPDSPubEntry(
            id: identifier,
            title: title,
            authors: authors,
            summary: summary,
            imageURL: imageLink.map { resolveHref($0.href, base: baseURL) },
            thumbnailURL: thumbnailLink.map { resolveHref($0.href, base: baseURL) },
            acquisitionURL: acquisitionLink.map { resolveHref($0.href, base: baseURL) },
            acquisitionType: acquisitionLink?.mediaType?.string
        )
    }

    private static func resolveHref(_ href: String, base: URL) -> String {
        if href.hasPrefix("http://") || href.hasPrefix("https://") {
            return href
        }
        return URL(string: href, relativeTo: base)?.absoluteString ?? href
    }
    #endif

    // MARK: - Android Feed Conversion

    #if SKIP
    private static func convertFeedKotlin(_ feed: Feed, baseURL: URL) -> OPDSFeedContent {
        var navLinks: [OPDSNavLink] = []
        for link in feed.navigation {
            let relStr = firstRel(link)
            navLinks.append(OPDSNavLink(
                title: link.title ?? "",
                href: resolveHrefKotlin(link.href.toString(), base: baseURL),
                rel: relStr
            ))
        }

        var pubEntries: [OPDSPubEntry] = []
        for pub in feed.publications {
            pubEntries.append(convertPublicationKotlin(pub, baseURL: baseURL))
        }

        var groupEntries: [OPDSGroupEntry] = []
        for group in feed.groups {
            var groupPubs: [OPDSPubEntry] = []
            for pub in group.publications {
                groupPubs.append(convertPublicationKotlin(pub, baseURL: baseURL))
            }
            var groupNav: [OPDSNavLink] = []
            for link in group.navigation {
                let relStr = firstRel(link)
                groupNav.append(OPDSNavLink(
                    title: link.title ?? "",
                    href: resolveHrefKotlin(link.href.toString(), base: baseURL),
                    rel: relStr
                ))
            }
            var moreHref: String? = nil
            for link in group.links {
                if link.rels.contains("self") {
                    moreHref = resolveHrefKotlin(link.href.toString(), base: baseURL)
                }
            }
            groupEntries.append(OPDSGroupEntry(
                title: group.metadata.title,
                publications: groupPubs,
                navigation: groupNav,
                moreURL: moreHref
            ))
        }

        var facetEntries: [OPDSFacetEntry] = []
        for facet in feed.facets {
            var facetLinks: [OPDSNavLink] = []
            for link in facet.links {
                let relStr = firstRel(link)
                facetLinks.append(OPDSNavLink(
                    title: link.title ?? "",
                    href: resolveHrefKotlin(link.href.toString(), base: baseURL),
                    rel: relStr
                ))
            }
            facetEntries.append(OPDSFacetEntry(title: facet.metadata.title, links: facetLinks))
        }

        var searchHref: String? = nil
        var nextHref: String? = nil
        var iconHref: String? = nil
        var infoLinks: [OPDSNavLink] = []
        let infoRels: Set<String> = ["alternate", "about", "related", "http://opds-spec.org/shelf", "help", "terms-of-service", "privacy-policy"]
        for link in feed.links {
            let linkHref = link.href.toString()
            if link.rels.contains("search") {
                searchHref = linkHref
            }
            if link.rels.contains("next") {
                nextHref = resolveHrefKotlin(linkHref, base: baseURL)
            }
            // Extract icon from image links
            if iconHref == nil {
                for rel in link.rels {
                    if rel.contains("opds-spec.org/image") {
                        iconHref = resolveHrefKotlin(linkHref, base: baseURL)
                    }
                }
                if iconHref == nil, let mediaType = link.mediaType?.toString(), mediaType.hasPrefix("image/") {
                    iconHref = resolveHrefKotlin(linkHref, base: baseURL)
                }
            }
            // Extract informational links
            for rel in link.rels {
                if infoRels.contains(rel) {
                    let title = link.title ?? rel.replacingOccurrences(of: "-", with: " ").capitalized
                    infoLinks.append(OPDSNavLink(title: title, href: resolveHrefKotlin(linkHref, base: baseURL), rel: rel))
                }
            }
        }

        // OpdsMetadata in Readium Kotlin does not have a description property
        let subtitle: String? = nil

        return OPDSFeedContent(
            title: feed.metadata.title,
            subtitle: subtitle,
            iconURL: iconHref,
            infoLinks: infoLinks,
            navigation: navLinks,
            publications: pubEntries,
            groups: groupEntries,
            facets: facetEntries,
            searchURL: searchHref,
            nextPageURL: nextHref,
            totalResults: nil
        )
    }

    /// Extract the first rel string from a Kotlin Link's rels set.
    private static func firstRel(_ link: Link) -> String? {
        for rel in link.rels {
            return rel
        }
        return nil
    }

    private static func convertPublicationKotlin(_ pub: Publication, baseURL: URL) -> OPDSPubEntry {
        let title = pub.metadata.localizedTitle?.string ?? "Untitled"
        var authorNames: [String] = []
        for author in pub.metadata.authors {
            authorNames.append(author.name)
        }
        let summary: String? = pub.metadata.description
        let identifier = pub.metadata.identifier ?? title

        // Get images from subcollections
        var thumbnailHref: String? = nil
        var imageHref: String? = nil
        let imageCollections = pub.manifest.subcollections["images"]
        if imageCollections != nil {
            let collections = imageCollections!
            for collection in collections {
                let imageLinks = collection.links
                for imgLink in imageLinks {
                    let hrefStr = imgLink.href.toString()
                    if imgLink.rels.contains("http://opds-spec.org/image/thumbnail") {
                        thumbnailHref = resolveHrefKotlin(hrefStr, base: baseURL)
                    }
                    if imgLink.rels.contains("http://opds-spec.org/image") {
                        imageHref = resolveHrefKotlin(hrefStr, base: baseURL)
                    }
                    // Use first image as fallback
                    if thumbnailHref == nil {
                        thumbnailHref = resolveHrefKotlin(hrefStr, base: baseURL)
                    }
                    if imageHref == nil {
                        imageHref = resolveHrefKotlin(hrefStr, base: baseURL)
                    }
                }
            }
        }

        // Find acquisition link — prefer EPUB, fall back to any format
        var acqHref: String? = nil
        var acqType: String? = nil
        var fallbackHref: String? = nil
        var fallbackType: String? = nil
        for link in pub.manifest.links {
            for rel in link.rels {
                if rel.contains("http://opds-spec.org/acquisition") {
                    let href = resolveHrefKotlin(link.href.toString(), base: baseURL)
                    let mediaType = link.mediaType?.toString()
                    if mediaType != nil && mediaType!.contains("epub") {
                        acqHref = href
                        acqType = mediaType
                    } else if fallbackHref == nil {
                        fallbackHref = href
                        fallbackType = mediaType
                    }
                }
            }
        }
        if acqHref == nil {
            acqHref = fallbackHref
            acqType = fallbackType
        }

        return OPDSPubEntry(
            id: identifier,
            title: title,
            authors: authorNames,
            summary: summary,
            imageURL: imageHref,
            thumbnailURL: thumbnailHref,
            acquisitionURL: acqHref,
            acquisitionType: acqType
        )
    }

    private static func resolveHrefKotlin(_ href: String, base: URL) -> String {
        if href.hasPrefix("http://") || href.hasPrefix("https://") {
            return href
        }
        return URL(string: href, relativeTo: base)?.absoluteString ?? href
    }
    #endif
}

public enum OPDSServiceError: Error {
    case noFeedContent
    case parseFailed
    case invalidURL
}
