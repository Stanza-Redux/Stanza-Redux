// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel
import AppFairUI

struct SettingsView: View {
    @Environment(StanzaSettings.self) var settings: StanzaSettings

    var body: some View {
        @Bindable var settings = settings
        AppFairSettings(bundle: .module) {
                Section {
                    Picker(selection: $settings.appearance) {
                        Text("System", bundle: .module).tag("")
                        Text("Light", bundle: .module).tag("light")
                        Text("Dark", bundle: .module).tag("dark")
                    } label: {
                        Text("Appearance", bundle: .module)
                    }
                    .accessibilityIdentifier("appearancePicker")
                    Picker(selection: $settings.readingTheme) {
                        Text("Original", bundle: .module).tag("original")
                        Text("Parchment", bundle: .module).tag("parchment")
                        Text("Cloister", bundle: .module).tag("cloister")
                        Text("Reverie", bundle: .module).tag("reverie")
                        Text("Sylvan", bundle: .module).tag("sylvan")
                        Text("Meridian", bundle: .module).tag("meridian")
                        Text("Vesper", bundle: .module).tag("vesper")
                        Text("Aurora", bundle: .module).tag("aurora")
                        Text("Solitude", bundle: .module).tag("solitude")
                    } label: {
                        Text("Reading Theme", bundle: .module)
                    }
                    .accessibilityIdentifier("readingThemePicker")
                    HStack {
                        Text("Font Size", bundle: .module)
                        Spacer()
                        Text("\(Int(settings.fontSize * 100))%")
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("fontSizeValue")
                    }
                    Slider(value: $settings.fontSize, in: 0.5...3.0, step: 0.1)
                        .accessibilityIdentifier("fontSizeSlider")
                        .accessibilityLabel(Text("Font Size", bundle: .module))
                    if settings.fontSize != 1.0 {
                        Button {
                            settings.fontSize = 1.0
                        } label: {
                            Text("Reset Font Size", bundle: .module)
                        }
                        .accessibilityIdentifier("resetFontSizeButton")
                    }
                    Picker(selection: $settings.fontFamily) {
                        ForEach(FontManager.allFonts) { font in
                            Text(font.name == "Default" ? "Publisher Default" : font.name).tag(font.tag)
                        }
                    } label: {
                        Text("Font", bundle: .module)
                    }
                    .accessibilityIdentifier("fontPicker")

                    Toggle(isOn: $settings.animatePageTurns) {
                        Text("Animate Page Turns", bundle: .module)
                    }
                        .accessibilityIdentifier("animatePageTurnsToggle")
                    Toggle(isOn: $settings.leftTapAdvances) {
                        Text("Left Tap Advances", bundle: .module)
                    }
                        .accessibilityIdentifier("leftTapAdvancesToggle")
                    Toggle(isOn: $settings.hideStatusBarInReader) {
                        Text("Hide Status Bar in Reader", bundle: .module)
                    }
                        .accessibilityIdentifier("hideStatusBarToggle")
                    Toggle(isOn: $settings.keepScreenOn) {
                        Text("Keep Screen On While Reading", bundle: .module)
                    }
                        .accessibilityIdentifier("keepScreenOnToggle")
                    Toggle(isOn: $settings.useInAppBrowser) {
                        Text("Open Web Pages in Embedded Browser", bundle: .module)
                    }
                        .accessibilityIdentifier("useInAppBrowserToggle")
                } header: {
                    Text("Reading", bundle: .module)
                }

                Section {
                    Toggle(isOn: $settings.ttsHighlightUtterance) {
                        Text("Highlight Spoken Text", bundle: .module)
                    }
                        .accessibilityIdentifier("ttsHighlightToggle")
                    Toggle(isOn: $settings.ttsAutoTurnPages) {
                        Text("Auto-Turn Pages", bundle: .module)
                    }
                        .accessibilityIdentifier("ttsAutoTurnToggle")
                    Toggle(isOn: $settings.ttsScrollMode) {
                        Text("Switch to Scroll Mode While Reading Aloud", bundle: .module)
                    }
                        .accessibilityIdentifier("ttsScrollModeToggle")
                } header: {
                    Text("Text-to-Speech", bundle: .module)
                }

                Section {
                    Picker(selection: $settings.columnCount) {
                        Text("Auto", bundle: .module).tag("")
                        Text("One", bundle: .module).tag("1")
                        Text("Two", bundle: .module).tag("2")
                    } label: {
                        Text("Columns", bundle: .module)
                    }
                    .accessibilityIdentifier("columnsPicker")

                    Picker(selection: $settings.fit) {
                        Text("Auto", bundle: .module).tag("")
                        Text("Page", bundle: .module).tag("page")
                        Text("Width", bundle: .module).tag("width")
                    } label: {
                        Text("Content Fit", bundle: .module)
                    }
                    .accessibilityIdentifier("contentFitPicker")

                    Picker(selection: $settings.hyphens) {
                        Text("Default", bundle: .module).tag("")
                        Text("On", bundle: .module).tag("true")
                        Text("Off", bundle: .module).tag("false")
                    } label: {
                        Text("Hyphenation", bundle: .module)
                    }
                    .accessibilityIdentifier("hyphenationPicker")

                    Picker(selection: $settings.textAlign) {
                        Text("Default", bundle: .module).tag("")
                        Text("Start", bundle: .module).tag("start")
                        Text("Left", bundle: .module).tag("left")
                        Text("Center", bundle: .module).tag("center")
                        Text("Right", bundle: .module).tag("right")
                        Text("Justify", bundle: .module).tag("justify")
                    } label: {
                        Text("Text Alignment", bundle: .module)
                    }
                    .accessibilityIdentifier("textAlignmentPicker")

                    Picker(selection: $settings.textNormalization) {
                        Text("Default", bundle: .module).tag("")
                        Text("On", bundle: .module).tag("true")
                        Text("Off", bundle: .module).tag("false")
                    } label: {
                        Text("Text Normalization", bundle: .module)
                    }
                    .accessibilityIdentifier("textNormalizationPicker")

                    Picker(selection: $settings.publisherStyles) {
                        Text("Default", bundle: .module).tag("")
                        Text("On", bundle: .module).tag("true")
                        Text("Off", bundle: .module).tag("false")
                    } label: {
                        Text("Publisher Styles", bundle: .module)
                    }
                    .accessibilityIdentifier("publisherStylesPicker")
                } header: {
                    Text("Text Layout", bundle: .module)
                }

                Section {
                    VStack(alignment: .leading) {
                        HStack {
                            Text("Line Height", bundle: .module)
                            Spacer()
                            Group {
                                if settings.lineHeight > 0.0 {
                                    Text(String(format: "%.1f", settings.lineHeight))
                                } else {
                                    Text("Default", bundle: .module)
                                }
                            }
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("lineHeightValue")
                        }
                        Slider(value: $settings.lineHeight, in: 0.0...3.0, step: 0.1)
                            .accessibilityIdentifier("lineHeightSlider")
                            .accessibilityLabel(Text("Line Height", bundle: .module))
                    }

                    VStack(alignment: .leading) {
                        HStack {
                            Text("Page Margins", bundle: .module)
                            Spacer()
                            Group {
                                if settings.pageMargins > 0.0 {
                                    Text(String(format: "%.1f", settings.pageMargins))
                                } else {
                                    Text("Default", bundle: .module)
                                }
                            }
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("pageMarginsValue")
                        }
                        Slider(value: $settings.pageMargins, in: 0.0...4.0, step: 0.1)
                            .accessibilityIdentifier("pageMarginsSlider")
                            .accessibilityLabel(Text("Page Margins", bundle: .module))
                    }

                    VStack(alignment: .leading) {
                        HStack {
                            Text("Paragraph Spacing", bundle: .module)
                            Spacer()
                            Group {
                                if settings.paragraphSpacing > 0.0 {
                                    Text(String(format: "%.1f", settings.paragraphSpacing))
                                } else {
                                    Text("Default", bundle: .module)
                                }
                            }
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("paragraphSpacingValue")
                        }
                        Slider(value: $settings.paragraphSpacing, in: 0.0...4.0, step: 0.1)
                            .accessibilityIdentifier("paragraphSpacingSlider")
                            .accessibilityLabel(Text("Paragraph Spacing", bundle: .module))
                    }

                    VStack(alignment: .leading) {
                        HStack {
                            Text("Word Spacing", bundle: .module)
                            Spacer()
                            Group {
                                if settings.wordSpacing > 0.0 {
                                    Text(String(format: "%.1f", settings.wordSpacing))
                                } else {
                                    Text("Default", bundle: .module)
                                }
                            }
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("wordSpacingValue")
                        }
                        Slider(value: $settings.wordSpacing, in: 0.0...2.0, step: 0.05)
                            .accessibilityIdentifier("wordSpacingSlider")
                            .accessibilityLabel(Text("Word Spacing", bundle: .module))
                    }
                } header: {
                    Text("Spacing", bundle: .module)
                }

                Section {
                    NavigationLink {
                        AdvancedSettingsView()
                    } label: {
                        Text("Advanced Settings", bundle: .module)
                    }
                    .accessibilityIdentifier("advancedSettingsLink")

                    Button {
                        settings.resetReadingPreferences()
                    } label: {
                        Text("Reset All Reading Preferences", bundle: .module)
                    }
                    .foregroundStyle(.red)
                    .accessibilityIdentifier("resetAllPreferencesButton")
                }
            }
            .navigationTitle(Text("Settings", bundle: .module))
    }
}

