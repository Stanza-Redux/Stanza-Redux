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

    public init() {
    }

    public var body: some View {
        TabView(selection: $tab) {
            LibraryView()
                .tabItem { Label(title: { Text("Library") }, icon: { Image("newsstand", bundle: .module) }) }
                .tag(Tab.home)
                .accessibilityIdentifier("libraryTab")

            #if DEBUG // BrowseView is in beta
            BrowseView()
                .tabItem { Label(title: { Text("Browse") }, icon: { Image("library_books", bundle: .module) }) }
                .tag(Tab.browse)
                .accessibilityIdentifier("browseTab")
            #endif

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
    }
}

enum Tab : String, Hashable {
    case home, browse, settings
}
