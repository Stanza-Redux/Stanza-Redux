// Copyright 2026 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later
import XCTest
import Foundation
@testable import Stanza

@available(macOS 13, *)
final class CatalogURLTests: XCTestCase {

    func testTrimsLeadingAndTrailingSpaces() {
        XCTAssertEqual("https://example.com/feed", normalizedCatalogURLString("  https://example.com/feed  "))
    }

    func testTrimsTabsAndNewlines() {
        XCTAssertEqual("https://example.com/feed", normalizedCatalogURLString("\t\nhttps://example.com/feed\n\t"))
    }

    func testLeavesCleanURLUnchanged() {
        XCTAssertEqual("https://example.com/feed", normalizedCatalogURLString("https://example.com/feed"))
    }

    func testAllWhitespaceBecomesEmpty() {
        XCTAssertEqual("", normalizedCatalogURLString("   \t\n  "))
    }

    func testEmptyStringStaysEmpty() {
        XCTAssertEqual("", normalizedCatalogURLString(""))
    }
}
