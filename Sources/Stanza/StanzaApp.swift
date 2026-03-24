// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later
import Foundation
import OSLog
import SwiftUI
import StanzaModel

let logger: Logger = Logger(subsystem: "org.appfair.app.Stanza", category: "Stanza")

/// The Android SDK number we are running against, or `nil` if not running on Android
let androidSDK = ProcessInfo.processInfo.environment["android.os.Build.VERSION.SDK_INT"].flatMap({ Int($0) })

/// The shared app settings instance.
private let appSettings = StanzaSettings()

/// The shared library manager instance.
private let appLibraryManager = LibraryManager()

/// The shared error manager instance.
private let appErrorManager = ErrorManager()

/// The shared top-level view for the app, loaded from the platform-specific App delegates below.
///
/// The default implementation merely loads the `ContentView` for the app and logs a message.
public struct StanzaRootView : View {
    public init() {
    }

    public var body: some View {
        ContentView()
            .environment(appSettings)
            .environment(appLibraryManager)
            .environment(appErrorManager)
            .task {
                logger.info("Welcome to Stanza on \(androidSDK != nil ? "Android" : "Darwin")!")
            }
    }
}

/// Notification posted when an epub file is opened from an external source.
/// The `userInfo` dictionary contains `"url"` with the file `URL`.
public let openEpubNotification = Notification.Name("openEpubFile")

/// Global application delegate functions.
///
/// These functions can update a shared observable object to communicate app state changes to interested views.
public final class StanzaAppDelegate : Sendable {
    public static let shared = StanzaAppDelegate()

    private init() {
    }

    /// Called when the app is asked to open an epub file from an external source.
    public func openEpubFile(url: URL) {
        logger.info("openEpubFile: \(url.absoluteString)")
        NotificationCenter.default.post(name: openEpubNotification, object: nil, userInfo: ["url": url])
    }

    public func onInit() {
        logger.debug("onInit")
    }

    public func onLaunch() {
        logger.debug("onLaunch")
    }

    public func onResume() {
        logger.debug("onResume")
    }

    public func onPause() {
        logger.debug("onPause")
    }

    public func onStop() {
        logger.debug("onStop")
    }

    public func onDestroy() {
        logger.debug("onDestroy")
    }

    public func onLowMemory() {
        logger.debug("onLowMemory")
    }
}
