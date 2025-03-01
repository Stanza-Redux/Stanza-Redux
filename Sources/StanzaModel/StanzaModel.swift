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

public class StanzaModel {
    public static func loadPublication(bookURL: URL) async throws -> Publication {

        #if !SKIP
        // Retrieve an `Asset` to access the file content.
        switch await assetRetriever.retrieve(url: bookURL.anyURL.absoluteURL!) {
        case .success(let asset):
            // Open a `Publication` from the `Asset`.
            switch await publicationOpener.open(asset: asset, allowUserInteraction: true) {
            case .success(let publication):
                logger.log("opened \(publication.metadata.title ?? "Unknown")")
                return publication


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
        let uri: java.net.URI = bookURL.kotlin() // e.g. jar:file:/data/app/~~KxZz_SOCG-tQVrrDgu-5KQ==/org.appfair.app.Stanza_Redux-dgdtNt79ZSigWaEiQOHNdA==/base.apk!/stanza/module/Resources/Alice.epub

        // FIXME: NPE because Uri.addFileAuthority doesn't understand jar:file: schemes
        // TODO: write out the book URL to a temporary file and open that
        //let absoluteUrl: org.readium.r2.shared.util.AbsoluteUrl = uri.toURL().toAbsoluteUrl()!
        logger.log("bookURL: \(uri)")
        let storageDir = ProcessInfo.processInfo.androidContext.getExternalFilesDir(android.os.Environment.DIRECTORY_DOCUMENTS)
        let ext = ".epub"
        let tmpFile = java.io.File.createTempFile("Stanza_\(UUID().uuidString)", ext, storageDir)

        uri.toURL().openStream().copyTo(java.io.FileOutputStream(tmpFile))

        logger.log("tmpFile: \(tmpFile)")
        let absoluteUrl: org.readium.r2.shared.util.AbsoluteUrl = tmpFile.toURL().toAbsoluteUrl()!
        let asset = assetRetriever.retrieve(absoluteUrl).getOrElse { _ in error("could not retrieve: \(absoluteUrl)") }

        logger.log("asset: \(asset)")

        let pub: Publication = publicationOpener.open(asset: asset, allowUserInteraction: true).getOrElse { _ in error("could not open: \(asset)") }
        logger.log("pub: \(pub)")

        return pub
        #endif
    }
}

