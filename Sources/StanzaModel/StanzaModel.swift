// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import OSLog
#if !SKIP
import ReadiumOPDS
//import ReadiumLCP
import ReadiumAdapterGCDWebServer
import ReadiumShared
import ReadiumStreamer
#else
import android.content.ContentResolver
import org.readium.r2.shared.opds.Acquisition
import org.readium.r2.shared.opds.Facet
import org.readium.r2.shared.opds.Feed
import org.readium.r2.shared.opds.Group
import org.readium.r2.shared.opds.ParseData
import org.readium.r2.shared.opds.Price
import org.readium.r2.shared.publication.Contributor
import org.readium.r2.shared.publication.Href
import org.readium.r2.shared.publication.Link
import org.readium.r2.shared.publication.LocalizedString
import org.readium.r2.shared.publication.Locator
import org.readium.r2.shared.publication.Manifest
import org.readium.r2.shared.publication.Metadata
import org.readium.r2.shared.publication.Properties
import org.readium.r2.shared.publication.Publication
import org.readium.r2.shared.publication.PublicationCollection
import org.readium.r2.shared.publication.Subject
import org.readium.r2.shared.toJSON
import org.readium.r2.shared.util.AbsoluteUrl
import org.readium.r2.shared.util.ErrorException
import org.readium.r2.shared.util.Instant
import org.readium.r2.shared.util.Try
import org.readium.r2.shared.util.Url
import org.readium.r2.shared.util.getOrElse
import org.readium.r2.shared.util.toUri
import org.readium.r2.shared.util.toUrl
import org.readium.r2.shared.util.toAbsoluteUrl
import org.readium.r2.shared.util.asset.AssetRetriever
import org.readium.r2.shared.util.http.DefaultHttpClient
import org.readium.r2.shared.util.http.HttpClient
import org.readium.r2.shared.util.http.HttpRequest
import org.readium.r2.shared.util.http.fetchWithDecoder
import org.readium.r2.shared.util.mediatype.MediaType
import org.readium.r2.shared.util.pdf.PdfDocumentFactory
import org.readium.r2.shared.util.xml.ElementNode
import org.readium.r2.shared.util.xml.XmlParser
import org.readium.r2.streamer.PublicationOpener
import org.readium.r2.streamer.parser.PublicationParser
import org.readium.r2.streamer.parser.DefaultPublicationParser
//import org.readium.adapter.pdfium.document.PdfiumDocumentFactory

// Kotlin has different capitalization than Swift for the type (DefaultHttpClient vs. DefaultHTTPClient)
typealias DefaultHTTPClient = org.readium.r2.shared.util.http.DefaultHttpClient
#endif

let logger = Logger(subsystem: "Stanza", category: "StanzaModelTests")
public let httpClient: DefaultHTTPClient = DefaultHTTPClient(userAgent: "Readium")

#if !SKIP
public let httpServer: GCDHTTPServer = GCDHTTPServer(assetRetriever: assetRetriever)
public let assetRetriever = AssetRetriever(httpClient: httpClient)
let pdfDocumentFactory = DefaultPDFDocumentFactory()
let publicationOpener = PublicationOpener(
    parser: DefaultPublicationParser(
        httpClient: httpClient,
        assetRetriever: assetRetriever,
        pdfFactory: pdfDocumentFactory
    )
)
#else
public let contentResolver: ContentResolver = ProcessInfo.processInfo.androidContext.contentResolver
public let assetRetriever: AssetRetriever = AssetRetriever(contentResolver: contentResolver, httpClient: httpClient)
//let pdfDocumentFactory: PdfiumDocumentFactory = PdfiumDocumentFactory(context: ProcessInfo.processInfo.androidContext)
let publicationOpener: PublicationOpener = PublicationOpener(
    publicationParser: DefaultPublicationParser(
        context: ProcessInfo.processInfo.androidContext,
        httpClient: httpClient,
        assetRetriever: assetRetriever,
        pdfFactory: nil // pdfDocumentFactory
    )
)
#endif


#if !SKIP
public typealias PlatformPublication = ReadiumShared.Publication
#else
public typealias PlatformPublication = org.readium.r2.shared.publication.Publication
#endif

