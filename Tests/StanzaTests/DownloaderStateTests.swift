// Copyright 2026 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later
import XCTest
import Foundation
@testable import Stanza

// These tests exercise only the parts of the download state machine that do not require
// a live network connection: construction, the initial idle state, and reset(). We never
// call start(), which would kick off a real URLSession/HttpURLConnection request.
@available(macOS 14, *)
final class DownloaderStateTests: XCTestCase {

    private func makeDownloader(name: String = "Test Book") -> FileDownloader {
        let source = URL(string: "https://example.com/book.epub")!
        let dest = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("book.epub")
        return FileDownloader(sourceURL: source, destinationURL: dest, displayName: name)
    }

    func testInitialStateIsIdle() {
        let dl = makeDownloader()
        XCTAssertEqual(.idle, dl.state)
        XCTAssertEqual(0.0, dl.progress)
        XCTAssertEqual(0, dl.bytesReceived)
        XCTAssertEqual(-1, dl.bytesTotal)
    }

    func testDisplayNameDefaultsToLastPathComponent() {
        let source = URL(string: "https://example.com/path/mybook.epub")!
        let dest = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("mybook.epub")
        let dl = FileDownloader(sourceURL: source, destinationURL: dest)
        XCTAssertEqual("mybook.epub", dl.displayName)
    }

    func testResetReturnsToIdle() {
        let dl = makeDownloader()
        // Simulate a non-idle state without touching the network.
        dl.cancel()
        XCTAssertEqual(.cancelled, dl.state)

        dl.reset()
        XCTAssertEqual(.idle, dl.state)
        XCTAssertEqual(0.0, dl.progress)
        XCTAssertEqual(0, dl.bytesReceived)
        XCTAssertEqual(-1, dl.bytesTotal)
    }
}
