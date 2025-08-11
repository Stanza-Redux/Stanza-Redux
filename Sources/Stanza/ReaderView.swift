// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import Foundation
import SwiftUI
import Observation
import StanzaModel

#if SKIP || canImport(ReadiumNavigator)
#if !SKIP
import ReadiumNavigator
import ReadiumShared
import ReadiumStreamer
import ReadiumOPDS
//import ReadiumLCP
#else
import android.content.Context
import android.content.ContextWrapper
import android.content.ContentResolver
import androidx.fragment.app.FragmentActivity
import androidx.fragment.app.FragmentContainerView
import androidx.fragment.app.FragmentFactory
import androidx.fragment.compose.AndroidFragment
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import org.readium.r2.navigator.epub.EpubNavigatorFactory
import org.readium.r2.navigator.epub.EpubDefaults
import org.readium.r2.navigator.epub.EpubNavigatorFragment
#endif

#if !SKIP
typealias PlatformDefaults = ReadiumNavigator.EPUBDefaults
#else
typealias PlatformDefaults = org.readium.r2.navigator.epub.EpubDefaults
#endif

let defaults = PlatformDefaults(columnCount: nil, fontSize: nil, fontWeight: nil, hyphens: nil, imageFilter: nil, language: nil, letterSpacing: nil, ligatures: nil, lineHeight: nil, pageMargins: nil, paragraphIndent: nil, paragraphSpacing: nil, publisherStyles: nil, readingProgression: nil, scroll: nil, spread: nil, textAlign: nil, textNormalization: nil, typeScale: nil, wordSpacing: nil)

#if !SKIP
var navConfig: EPUBNavigatorViewController.Configuration = EPUBNavigatorViewController.Configuration(defaults: defaults)
#else
var navConfig: org.readium.r2.navigator.epub.EpubNavigatorFactory.Configuration = EpubNavigatorFactory.Configuration(defaults: defaults)
#endif

@Observable class ReaderViewModel {
    var publication: Pub
    var isFullscreen = false

    init(publication: Pub, isFullscreen: Bool = false) {
        self.publication = publication
        self.isFullscreen = isFullscreen
    }
}

struct ReaderView: View {
    let bookURL: URL = Bundle.module.url(forResource: "Alice", withExtension: "epub")!
    @State var viewModel: ReaderViewModel? = nil
    @State var error: Error? = nil
    @State var locator: Loc? = nil
    @State var isFullscreen: Bool = false

    #if !SKIP
    @State var navigator: EPUBNavigatorViewController? = nil
    #endif

    var body: some View {
        if let publication = viewModel?.publication {
            Text("Opening \(publication.metadata.title ?? "Book")")
                .fullScreenCover(isPresented: $isFullscreen) {
                    readerViewContainer(publication: publication)
                }
        } else {
            VStack {
                Button("Load Book") {
                    Task {
                        await loadDefaultBook()
                    }
                }

                if let error {
                    Text("Error: \(error)")
                }
            }
            .task {
                await loadDefaultBook()
                self.isFullscreen = true
            }
        }
//        config.editingActions.append(
//            EditingAction(
//                title: "Highlight"
//                action: #selector(highlightSelection)
//            )
//        )

    }

    func loadDefaultBook() async {
        do {
            try await loadPublication()
        } catch {
            self.error = error
        }
    }

    func loadPublication() async throws {
        let publication: Pub = try await Pub.loadPublication(from: bookURL)
        self.viewModel = ReaderViewModel(publication: publication)
        #if !SKIP
        self.navigator = try EPUBNavigatorViewController(publication: publication.platformValue, initialLocation: locator?.platformValue, config: navConfig, httpServer: httpServer)
        #endif
    }

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
            // create a EpubReaderFragment
            // https://github.com/readium/kotlin-toolkit/blob/develop/docs/guides/navigator/navigator.md#epubnavigatorfragment
            let navigatorFactory = EpubNavigatorFactory(publication: publication.platformValue, configuration: navConfig)
            let fragmentFactory = navigatorFactory.createFragmentFactory(initialLocator: locator?.platformValue, listener: nil)
            //let fragmentManager = (LocalContext.current as FragmentActivity).supportFragmentManager
            guard let fragmentActivity = LocalContext.current.fragmentActivity else {
                fatalError("could not extract FragmentActivity from LocalContext.current")
            }
            let fragmentManager = fragmentActivity.supportFragmentManager
            fragmentManager.fragmentFactory = fragmentFactory
            AndroidFragment<EpubNavigatorFragment>(
                onUpdate: { fragment in
                    // e.g. ReaderView: onUpdate: EpubNavigatorFragment{14dad07} (42dc86aa-c6a4-49a6-87a2-0341674f9485 id=0x88427037 tag=-2008911817)
                    logger.info("ReaderView: onUpdate: \(fragment)")
                }
            )
        }
        #endif
    }
}

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
#endif


