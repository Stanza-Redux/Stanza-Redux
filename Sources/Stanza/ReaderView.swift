// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel
import SkipKit
#if !SKIP
import ReadiumNavigator
import ReadiumShared
import UIKit
#else
import android.content.Context
import android.content.ContextWrapper
import android.content.ContentResolver
import android.view.WindowInsets

import androidx.fragment.app.FragmentActivity
import androidx.fragment.app.FragmentFactory
import androidx.fragment.compose.AndroidFragment
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView

import org.readium.r2.navigator.epub.EpubDefaults
import org.readium.r2.navigator.epub.EpubNavigatorFactory
import org.readium.r2.navigator.epub.EpubNavigatorFragment
import org.readium.r2.navigator.input.InputListener
import org.readium.r2.navigator.input.TapEvent
import org.readium.r2.navigator.input.DragEvent
import org.readium.r2.navigator.input.KeyEvent
import org.readium.r2.shared.publication.services.cover
import org.readium.r2.navigator.preferences.Theme
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
#endif

#if !SKIP
typealias PlatformNavigator = EPUBNavigatorViewController
typealias PlatformPreferences = EPUBPreferences
typealias PlatformDefaults = ReadiumNavigator.EPUBDefaults
#else
typealias PlatformNavigator = org.readium.r2.navigator.epub.EpubNavigatorFragment
typealias PlatformPreferences = org.readium.r2.navigator.epub.EpubPreferences
typealias PlatformDefaults = org.readium.r2.navigator.epub.EpubDefaults
#endif

let defaults = PlatformDefaults(columnCount: nil, fontSize: nil, fontWeight: nil, hyphens: nil, imageFilter: nil, language: nil, letterSpacing: nil, ligatures: nil, lineHeight: nil, pageMargins: nil, paragraphIndent: nil, paragraphSpacing: nil, publisherStyles: nil, readingProgression: nil, scroll: nil, spread: nil, textAlign: nil, textNormalization: nil, typeScale: nil, wordSpacing: nil)

#if !SKIP
var navConfig: EPUBNavigatorViewController.Configuration = EPUBNavigatorViewController.Configuration(defaults: defaults, fontFamilyDeclarations: FontManager.fontFamilyDeclarations)
#else
var navConfig: org.readium.r2.navigator.epub.EpubNavigatorFactory.Configuration = EpubNavigatorFactory.Configuration(defaults: defaults)
#endif



struct ReaderView: View {
    let bookID: Int64
    let filePath: String
    let database: BookDatabase?
    @State var viewModel: ReaderViewModel? = nil
    @State var error: Error? = nil
    @State var locator: Loc? = nil
    @State var initialLocator: Loc? = nil
    @State var hasRestoredPosition: Bool = false
    @State var showHUD: Bool = false
    @State var showTOC: Bool = false
    @State var showBookDetail: Bool = false
    @State var showExtendedHUD: Bool = false
    @State var chapterPageIndex: Int = 0
    @State var chapterPageCount: Int = 0
    @State var positionCountsByChapter: [Int] = []
    @State var bookmarks: [BookmarkRecord] = []
    @State var isCurrentPageBookmarked: Bool = false
    @Environment(StanzaSettings.self) var settings: StanzaSettings
    @Environment(\.colorScheme) var colorScheme
    @State var initialPrefsApplied: Bool = false
    @Environment(\.dismiss) var dismiss

    @State var navigator: PlatformNavigator? = nil

    #if !SKIP
    @State var navigatorDelegate: ReaderLocationDelegate? = nil
    #endif
    #if SKIP
    @State var inputListenerAdded: Bool = false
    #endif

    // MARK: - Text-to-Speech State
    @State var isSpeaking: Bool = false
    @State var wasSpeakingBeforeHUD: Bool = false
    #if !SKIP
    @State var speechSynthesizer: PublicationSpeechSynthesizer? = nil
    @State var ttsDelegate: TTSSynthesizerDelegate? = nil
    #else
    @State var androidTts: TextToSpeech? = nil
    #endif

    var body: some View {
        readerBodyWithPreferenceHandlers
        .onChange(of: settings.appearance) { applyPreferences() }
        .onChange(of: settings.sepiaTheme) { applyPreferences() }
        .onChange(of: colorScheme) { applyPreferences() }
        .onChange(of: showHUD) { handleHUDChange() }
    }

    /// Splits the onChange chain to help the compiler type-check.
    private var readerBodyWithPreferenceHandlers: some View {
        readerBody
        .onChange(of: settings.fontSize) { applyPreferences() }
        .onChange(of: settings.fontFamily) { applyPreferences() }
        .onChange(of: settings.columnCount) { applyPreferences() }
        .onChange(of: settings.fit) { applyPreferences() }
        .onChange(of: settings.hyphens) { applyPreferences() }
        .onChange(of: settings.lineHeight) { applyPreferences() }
        .onChange(of: settings.pageMargins) { applyPreferences() }
        .onChange(of: settings.paragraphSpacing) { applyPreferences() }
        .onChange(of: settings.publisherStyles) { applyPreferences() }
        .onChange(of: settings.textAlign) { applyPreferences() }
        .onChange(of: settings.textNormalization) { applyPreferences() }
        .onChange(of: settings.wordSpacing) { applyPreferences() }
        .onChange(of: settings.letterSpacing) { applyPreferences() }
    }

    private func handleHUDChange() {
        // Pause TTS when HUD is shown, resume when hidden
        if showHUD && isSpeaking {
            wasSpeakingBeforeHUD = true
            pauseSpeaking()
        } else if !showHUD && wasSpeakingBeforeHUD {
            wasSpeakingBeforeHUD = false
            resumeSpeaking()
        }
        #if SKIP
        updateAndroidStatusBar()
        #endif
    }

    var readerBody: some View {
        Group {
            if let publication = viewModel?.publication {
                readerViewContainer(publication: publication)
                    .overlay {
                        hudOverlay(publication: publication)
                    }
                    .sheet(isPresented: $showTOC) {
                        tocSheet(publication: publication)
                    }
                    .sheet(isPresented: $showBookDetail) {
                        NavigationStack {
                            BookDetailView(bookID: bookID, database: database)
                        }
                    }
            } else if let error = error {
                VStack {
                    Text("Error: \(String(describing: error))")
                        .accessibilityIdentifier("readerErrorMessage")
                    Button("Dismiss") { dismiss() }
                        .accessibilityIdentifier("readerErrorDismissButton")
                }
            } else {
                ProgressView("Loading...")
                    .accessibilityIdentifier("readerLoadingIndicator")
            }
        }
        .preferredColorScheme(settings.sepiaTheme ? .light : settings.appearance == "dark" ? .dark : settings.appearance == "light" ? .light : nil)
        .background(settings.sepiaTheme ? Color(red: 250.0/255.0, green: 244.0/255.0, blue: 232.0/255.0) : colorScheme == .dark ? Color.black : Color.white)
        #if !SKIP // unavailable in Skip
        .statusBarHidden(settings.hideStatusBarInReader && !showHUD)
        // this hides the bottom horizontal bar on iOS successfully:
        // “The Home indicator doesn’t appear without specific user intent when you set visibility to hidden.”
        // https://developer.apple.com/documentation/swiftui/view/persistentsystemoverlays(_:)#discussion
        // however, it re-apppears briefly (and then fades out again) whenever you tap the next the previous page zone, which is even *more* distracting than just having it always present
        //.persistentSystemOverlays(settings.hideStatusBarInReader && !showHUD ? .hidden : .automatic)
        #endif
        .task {
            settings.lastOpenBookID = bookID
            await loadBook()
        }
        .onDisappear {
            if isSpeaking { stopSpeaking() }
            saveCurrentLocator()
            settings.lastOpenBookID = 0
        }
    }

