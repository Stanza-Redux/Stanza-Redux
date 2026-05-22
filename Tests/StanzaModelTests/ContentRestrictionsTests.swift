// Copyright 2026 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later
import XCTest
import Foundation
@testable import StanzaModel

@available(macOS 14, *)
final class ContentRestrictionsTests: XCTestCase {

    func testParseSingleCoverRestriction() throws {
        let json = """
        {
          "restrictions": [
            {
              "uid": "https://standardebooks.org/ebooks/apuleius/the-golden-ass/william-adlington",
              "storefronts": {
                "googleplay": {
                  "mode": "cover",
                  "reason": "Censored by Google Play policy (appeals ID 5-1708000040527)"
                }
              }
            }
          ]
        }
        """
        let data = try XCTUnwrap(json.data(using: .utf8))
        let restrictions = try parseRestrictions(data: data)

        XCTAssertEqual(1, restrictions.count)
        let entry = try XCTUnwrap(restrictions.first)
        XCTAssertEqual("https://standardebooks.org/ebooks/apuleius/the-golden-ass/william-adlington", entry.uid)
        XCTAssertEqual(1, entry.storefronts.count)

        let storefront = try XCTUnwrap(entry.storefronts["googleplay"])
        XCTAssertEqual(ContentRestrictionMode.cover, storefront.mode)
        XCTAssertEqual("Censored by Google Play policy (appeals ID 5-1708000040527)", storefront.reason)
    }

    func testParseContentModeRestriction() throws {
        let json = """
        {
          "restrictions": [
            {
              "uid": "urn:isbn:1234567890",
              "storefronts": {
                "appstore": {
                  "mode": "content",
                  "reason": "Removed for App Store policy violation"
                }
              }
            }
          ]
        }
        """
        let data = try XCTUnwrap(json.data(using: .utf8))
        let restrictions = try parseRestrictions(data: data)
        let entry = try XCTUnwrap(restrictions.first)
        let storefront = try XCTUnwrap(entry.storefronts["appstore"])
        XCTAssertEqual(ContentRestrictionMode.content, storefront.mode)
        XCTAssertEqual("Removed for App Store policy violation", storefront.reason)
    }

    func testParseMultipleStorefronts() throws {
        let json = """
        {
          "restrictions": [
            {
              "uid": "urn:isbn:9990001112223",
              "storefronts": {
                "googleplay": {
                  "mode": "cover",
                  "reason": "Cover blurred per Google Play policy"
                },
                "appstore": {
                  "mode": "content",
                  "reason": "Blocked per App Store policy"
                }
              }
            }
          ]
        }
        """
        let data = try XCTUnwrap(json.data(using: .utf8))
        let restrictions = try parseRestrictions(data: data)
        let entry = try XCTUnwrap(restrictions.first)
        XCTAssertEqual(2, entry.storefronts.count)
        XCTAssertEqual(ContentRestrictionMode.cover, entry.storefronts["googleplay"]?.mode)
        XCTAssertEqual(ContentRestrictionMode.content, entry.storefronts["appstore"]?.mode)
    }

    func testServiceFiltersByStorefront() throws {
        let googlePlayService = ContentRestrictionService(storefront: Storefront.googlePlayStore)
        let goldenAssUID = "https://standardebooks.org/ebooks/apuleius/the-golden-ass/william-adlington"
        let restriction = try XCTUnwrap(googlePlayService.restriction(forUID: goldenAssUID))
        XCTAssertEqual(ContentRestrictionMode.cover, restriction.mode)
        XCTAssertTrue(restriction.reason.contains("Google Play"))

        // The bundled file only has a googleplaystore entry, so the App Store should not be restricted.
        let appStoreService = ContentRestrictionService(storefront: Storefront.appleAppStore)
        XCTAssertNil(appStoreService.restriction(forUID: goldenAssUID))

        // An unknown uid should never be restricted.
        XCTAssertNil(googlePlayService.restriction(forUID: "urn:not-a-real-book"))
    }
}
