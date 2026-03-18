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

    var body: some View {
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
        .onChange(of: settings.appearance) { applyPreferences() }
        .onChange(of: colorScheme) { applyPreferences() }
        #if SKIP
        .onChange(of: showHUD) { updateAndroidStatusBar() }
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
        .preferredColorScheme(settings.appearance == "dark" ? .dark : settings.appearance == "light" ? .light : nil)
        .background(colorScheme == .dark ? Color.black : Color.white)
        #if !SKIP
        .statusBarHidden(settings.hideStatusBarInReader && !showHUD)
        #endif
        .task {
            await loadBook()
        }
        .onDisappear {
            saveCurrentLocator()
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
        #if !SKIP
        if let nav = navigator {
            Task { await nav.goForward(options: animated ? .animated : .none) }
        }
        #else
        if let nav = navigator {
            Task { nav.goForward(animated) }
        }
        #endif
    }

    func goBackward() {
        let animated = settings.animatePageTurns
        #if !SKIP
        if let nav = navigator {
            Task { await nav.goBackward(options: animated ? .animated : .none) }
        }
        #else
        if let nav = navigator {
            Task { nav.goBackward(animated) }
        }
        #endif
    }

    func navigateToTOCEntry(_ link: Lnk) {
        logger.info("Navigating to TOC entry: '\(link.title ?? "unknown")' href=\(link.href)")
        let animated = settings.animatePageTurns
        #if !SKIP
        if let nav = navigator {
            Task { await nav.go(to: link.platformValue, options: animated ? .animated : .none) }
        }
        #else
        if let nav = navigator {
            Task { nav.go(link.platformValue, animated) }
        }
        #endif
        showTOC = false
        showHUD = false
    }

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
        let publisherStylesVal: Bool? = s.publisherStyles == "true" ? true : s.publisherStyles == "false" ? false : nil
        let textNormalizationVal: Bool? = s.textNormalization == "true" ? true : s.textNormalization == "false" ? false : nil
        let wordSpacingVal: Double? = s.wordSpacing > 0.0 ? s.wordSpacing : nil

        #if !SKIP
        if let nav = navigator {
            let fontFamilyVal: ReadiumNavigator.FontFamily? = s.fontFamily.isEmpty ? nil : ReadiumNavigator.FontFamily(rawValue: s.fontFamily)
            let columnCountVal = s.columnCount.isEmpty ? nil : ReadiumNavigator.ColumnCount(rawValue: s.columnCount)
            let fitVal = s.fit.isEmpty ? nil : ReadiumNavigator.Fit(rawValue: s.fit)
            let textAlignVal = s.textAlign.isEmpty ? nil : ReadiumNavigator.TextAlignment(rawValue: s.textAlign)
            let themeVal: ReadiumNavigator.Theme = isDark ? .dark : .light
            let prefs: PlatformPreferences = EPUBPreferences(
                columnCount: columnCountVal,
                fit: fitVal,
                fontFamily: fontFamilyVal,
                fontSize: s.fontSize,
                hyphens: hyphensVal,
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
            let themeVal: org.readium.r2.navigator.preferences.Theme = isDark ? Theme.DARK : Theme.LIGHT
            let prefs: PlatformPreferences = org.readium.r2.navigator.epub.EpubPreferences(
                fontFamily: fontFamilyVal,
                fontSize: s.fontSize,
                hyphens: hyphensVal,
                lineHeight: lineHeightVal,
                pageMargins: pageMarginsVal,
                paragraphSpacing: paragraphSpacingVal,
                publisherStyles: publisherStylesVal,
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
        if let activity = currentAndroidActivity {
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

    func navigateToBookmark(_ bookmark: BookmarkRecord) {
        guard let loc = Loc.fromJSON(bookmark.locatorJSON) else {
            logger.error("Invalid bookmark locator JSON")
            return
        }
        logger.info("Navigating to bookmark id=\(bookmark.id): \(bookmark.progressLabel)")
        let animated = settings.animatePageTurns
        #if !SKIP
        if let nav = navigator {
            Task { await nav.go(to: loc.platformValue, options: animated ? .animated : .none) }
        }
        #else
        if let nav = navigator {
            Task { nav.go(loc.platformValue, animated) }
        }
        #endif
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
        let overlayButtonSize: CGFloat = 28
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
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("readerCloseButton")
                    .accessibilityLabel("Close reader")

                    Spacer()

                    // Bookmark Button
                    Button {
                        toggleBookmark()
                    } label: {
                        Image(isCurrentPageBookmarked ? "bookmark_filled" : "bookmark", bundle: .module)
                            .font(.system(size: overlayButtonSize))
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("toggleBookmarkButton")
                    .accessibilityLabel(isCurrentPageBookmarked ? "Remove bookmark" : "Add bookmark")
                }
                .padding(.top, 16)
                .padding(.horizontal, 16)

                Spacer()

                // Extended HUD: font picker
                if showExtendedHUD {
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
            if currentAndroidActivity == nil {
                currentAndroidActivity = fragmentActivity
                if self.settings.hideStatusBarInReader {
                    fragmentActivity.window.insetsController?.hide(android.view.WindowInsets.Type.statusBars())
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

/// Module-level reference to the current Activity, used for status bar control.
/// Set from the ComposeView context where LocalContext is available.
var currentAndroidActivity: android.app.Activity? = nil

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
#endif
