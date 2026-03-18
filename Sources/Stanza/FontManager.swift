// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
#if !SKIP
import ReadiumNavigator
import ReadiumShared
#else
// No special imports needed for Android; Readium's FontFamily(name) resolves
// custom fonts from Android assets automatically when the TTF is bundled.
#endif

/// A single font entry with display name and the tag stored in settings / passed to Readium.
struct FontEntry: Identifiable {
    let name: String
    let tag: String

    var id: String { tag }
}

/// Central registry of available reading fonts, used by both SettingsView and ReaderView.
enum FontManager {
    /// Platform system fonts suitable for reading.
    static var systemFonts: [FontEntry] {
        #if SKIP
        return [
            FontEntry(name: "Roboto", tag: "Roboto"),
            FontEntry(name: "Roboto Slab", tag: "Roboto Slab"),
            FontEntry(name: "Serif", tag: "serif"),
            FontEntry(name: "Sans-Serif", tag: "sans-serif"),
        ]
        #else
        return [
            FontEntry(name: "Athelas", tag: "Athelas"),
            FontEntry(name: "Charter", tag: "Charter"),
            FontEntry(name: "Georgia", tag: "Georgia"),
            FontEntry(name: "Iowan Old Style", tag: "Iowan Old Style"),
            FontEntry(name: "Palatino", tag: "Palatino"),
            FontEntry(name: "Seravek", tag: "Seravek"),
            FontEntry(name: "New York", tag: "New York"),
            FontEntry(name: "San Francisco", tag: "SF Pro"),
        ]
        #endif
    }

    /// Custom fonts bundled with the app (both platforms).
    /// Each entry pairs a display name / Readium tag with the resource filename.
    static let customFonts: [(entry: FontEntry, filename: String)] = [
        (FontEntry(name: "Montserrat", tag: "Montserrat"), "Montserrat-Regular.ttf"),
        (FontEntry(name: "Noto Serif", tag: "Noto Serif"), "NotoSerif.ttf"),
        (FontEntry(name: "Noto Sans", tag: "Noto Sans"), "NotoSans.ttf"),
    ]

    /// All available fonts: publisher default, then system fonts, then custom fonts.
    static var allFonts: [FontEntry] {
        var fonts = [FontEntry(name: "Default", tag: "")]
        fonts += systemFonts
        fonts += customFonts.map(\.entry)
        return fonts
    }

    // MARK: - iOS Readium font declarations

    #if !SKIP
    /// Builds `CSSFontFamilyDeclaration` entries for each custom font so that
    /// Readium injects the necessary `@font-face` CSS into the EPUB WebView.
    static var fontFamilyDeclarations: [AnyHTMLFontFamilyDeclaration] {
        customFonts.compactMap { custom in
            let parts = custom.filename.split(separator: ".")
            let name = String(parts.first ?? "")
            let ext = parts.count > 1 ? String(parts.last!) : nil
            guard let url = Bundle.module.url(forResource: name, withExtension: ext) else {
                return nil
            }
            let fileURL = FileURL(url: url)!
            let face = CSSFontFace(file: fileURL)
            let decl = CSSFontFamilyDeclaration(
                fontFamily: FontFamily(rawValue: custom.entry.tag),
                fontFaces: [face]
            )
            return decl.eraseToAnyHTMLFontFamilyDeclaration()
        }
    }
    #endif
}
