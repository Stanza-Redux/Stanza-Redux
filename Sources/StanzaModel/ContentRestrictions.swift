// Copyright 2026 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import Foundation
import OSLog

let restrictionsLogger = Logger(subsystem: "Stanza", category: "ContentRestrictions")

/// How a content restriction should be applied to a book.
public enum ContentRestrictionMode: String, Codable, Sendable {
    /// The book's cover is blurred and overlaid with a censorship notice, but the book may still be downloaded and read.
    case cover
    /// The book may not be downloaded at all; an error with the configured reason is shown to the user instead.
    case content
}

/// Restriction details for a single storefront.
public struct StorefrontRestriction: Codable, Hashable, Sendable {
    public let mode: ContentRestrictionMode
    public let reason: String

    public init(mode: ContentRestrictionMode, reason: String) {
        self.mode = mode
        self.reason = reason
    }
}

/// A restriction record for a single book uid, along with the storefronts where the restriction applies.
public struct ContentRestriction: Codable, Hashable, Sendable {
    public let uid: String
    public let storefronts: [String: StorefrontRestriction]

    public init(uid: String, storefronts: [String: StorefrontRestriction]) {
        self.uid = uid
        self.storefronts = storefronts
    }
}

/// Top-level container matching the on-disk JSON layout.
struct ContentRestrictionList: Codable {
    let restrictions: [ContentRestriction]
}

/// Identifier for the storefront the running app was installed from.
///
/// Defaults to `googleplay` on Android and `appstore` on Apple platforms.
public enum Storefront {
    public static let googlePlayStore = "googleplaystore"
    public static let appleAppStore = "appleappstore"

    /// The storefront identifier for the current platform.
    public static var current: String {
        #if os(Android)
        return googlePlayStore
        #else
        return appleAppStore
        #endif
    }
}

/// Loads the bundled content-restrictions.json file once at construction time
/// and exposes a lookup by book uid. Naturally `Sendable` because all state is immutable.
public final class ContentRestrictionService: Sendable {
    /// Shared default service that reads from the StanzaModel bundle.
    public static let shared = ContentRestrictionService()

    /// Map of restricted book uid to the restriction entry for this service's storefront.
    public let restrictionsByUID: [String: StorefrontRestriction]

    public init(storefront: String = Storefront.current) {
        self.restrictionsByUID = Self.loadRestrictions(forStorefront: storefront)
    }

    /// Returns the restriction for the given book uid in the current storefront, or `nil` if the book is unrestricted.
    public func restriction(forUID uid: String) -> StorefrontRestriction? {
        let restriction = restrictionsByUID[uid]
        if let restriction {
            restrictionsLogger.info("restriction for \(uid): mode=\(restriction.mode.rawValue) reason=\(restriction.reason)")
        }
        return restriction
    }

    /// Reads the bundled restrictions file and returns the entries that apply to the given storefront.
    private static func loadRestrictions(forStorefront storefront: String) -> [String: StorefrontRestriction] {
        guard let url = Bundle.module.url(forResource: "content-restrictions", withExtension: "json") else {
            restrictionsLogger.warning("content-restrictions.json not found in bundle")
            return [:]
        }
        do {
            let data = try Data(contentsOf: url)
            let list = try parseRestrictions(data: data)
            var map: [String: StorefrontRestriction] = [:]
            for restriction in list {
                if let entry = restriction.storefronts[storefront] {
                    map[restriction.uid] = entry
                }
            }
            restrictionsLogger.info("Loaded \(map.count) content restrictions for storefront '\(storefront)' (\(list.count) total entries)")
            return map
        } catch {
            restrictionsLogger.error("Failed to load content-restrictions.json: \(error)")
            return [:]
        }
    }
}

/// Parses a content-restrictions JSON payload into the underlying list of records.
public func parseRestrictions(data: Data) throws -> [ContentRestriction] {
    let list = try JSONDecoder().decode(ContentRestrictionList.self, from: data)
    return list.restrictions
}
