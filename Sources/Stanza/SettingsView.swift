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
                    Picker("Appearance", selection: $settings.appearance) {
                        Text("System").tag("")
                        Text("Light").tag("light")
                        Text("Dark").tag("dark")
                    }
                    .accessibilityIdentifier("appearancePicker")
                    Toggle("Sepia Theme", isOn: $settings.sepiaTheme)
                        .accessibilityIdentifier("sepiaThemeToggle")
                    HStack {
                        Text("Font Size")
                        Spacer()
                        Text("\(Int(settings.fontSize * 100))%")
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("fontSizeValue")
                    }
                    Slider(value: $settings.fontSize, in: 0.5...3.0, step: 0.1)
                        .accessibilityIdentifier("fontSizeSlider")
                        .accessibilityLabel("Font Size")
                    if settings.fontSize != 1.0 {
                        Button("Reset Font Size") {
                            settings.fontSize = 1.0
                        }
                        .accessibilityIdentifier("resetFontSizeButton")
                    }
                    Picker("Font", selection: $settings.fontFamily) {
                        ForEach(FontManager.allFonts) { font in
                            Text(font.name == "Default" ? "Publisher Default" : font.name).tag(font.tag)
                        }
                    }
                    .accessibilityIdentifier("fontPicker")

                    Toggle("Animate Page Turns", isOn: $settings.animatePageTurns)
                        .accessibilityIdentifier("animatePageTurnsToggle")
                    Toggle("Left Tap Advances", isOn: $settings.leftTapAdvances)
                        .accessibilityIdentifier("leftTapAdvancesToggle")
                    Toggle("Hide Status Bar in Reader", isOn: $settings.hideStatusBarInReader)
                        .accessibilityIdentifier("hideStatusBarToggle")
                    Toggle("Open Web Pages in Embedded Browser", isOn: $settings.useInAppBrowser)
                        .accessibilityIdentifier("useInAppBrowserToggle")
                }

                Section("Text Layout") {
                    Picker("Columns", selection: $settings.columnCount) {
                        Text("Auto").tag("")
                        Text("One").tag("1")
                        Text("Two").tag("2")
                    }
                    .accessibilityIdentifier("columnsPicker")

                    Picker("Content Fit", selection: $settings.fit) {
                        Text("Auto").tag("")
                        Text("Page").tag("page")
                        Text("Width").tag("width")
                    }
                    .accessibilityIdentifier("contentFitPicker")

                    Picker("Hyphenation", selection: $settings.hyphens) {
                        Text("Default").tag("")
                        Text("On").tag("true")
                        Text("Off").tag("false")
                    }
                    .accessibilityIdentifier("hyphenationPicker")

                    Picker("Text Alignment", selection: $settings.textAlign) {
                        Text("Default").tag("")
                        Text("Start").tag("start")
                        Text("Left").tag("left")
                        Text("Center").tag("center")
                        Text("Right").tag("right")
                        Text("Justify").tag("justify")
                    }
                    .accessibilityIdentifier("textAlignmentPicker")

                    Picker("Text Normalization", selection: $settings.textNormalization) {
                        Text("Default").tag("")
                        Text("On").tag("true")
                        Text("Off").tag("false")
                    }
                    .accessibilityIdentifier("textNormalizationPicker")

                    Picker("Publisher Styles", selection: $settings.publisherStyles) {
                        Text("Default").tag("")
                        Text("On").tag("true")
                        Text("Off").tag("false")
                    }
                    .accessibilityIdentifier("publisherStylesPicker")
                }

                Section("Spacing") {
                    VStack(alignment: .leading) {
                        HStack {
                            Text("Line Height")
                            Spacer()
                            Text(settings.lineHeight > 0.0 ? String(format: "%.1f", settings.lineHeight) : "Default")
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("lineHeightValue")
                        }
                        Slider(value: $settings.lineHeight, in: 0.0...3.0, step: 0.1)
                            .accessibilityIdentifier("lineHeightSlider")
                            .accessibilityLabel("Line Height")
                    }

                    VStack(alignment: .leading) {
                        HStack {
                            Text("Page Margins")
                            Spacer()
                            Text(settings.pageMargins > 0.0 ? String(format: "%.1f", settings.pageMargins) : "Default")
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("pageMarginsValue")
                        }
                        Slider(value: $settings.pageMargins, in: 0.0...4.0, step: 0.1)
                            .accessibilityIdentifier("pageMarginsSlider")
                            .accessibilityLabel("Page Margins")
                    }

                    VStack(alignment: .leading) {
                        HStack {
                            Text("Paragraph Spacing")
                            Spacer()
                            Text(settings.paragraphSpacing > 0.0 ? String(format: "%.1f", settings.paragraphSpacing) : "Default")
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("paragraphSpacingValue")
                        }
                        Slider(value: $settings.paragraphSpacing, in: 0.0...4.0, step: 0.1)
                            .accessibilityIdentifier("paragraphSpacingSlider")
                            .accessibilityLabel("Paragraph Spacing")
                    }

                    VStack(alignment: .leading) {
                        HStack {
                            Text("Word Spacing")
                            Spacer()
                            Text(settings.wordSpacing > 0.0 ? String(format: "%.1f", settings.wordSpacing) : "Default")
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("wordSpacingValue")
                        }
                        Slider(value: $settings.wordSpacing, in: 0.0...2.0, step: 0.05)
                            .accessibilityIdentifier("wordSpacingSlider")
                            .accessibilityLabel("Word Spacing")
                    }
                }

                Section {
                    NavigationLink("Advanced Settings") {
                        AdvancedSettingsView()
                    }
                    .accessibilityIdentifier("advancedSettingsLink")

                    Button("Reset All Reading Preferences") {
                        settings.resetReadingPreferences()
                    }
                    .foregroundStyle(.red)
                    .accessibilityIdentifier("resetAllPreferencesButton")
                }
            }
            .navigationTitle("Settings")
        }
    }
}