struct AdvancedSettingsView: View {
    @Environment(StanzaSettings.self) var settings: StanzaSettings
    @Environment(ErrorManager.self) var errorManager: ErrorManager

    var body: some View {
        @Bindable var settings = settings
        Form {
/*
            Section {
                Toggle("Enable Catalogs", isOn: $settings.enableCatalogs)
                    .accessibilityIdentifier("enableCatalogsToggle")
            } footer: {
                Text("Show the Catalogs tab for browsing and downloading books from OPDS catalogs.")
            }
*/

//            Section("Error Alert Testing") {
//                Button("Simple Error") {
//                    errorManager.errorOccurred(info: AppErrorInfo(
//                        message: "This is a simple test error."
//                    ))
//                }
//                Button("Error with Title") {
//                    errorManager.errorOccurred(info: AppErrorInfo(
//                        title: "Network Failure",
//                        message: "Could not connect to the server. Please check your internet connection and try again."
//                    ))
//                }
//                Button("Error with Code") {
//                    errorManager.errorOccurred(info: AppErrorInfo(
//                        title: "Database Error",
//                        message: "Failed to write book record.",
//                        code: 1032
//                    ))
//                }
//                Button("Error with Help URL") {
//                    errorManager.errorOccurred(info: AppErrorInfo(
//                        title: "Import Failed",
//                        message: "The EPUB file appears to be corrupted or in an unsupported format.",
//                        helpURL: URL(string: "https://github.com/nicklama/Stanza-Redux/issues")
//                    ))
//                }
//                Button("Error from NSError") {
//                    let nsError = NSError(domain: "org.appfair.stanza", code: 404, userInfo: [
//                        NSLocalizedDescriptionKey: "The requested resource was not found."
//                    ])
//                    errorManager.errorOccurred(info: AppErrorInfo(
//                        title: "Not Found",
//                        error: nsError,
//                        code: 404
//                    ))
//                }
//                Button("Error from Background Thread") {
//                    Task.detached {
//                        // Simulate work on background thread
//                        try? await Task.sleep(nanoseconds: 50_000_000)
//                        errorManager.errorOccurred(info: AppErrorInfo(
//                            title: "Background Task Failed",
//                            message: "An operation running in the background encountered an error."
//                        ))
//                    }
//                }
//            }
        }
        .navigationTitle(Text("Advanced Settings", bundle: .module))
    }
}