    func loadBook() async {
        logger.info("Opening book id=\(bookID) from \(filePath)")
        do {
            // Load saved locator from database
            if let db = database, let record = try? db.book(id: bookID),
               let json = record.locatorJSON {
                let savedLoc = Loc.fromJSON(json)
                self.locator = savedLoc
                self.initialLocator = savedLoc
                logger.debug("Restored reading position: progress=\(savedLoc?.totalProgression ?? 0.0)")
            }

            let bookURL = URL(fileURLWithPath: filePath)
            let publication = try await Pub.loadPublication(from: bookURL)
            logger.info("Publication loaded: '\(publication.metadata.title ?? "Unknown")' with \(publication.manifest.readingOrder.count) chapters")
            self.viewModel = ReaderViewModel(publication: publication)
            #if !SKIP
            self.navigator = try EPUBNavigatorViewController(publication: publication.platformValue, initialLocation: locator?.platformValue, config: navConfig)
            // Load position counts per chapter for accurate page estimation
            if case .success(let positionsByChapter) = await publication.platformValue.positionsByReadingOrder() {
                self.positionCountsByChapter = positionsByChapter.map { $0.count }
            }
            let delegate = ReaderLocationDelegate { loc in
                self.locator = loc
                self.updateChapterPageInfo(loc: loc)
                self.persistLocator(loc)
            }
            delegate.onTap = { point, viewSize in
                self.handleTap(x: Double(point.x), width: Double(viewSize.width))
            }
            self.navigatorDelegate = delegate
            self.navigator?.delegate = delegate
            applyPreferences()
            #endif
            if let db = database {
                try? db.markOpened(bookID: bookID)
                self.bookmarks = (try? db.bookmarks(forBookID: bookID)) ?? []
            }
        } catch {
            logger.error("Failed to open book id=\(bookID): \(error)")
            self.error = error
        }
    }

    func persistLocator(_ loc: Loc) {
        guard let db = database else { return }
        guard let json = loc.jsonString else { return }
        let progress = loc.totalProgression ?? 0.0
        logger.debug("Persisting reading position for book id=\(bookID): progress=\(progress)")
        try? db.saveReadingPosition(bookID: bookID, locatorJSON: json, progress: progress)
        updateBookmarkState()
    }

    func saveCurrentLocator() {
        logger.info("Saving current locator for book id=\(bookID)")
        #if !SKIP
        if let nav = navigator, let platformLoc = nav.currentLocation {
            let loc = Loc(platformValue: platformLoc)
            persistLocator(loc)
        }
        #endif
        if let loc = locator {
            persistLocator(loc)
        }
    }

    // MARK: - Tap Handling

    func handleTap(x: Double, width: Double) {
        let third = width / 3.0
        if showHUD {
            showHUD = false
            showExtendedHUD = false
        } else if x < third {
            if settings.leftTapAdvances { goForward() } else { goBackward() }
        } else if x > third * 2.0 {
            goForward()
        } else {
            showHUD = true
        }
    }

    // MARK: - Navigation

    func goForward() {
        let animated = settings.animatePageTurns
        if let nav = navigator {
            #if !SKIP
            Task { await nav.goForward(options: animated ? .animated : .none) }
            #else
            Task { nav.goForward(animated) }
            #endif
        }
    }

    func goBackward() {
        let animated = settings.animatePageTurns
        if let nav = navigator {
            #if !SKIP
            Task { await nav.goBackward(options: animated ? .animated : .none) }
            #else
            Task { nav.goBackward(animated) }
            #endif
        }
    }

    func navigateToTOCEntry(_ link: Lnk) {
        logger.info("Navigating to TOC entry: '\(link.title ?? "unknown")' href=\(link.href)")
        let animated = settings.animatePageTurns
        if let nav = navigator {
            #if !SKIP
            Task { await nav.go(to: link.platformValue, options: animated ? .animated : .none) }
            #else
            // go(Link, boolean) is synchronous and must run on the main thread
            // because it manipulates the ViewPager directly.
            nav.go(link.platformValue, animated)
            #endif
        }
        showTOC = false
        showHUD = false
    }

    // MARK: - Text-to-Speech