struct AdvancedSettingsView: View {
    @Environment(StanzaSettings.self) var settings: StanzaSettings
    @Environment(ErrorManager.self) var errorManager: ErrorManager

    var body: some View {
        @Bindable var settings = settings
        Form {
            Section {
                Toggle("Enable Catalogs", isOn: $settings.enableCatalogs)
                    .accessibilityIdentifier("enableCatalogsToggle")
            } footer: {
                Text("Show the Catalogs tab for browsing and downloading books from OPDS catalogs.")
            }

            Section("Error Alert Testing") {
                Button("Simple Error") {
                    errorManager.errorOccurred(info: AppErrorInfo(
                        message: "This is a simple test error."
                    ))
                }
                Button("Error with Title") {
                    errorManager.errorOccurred(info: AppErrorInfo(
                        title: "Network Failure",
                        message: "Could not connect to the server. Please check your internet connection and try again."
                    ))
                }
                Button("Error with Code") {
                    errorManager.errorOccurred(info: AppErrorInfo(
                        title: "Database Error",
                        message: "Failed to write book record.",
                        code: 1032
                    ))
                }
                Button("Error with Help URL") {
                    errorManager.errorOccurred(info: AppErrorInfo(
                        title: "Import Failed",
                        message: "The EPUB file appears to be corrupted or in an unsupported format.",
                        helpURL: URL(string: "https://github.com/nicklama/Stanza-Redux/issues")
                    ))
                }
                Button("Error from NSError") {
                    let nsError = NSError(domain: "org.appfair.stanza", code: 404, userInfo: [
                        NSLocalizedDescriptionKey: "The requested resource was not found."
                    ])
                    errorManager.errorOccurred(info: AppErrorInfo(
                        title: "Not Found",
                        error: nsError,
                        code: 404
                    ))
                }
                Button("Error from Background Thread") {
                    Task.detached {
                        // Simulate work on background thread
                        try? await Task.sleep(nanoseconds: 50_000_000)
                        errorManager.errorOccurred(info: AppErrorInfo(
                            title: "Background Task Failed",
                            message: "An operation running in the background encountered an error."
                        ))
                    }
                }
            }
        }
        .navigationTitle("Advanced Settings")
    }
}
