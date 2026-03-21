// Copyright 2026 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import Foundation
import SkipXML

/// Utility for converting simple HTML content to Markdown.
public struct HTMLMarkdown {
    let string: String
    let options: Options

    /// Options controlling how HTML elements are converted to Markdown.
    public struct Options: OptionSet {
        public let rawValue: Int
        public init(rawValue: Int) { self.rawValue = rawValue }

        /// Links are rendered as plain text without href URLs.
        public static let noLinks = Options(rawValue: 1 << 0)
        /// Images are rendered as bracketed alt text (e.g. "[photo]") instead of Markdown image syntax.
        public static let noImages = Options(rawValue: 1 << 1)
    }

    public init(_ string: String, options: Options = []) {
        self.string = string
        self.options = options
    }

    /// Errors that can occur during HTML-to-Markdown conversion.
    public enum ConversionError: Error {
        case parseError(String)
    }

    /// Try to parse the given HTML String into markdown, and fall back to the basic text if there is an error
    /// - Parameter string: the potentially HTML-decorated text to put in the string
    /// - Returns: A AttributedString element with the parsed string, or just the verbatim string if there was an error
    public func attributedStringFromHTMLString() -> AttributedString {
        let start = Date.now
        guard containsHTML() else {
            return AttributedString(string)
        }

        do {
            let astr = try AttributedString(markdown: convertHTMLToMarkdown())
            let end = Date.now
            logger.debug("parsed HTML into markdown in \(string.count) bytes in \(end.timeIntervalSince(start)) seconds")
            return astr
        } catch {
            logger.error("Failed to parse HTML into markdown: \(error)")
            return AttributedString(string)
        }
    }

    /// Attempts to convert an HTML string to Markdown.
    ///
    /// The input is wrapped in a synthetic `<div>` element before parsing
    /// so that fragments without a single root element can be handled.
    /// If the input contains no HTML elements, it is returned unchanged.
    ///
    /// - Parameter html: The HTML string to convert.
    /// - Returns: The Markdown equivalent of the HTML content.
    /// - Throws: `ConversionError.parseError` if the content cannot be parsed as XML.
    public func convertHTMLToMarkdown() throws -> String {
        let wrapped = "<div>" + string + "</div>"
        let doc: XMLNode
        do {
            doc = try XMLNode.parse(string: wrapped)
        } catch {
            throw ConversionError.parseError("Failed to parse HTML: \(error)")
        }

        // The parsed document has a root element; the first child is our <div> wrapper
        guard let div = doc.elementChildren.first else {
            throw ConversionError.parseError("No root element found after parsing")
        }

        let result = convertChildren(of: div)
        return result.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Returns `true` if the string appears to contain HTML tags.
    public func containsHTML() -> Bool {
        return string.contains("<") && string.contains(">")
    }

    // MARK: - Private

    private func convertChildren(of node: XMLNode) -> String {
        var result = ""
        for child in node.children {
            switch child {
            case .element(let element):
                result += convertElement(element)
            case .content(let text):
                result += text
            case .whitespace(let ws):
                result += ws
            case .cdata(let data):
                if let text = String(data: data, encoding: .utf8) {
                    result += text
                }
            case .comment:
                break
            case .processingInstruction:
                break
            }
        }
        return result
    }

    private func convertElement(_ node: XMLNode) -> String {
        let tag = node.elementName.lowercased()
        let inner = convertChildren(of: node)

        switch tag {
        // Bold
        case "b", "strong":
            let trimmed = inner.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty { return "" }
            return "**" + trimmed + "**"

        // Italic
        case "i", "em":
            let trimmed = inner.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty { return "" }
            return "*" + trimmed + "*"

        // Bold + Italic (rare but valid)
        case "b > i", "strong > em":
            let trimmed = inner.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty { return "" }
            return "***" + trimmed + "***"

        // Strikethrough
        case "s", "strike", "del":
            let trimmed = inner.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty { return "" }
            return "~~" + trimmed + "~~"

        // Underline — no direct Markdown equivalent, use italic
        case "u":
            let trimmed = inner.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty { return "" }
            return "*" + trimmed + "*"

        // Line break
        case "br":
            return "\n"

        // Paragraph
        case "p":
            return "\n\n" + inner.trimmingCharacters(in: .whitespaces) + "\n\n"

        // Headings
        case "h1":
            return "\n\n# " + inner.trimmingCharacters(in: .whitespaces) + "\n\n"
        case "h2":
            return "\n\n## " + inner.trimmingCharacters(in: .whitespaces) + "\n\n"
        case "h3":
            return "\n\n### " + inner.trimmingCharacters(in: .whitespaces) + "\n\n"
        case "h4":
            return "\n\n#### " + inner.trimmingCharacters(in: .whitespaces) + "\n\n"
        case "h5":
            return "\n\n##### " + inner.trimmingCharacters(in: .whitespaces) + "\n\n"
        case "h6":
            return "\n\n###### " + inner.trimmingCharacters(in: .whitespaces) + "\n\n"

        // Links
        case "a":
            let href = node.attributes["href"] ?? ""
            let text = inner.trimmingCharacters(in: .whitespaces)
            if options.contains(.noLinks) {
                return text.isEmpty ? href : text
            }
            if href.isEmpty {
                return text
            }
            if text.isEmpty {
                return href
            }
            return "[" + text + "](" + href + ")"

        // Images
        case "img":
            if options.contains(.noImages) {
                let alt = node.attributes["alt"] ?? ""
                return alt.isEmpty ? "[image]" : "[" + alt + "]"
            }
            let src = node.attributes["src"] ?? ""
            let alt = node.attributes["alt"] ?? ""
            return "![" + alt + "](" + src + ")"

        // Code (inline)
        case "code":
            return "`" + inner + "`"

        // Preformatted / code block
        case "pre":
            return "\n\n```\n" + inner + "\n```\n\n"

        // Blockquote
        case "blockquote":
            let lines = inner.trimmingCharacters(in: .whitespacesAndNewlines)
                .components(separatedBy: "\n")
                .map { "> " + $0 }
                .joined(separator: "\n")
            return "\n\n" + lines + "\n\n"

        // Unordered list
        case "ul":
            return "\n" + inner + "\n"

        // Ordered list
        case "ol":
            return "\n" + inner + "\n"

        // List item
        case "li":
            return "- " + inner.trimmingCharacters(in: .whitespacesAndNewlines) + "\n"

        // Horizontal rule
        case "hr":
            return "\n\n---\n\n"

        // Div and span — transparent wrappers
        case "div", "span", "section", "article", "main", "header", "footer", "nav", "aside":
            return inner

        // Superscript / subscript — no Markdown equivalent, pass through
        case "sup", "sub", "small":
            return inner

        // Unknown elements — just use their text content
        default:
            return inner
        }
    }
}
