// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later
import XCTest
import Foundation
@testable import StanzaModel

@available(macOS 14, *)
final class StanzaSettingsKeepScreenOnTests: XCTestCase {

    /// A dedicated suite so the test never pollutes the standard UserDefaults.
    private let suiteName = "org.appfair.stanza.tests.keepScreenOn"

    private func makeDefaults() -> UserDefaults {
        UserDefaults(suiteName: suiteName)!
    }

    override func setUp() {
        super.setUp()
        // `removePersistentDomain(forName:)` isn't available in Skip; clearing the
        // specific key resets the suite to the default and works on both platforms.
        makeDefaults().removeObject(forKey: "keepScreenOn")
    }

    override func tearDown() {
        makeDefaults().removeObject(forKey: "keepScreenOn")
        super.tearDown()
    }

    func testDefaultsToTrue() {
        let settings = StanzaSettings(defaults: makeDefaults())
        XCTAssertTrue(settings.keepScreenOn, "keepScreenOn should default to true on a fresh instance")
    }

    func testPersistsFalseAcrossInstances() {
        let defaults = makeDefaults()
        let settings = StanzaSettings(defaults: defaults)
        settings.keepScreenOn = false

        // A new instance reading the same suite should observe the persisted value.
        let reloaded = StanzaSettings(defaults: makeDefaults())
        XCTAssertFalse(reloaded.keepScreenOn, "keepScreenOn should persist false across instances using the same suite")
    }

    func testToggling() {
        let settings = StanzaSettings(defaults: makeDefaults())
        XCTAssertTrue(settings.keepScreenOn)
        settings.keepScreenOn = false
        XCTAssertFalse(settings.keepScreenOn)
        settings.keepScreenOn = true
        XCTAssertTrue(settings.keepScreenOn)

        // The final value should also survive a reload.
        let reloaded = StanzaSettings(defaults: makeDefaults())
        XCTAssertTrue(reloaded.keepScreenOn)
    }
}
