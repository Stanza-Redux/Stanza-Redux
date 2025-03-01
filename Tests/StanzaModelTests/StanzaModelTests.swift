// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-3.0-only
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
        XCTAssertEqual("Alice's Adventures in Wonderland", pub.title)

        XCTAssertEqual("http://www.gutenberg.org/11", pub.platformValue.metadata.identifier)
        //XCTAssertEqual("en", pub.platformValue.metadata.language) // java.lang.AssertionError: expected:<Language(en)> but was:<en>
        //XCTAssertEqual("", pub.platformValue.metadata.localizedTitle)
        XCTAssertEqual(nil, pub.platformValue.metadata.sortAs)
        //XCTAssertEqual("", pub.platformValue.metadata.authors)

        XCTAssertEqual(nil, pub.platformValue.metadata.localizedSubtitle)
        //XCTAssertEqual("", pub.platformValue.metadata.subtitle) // Unresolved reference 'subtitle'.

        XCTAssertEqual(nil, pub.platformValue.metadata.duration)
        XCTAssertEqual(nil, pub.platformValue.metadata.numberOfPages)
        XCTAssertEqual(nil, pub.platformValue.metadata.type)
        XCTAssertEqual(["en"], Array(pub.platformValue.metadata.languages))

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
