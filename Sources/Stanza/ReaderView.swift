// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-3.0-only

import Foundation
import SwiftUI
import Observation
import StanzaModel

#if SKIP || canImport(ReadiumNavigator)
#if !SKIP
import ReadiumNavigator
import ReadiumShared
import ReadiumStreamer
//import ReadiumLCP
import ReadiumOPDS
//import ReadiumAdapterGCDWebServer
#else
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
import org.readium.r2.shared.opds.Acquisition
import org.readium.r2.shared.opds.Facet
import org.readium.r2.shared.opds.Feed
import org.readium.r2.shared.opds.Group
import org.readium.r2.shared.opds.ParseData
import org.readium.r2.shared.opds.Price
import org.readium.r2.shared.publication.Contributor
import org.readium.r2.shared.publication.Href
import org.readium.r2.shared.publication.Link
import org.readium.r2.shared.publication.LocalizedString
import org.readium.r2.shared.publication.Locator
import org.readium.r2.shared.publication.Manifest
import org.readium.r2.shared.publication.Metadata
import org.readium.r2.shared.publication.Properties
import org.readium.r2.shared.publication.Publication
import org.readium.r2.shared.publication.PublicationCollection
import org.readium.r2.shared.publication.Subject
import org.readium.r2.shared.toJSON
import org.readium.r2.shared.util.AbsoluteUrl
import org.readium.r2.shared.util.ErrorException
import org.readium.r2.shared.util.Instant
import org.readium.r2.shared.util.Try
import org.readium.r2.shared.util.Url
import org.readium.r2.shared.util.getOrElse
import org.readium.r2.shared.util.toUri
import org.readium.r2.shared.util.toUrl
import org.readium.r2.shared.util.toAbsoluteUrl
import org.readium.r2.shared.util.asset.AssetRetriever
import org.readium.r2.shared.util.mediatype.MediaType
import org.readium.r2.shared.util.pdf.PdfDocumentFactory
import org.readium.r2.shared.util.xml.ElementNode
import org.readium.r2.shared.util.xml.XmlParser
import org.readium.r2.streamer.PublicationOpener
import org.readium.r2.streamer.parser.PublicationParser
import org.readium.r2.streamer.parser.DefaultPublicationParser
#endif

#if !SKIP
var navConfig = EPUBNavigatorViewController.Configuration()
#endif

@Observable class ReaderViewModel {
    var publication: Publication
    var isFullscreen = false

    init(publication: Publication, isFullscreen: Bool = false) {
        self.publication = publication
        self.isFullscreen = isFullscreen
    }
}

struct ReaderView: View {
    let bookURL: URL = Bundle.module.url(forResource: "Alice", withExtension: "epub")!
    @State var publication: Publication? = nil
    @State var viewModel: ReaderViewModel? = nil
    @State var error: Error? = nil
    @State var locator: Locator? = nil

    #if !SKIP
    @State var navigator: EPUBNavigatorViewController? = nil
    #endif

    var body: some View {
        if let publication {
            readerViewContainer(publication: publication)
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
        let publication = try await StanzaModel.loadPublication(bookURL: bookURL)
        self.publication = publication
        #if !SKIP
        self.viewModel = ReaderViewModel(publication: publication)
        self.navigator = try EPUBNavigatorViewController(publication: publication, initialLocation: locator, config: navConfig, httpServer: httpServer)
        #endif
    }

    func readerViewContainer(publication: Publication) -> some View {
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

            let navigatorFactory = EpubNavigatorFactory(
                publication: publication,
                configuration: EpubNavigatorFactory.Configuration(
                    defaults: EpubDefaults(
                        pageMargins: 1.4
                    )
                )
            )

            let fragmentFactory = navigatorFactory.createFragmentFactory(
                initialLocator: nil,
                listener: nil
            )

            let fragmentManager = (LocalContext.current as FragmentActivity).supportFragmentManager
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
#endif
