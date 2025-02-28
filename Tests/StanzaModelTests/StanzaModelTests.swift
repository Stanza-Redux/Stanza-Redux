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
    func testStanzaModel() throws {
        let epubURL = try XCTUnwrap(Bundle.module.url(forResource: "Alice", withExtension: "epub"))
        print("epubURL: \(epubURL.absoluteString)")
//        logger.log("checking epub: \(epubURL.absoluteString)")
    }
}
