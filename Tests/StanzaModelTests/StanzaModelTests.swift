// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-3.0-only
import XCTest
import OSLog
import Foundation
@testable import Stanza

let logger: Logger = Logger(subsystem: "Stanza", category: "StanzaModelTests")

@available(macOS 13, *)
final class StanzaModelTests: XCTestCase {
    func testStanza() throws {
        logger.log("running testStanza")
        XCTAssertEqual(1 + 2, 3, "basic test")
        
        // load the TestData.json file from the Resources folder and decode it into a struct
        let resourceURL: URL = try XCTUnwrap(Bundle.module.url(forResource: "TestData", withExtension: "json"))
        let testData = try JSONDecoder().decode(TestData.self, from: Data(contentsOf: resourceURL))
        XCTAssertEqual("Stanza", testData.testModuleName)
    }
}

struct TestData : Codable, Hashable {
    var testModuleName: String
}
