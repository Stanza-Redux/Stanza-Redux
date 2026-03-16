// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel

public struct ContentView: View {
    @State var tab = Tab.home
    @Environment(StanzaSettings.self) var settings: StanzaSettings

    public init() {
    }

    public var body: some View {
        TabView(selection: $tab) {
            LibraryView()
                .tabItem { Label(title: { Text("Library") }, icon: { Image("library_books", bundle: .module) }) }
                .tag(Tab.home)

            BrowseView()
                .tabItem { Label(title: { Text("Browse") }, icon: { Image("explore", bundle: .module) }) }
                .tag(Tab.browse)

            SettingsView()
                .tabItem { Label(title: { Text("Settings") }, icon: { Image("settings", bundle: .module) }) }
                .tag(Tab.settings)
        }
        .preferredColorScheme(settings.appearance == "dark" ? .dark : settings.appearance == "light" ? .light : nil)
    }
}

enum Tab : String, Hashable {
    case home, browse, settings
}
