// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later
import XCTest
import OSLog
import Foundation
@testable import StanzaModel
#if !SKIP
import ReadiumShared
#endif

//let logger: Logger = Logger(subsystem: "Stanza", category: "StanzaModelTests")

@available(macOS 14, *)
final class StanzaModelTests: XCTestCase {
    func testStanzaModel() async throws {
        let epubURL = try XCTUnwrap(Bundle.module.url(forResource: "Alice", withExtension: "epub"))
        print("epubURL: \(epubURL.absoluteString)")
        let pub = try await Pub.loadPublication(from: epubURL)

        XCTAssertEqual("http://www.gutenberg.org/11", pub.metadata.identifier)
        XCTAssertEqual("Alice's Adventures in Wonderland", pub.metadata.title)
        XCTAssertNil(pub.metadata.subtitle)
        //XCTAssertEqual("en", pub.language)
        XCTAssertEqual("2008-06-27 00:00:00 +0000", pub.metadata.published?.description)

        //XCTAssertEqual("", pub.platformValue.metadata.localizedTitle)
        XCTAssertEqual(nil, pub.platformValue.metadata.sortAs)
        //XCTAssertEqual("", pub.platformValue.metadata.authors)

        XCTAssertEqual(nil, pub.platformValue.metadata.localizedSubtitle)
        //XCTAssertEqual("", pub.platformValue.metadata.subtitle) // Unresolved reference 'subtitle'.

        XCTAssertEqual(nil, pub.platformValue.metadata.duration)
        XCTAssertEqual(nil, pub.platformValue.metadata.numberOfPages)
        XCTAssertEqual(nil, pub.platformValue.metadata.type)
        XCTAssertEqual(["en"], Array(pub.platformValue.metadata.languages))

        let links = pub.manifest.links
        // XCTAssertEqual(1, links.count) // 0 on Kotlin?!?

        let readingOrder = pub.manifest.readingOrder
        XCTAssertEqual(15, readingOrder.count)

        let resources = pub.manifest.resources
        XCTAssertEqual(6, resources.count)

        let toc = pub.manifest.tableOfContents
        XCTAssertEqual(16, toc.count)
        XCTAssertEqual("Alice’s Adventures in Wonderland", toc.first?.title)
        XCTAssertEqual("OEBPS/8149966833358938453_11-h-0.htm.xhtml#pgepubid00000", toc.first?.href)
        XCTAssertEqual("THE FULL PROJECT GUTENBERG LICENSE", toc.last?.title)
        XCTAssertEqual("OEBPS/8149966833358938453_11-h-13.htm.xhtml#pg-footer-heading", toc.last?.href)

        let subjects = pub.manifest.metadata.subjects
        XCTAssertEqual(4, subjects.count)

        //XCTAssertEqual("", pub.platformValue.metadata.json)

        //XCTAssertEqual([], pub.platformValue.metadata.publishers)
        //XCTAssertEqual(ReadingProgression, pub.platformValue.metadata.readingProgression)
        //XCTAssertEqual(Date(timeIntervalSince1970: 0), pub.platformValue.metadata.modified)
        //XCTAssertEqual([], pub.platformValue.metadata.subjects) // XCTAssertEqual failed: ("[]") is not equal to ("[ReadiumShared.Subject(localizedName: Fantasy fiction, sortAs: nil, scheme: nil, code: nil, links: []), ReadiumShared.Subject(localizedName: Children's stories, sortAs: nil, scheme: nil, code: nil, links: []), ReadiumShared.Subject(localizedName: Imaginary places -- Juvenile fiction, sortAs: nil, scheme: nil, code: nil, links: []), ReadiumShared.Subject(localizedName: Alice (Fictitious character from Carroll) -- Juvenile fiction, sortAs: nil, scheme: nil, code: nil, links: [])]")
        //XCTAssertEqual("", pub.platformValue.metadata.otherMetadata)
        //XCTAssertEqual("", pub.platformValue.metadata.presentation)
        //XCTAssertEqual(Date(timeIntervalSince1970: 0), pub.platformValue.metadata.published)
        //XCTAssertEqual("", pub.platformValue.metadata.accessibility)
        //XCTAssertEqual("", pub.platformValue.metadata.belongsToCollections)
        //XCTAssertEqual("", pub.platformValue.metadata.belongsToSeries)
        //XCTAssertEqual("", pub.platformValue.metadata.conformsTo)
        //XCTAssertEqual("", pub.platformValue.metadata.artists)
        //XCTAssertEqual("", pub.platformValue.metadata.contributors)
        //XCTAssertEqual("", pub.platformValue.metadata.illustrators)
        //XCTAssertEqual("", pub.platformValue.metadata.belongsTo)
        //XCTAssertEqual("", pub.platformValue.metadata.colorists)
        //XCTAssertEqual("", pub.platformValue.metadata.editors)
        //XCTAssertEqual("", pub.platformValue.metadata.imprints)
        //XCTAssertEqual("", pub.platformValue.metadata.inkers)
        //XCTAssertEqual("", pub.platformValue.metadata.letterers)
        //XCTAssertEqual("", pub.platformValue.metadata.narrators)
        //XCTAssertEqual("", pub.platformValue.metadata.pencilers)
        //XCTAssertEqual("", pub.platformValue.metadata.translators)
    }
}