/// A wrapper for an underlying `Publication` type.
public class Pub {
    public var platformValue: PlatformPublication

    public init(platformValue: Publication) {
        self.platformValue = platformValue
    }

    public var manifest: Man {
        Man(platformValue: platformValue.manifest)
    }

    public var metadata: Meta {
        manifest.metadata
    }

    public var language: String? {
        #if !SKIP
        platformValue.metadata.language?.code.bcp47
        #else
        platformValue.metadata.language?.code
        #endif
    }
}

#if SKIP
extension Pub: KotlinConverting<PlatformPublication> {
    public override func kotlin(nocopy: Bool = false) -> PlatformPublication {
        return platformValue
    }
}
#endif

extension Pub {
    /// Loads the publication from the given URL.
    public static func loadPublication(from bookURL: URL, allowUserInteraction: Bool = true) async throws -> Pub {

        #if !SKIP
        // Retrieve an `Asset` to access the file content.
        switch await assetRetriever.retrieve(url: bookURL.anyURL.absoluteURL!) {
        case .success(let asset):
            // Open a `Publication` from the `Asset`.
            switch await publicationOpener.open(asset: asset, allowUserInteraction: allowUserInteraction) {
            case .success(let publication):
                logger.log("opened \(publication.metadata.title ?? "Unknown")")
                return Pub(platformValue: publication)
            case .failure(let error):
                // Failed to access or parse the publication
                logger.log("error \(error)")
                throw error
            }

        case .failure(let error):
            // Failed to retrieve the asset
            logger.log("error \(error)")
            throw error
        }
        #else
        // e.g. asset:/stanza/module/Resources/Alice.epub
        var url = bookURL

        logger.log("opening bookURL: \(url)")
        // Cannot open assets directly from the apk, so we need to copy them out to a temporary file
        // org.readium.r2.shared.util.asset.AssetRetriever$RetrieveUrlError$SchemeNotSupported
        if bookURL.scheme == "asset" {
            // copy it out to a file, then open the file on disk TODO: cleanup
            let tmpURL = URL.cachesDirectory.appendingPathComponent(url.lastPathComponent!)
            logger.debug("copying asset to tmpFile: \(tmpURL.path)")
            try url.kotlin().toURL().openStream().copyTo(java.io.FileOutputStream(tmpURL.path))
            url = tmpURL
        }

        let absoluteUrl: org.readium.r2.shared.util.AbsoluteUrl = url.kotlin().toURL().toAbsoluteUrl()!
        let asset = assetRetriever.retrieve(absoluteUrl)
            .getOrElse { error in
                logger.error("could not retrieve: \(absoluteUrl): \(error)")
                throw error
            }

        let publication: Publication = publicationOpener.open(asset: asset, allowUserInteraction: allowUserInteraction)
            .getOrElse { error in
                logger.error("could not open: \(absoluteUrl): \(error)")
                throw error
            }
        logger.info("opened publication: \(publication)")

        return Pub(platformValue: publication)
        #endif
    }
}


#if !SKIP
public typealias PlatformManifest = ReadiumShared.Manifest
#else
public typealias PlatformManifest = org.readium.r2.shared.publication.Manifest
#endif

/// Holds the metadata of a Readium publication, as described in the Readium Web Publication
/// Manifest.
///
/// See. https://readium.org/webpub-manifest/
public struct Man {
    public var platformValue: PlatformManifest

    public init(platformValue: PlatformManifest) {
        self.platformValue = platformValue
    }

    public var metadata: Meta {
        Meta(platformValue: platformValue.metadata)
    }

    public var links: [Lnk] {
        Array(platformValue.links).map({ Lnk(platformValue: $0)} )
    }

    /// Identifies a list of resources in reading order for the publication.
    public var readingOrder: [Lnk] {
        Array(platformValue.readingOrder).map({ Lnk(platformValue: $0)} )
    }

    /// Identifies resources that are necessary for rendering the publication.
    public var resources: [Lnk] {
        Array(platformValue.resources).map({ Lnk(platformValue: $0)} )
    }

    /// Identifies the collection that contains a table of contents.
    public var tableOfContents: [Lnk] {
        Array(platformValue.tableOfContents).map({ Lnk(platformValue: $0)} )
    }
}