#if !SKIP
public typealias PlatformPrefs = ReadiumNavigator.EPUBPreferences
#else
public typealias PlatformPrefs = org.readium.r2.navigator.epub.EpubPreferences
#endif

/// A wrapper for an underlying `Link` type.
public class Prefs {
    public var platformValue: PlatformPrefs

    public init(platformValue: PlatformPrefs) {
        self.platformValue = platformValue
    }

//    /// Default page background color.
//    public var backgroundColor: Color? {
//        get { platformValue.backgroundColor }
//        set { platformValue.backgroundColor = newValue }
//    }

//    /// Number of reflowable columns to display (one-page view or two-page
//    /// spread).
//    public var columnCount: ColumnCount? {
//        get { platformValue.columnCount }
//        set { platformValue.columnCount = newValue }
//    }

//    /// Default typeface for the text.
//    public var fontFamily: FontFamily? {
//        get { platformValue.fontFamily }
//        set { platformValue.fontFamily = newValue }
//    }

    /// Base text font size.
    public var fontSize: Double? {
        get { platformValue.fontSize }
        //set { platformValue.fontSize = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

    /// Default boldness for the text.
    public var fontWeight: Double? {
        get { platformValue.fontWeight }
        //set { platformValue.fontWeight = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

    /// Enable hyphenation.
    public var hyphens: Bool? {
        get { platformValue.hyphens }
        //set { platformValue.hyphens = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

//    /// Filter applied to images in dark theme.
//    public var imageFilter: ImageFilter? {
//        get { platformValue.imageFilter }
//        set { platformValue.imageFilter = newValue }
//    }

//    /// Language of the publication content.
//    public var language: Language? {
//        get { platformValue.language }
//        set { platformValue.language = newValue }
//    }

    /// Space between letters.
    public var letterSpacing: Double? {
        get { platformValue.letterSpacing }
        //set { platformValue.letterSpacing = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

    /// Enable ligatures in Arabic.
    public var ligatures: Bool? {
        get { platformValue.ligatures }
        //set { platformValue.ligatures = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

    /// Leading line height.
    public var lineHeight: Double? {
        get { platformValue.lineHeight }
        //set { platformValue.lineHeight = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

    /// Factor applied to horizontal margins.
    public var pageMargins: Double? {
        get { platformValue.pageMargins }
        //set { platformValue.pageMargins = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

    /// Text indentation for paragraphs.
    public var paragraphIndent: Double? {
        get { platformValue.paragraphIndent }
        //set { platformValue.paragraphIndent = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

    /// Vertical margins for paragraphs.
    public var paragraphSpacing: Double? {
        get { platformValue.paragraphSpacing }
        //set { platformValue.paragraphSpacing = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

    /// Indicates whether the original publisher styles should be observed.
    ///
    /// Many settings require this to be off.
    public var publisherStyles: Bool? {
        get { platformValue.publisherStyles }
        //set { platformValue.publisherStyles = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

//    /// Direction of the reading progression across resources.
//    public var readingProgression: ReadingProgression? {
//        get { platformValue.readingProgression }
//        set { platformValue.readingProgression = newValue }
//    }

    /// Indicates if the overflow of resources should be handled using
    /// scrolling instead of synthetic pagination.
    public var scroll: Bool? {
        get { platformValue.scroll }
        //set { platformValue.scroll = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

//    /// Indicates if the fixed-layout publication should be rendered with a
//    /// synthetic spread (dual-page).
//    public var spread: Spread? {
//        get { platformValue.spread }
//        set { platformValue.spread = newValue }
//    }

//    /// Page text alignment.
//    public var textAlign: TextAlignment? {
//        get { platformValue.textAlign }
//        set { platformValue.textAlign = newValue }
//    }

//    /// Default page text color.
//    public var textColor: Color? {
//        get { platformValue.textColor }
//        set { platformValue.textColor = newValue }
//    }

//    /// Normalize text styles to increase accessibility.
//    public var textColor: Bool? {
//        get { platformValue.textColor }
//        set { platformValue.textColor = newValue }
//    }

//    /// Reader theme.
//    public var theme: Theme? {
//        get { platformValue.theme }
//        set { platformValue.theme = newValue }
//    }

    /// Scale applied to all element font sizes.
    public var typeScale: Double? {
        get { platformValue.typeScale }
        //set { platformValue.typeScale = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

    /// Indicates whether the text should be laid out vertically.
    ///
    /// This is used for example with CJK languages. This setting is
    /// automatically derived from the language if no preference is given.
    public var verticalText: Bool? {
        get { platformValue.verticalText }
        //set { platformValue.verticalText = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

    /// Space between words.
    public var wordSpacing: Double? {
        get { platformValue.wordSpacing }
        //set { platformValue.wordSpacing = newValue } // needs EpubNavigatorFactory(publication).createPreferencesEditor(preferences).apply { }
    }

}

#if SKIP
extension Prefs: KotlinConverting<PlatformPrefs> {
    public override func kotlin(nocopy: Bool = false) -> PlatformPrefs {
        return platformValue
    }
}
#endif

#endif