    func startSpeaking() {
        logger.info("Starting TTS")
        #if !SKIP
        guard let publication = viewModel?.publication else { return }
        if speechSynthesizer == nil {
            guard PublicationSpeechSynthesizer.canSpeak(publication: publication.platformValue) else {
                logger.warning("TTS not available for this publication")
                return
            }
            let synth = PublicationSpeechSynthesizer(publication: publication.platformValue)
            let delegate = TTSSynthesizerDelegate()
            delegate.navigator = navigator
            delegate.settings = settings
            delegate.onStopped = { [self] in
                self.isSpeaking = false
            }
            synth?.delegate = delegate
            self.ttsDelegate = delegate
            self.speechSynthesizer = synth
        }
        if let loc = locator {
            speechSynthesizer?.start(from: loc.platformValue)
        } else {
            speechSynthesizer?.start()
        }
        isSpeaking = true
        #else
        guard let context = ProcessInfo.processInfo.androidContext else { return }
        let nav = navigator
        let currentSettings = settings
        // Callback to continue speaking after a page turn
        let onPageFinished: () -> Void = {
            // Wait briefly for the new page to load, then speak it
            android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                if let tts = self.androidTts, self.isSpeaking {
                    Self.speakCurrentPage(tts: tts, navigator: self.navigator, settings: currentSettings, onPageFinished: nil)
                }
            }, 1500) // 1.5s delay for page load
        }
        if let existingTts = androidTts {
            Self.speakCurrentPage(tts: existingTts, navigator: nav, settings: currentSettings, onPageFinished: onPageFinished)
        } else {
            var newTts: TextToSpeech? = nil
            newTts = TextToSpeech(context) { status in
                if status == TextToSpeech.SUCCESS {
                    logger.info("Android TTS initialized successfully")
                    if let readyTts = newTts {
                        Self.speakCurrentPage(tts: readyTts, navigator: nav, settings: currentSettings, onPageFinished: onPageFinished)
                    }
                } else {
                    logger.error("Android TTS initialization failed: \(status)")
                }
            }
            self.androidTts = newTts
        }
        isSpeaking = true
        #endif
    }

    func stopSpeaking() {
        logger.info("Stopping TTS")
        #if !SKIP
        speechSynthesizer?.stop()
        speechSynthesizer = nil
        ttsDelegate = nil
        #else
        androidTts?.stop()
        androidTts?.shutdown()
        androidTts = nil
        // Clear any TTS highlight from the WebView
        if let nav = navigator, let fragmentView = nav.view, let webView = Self.findWebView(in: fragmentView) {
            android.os.Handler(android.os.Looper.getMainLooper()).post {
                webView.evaluateJavascript("""
                (function() {
                    var prev = document.querySelector('.stanza-tts-highlight');
                    if (prev) {
                        var parent = prev.parentNode;
                        while (prev.firstChild) parent.insertBefore(prev.firstChild, prev);
                        parent.removeChild(prev);
                    }
                })();
                """, nil)
            }
        }
        #endif
        isSpeaking = false
    }

    func pauseSpeaking() {
        #if !SKIP
        speechSynthesizer?.pause()
        #else
        androidTts?.stop()
        #endif
    }

    func resumeSpeaking() {
        #if !SKIP
        speechSynthesizer?.resume()
        #else
        // Re-extract and speak text on resume
        if let tts = androidTts {
            Self.speakCurrentPage(tts: tts, navigator: navigator, settings: settings)
        }
        #endif
    }

    #if SKIP
    /// Finds the WebView inside the navigator fragment's view hierarchy.
    private static func findWebView(in view: android.view.View) -> android.webkit.WebView? {
        if let wv = view as? android.webkit.WebView { return wv }
        if let vg = view as? android.view.ViewGroup {
            for i in 0..<vg.childCount {
                if let found = findWebView(in: vg.getChildAt(i)) { return found }
            }
        }
        return nil
    }

    /// Cleans a JSON-encoded string result from evaluateJavascript.
    private static func cleanJSResult(_ text: String) -> String {
        var s = text
        if s.hasPrefix("\"") && s.hasSuffix("\"") {
            s = String(s.dropFirst().dropLast())
        }
        s = s.replacingOccurrences(of: "\\n", with: "\n")
        s = s.replacingOccurrences(of: "\\\"", with: "\"")
        s = s.replacingOccurrences(of: "\\\\", with: "\\")
        return s.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Splits text into sentence-like utterances for TTS.
    private static func splitIntoUtterances(_ text: String) -> [String] {
        // Split on sentence-ending punctuation followed by whitespace
        var utterances: [String] = []
        var current = ""
        for char in text {
            current += String(char)
            if (char == "." || char == "!" || char == "?" || char == "\n") && !current.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                utterances.append(current.trimmingCharacters(in: .whitespacesAndNewlines))
                current = ""
            }
        }
        if !current.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            utterances.append(current.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        return utterances
    }

    /// Extracts visible text from the navigator's WebView, splits into utterances,
    /// and speaks them with highlighting and auto-page-turning support.
    private static func speakCurrentPage(tts: TextToSpeech, navigator: PlatformNavigator?, settings: StanzaSettings? = nil, onPageFinished: (() -> Void)? = nil) {
        guard let nav = navigator, let fragmentView = nav.view else {
            logger.warning("TTS: no navigator view available")
            return
        }
        guard let webView = findWebView(in: fragmentView) else {
            logger.warning("TTS: could not find WebView in navigator")
            return
        }

        let highlightEnabled = settings?.ttsHighlightUtterance ?? true
        let autoTurnPages = settings?.ttsAutoTurnPages ?? true

        android.os.Handler(android.os.Looper.getMainLooper()).post {
            webView.evaluateJavascript("(document.body ? document.body.innerText : '')") { result in
                guard let rawText = result, !rawText.isEmpty else {
                    logger.warning("TTS: evaluateJavascript returned empty result")
                    return
                }
                let cleanText = cleanJSResult(rawText)
                guard !cleanText.isEmpty else {
                    logger.warning("TTS: extracted text was empty after cleaning")
                    return
                }

                let utterances = splitIntoUtterances(cleanText)
                logger.info("TTS: speaking \(utterances.count) utterances (\(cleanText.count) chars)")

                // Inject highlight CSS if not already present
                if highlightEnabled {
                    webView.evaluateJavascript("""
                    (function() {
                        if (!document.getElementById('stanza-tts-style')) {
                            var style = document.createElement('style');
                            style.id = 'stanza-tts-style';
                            style.textContent = '.stanza-tts-highlight { background-color: rgba(255, 200, 0, 0.35); border-radius: 2px; }';
                            document.head.appendChild(style);
                        }
                    })();
                    """, nil)
                }

                // Set up utterance progress listener for highlighting and auto-page-turn
                let listener = TTSUtteranceListener(
                    utterances: utterances,
                    webView: webView,
                    highlightEnabled: highlightEnabled,
                    autoTurnPages: autoTurnPages,
                    navigator: nav,
                    onPageFinished: onPageFinished
                )
                tts.setOnUtteranceProgressListener(listener)

                // Queue each utterance with a unique ID
                tts.speak("", TextToSpeech.QUEUE_FLUSH, nil, nil) // Clear queue
                for i in 0..<utterances.count {
                    tts.speak(utterances[i], TextToSpeech.QUEUE_ADD, nil, "utt_\(i)")
                }
            }
        }
    }

    /// Listener that highlights the current utterance and auto-advances pages.
    private class TTSUtteranceListener: UtteranceProgressListener {
        let utterances: [String]
        let webView: android.webkit.WebView
        let highlightEnabled: Bool
        let autoTurnPages: Bool
        let navigator: PlatformNavigator?
        let onPageFinished: (() -> Void)?

        init(utterances: [String], webView: android.webkit.WebView, highlightEnabled: Bool, autoTurnPages: Bool, navigator: PlatformNavigator?, onPageFinished: (() -> Void)?) {
            self.utterances = utterances
            self.webView = webView
            self.highlightEnabled = highlightEnabled
            self.autoTurnPages = autoTurnPages
            self.navigator = navigator
            self.onPageFinished = onPageFinished
        }

        override func onStart(_ utteranceId: String?) {
            guard highlightEnabled else { return }
            guard let uttId = utteranceId, uttId.hasPrefix("utt_") else { return }
            let indexStr = String(uttId.dropFirst(4))
            guard let index = Int(indexStr), index < utterances.count else { return }

            let utteranceText = utterances[index]
            // Escape the text for use in a JavaScript string
            let escaped = utteranceText
                .replacingOccurrences(of: "\\", with: "\\\\")
                .replacingOccurrences(of: "'", with: "\\'")
                .replacingOccurrences(of: "\n", with: "\\n")

            // Highlight the utterance text in the WebView using find-and-mark
            android.os.Handler(android.os.Looper.getMainLooper()).post {
                self.webView.evaluateJavascript("""
                (function() {
                    // Remove previous highlight
                    var prev = document.querySelector('.stanza-tts-highlight');
                    if (prev) {
                        var parent = prev.parentNode;
                        while (prev.firstChild) parent.insertBefore(prev.firstChild, prev);
                        parent.removeChild(prev);
                    }
                    // Find and highlight the utterance text
                    var text = '\(escaped)';
                    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null, false);
                    while (walker.nextNode()) {
                        var node = walker.currentNode;
                        var idx = node.textContent.indexOf(text.substring(0, Math.min(40, text.length)));
                        if (idx >= 0) {
                            var range = document.createRange();
                            range.setStart(node, idx);
                            range.setEnd(node, Math.min(idx + text.length, node.textContent.length));
                            var span = document.createElement('span');
                            span.className = 'stanza-tts-highlight';
                            range.surroundContents(span);
                            span.scrollIntoView({behavior: 'smooth', block: 'center'});
                            break;
                        }
                    }
                })();
                """, nil)
            }
        }

        override func onDone(_ utteranceId: String?) {
            guard let uttId = utteranceId, uttId.hasPrefix("utt_") else { return }
            let indexStr = String(uttId.dropFirst(4))
            guard let index = Int(indexStr) else { return }

            // If this was the last utterance on the page, auto-advance
            if index >= utterances.count - 1 && autoTurnPages {
                android.os.Handler(android.os.Looper.getMainLooper()).post {
                    // Clear highlight
                    self.webView.evaluateJavascript("""
                    (function() {
                        var prev = document.querySelector('.stanza-tts-highlight');
                        if (prev) {
                            var parent = prev.parentNode;
                            while (prev.firstChild) parent.insertBefore(prev.firstChild, prev);
                            parent.removeChild(prev);
                        }
                    })();
                    """, nil)
                    // Advance to next page
                    self.navigator?.goForward(true)
                    // Notify that page finished so caller can continue speaking
                    self.onPageFinished?()
                }
            }
        }

        override func onError(_ utteranceId: String?) {
            logger.error("TTS utterance error: \(utteranceId ?? "nil")")
        }
    }
    #endif

    // MARK: - Preferences

    /// Returns `true` if any reading preference differs from its default value.
    func hasNonDefaultPreferences() -> Bool {
        let s = settings
        return s.fontSize != 1.0
            || !s.fontFamily.isEmpty
            || !s.columnCount.isEmpty
            || !s.fit.isEmpty
            || !s.hyphens.isEmpty
            || s.lineHeight > 0.0
            || s.pageMargins > 0.0
            || s.paragraphSpacing > 0.0
            || !s.publisherStyles.isEmpty
            || !s.textAlign.isEmpty
            || !s.textNormalization.isEmpty
            || s.wordSpacing > 0.0
    }

    func adjustFontSize(increase: Bool) {
        if increase {
            settings.fontSize = min(settings.fontSize + 0.1, 3.0)
        } else {
            settings.fontSize = max(settings.fontSize - 0.1, 0.5)
        }
        logger.info("Reader font size changed to: \(Int(settings.fontSize * 100))%")
        applyPreferences()
    }

    /// Determines whether the effective appearance is dark.
    func effectiveIsDark() -> Bool {
        if settings.appearance == "dark" { return true }
        if settings.appearance == "light" { return false }
        return colorScheme == .dark
    }

    func applyPreferences() {
        let s = settings
        let isDark = effectiveIsDark()

        // Map string settings to typed optionals
        let hyphensVal: Bool? = s.hyphens == "true" ? true : s.hyphens == "false" ? false : nil
        let lineHeightVal: Double? = s.lineHeight > 0.0 ? s.lineHeight : nil
        let pageMarginsVal: Double? = s.pageMargins > 0.0 ? s.pageMargins : nil
        let paragraphSpacingVal: Double? = s.paragraphSpacing > 0.0 ? s.paragraphSpacing : nil
        let textNormalizationVal: Bool? = s.textNormalization == "true" ? true : s.textNormalization == "false" ? false : nil
        let wordSpacingVal: Double? = s.wordSpacing > 0.0 ? s.wordSpacing : nil
        let letterSpacingVal: Double? = s.letterSpacing > 0.0 ? s.letterSpacing : nil

        // Readium's CSS requires publisherStyles=false (readium-advanced-on) for user
        // overrides of spacing and text alignment to take effect.
        // When any such value is set, force publisherStyles off in the preferences.
        let hasUserOverride = lineHeightVal != nil || letterSpacingVal != nil || wordSpacingVal != nil || !s.textAlign.isEmpty
        let publisherStylesVal: Bool? = hasUserOverride ? false : (s.publisherStyles == "true" ? true : s.publisherStyles == "false" ? false : nil)

        #if !SKIP
        if let nav = navigator {
            let fontFamilyVal: ReadiumNavigator.FontFamily? = s.fontFamily.isEmpty ? nil : ReadiumNavigator.FontFamily(rawValue: s.fontFamily)
            let columnCountVal = s.columnCount.isEmpty ? nil : ReadiumNavigator.ColumnCount(rawValue: s.columnCount)
            let fitVal = s.fit.isEmpty ? nil : ReadiumNavigator.Fit(rawValue: s.fit)
            let textAlignVal = s.textAlign.isEmpty ? nil : ReadiumNavigator.TextAlignment(rawValue: s.textAlign)
            let themeVal: ReadiumNavigator.Theme = s.sepiaTheme ? .sepia : isDark ? .dark : .light
            let prefs: PlatformPreferences = EPUBPreferences(
                columnCount: columnCountVal,
                fit: fitVal,
                fontFamily: fontFamilyVal,
                fontSize: s.fontSize,
                hyphens: hyphensVal,
                letterSpacing: letterSpacingVal,
                lineHeight: lineHeightVal,
                pageMargins: pageMarginsVal,
                paragraphSpacing: paragraphSpacingVal,
                publisherStyles: publisherStylesVal,
                textAlign: textAlignVal,
                textNormalization: textNormalizationVal,
                theme: themeVal,
                wordSpacing: wordSpacingVal
            )
            nav.submitPreferences(prefs)
        }
        #else
        if let nav = navigator {
            let fontFamilyVal: org.readium.r2.navigator.preferences.FontFamily? = s.fontFamily.isEmpty ? nil : org.readium.r2.navigator.preferences.FontFamily(s.fontFamily)
            let textAlignVal: org.readium.r2.navigator.preferences.TextAlign? = s.textAlign == "justify" ? org.readium.r2.navigator.preferences.TextAlign.JUSTIFY : s.textAlign == "center" ? org.readium.r2.navigator.preferences.TextAlign.CENTER : s.textAlign == "left" ? org.readium.r2.navigator.preferences.TextAlign.LEFT : s.textAlign == "right" ? org.readium.r2.navigator.preferences.TextAlign.RIGHT : s.textAlign == "start" ? org.readium.r2.navigator.preferences.TextAlign.START : s.textAlign == "end" ? org.readium.r2.navigator.preferences.TextAlign.END : nil
            let themeVal: org.readium.r2.navigator.preferences.Theme = s.sepiaTheme ? Theme.SEPIA : isDark ? Theme.DARK : Theme.LIGHT
            let prefs: PlatformPreferences = org.readium.r2.navigator.epub.EpubPreferences(
                fontFamily: fontFamilyVal,
                fontSize: s.fontSize,
                hyphens: hyphensVal,
                letterSpacing: letterSpacingVal,
                lineHeight: lineHeightVal,
                pageMargins: pageMarginsVal,
                paragraphSpacing: paragraphSpacingVal,
                publisherStyles: publisherStylesVal,
                textAlign: textAlignVal,
                textNormalization: textNormalizationVal,
                theme: themeVal,
                wordSpacing: wordSpacingVal
            )
            nav.submitPreferences(prefs)
        }
        #endif
    }

    // MARK: - Status Bar (Android)

    #if SKIP
    func updateAndroidStatusBar() {
        guard settings.hideStatusBarInReader else { return }
        if let activity = UIApplication.shared.androidActivity {
            let controller = activity.window.insetsController
            if showHUD {
                controller?.show(android.view.WindowInsets.Type.statusBars())
            } else {
                controller?.hide(android.view.WindowInsets.Type.statusBars())
            }
        }
    }
    #endif

    // MARK: - Bookmarks

    func refreshBookmarks() {
        guard let db = database else { return }
        do {
            self.bookmarks = try db.bookmarks(forBookID: bookID)
            updateBookmarkState()
        } catch {
            logger.error("Failed to refresh bookmarks: \(error)")
        }
    }

    func updateBookmarkState() {
        guard let loc = locator, let json = loc.jsonString else {
            isCurrentPageBookmarked = false
            return
        }
        // Check if current locator matches any bookmark by comparing locator JSON
        var found = false
        for bookmark in bookmarks {
            if bookmark.locatorJSON == json {
                found = true
            }
        }
        isCurrentPageBookmarked = found
    }

    func toggleBookmark() {
        guard let db = database else { return }
        guard let loc = locator, let json = loc.jsonString else {
            logger.warning("Cannot bookmark: no current locator")
            return
        }

        if isCurrentPageBookmarked {
            // Remove the bookmark matching the current locator
            for bookmark in bookmarks {
                if bookmark.locatorJSON == json {
                    logger.info("Removing bookmark id=\(bookmark.id)")
                    do {
                        try db.deleteBookmark(id: bookmark.id)
                    } catch {
                        logger.error("Failed to delete bookmark: \(error)")
                    }
                }
            }
        } else {
            // Add a new bookmark
            let prog = loc.totalProgression ?? 0.0
            let progressLabel = "\(Int(prog * 100))%"
            let chapter = loc.title ?? ""
            var excerpt = ""
            if let highlight = loc.textHighlight {
                excerpt = highlight
            } else if let before = loc.textBefore {
                excerpt = before
            }
            // Truncate excerpt to 200 characters
            if excerpt.count > 200 {
                excerpt = String(excerpt.prefix(200))
            }
            let sortOrder = Int64(bookmarks.count)
            let record = BookmarkRecord(
                bookID: bookID,
                locatorJSON: json,
                progressLabel: progressLabel,
                excerpt: excerpt,
                chapter: chapter,
                sortOrder: sortOrder
            )
            logger.info("Adding bookmark at \(progressLabel) chapter='\(chapter)'")
            do {
                try db.addBookmark(record)
            } catch {
                logger.error("Failed to add bookmark: \(error)")
            }
        }
        refreshBookmarks()
    }

    // MARK: - Share

    func shareBook() {
        let bookURL = URL(fileURLWithPath: filePath)
        #if !SKIP
        let activityVC = UIActivityViewController(activityItems: [bookURL], applicationActivities: nil)
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
           let rootVC = windowScene.windows.first?.rootViewController {
            var topVC = rootVC
            while let presented = topVC.presentedViewController {
                topVC = presented
            }
            topVC.present(activityVC, animated: true)
        }
        #else
        guard let context = ProcessInfo.processInfo.androidContext else { return }
        let file = java.io.File(filePath)
        let authority = context.getPackageName() + ".fileprovider"
        let uri = androidx.core.content.FileProvider.getUriForFile(context, authority, file)
        let intent = android.content.Intent(android.content.Intent.ACTION_SEND)
        intent.setType("application/epub+zip")
        intent.putExtra(android.content.Intent.EXTRA_STREAM, uri)
        intent.addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION)
        let chooser = android.content.Intent.createChooser(intent, nil)
        chooser.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(chooser)
        #endif
    }

    func navigateToBookmark(_ bookmark: BookmarkRecord) {
        guard let loc = Loc.fromJSON(bookmark.locatorJSON) else {
            logger.error("Invalid bookmark locator JSON")
            return
        }
        logger.info("Navigating to bookmark id=\(bookmark.id): \(bookmark.progressLabel)")
        let animated = settings.animatePageTurns
        if let nav = navigator {
            #if !SKIP
            Task { await nav.go(to: loc.platformValue, options: animated ? .animated : .none) }
            #else
            // go(Locator, boolean) is synchronous and must run on the main thread
            // because it manipulates the ViewPager directly.
            nav.go(loc.platformValue, animated)
            #endif
        }
        showTOC = false
        showHUD = false
    }

    // MARK: - Page Estimation

    #if !SKIP
    /// Updates chapterPageIndex/chapterPageCount on iOS using the position
    /// counts loaded from the publication and the locator's progression.
    func updateChapterPageInfo(loc: Loc) {
        guard let publication = viewModel?.publication else { return }
        let readingOrder = publication.manifest.readingOrder
        let hrefString = loc.platformValue.href.string

        // Find which reading order index this locator belongs to
        var chapterIndex: Int? = nil
        for i in 0..<readingOrder.count {
            if hrefString == readingOrder[i].href || hrefString.hasSuffix(readingOrder[i].href) || readingOrder[i].href.hasSuffix(hrefString) {
                chapterIndex = i
                break
            }
        }

        guard let idx = chapterIndex, idx < positionCountsByChapter.count else { return }
        let totalPositions = positionCountsByChapter[idx]
        guard totalPositions > 0 else { return }

        let progression = loc.progression ?? 0.0
        self.chapterPageCount = totalPositions
        self.chapterPageIndex = min(Int((progression * Double(totalPositions)).rounded()), totalPositions - 1)
    }
    #endif

    /// Number of pages remaining in the current chapter, or nil if unknown.
    var pagesLeftInChapter: Int? {
        if chapterPageCount > 0 {
            return max(0, chapterPageCount - chapterPageIndex - 1)
        }
        return nil
    }

    // MARK: - Font Picker

    // MARK: - Spacing Presets

    private static let lineHeightPresets: [Double] = [0.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.5]
    private static let letterSpacingPresets: [Double] = [0.0, 0.05, 0.1, 0.2, 0.35, 0.5]
    private static let wordSpacingPresets: [Double] = [0.0, 0.25, 0.5, 0.75, 1.0, 1.5]
    private static let pageMarginsPresets: [Double] = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0]

    func cycleLineHeight() {
        settings.lineHeight = nextPreset(current: settings.lineHeight, presets: Self.lineHeightPresets)
        applyPreferences()
    }

    func cycleLetterSpacing() {
        settings.letterSpacing = nextPreset(current: settings.letterSpacing, presets: Self.letterSpacingPresets)
        applyPreferences()
    }

    func cycleWordSpacing() {
        settings.wordSpacing = nextPreset(current: settings.wordSpacing, presets: Self.wordSpacingPresets)
        applyPreferences()
    }

    func cyclePageMargins() {
        settings.pageMargins = nextPreset(current: settings.pageMargins, presets: Self.pageMarginsPresets)
        applyPreferences()
    }

    private func nextPreset(current: Double, presets: [Double]) -> Double {
        // Find the next preset value after the current one; wrap to first (default)
        for i in 0..<presets.count {
            if current <= presets[i] + 0.001 {
                if i + 1 < presets.count {
                    return presets[i + 1]
                } else {
                    return presets[0]
                }
            }
        }
        return presets[0]
    }

    private func presetLabel(value: Double, suffix: String = "") -> String {
        if value <= 0.001 { return "Default" }
        return String(format: "%.1f", value) + suffix
    }

    @ViewBuilder func spacingControlsPanel() -> some View {
        HStack(spacing: 0) {
            Spacer()
            Button { cycleLineHeight() } label: {
                VStack(spacing: 4) {
                    Image("format_line_spacing", bundle: .module)
                        .foregroundStyle(settings.lineHeight > 0.0 ? Color.accentColor : Color.white)
                    Text(presetLabel(value: settings.lineHeight))
                        .font(.caption2)
                        .foregroundStyle(settings.lineHeight > 0.0 ? Color.accentColor : Color.white.opacity(0.7))
                }
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Line spacing: \(presetLabel(value: settings.lineHeight))")
            Spacer()
            Button { cycleLetterSpacing() } label: {
                VStack(spacing: 4) {
                    Image("format_letter_spacing", bundle: .module)
                        .foregroundStyle(settings.letterSpacing > 0.0 ? Color.accentColor : Color.white)
                    Text(presetLabel(value: settings.letterSpacing))
                        .font(.caption2)
                        .foregroundStyle(settings.letterSpacing > 0.0 ? Color.accentColor : Color.white.opacity(0.7))
                }
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Character spacing: \(presetLabel(value: settings.letterSpacing))")
            Spacer()
            Button { cycleWordSpacing() } label: {
                VStack(spacing: 4) {
                    Image("space_bar", bundle: .module)
                        .foregroundStyle(settings.wordSpacing > 0.0 ? Color.accentColor : Color.white)
                    Text(presetLabel(value: settings.wordSpacing))
                        .font(.caption2)
                        .foregroundStyle(settings.wordSpacing > 0.0 ? Color.accentColor : Color.white.opacity(0.7))
                }
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Word spacing: \(presetLabel(value: settings.wordSpacing))")
            Spacer()
            Button { cyclePageMargins() } label: {
                VStack(spacing: 4) {
                    Image("padding", bundle: .module)
                        .foregroundStyle(settings.pageMargins > 0.0 ? Color.accentColor : Color.white)
                    Text(presetLabel(value: settings.pageMargins))
                        .font(.caption2)
                        .foregroundStyle(settings.pageMargins > 0.0 ? Color.accentColor : Color.white.opacity(0.7))
                }
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Margins: \(presetLabel(value: settings.pageMargins))")
            Spacer()
        }
        .padding(.vertical, 10)
        .background(Color.black.opacity(0.7))
    }

    @ViewBuilder func fontPickerPanel() -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 16) {
                ForEach(FontManager.allFonts) { font in
                    let name = font.name
                    let tag = font.tag
                    let isSelected = settings.fontFamily == tag
                    Button {
                        settings.fontFamily = tag
                        applyPreferences()
                    } label: {
                        VStack(spacing: 4) {
                            Text("Abc")
                                .font(tag.isEmpty ? .system(size: 22) : .custom(tag, size: 22))
                                .foregroundStyle(isSelected ? Color.accentColor : Color.white)
                                .frame(width: 64, height: 44)
                                .background(isSelected ? Color.white.opacity(0.2) : Color.white.opacity(0.1))
                                .cornerRadius(8)
                            Text(name)
                                .font(.caption2)
                                .foregroundStyle(isSelected ? Color.accentColor : Color.white.opacity(0.7))
                                .lineLimit(1)
                        }
                        .frame(width: 72)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Font: \(name)")
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
        .background(Color.black.opacity(0.7))
    }

    // MARK: - HUD Overlay

    @ViewBuilder func hudOverlay(publication: Pub) -> some View {
        let overlayButtonSize: CGFloat = 35
        let hudButtonSize: CGFloat = 28

        if showHUD {
            // Main HUD content
            VStack(spacing: 0) {
                HStack {
                    // Close button
                    Button {
                        saveCurrentLocator()
                        dismiss()
                    } label: {
                        Image("cancel", bundle: .module)
                            .font(.system(size: overlayButtonSize))
                            .background(Circle().fill(Color(.systemBackground).opacity(0.5)))
                            #if !os(Android)
                            .contentShape(Circle())
                            #endif
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("readerCloseButton")
                    .accessibilityLabel("Close reader")

                    Spacer()

                    // More menu
                    Menu {
                        Button {
                            if isSpeaking {
                                stopSpeaking()
                            } else {
                                showHUD = false
                                startSpeaking()
                            }
                        } label: {
                            Label(
                                title: { Text(isSpeaking ? "Stop Speaking" : "Start Speaking") },
                                icon: { Image(isSpeaking ? "stop_circle" : "volume_up", bundle: .module) }
                            )
                        }
                        Button {
                            toggleBookmark()
                        } label: {
                            Label(
                                title: { Text(isCurrentPageBookmarked ? "Remove Bookmark" : "Add Bookmark") },
                                icon: { Image(isCurrentPageBookmarked ? "bookmark_filled" : "bookmark", bundle: .module) }
                            )
                        }
                        Button {
                            shareBook()
                        } label: {
                            Label("Share", systemImage: "square.and.arrow.up")
                        }
                    } label: {
                        Image("more_horiz", bundle: .module)
                            .font(.system(size: overlayButtonSize))
                            .background(Rectangle().fill(Color(.systemBackground).opacity(0.5)))
                            #if !os(Android)
                            .contentShape(Circle())
                            #endif
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("readerMoreMenu")
                    .accessibilityLabel("More options")
                }
                .padding(.top, 16)
                .padding(.horizontal, 16)

                Spacer()

                // Extended HUD: spacing controls + font picker
                if showExtendedHUD {
                    spacingControlsPanel()
                    fontPickerPanel()
                }

                // Bottom controls
                VStack(spacing: 16) {
                    // Progress indicator
                    HStack {
                        Text(locator?.title ?? "")
                            .foregroundStyle(Color.white)
                            .lineLimit(1)
                            .accessibilityIdentifier("readerChapterTitle")
                        Spacer()
                        if let pagesLeft = pagesLeftInChapter {
                            Text("\(pagesLeft) pages left in chapter")
                                .foregroundStyle(Color.white.opacity(0.7))
                                .accessibilityIdentifier("readerPagesLeft")
                        }
                        let prog = locator?.totalProgression ?? 0.0
                        Text("\(Int(prog * 100))%")
                            .foregroundStyle(Color.white)
                            .accessibilityIdentifier("readerProgressPercent")
                    }
                    .font(.headline)
                    .padding(.horizontal, 16)

                    ProgressView(value: locator?.totalProgression ?? 0.0)
                        .tint(.white)
                        .padding(.horizontal, 16)
                        .accessibilityIdentifier("readerProgressBar")
                        .accessibilityLabel("Reading progress")

                    // Font size and TOC controls
                    HStack {
                        // Book info button
                        Button {
                            showBookDetail = true
                        } label: {
                            Image("info", bundle: .module)
                                .font(.system(size: hudButtonSize))
                                .foregroundStyle(Color.white)
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(Color.white)
                        .accessibilityIdentifier("bookInfoButton")
                        .accessibilityLabel("Book info")

                        Spacer()

                        Button {
                            adjustFontSize(increase: false)
                        } label: {
                            Image("remove_circle", bundle: .module)
                                .font(.system(size: hudButtonSize))
                                .foregroundStyle(Color.white)
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(Color.white)
                        .accessibilityIdentifier("decreaseFontSizeButton")
                        .accessibilityLabel("Decrease font size")

                        Spacer()

                        Button {
                            showExtendedHUD.toggle()
                        } label: {
                            Text("Aa")
                                .font(.largeTitle)
                                .foregroundStyle(showExtendedHUD ? Color.accentColor : Color.white)
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("fontSizeIndicator")
                        .accessibilityLabel("Font options")

                        Spacer()

                        Button {
                            adjustFontSize(increase: true)
                        } label: {
                            Image("add_circle", bundle: .module)
                                .font(.system(size: hudButtonSize))
                                .foregroundStyle(Color.white)
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(Color.white)
                        .accessibilityIdentifier("increaseFontSizeButton")
                        .accessibilityLabel("Increase font size")

                        Spacer()

                        Button {
                            showTOC = true
                        } label: {
                            Image("toc", bundle: .module)
                                .font(.system(size: hudButtonSize))
                                .foregroundStyle(Color.white)
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(Color.white)
                        .accessibilityIdentifier("tableOfContentsButton")
                        .accessibilityLabel("Table of contents")
                    }
                    .padding(.horizontal, 24)
                    .padding(.bottom, 40)
                }
                .padding(.top, 12)
                .background(Color.black.opacity(0.7))
            }
        }
    }

    // MARK: - Table of Contents & Bookmarks Sheet

    func tocSheet(publication: Pub) -> some View {
        BookLocationsBrowser(
            publication: publication,
            bookmarks: bookmarks,
            currentLocator: locator,
            database: database,
            bookID: bookID,
            onNavigateToTOC: { link in
                navigateToTOCEntry(link)
            },
            onNavigateToBookmark: { bookmark in
                navigateToBookmark(bookmark)
            },
            onBookmarksChanged: {
                refreshBookmarks()
            },
            onDismiss: {
                showTOC = false
            }
        )
    }

    // MARK: - Reader Container

    func readerViewContainer(publication: Pub) -> some View {
        #if !SKIP
        ReaderViewContainer(
            viewModel: viewModel!,
            viewControllerWrapper: ReaderViewControllerWrapper(
                viewController: ReaderViewController(
                    viewModel: viewModel!,
                    navigator: navigator!
                )
            )
        )
        #else
        ComposeView { context in
            let savedLocator = initialLocator?.platformValue
            let navigatorFactory = EpubNavigatorFactory(publication: publication.platformValue, configuration: navConfig)
            let paginationListener = ReaderPaginationListener(pageChanged: { pageIndex, totalPages, platformLocator in
                let loc = Loc(platformValue: platformLocator)
                self.locator = loc
                self.chapterPageIndex = pageIndex
                self.chapterPageCount = totalPages
                self.persistLocator(loc)
            })
            let fragmentFactory = navigatorFactory.createFragmentFactory(initialLocator: savedLocator, listener: nil, paginationListener: paginationListener)
            guard let fragmentActivity = LocalContext.current.fragmentActivity else {
                fatalError("could not extract FragmentActivity from LocalContext.current")
            }

            // Capture activity for status bar control
            if let currentAndroidActivity = UIApplication.shared.androidActivity {
                if self.settings.hideStatusBarInReader {
                    currentAndroidActivity.window.insetsController?.hide(android.view.WindowInsets.Type.statusBars())
                }
            }

            let fragmentManager = fragmentActivity.supportFragmentManager
            fragmentManager.fragmentFactory = fragmentFactory
            AndroidFragment<EpubNavigatorFragment>(
                onUpdate: { nav in
                    self.navigator = nav
                    if !self.inputListenerAdded {
                        self.inputListenerAdded = true
                        let listener = ReaderTapListener(tapHandler: { x, y in
                            let w = Double(nav.view?.width ?? 1)
                            self.handleTap(x: x, width: w)
                        })
                        nav.addInputListener(listener)
                    }
                    if !self.hasRestoredPosition {
                        self.hasRestoredPosition = true
                        if let savedLoc = self.initialLocator?.platformValue {
                            nav.go(savedLoc, false)
                        }
                    }
                    if !self.initialPrefsApplied {
                        self.initialPrefsApplied = true
                        self.applyPreferences()
                    }
                }
            )
        }
        #endif
    }
}

@Observable class ReaderViewModel {
    var publication: Pub
    var isFullscreen = false

    init(publication: Pub, isFullscreen: Bool = false) {
        self.publication = publication
        self.isFullscreen = isFullscreen
    }
}

#if SKIP

class ReaderTapListener: InputListener {
    let tapHandler: (Double, Double) -> Void

    init(tapHandler: @escaping (Double, Double) -> Void) {
        self.tapHandler = tapHandler
    }

    override func onTap(_ event: TapEvent) -> Bool {
        tapHandler(Double(event.point.x), Double(event.point.y))
        return true
    }

    override func onDrag(_ event: DragEvent) -> Bool {
        return false
    }

    override func onKey(_ event: KeyEvent) -> Bool {
        return false
    }
}

class ReaderPaginationListener: EpubNavigatorFragment.PaginationListener {
    let pageChanged: (Int, Int, org.readium.r2.shared.publication.Locator) -> Void

    init(pageChanged: @escaping (Int, Int, org.readium.r2.shared.publication.Locator) -> Void) {
        self.pageChanged = pageChanged
    }

    override func onPageChanged(_ pageIndex: Int, _ totalPages: Int, _ locator: org.readium.r2.shared.publication.Locator) {
        pageChanged(pageIndex, totalPages, locator)
    }

    override func onPageLoaded() {
    }
}
#endif


#if SKIP
extension android.content.Context {
    /// Extract the `FragmentActivity` from unwrapping this `Context`
    var fragmentActivity: FragmentActivity? {
        if let activity = self as? FragmentActivity {
            return activity
        }
        guard let contextWrapper = self as? ContextWrapper else {
            return nil
        }
        return contextWrapper.baseContext.fragmentActivity
    }
}
#endif

#if !SKIP
struct ReaderViewContainer: View {

    /// View model provided by your application.
    @State var viewModel: ReaderViewModel

    let viewControllerWrapper: ReaderViewControllerWrapper

    var body: some View {
        viewControllerWrapper
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .ignoresSafeArea(.all)
            .navigationTitle(viewModel.publication.metadata.title ?? "Unknown Title")
            .navigationBarTitleDisplayMode(.inline)
            .navigationBarHidden(viewModel.isFullscreen)
            .statusBarHidden(viewModel.isFullscreen)
    }
}

/// SwiftUI wrapper for the `ReaderViewController`.
struct ReaderViewControllerWrapper: UIViewControllerRepresentable {
    let viewController: ReaderViewController

    func makeUIViewController(context: Context) -> ReaderViewController {
        viewController
    }

    func updateUIViewController(_ uiViewController: ReaderViewController, context: Context) {
    }
}

/// Host view controller for a Readium Navigator.
class ReaderViewController: UIViewController {

    /// View model provided by your application.
    private let viewModel: ReaderViewModel

    /// Readium Navigator instance.
    private let navigator: Navigator & UIViewController

    init(viewModel: ReaderViewModel, navigator: Navigator & UIViewController) {
        self.viewModel = viewModel
        self.navigator = navigator

        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init?(coder: NSCoder) not implemented")
    }

    override func viewDidLoad() {
        super.viewDidLoad()

        addChild(navigator)
        navigator.view.frame = view.bounds
        navigator.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(navigator.view)
        navigator.didMove(toParent: self)
    }

    /// Handler for a custom editing action.
    @objc func makeHighlight(_ sender: Any) {
        //viewModel.makeHighlight()
    }
}

/// Delegate that receives location change callbacks from the Readium Navigator.
@MainActor class ReaderLocationDelegate: NSObject, EPUBNavigatorDelegate {
    let onLocationChanged: (Loc) -> Void
    var onTap: ((CGPoint, CGSize) -> Void)? = nil

    init(onLocationChanged: @escaping (Loc) -> Void) {
        self.onLocationChanged = onLocationChanged
    }

    func navigator(_ navigator: Navigator, locationDidChange locator: Locator) {
        let loc = Loc(platformValue: locator)
        onLocationChanged(loc)
    }

    func navigator(_ navigator: Navigator, presentError error: NavigatorError) {
        logger.error("Navigator error: \(error)")
    }

    func navigator(_ navigator: VisualNavigator, didTapAt point: CGPoint) {
        let viewSize = (navigator as? UIViewController)?.view.bounds.size ?? CGSize(width: 1.0, height: 1.0)
        onTap?(point, viewSize)
    }
}

/// Delegate that receives TTS state change callbacks and drives auto-page-turning.
class TTSSynthesizerDelegate: PublicationSpeechSynthesizerDelegate {
    /// The EPUB navigator used for auto-page-turning.
    var navigator: EPUBNavigatorViewController?

    /// The settings object for checking whether auto-page-turning is enabled.
    var settings: StanzaSettings?

    /// Called when the TTS playback finishes naturally.
    var onStopped: (() -> Void)?

    /// Tracks the last locator we navigated to, to avoid redundant go() calls.
    private var lastNavigatedLocator: Locator?

    init() {
    }

    func publicationSpeechSynthesizer(_ synthesizer: PublicationSpeechSynthesizer, stateDidChange synthesizerState: PublicationSpeechSynthesizer.State) {
        switch synthesizerState {
        case .stopped:
            lastNavigatedLocator = nil
            // Clear utterance highlight
            clearHighlight()
            Task { @MainActor in
                onStopped?()
            }

        case .paused:
            // Keep the highlight visible while paused
            break

        case let .playing(utterance, range: range):
            // Highlight the current utterance
            if settings?.ttsHighlightUtterance == true {
                applyHighlight(for: utterance.locator)
            }

            // Auto-turn pages to follow the spoken text
            guard settings?.ttsAutoTurnPages == true else { break }
            guard let nav = navigator else { break }

            // Prefer the word-level range locator for smoother tracking;
            // fall back to the utterance-level locator for page turns.
            let targetLocator = range ?? utterance.locator

            // Avoid navigating to the same locator repeatedly
            if targetLocator.href == lastNavigatedLocator?.href
                && targetLocator.locations.progression == lastNavigatedLocator?.locations.progression {
                break
            }
            lastNavigatedLocator = targetLocator

            Task { @MainActor in
                await nav.go(to: targetLocator, options: .init(animated: settings?.animatePageTurns ?? false))
            }

        @unknown default:
            break
        }
    }

    /// Applies a highlight decoration on the given locator.
    private func applyHighlight(for locator: Locator) {
        guard let nav = navigator else { return }
        let decoration = Decoration(
            id: "tts-utterance",
            locator: locator,
            style: .highlight(tint: .red, isActive: false)
        )
        Task { @MainActor in
            nav.apply(decorations: [decoration], in: "tts")
        }
    }

    /// Clears the TTS utterance highlight.
    private func clearHighlight() {
        guard let nav = navigator else { return }
        Task { @MainActor in
            nav.apply(decorations: [], in: "tts")
        }
    }

    func publicationSpeechSynthesizer(_ synthesizer: PublicationSpeechSynthesizer, utterance: PublicationSpeechSynthesizer.Utterance, didFailWithError error: PublicationSpeechSynthesizer.Error) {
        logger.error("TTS error: \(error)")
    }
}
#endif
