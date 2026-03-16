// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel

struct SettingsView: View {
    @Environment(StanzaSettings.self) var settings: StanzaSettings

    var body: some View {
        @Bindable var settings = settings
        NavigationStack {
            Form {
                Section("Reading") {
                    HStack {
                        Text("Font Size")
                        Spacer()
                        Text("\(Int(settings.fontSize * 100))%")
                            .foregroundStyle(.secondary)
                    }
                    Slider(value: $settings.fontSize, in: 0.5...3.0, step: 0.1)
                    if settings.fontSize != 1.0 {
                        Button("Reset Font Size") {
                            settings.fontSize = 1.0
                        }
                    }
                    Toggle("Animate Page Turns", isOn: $settings.animatePageTurns)
                }

                Section("Text Layout") {
                    Picker("Columns", selection: $settings.columnCount) {
                        Text("Auto").tag("")
                        Text("One").tag("1")
                        Text("Two").tag("2")
                    }

                    Picker("Content Fit", selection: $settings.fit) {
                        Text("Auto").tag("")
                        Text("Page").tag("page")
                        Text("Width").tag("width")
                    }

                    Picker("Hyphenation", selection: $settings.hyphens) {
                        Text("Default").tag("")
                        Text("On").tag("true")
                        Text("Off").tag("false")
                    }

                    Picker("Text Alignment", selection: $settings.textAlign) {
                        Text("Default").tag("")
                        Text("Start").tag("start")
                        Text("Left").tag("left")
                        Text("Center").tag("center")
                        Text("Right").tag("right")
                        Text("Justify").tag("justify")
                    }

                    Picker("Text Normalization", selection: $settings.textNormalization) {
                        Text("Default").tag("")
                        Text("On").tag("true")
                        Text("Off").tag("false")
                    }

                    Picker("Publisher Styles", selection: $settings.publisherStyles) {
                        Text("Default").tag("")
                        Text("On").tag("true")
                        Text("Off").tag("false")
                    }
                }

                Section("Spacing") {
                    VStack(alignment: .leading) {
                        HStack {
                            Text("Line Height")
                            Spacer()
                            Text(settings.lineHeight > 0.0 ? String(format: "%.1f", settings.lineHeight) : "Default")
                                .foregroundStyle(.secondary)
                        }
                        Slider(value: $settings.lineHeight, in: 0.0...3.0, step: 0.1)
                    }

                    VStack(alignment: .leading) {
                        HStack {
                            Text("Page Margins")
                            Spacer()
                            Text(settings.pageMargins > 0.0 ? String(format: "%.1f", settings.pageMargins) : "Default")
                                .foregroundStyle(.secondary)
                        }
                        Slider(value: $settings.pageMargins, in: 0.0...4.0, step: 0.1)
                    }

                    VStack(alignment: .leading) {
                        HStack {
                            Text("Paragraph Spacing")
                            Spacer()
                            Text(settings.paragraphSpacing > 0.0 ? String(format: "%.1f", settings.paragraphSpacing) : "Default")
                                .foregroundStyle(.secondary)
                        }
                        Slider(value: $settings.paragraphSpacing, in: 0.0...4.0, step: 0.1)
                    }

                    VStack(alignment: .leading) {
                        HStack {
                            Text("Word Spacing")
                            Spacer()
                            Text(settings.wordSpacing > 0.0 ? String(format: "%.1f", settings.wordSpacing) : "Default")
                                .foregroundStyle(.secondary)
                        }
                        Slider(value: $settings.wordSpacing, in: 0.0...2.0, step: 0.05)
                    }
                }

                Section {
                    Button("Reset All Reading Preferences") {
                        settings.resetReadingPreferences()
                    }
                    .foregroundStyle(.red)
                }

                Section("General") {
                    Picker("Appearance", selection: $settings.appearance) {
                        Text("System").tag("")
                        Text("Light").tag("light")
                        Text("Dark").tag("dark")
                    }
                }
            }
            .navigationTitle("Settings")
        }
    }
}
