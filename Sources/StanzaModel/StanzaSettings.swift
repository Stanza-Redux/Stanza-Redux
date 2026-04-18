// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import Foundation
import OSLog
import Observation

let settingsLogger = Logger(subsystem: "Stanza", category: "Settings")

/// Observable settings model that persists all values to UserDefaults.
/// A single instance is created at app launch and shared via the environment.
@Observable public class StanzaSettings {
    private let defaults: UserDefaults

    // MARK: - General

    /// The appearance mode: "" = system, "light", "dark".
    public var appearance: String {
        didSet { defaults.set(appearance, forKey: "appearance"); settingsLogger.info("Appearance changed to: '\(self.appearance)'") }
    }

    // MARK: - Reading

    /// Whether to animate page turns.
    public var animatePageTurns: Bool {
        didSet { defaults.set(animatePageTurns, forKey: "animatePageTurns"); settingsLogger.info("Animate page turns: \(self.animatePageTurns)") }
    }

    /// Base text font size as a multiplier (1.0 = 100%).
    public var fontSize: Double {
        didSet { defaults.set(fontSize, forKey: "readerFontSize"); settingsLogger.info("Font size changed to: \(Int(self.fontSize * 100))%") }
    }

    /// Font family for reading: "" = publisher default.
    public var fontFamily: String {
        didSet { defaults.set(fontFamily, forKey: "epubFontFamily"); settingsLogger.info("Font family: '\(self.fontFamily)'") }
    }

    // MARK: - EPUB Preferences

    /// Number of columns: "" = auto, "1" = one, "2" = two.
    public var columnCount: String {
        didSet { defaults.set(columnCount, forKey: "epubColumnCount"); settingsLogger.info("Column count: '\(self.columnCount)'") }
    }

    /// Content fitting: "" = auto, "page", "width".
    public var fit: String {
        didSet { defaults.set(fit, forKey: "epubFit"); settingsLogger.info("Fit: '\(self.fit)'") }
    }

    /// Enable hyphenation. Empty string means unset (use default).
    public var hyphens: String {
        didSet { defaults.set(hyphens, forKey: "epubHyphens"); settingsLogger.info("Hyphens: '\(self.hyphens)'") }
    }

    /// Leading line height multiplier (0 means unset).
    public var lineHeight: Double {
        didSet { defaults.set(lineHeight, forKey: "epubLineHeight"); settingsLogger.info("Line height: \(self.lineHeight)") }
    }

    /// Factor applied to horizontal margins (0 means unset).
    public var pageMargins: Double {
        didSet { defaults.set(pageMargins, forKey: "epubPageMargins"); settingsLogger.info("Page margins: \(self.pageMargins)") }
    }

    /// Vertical margins for paragraphs (0 means unset).
    public var paragraphSpacing: Double {
        didSet { defaults.set(paragraphSpacing, forKey: "epubParagraphSpacing"); settingsLogger.info("Paragraph spacing: \(self.paragraphSpacing)") }
    }

    /// Whether to use publisher styles.  Empty string means unset.
    public var publisherStyles: String {
        didSet { defaults.set(publisherStyles, forKey: "epubPublisherStyles"); settingsLogger.info("Publisher styles: '\(self.publisherStyles)'") }
    }

    /// Text alignment: "" = default, "start", "end", "left", "right", "center", "justify".
    public var textAlign: String {
        didSet { defaults.set(textAlign, forKey: "epubTextAlign"); settingsLogger.info("Text align: '\(self.textAlign)'") }
    }

    /// Normalize text styles. Empty string means unset.
    public var textNormalization: String {
        didSet { defaults.set(textNormalization, forKey: "epubTextNormalization"); settingsLogger.info("Text normalization: '\(self.textNormalization)'") }
    }

    /// Space between words (0 means unset).
    public var wordSpacing: Double {
        didSet { defaults.set(wordSpacing, forKey: "epubWordSpacing"); settingsLogger.info("Word spacing: \(self.wordSpacing)") }
    }

    /// Space between characters/letters (0 means unset).
    public var letterSpacing: Double {
        didSet { defaults.set(letterSpacing, forKey: "epubLetterSpacing"); settingsLogger.info("Letter spacing: \(self.letterSpacing)") }
    }

    /// Whether to hide the system status bar when the reader is active and HUD is not shown.
    public var hideStatusBarInReader: Bool {
        didSet { defaults.set(hideStatusBarInReader, forKey: "hideStatusBarInReader"); settingsLogger.info("Hide status bar in reader: \(self.hideStatusBarInReader)") }
    }

    /// When enabled, tapping the left 1/3 of the reader advances forward instead of going back.
    public var leftTapAdvances: Bool {
        didSet { defaults.set(leftTapAdvances, forKey: "leftTapAdvances"); settingsLogger.info("Left tap advances: \(self.leftTapAdvances)") }
    }

