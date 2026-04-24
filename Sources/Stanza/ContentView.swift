// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel
import SkipKit

public struct ContentView: View {
    @State var tab = Tab.home
    @State var browserURL: URL = URL(string: "https://example.com")!
    @State var showBrowser: Bool = false
    @State var pendingCatalogURL: CatalogURLItem? = nil
    @Environment(StanzaSettings.self) var settings: StanzaSettings
    @Environment(ErrorManager.self) var errorManager: ErrorManager

    public init() {
    }

    public var body: some View {
        TabView(selection: $tab) {
            LibraryView()
                .tabItem { Label(title: { Text("Library") }, icon: { Image("newsstand", bundle: .module) }) }
                .tag(Tab.home)
                .accessibilityIdentifier("libraryTab")

            BrowseView()
                .tabItem { Label(title: { Text("Catalogs") }, icon: { Image("library_books", bundle: .module) }) }
                .tag(Tab.browse)
                .accessibilityIdentifier("browseTab")

            SettingsView()
                .tabItem { Label(title: { Text("Settings") }, icon: { Image("settings", bundle: .module) }) }
                .tag(Tab.settings)
                .accessibilityIdentifier("settingsTab")
        }
        .accessibilityIdentifier("mainTabView")
        .preferredColorScheme(settings.appearance == "dark" ? .dark : settings.appearance == "light" ? .light : nil)
        .environment(\.openURL, OpenURLAction { url in
            if settings.useInAppBrowser {
                browserURL = url
                showBrowser = true
                return .handled
            }
            return .systemAction
        })
        .onOpenURL { url in
            handleOPDSURL(url)
        }
        .sheet(item: $pendingCatalogURL) { item in
            AddCatalogView(prefilledURL: item.url, catalogDB: openCatalogDB(), onAdd: {})
        }
        .openWebBrowser(isPresented: $showBrowser, url: browserURL, mode: .embeddedBrowser(params: nil))
        .withErrorManager(errorManager)
    }

    private func openCatalogDB() -> CatalogDatabase? {
        do {
            let dir = URL.applicationSupportDirectory
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            let path = dir.appendingPathComponent("catalogs.sqlite").path
            return try CatalogDatabase(path: path)
        } catch {
            logger.error("Failed to open catalog database: \(error)")
            return nil
        }
    }

    private func handleOPDSURL(_ url: URL) {
        logger.info("Received opds:// URL: \(url.absoluteString)")
        // Convert opds:// scheme to https://
        var catalogURL = url.absoluteString
        if catalogURL.hasPrefix("opds://") {
            catalogURL = catalogURL.replacingOccurrences(of: "opds://", with: "https://")
        }
        logger.info("Converted to catalog URL: \(catalogURL)")
        // Close any open book
        settings.lastOpenBookID = 0
        // Switch to catalogs tab
        tab = .browse
        // Show the add catalog sheet with the URL
        // Use a short delay to ensure the tab switch completes first
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            pendingCatalogURL = CatalogURLItem(url: catalogURL)
        }
    }
}

/// Wrapper to make a catalog URL usable with `.sheet(item:)`
struct CatalogURLItem: Identifiable {
    let url: String
    var id: String { url }
}

enum Tab : String, Hashable {
    case home, browse, settings
}