#if SKIP
extension Man: KotlinConverting<PlatformManifest> {
    public override func kotlin(nocopy: Bool = false) -> PlatformManifest {
        return platformValue
    }
}
#endif



#if !SKIP
public typealias PlatformMetadata = ReadiumShared.Metadata
#else
public typealias PlatformMetadata = org.readium.r2.shared.publication.Metadata
#endif

/// A wrapper for an underlying `Metadata` type.
public struct Meta {
    public var platformValue: PlatformMetadata

    public init(platformValue: PlatformMetadata) {
        self.platformValue = platformValue
    }

    public var identifier: String? {
        platformValue.identifier
    }

    public var title: String? {
        platformValue.localizedTitle?.string
    }

    public var subtitle: String? {
        platformValue.localizedSubtitle?.string
    }

    public var sortAs: String? {
        platformValue.sortAs
    }

    public var numberOfPages: Int? {
        platformValue.numberOfPages
    }

    public var duration: Double? {
        platformValue.duration
    }

    public var subjects: [Sub] {
        Array(platformValue.subjects).map({ Sub(platformValue: $0)} )
    }

    public var published: Date? {
        #if !SKIP
        return platformValue.published
        #else
        // org.readium.r2.shared.util.Instant
        guard let instant = platformValue.published else { return nil }
        return Date(platformValue: instant.toJavaDate())
        #endif
    }
}

#if SKIP
extension Meta: KotlinConverting<PlatformMetadata> {
    public override func kotlin(nocopy: Bool = false) -> PlatformMetadata {
        return platformValue
    }
}
#endif


#if !SKIP
public typealias PlatformLink = ReadiumShared.Link
#else
public typealias PlatformLink = org.readium.r2.shared.publication.Link
#endif

/// A wrapper for an underlying `Link` type.
public struct Lnk {
    public var platformValue: PlatformLink

    public init(platformValue: PlatformLink) {
        self.platformValue = platformValue
    }

    public var title: String? {
        platformValue.title
    }

    /// URI or URI template of the linked resource.
    public var href: String {
        #if !SKIP
        return platformValue.href
        #else
        // org.readium.r2.shared.publication.Href
        return platformValue.href.toString()
        #endif
    }

    public var templated: Bool {
        #if !SKIP
        platformValue.templated
        #else
        // org.readium.r2.shared.publication.Href
        return platformValue.href.isTemplated
        #endif
    }

    /// Resources that are children of the linked resource, in the context of a given collection role.
    public var children: [Lnk] {
        Array(platformValue.children).map({ Lnk(platformValue: $0) })
    }

    /// Alternate resources for the linked resource.
    public var alternates: [Lnk] {
        Array(platformValue.alternates).map({ Lnk(platformValue: $0) })
    }

//    public var mediaType: MediaType? {
//        platformValue.mediaType
//    }
}

#if SKIP
extension Lnk: KotlinConverting<PlatformLink> {
    public override func kotlin(nocopy: Bool = false) -> PlatformLink {
        return platformValue
    }
}
#endif


#if !SKIP
public typealias PlatformLocator = ReadiumShared.Locator
#else
public typealias PlatformLocator = org.readium.r2.shared.publication.Locator
#endif

/// A wrapper for an underlying `Locator` type.
public struct Loc {
    public var platformValue: PlatformLocator

    public init(platformValue: PlatformLocator) {
        self.platformValue = platformValue
    }
}

#if SKIP
extension Loc: KotlinConverting<PlatformLocator> {
    public override func kotlin(nocopy: Bool = false) -> PlatformLocator {
        return platformValue
    }
}
#endif

#if !SKIP
public typealias PlatformSubject = ReadiumShared.Subject
#else
public typealias PlatformSubject = org.readium.r2.shared.publication.Subject
#endif

/// A wrapper for an underlying `Subject` type.
public struct Sub {
    public var platformValue: PlatformSubject

    public init(platformValue: PlatformSubject) {
        self.platformValue = platformValue
    }
}

#if SKIP
extension Sub: KotlinConverting<PlatformSubject> {
    public override func kotlin(nocopy: Bool = false) -> PlatformSubject {
        return platformValue
    }
}
#endif
