// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel

public struct ContentView: View {
    @AppStorage("tab") var tab = Tab.welcome
    @AppStorage("name") var name = "Skipper"
    @AppStorage("readerFontSize") var readerFontSize: Double = 1.0
    @State var appearance = ""
    @State var isBeating = false

    public init() {
    }

    public var body: some View {
        TabView(selection: $tab) {
            LibraryView()
                .tabItem { Label("Library", systemImage: "books.vertical") }
                .tag(Tab.home)

            #if SKIP || canImport(ReadiumNavigator)
            ReaderView(tab: $tab)
                .tabItem { Label("Reading", systemImage: "heart.fill") }
                .tag(Tab.welcome)
            #endif

            NavigationStack {
                Form {
                    Section("Reading") {
                        HStack {
                            Text("Font Size")
                            Spacer()
                            Text("\(Int(readerFontSize * 100))%")
                                .foregroundStyle(.secondary)
                        }
                        Slider(value: $readerFontSize, in: 0.5...3.0, step: 0.1)
                        if readerFontSize != 1.0 {
                            Button("Reset to Default") {
                                readerFontSize = 1.0
                            }
                        }
                    }
                    Section("General") {
                        TextField("Name", text: $name)
                        Picker("Appearance", selection: $appearance) {
                            Text("System").tag("")
                            Text("Light").tag("light")
                            Text("Dark").tag("dark")
                        }
                    }
                    Section {
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
