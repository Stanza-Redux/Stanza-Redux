// Copyright 2026 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import Foundation
import SwiftUI
import OSLog
import Observation

// MARK: - ErrorInfo

/// A protocol describing structured error information for user-facing error alerts.
///
/// Conforming types provide a title, message, optional underlying error, optional
/// help URL, and optional numeric code. The ``ErrorManager`` uses these properties
/// to present a well-formatted alert to the user.
public protocol ErrorInfo {
    /// A short title for the alert, or `nil` to use a default "Error" title.
    var title: String? { get }

    /// A human-readable description of the error shown in the alert body.
    var message: String? { get }

    /// The underlying `Error`, if any. Its `localizedDescription` is used when
    /// `message` is `nil`.
    var error: Error? { get }

    /// A URL to open when the user taps "Help", or `nil` for no help action.
    var helpURL: URL? { get }

    /// An optional numeric error code for identification and logging.
    var code: Int? { get }
}

/// Default concrete implementation of ``ErrorInfo``.
///
/// Create instances to describe errors for the ``ErrorManager``:
/// ```swift
/// errorManager.errorOccurred(info: AppErrorInfo(
///     title: "Import Failed",
///     message: "The file could not be read.",
///     error: underlyingError,
///     helpURL: URL(string: "https://example.com/help")
/// ))
/// ```
public struct AppErrorInfo: ErrorInfo {
    public let title: String?
    public let message: String?
    public let error: Error?
    public let helpURL: URL?
    public let code: Int?

    public init(
        title: String? = nil,
        message: String? = nil,
        error: Error? = nil,
        helpURL: URL? = nil,
        code: Int? = nil
    ) {
        self.title = title
        self.message = message
        self.error = error
        self.helpURL = helpURL
        self.code = code
    }
}

// MARK: - ErrorManager

/// An observable object that provides centralized error handling for the app.
///
/// Place a single instance in the SwiftUI environment and call
/// ``errorOccurred(info:)`` from any view or model object to present
/// a consistent error alert to the user.
///
/// ```swift
/// // In your root view:
/// @State var errorManager = ErrorManager()
///
/// var body: some View {
///     ContentView()
///         .environment(errorManager)
///         .withErrorManager(errorManager)
/// }
///
/// // Anywhere in the app:
/// @Environment(ErrorManager.self) var errorManager
/// errorManager.errorOccurred(info: AppErrorInfo(message: "Something went wrong"))
/// ```
///
/// The method is safe to call from any thread — it dispatches
/// to the main actor internally.
@Observable public class ErrorManager {
    /// The most recently reported error, or `nil` when dismissed.
    /// Setting this to a non-nil value triggers the alert presentation.
    public var lastErrorInfo: (any ErrorInfo)? = nil

    private let errorLogger = Logger(subsystem: "ErrorManager", category: "Errors")

    public init() {}

    /// Reports an error to the manager. The error is logged and an alert
    /// is presented to the user on the main thread.
    ///
    /// This method is safe to call from any thread.
    ///
    /// - Parameter info: Structured information about the error.
    public func errorOccurred(info: any ErrorInfo) {
        let description = info.message ?? info.error?.localizedDescription ?? "Unknown error"
        let title = info.title ?? "Error"
        let codeStr = info.code.map { " (code \($0))" } ?? ""
        errorLogger.error("[\(title)]\(codeStr) \(description)")

        if let error = info.error {
            errorLogger.error("Underlying error: \(error)")
        }

        Task { @MainActor in
            self.lastErrorInfo = info
        }
    }

    /// Dismisses the currently displayed error alert.
    @MainActor public func dismiss() {
        lastErrorInfo = nil
    }

    /// The display title for the current error.
    var displayTitle: String {
        lastErrorInfo?.title ?? "Error"
    }

    /// The display message for the current error.
    var displayMessage: String {
        if let message = lastErrorInfo?.message {
            return message
        }
        if let error = lastErrorInfo?.error {
            return error.localizedDescription
        }
        return "An unknown error occurred."
    }
}

// MARK: - View Extension

extension View {
    /// Attaches the error manager's alert presentation to this view.
    ///
    /// When the manager's ``ErrorManager/lastErrorInfo`` becomes non-nil,
    /// an alert is presented with:
    /// - **OK** button to dismiss
    /// - **Help** button (if a `helpURL` is provided) to open a help page
    ///
    /// The help URL is opened through the environment's `openURL` action,
    /// so it respects any custom URL handling (such as an in-app browser).
    ///
    /// Usage:
    /// ```swift
    /// @State var errorManager = ErrorManager()
    ///
    /// var body: some View {
    ///     MyContent()
    ///         .environment(errorManager)
    ///         .withErrorManager(errorManager)
    /// }
    /// ```
    public func withErrorManager(_ manager: ErrorManager) -> some View {
        self.modifier(ErrorManagerAlertModifier(manager: manager))
    }
}

private struct ErrorManagerAlertModifier: ViewModifier {
    @Bindable var manager: ErrorManager
    @Environment(\.openURL) private var openURL

    private var isPresented: Binding<Bool> {
        Binding(
            get: { manager.lastErrorInfo != nil },
            set: { if !$0 { manager.lastErrorInfo = nil } }
        )
    }

    func body(content: Content) -> some View {
        content.alert(manager.displayTitle, isPresented: isPresented) {
            Button("OK", role: .cancel) {
                manager.lastErrorInfo = nil
            }
            if let helpURL = manager.lastErrorInfo?.helpURL {
                Button("Help") {
                    openURL(helpURL)
                    manager.lastErrorInfo = nil
                }
            }
        } message: {
            Text(manager.displayMessage)
        }
    }
}
