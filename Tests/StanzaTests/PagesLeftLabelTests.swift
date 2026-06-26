// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later
import XCTest
import Foundation
@testable import Stanza

@available(macOS 13, *)
final class PagesLeftLabelTests: XCTestCase {

    func testSingularUsesPageNotPages() {
        let label = pagesLeftInChapterLabel(1)
        XCTAssertTrue(label.contains("1 page"), "singular label should contain '1 page' but was '\(label)'")
        XCTAssertFalse(label.contains("pages"), "singular label should not contain 'pages' but was '\(label)'")
    }

    func testPluralUsesPages() {
        let label = pagesLeftInChapterLabel(3)
        XCTAssertTrue(label.contains("3 pages"), "plural label should contain '3 pages' but was '\(label)'")
    }

    func testZeroUsesPages() {
        let label = pagesLeftInChapterLabel(0)
        XCTAssertTrue(label.contains("0 pages"), "zero label should contain '0 pages' but was '\(label)'")
    }
}
