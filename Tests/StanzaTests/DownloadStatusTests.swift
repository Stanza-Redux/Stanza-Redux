// Copyright 2026 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later
import XCTest
import Foundation
@testable import Stanza

@available(macOS 13, *)
final class DownloadStatusTests: XCTestCase {

    func testSuccessStatusCodesAreNotFailures() {
        XCTAssertFalse(isFailureHTTPStatus(200))
        XCTAssertFalse(isFailureHTTPStatus(299))
    }

    func testRedirectStatusCodesAreNotFailures() {
        XCTAssertFalse(isFailureHTTPStatus(399))
    }

    func testClientAndServerErrorsAreFailures() {
        XCTAssertTrue(isFailureHTTPStatus(400))
        XCTAssertTrue(isFailureHTTPStatus(404))
        XCTAssertTrue(isFailureHTTPStatus(500))
    }
}