    /// When enabled, the reader uses a sepia theme, overriding the light/dark appearance setting.
    public var sepiaTheme: Bool {
        didSet { defaults.set(sepiaTheme, forKey: "sepiaTheme"); settingsLogger.info("Sepia theme: \(self.sepiaTheme)") }
    }

    /// Reading theme: "original", "quiet", "paper", "bold", "calm", "focus".
    public var readingTheme: String {
        didSet { defaults.set(readingTheme, forKey: "readingTheme"); settingsLogger.info("Reading theme: '\(self.readingTheme)'") }
    }

    /// When enabled, web links open in an embedded in-app browser instead of the system browser.
    public var useInAppBrowser: Bool {
        didSet { defaults.set(useInAppBrowser, forKey: "useInAppBrowser"); settingsLogger.info("Use in-app browser: \(self.useInAppBrowser)") }
    }

    // MARK: - Text-to-Speech

    /// When enabled, the currently spoken utterance is highlighted in the reader.
    public var ttsHighlightUtterance: Bool {
        didSet { defaults.set(ttsHighlightUtterance, forKey: "ttsHighlightUtterance"); settingsLogger.info("TTS highlight utterance: \(self.ttsHighlightUtterance)") }
    }

    /// When enabled, pages turn automatically to follow the spoken text.
    public var ttsAutoTurnPages: Bool {
        didSet { defaults.set(ttsAutoTurnPages, forKey: "ttsAutoTurnPages"); settingsLogger.info("TTS auto turn pages: \(self.ttsAutoTurnPages)") }
    }

    // MARK: - Book Restoration

    /// The book ID that is currently open in the reader, or 0 if none.
    /// Persisted so we can re-open the book on next launch.
    public var lastOpenBookID: Int64 {
        didSet { defaults.set(Int(lastOpenBookID), forKey: "lastOpenBookID") }
    }

    // MARK: - Init

    public init(defaults: UserDefaults = UserDefaults.standard) {
        self.defaults = defaults

        // General
        self.appearance = defaults.string(forKey: "appearance") ?? ""
        self.hideStatusBarInReader = defaults.object(forKey: "hideStatusBarInReader") != nil ? defaults.bool(forKey: "hideStatusBarInReader") : true
        self.leftTapAdvances = defaults.bool(forKey: "leftTapAdvances")
        self.sepiaTheme = defaults.bool(forKey: "sepiaTheme")
        self.readingTheme = defaults.string(forKey: "readingTheme") ?? "original"
        self.useInAppBrowser = defaults.object(forKey: "useInAppBrowser") != nil ? defaults.bool(forKey: "useInAppBrowser") : true
        self.ttsHighlightUtterance = defaults.object(forKey: "ttsHighlightUtterance") != nil ? defaults.bool(forKey: "ttsHighlightUtterance") : true
        self.ttsAutoTurnPages = defaults.object(forKey: "ttsAutoTurnPages") != nil ? defaults.bool(forKey: "ttsAutoTurnPages") : true
        self.animatePageTurns = defaults.object(forKey: "animatePageTurns") != nil ? defaults.bool(forKey: "animatePageTurns") : true
        self.fontSize = defaults.object(forKey: "readerFontSize") != nil ? defaults.double(forKey: "readerFontSize") : 1.0
        self.fontFamily = defaults.string(forKey: "epubFontFamily") ?? ""
        self.lastOpenBookID = Int64(defaults.integer(forKey: "lastOpenBookID"))

        // EPUB preferences
        self.columnCount = defaults.string(forKey: "epubColumnCount") ?? ""
        self.fit = defaults.string(forKey: "epubFit") ?? ""
        self.hyphens = defaults.string(forKey: "epubHyphens") ?? ""
        self.lineHeight = defaults.double(forKey: "epubLineHeight")
        self.pageMargins = defaults.double(forKey: "epubPageMargins")
        self.paragraphSpacing = defaults.double(forKey: "epubParagraphSpacing")
        self.publisherStyles = defaults.string(forKey: "epubPublisherStyles") ?? ""
        self.textAlign = defaults.string(forKey: "epubTextAlign") ?? ""
        self.textNormalization = defaults.string(forKey: "epubTextNormalization") ?? ""
        self.wordSpacing = defaults.double(forKey: "epubWordSpacing")
        self.letterSpacing = defaults.double(forKey: "epubLetterSpacing")

        settingsLogger.info("Settings loaded: fontSize=\(self.fontSize), animatePageTurns=\(self.animatePageTurns)")
    }

    /// Resets all EPUB reading preferences to their defaults.
    public func resetReadingPreferences() {
        settingsLogger.info("Resetting all reading preferences to defaults")
        fontSize = 1.0
        fontFamily = ""
        readingTheme = "original"
        columnCount = ""
        fit = ""
        hyphens = ""
        lineHeight = 0.0
        pageMargins = 0.0
        paragraphSpacing = 0.0
        publisherStyles = ""
        textAlign = ""
        textNormalization = ""
        wordSpacing = 0.0
        letterSpacing = 0.0
    }
}
