// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel
import SkipKit

public struct ContentView: View {
    @State var tab = Tab.home
    @State var browserURL: URL = URL(string: "https://example.com")!
    @State var showBrowser: Bool = false
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

            if settings.enableCatalogs {
                BrowseView()
                    .tabItem { Label(title: { Text("Catalogs") }, icon: { Image("library_books", bundle: .module) }) }
                    .tag(Tab.browse)
                    .accessibilityIdentifier("browseTab")
            }

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
        .openWebBrowser(isPresented: $showBrowser, url: browserURL, mode: .embeddedBrowser(params: nil))
        .withErrorManager(errorManager)
    }
}

enum Tab : String, Hashable {
    case home, browse, settings
}
