// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-3.0-only

import SwiftUI

public struct ContentView: View {
    @AppStorage("tab") var tab = Tab.welcome
    @AppStorage("name") var name = "Skipper"
    @State var appearance = ""
    @State var isBeating = false

    public init() {
    }

    public var body: some View {
        TabView(selection: $tab) {
            NavigationStack {
                List {
                    ForEach(1..<1_000) { i in
                        NavigationLink("Item \(i)", value: i)
                    }
                }
                .navigationTitle("Library")
                .navigationDestination(for: Int.self) { i in
                    Text("Book \(i)")
                        .font(.title)
                        .navigationTitle("Book \(i)")
                }
            }
            .tabItem { Label("Library", systemImage: "list.bullet") }
            .tag(Tab.home)

            #if SKIP || canImport(ReadiumNavigator)
            ReaderView()
                .tabItem { Label("Reading", systemImage: "heart.fill") }
                .tag(Tab.welcome)
            #endif

            NavigationStack {
                Form {
                    TextField("Name", text: $name)
                    Picker("Appearance", selection: $appearance) {
                        Text("System").tag("")
                        Text("Light").tag("light")
                        Text("Dark").tag("dark")
                    }
                    HStack {
                        #if SKIP
                        ComposeView { ctx in // Mix in Compose code!
                            androidx.compose.material3.Text("💚", modifier: ctx.modifier)
                        }
                        #else
                        Text(verbatim: "💙")
                        #endif
                        Text("Powered by \(androidSDK != nil ? "Jetpack Compose" : "SwiftUI")")
                    }
                    .foregroundStyle(.gray)
                }
                .navigationTitle("Settings")
            }
            .tabItem { Label("Settings", systemImage: "gearshape.fill") }
            .tag(Tab.settings)
        }
        .preferredColorScheme(appearance == "dark" ? .dark : appearance == "light" ? .light : nil)
    }
}

enum Tab : String, Hashable {
    case welcome, home, settings
}
