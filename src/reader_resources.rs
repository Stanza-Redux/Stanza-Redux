// SPDX-License-Identifier: GPL-2.0-or-later
//! One resource namespace per reader. The engine resolves all EPUB-relative URLs.
use crate::*;
use day_piece_webview::{ResourceProvider, ResourceResponse, web_view_resources};
const CONTENT_POLICY: &str = "default-src 'none'; script-src 'none'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; media-src 'self' data:; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";
pub fn view(a: App) -> day_piece_webview::WebView {
    a.reader_scope.set(Some(Scope::current()));
    let book = a.book.get_untracked();
    if let (Some(db), Some(book)) = (a.database.get_untracked(), &book) {
        // The query belongs to this reader scope. Insert/delete and cross-window updates
        // update the shell; there is no app-owned refresh signal or browser-side storage.
        let query = db.bookmark_query(&book.id);
        watch(
            move || {
                (
                    a.ready.get(),
                    query.try_collect().map_err(|e| e.to_string()),
                )
            },
            move |(ready, rows), _| {
                if !ready {
                    return;
                }
                match rows {
                    Ok(rows) => a.eval(format!(
                        "stanza.bookmarks({})",
                        serde_json::to_string(rows)
                            .expect("bookmark rows are plain fields, which serialize")
                    )),
                    Err(error) => a.error(error.clone()),
                }
            },
        );
    }
    // Only the UI task reads policy/locale signals. Providers read this immutable
    // byte snapshot on their worker; no resource callback touches UI state.
    let hidden_cover = Arc::new(std::sync::RwLock::new(
        book.as_ref()
            .and_then(|book| content_restrictions::reader_placeholder(book)),
    ));
    let policy_book = book.clone();
    let policy_cover = hidden_cover.clone();
    let pending = std::cell::Cell::new(false);
    watch(
        move || {
            (
                content_restrictions::revision(),
                day::locale().get(),
                a.ready.get(),
            )
        },
        move |(_, _, ready), _| {
            let next = policy_book
                .as_ref()
                .and_then(|book| content_restrictions::reader_placeholder(book));
            let mut current = policy_cover.write().expect("reader policy snapshot");
            if *current != next {
                *current = next;
                pending.set(true);
            }
            drop(current);
            if *ready && pending.replace(false) {
                a.eval("stanza.refreshContent()".into());
            }
        },
    );
    let provider = ResourceProvider::with_site(res::assets::reader, move |request| {
        let Some(book) = &book else {
            return ResourceResponse::not_found();
        };
        let Some(path) = request.path.strip_prefix("book/") else {
            return ResourceResponse::not_found();
        };
        let hidden = hidden_cover.read().expect("reader policy snapshot");
        book_response(book, path, hidden.as_deref())
    });
    web_view_resources(provider, "__day_assets/index.html")
        .on_external_link(move |url| a.on_link(url))
}
fn book_response(book: &epub::Book, path: &str, hidden_cover: Option<&[u8]>) -> ResourceResponse {
    let Some(mime) = book.resources.get(path) else {
        return ResourceResponse::not_found();
    };
    let cover = content_restrictions::cover_path(book.cover.as_deref(), path);
    if cover && let Some(hidden_cover) = hidden_cover {
        let mut response = ResourceResponse::new("image/svg+xml", hidden_cover.to_vec());
        response
            .headers
            .push(("Cache-Control".into(), "no-store".into()));
        response
            .headers
            .push(("Content-Security-Policy".into(), CONTENT_POLICY.into()));
        return response;
    }
    let Ok(bytes) = book.resource(path) else {
        return ResourceResponse::error(500);
    };
    let mut response = if mime.contains("html") || mime == "image/svg+xml" {
        let Some(html) = sanitize(&bytes) else {
            return ResourceResponse::error(422);
        };
        ResourceResponse::new(
            if mime.contains("html") {
                "text/html"
            } else {
                mime
            },
            html.into_bytes(),
        )
    } else {
        ResourceResponse::new(mime.clone(), bytes)
    };
    if cover {
        response
            .headers
            .push(("Cache-Control".into(), "no-store".into()));
    }
    response
        .headers
        .push(("Content-Security-Policy".into(), CONTENT_POLICY.into()));
    response
}
/// The HTML elements with no end tag, which keep their `<br/>` form. Any other XHTML element
/// written self-closed (`<a id="x"/>`, `<div/>`) would be read by the HTML parser as an open
/// tag that swallows what follows it.
const VOID_ELEMENTS: [&str; 13] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];
const XHTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";
// EPUB content is XHTML. Strip active elements before the browser sees it; CSP independently
// denies scripts and remote requests. Keep image/CSS/font URLs exactly as the publisher wrote them.
// Chapters are served as text/html, so the XHTML is also made to parse as written there: a
// doctype keeps the browser out of quirks mode (whose line layout packed a multi-line heading's
// lines together), and empty non-void elements get an end tag.
fn sanitize(bytes: &[u8]) -> Option<String> {
    let source = std::str::from_utf8(bytes).ok()?;
    let doc = roxmltree::Document::parse_with_options(
        source,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .ok()?;
    let mut edits = Vec::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        let name = node.tag_name().name().to_ascii_lowercase();
        if [
            "script",
            "iframe",
            "object",
            "embed",
            "form",
            "base",
            "animate",
            "animatetransform",
            "animatemotion",
            "set",
            "discard",
        ]
        .contains(&name.as_str())
            || (name == "meta"
                && node
                    .attributes()
                    .any(|a| a.name().eq_ignore_ascii_case("http-equiv")))
        {
            edits.push((node.range(), String::new()));
            continue;
        }
        for attr in node.attributes() {
            if attr.name().to_ascii_lowercase().starts_with("on")
                || attr.name().eq_ignore_ascii_case("srcdoc")
                || (matches!(attr.name(), "href" | "src" | "action")
                    && attr
                        .value()
                        .chars()
                        .filter(|c| !c.is_ascii_whitespace())
                        .collect::<String>()
                        .to_ascii_lowercase()
                        .starts_with("javascript:"))
            {
                edits.push((attr.range(), String::new()));
            }
        }
        let range = node.range();
        if node.tag_name().namespace() == Some(XHTML_NAMESPACE)
            && !node.has_children()
            && source[range.clone()].ends_with("/>")
            && !VOID_ELEMENTS.contains(&name.as_str())
        {
            // The name as the publisher wrote it, prefix included.
            let written = &source[range.start + 1..range.end];
            let qualified = written
                .split(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
                .next()
                .unwrap_or_default();
            edits.push((range.end - 2..range.end, format!("></{qualified}>")));
        }
    }
    // Qt before 6.6 cannot attach response headers. Put the same policy first in the
    // document head as well, before the parser can encounter publisher resources.
    let root = doc.root_element();
    let head = root
        .children()
        .find(|n| n.has_tag_name((root.tag_name().namespace().unwrap_or(""), "head")))
        .unwrap_or(root);
    let start = head.range().start;
    let insertion = start + source[start..].find('>')? + 1;
    if root.tag_name().name().eq_ignore_ascii_case("html") {
        edits.push((
            insertion..insertion,
            format!(r#"<meta http-equiv="Content-Security-Policy" content="{CONTENT_POLICY}">"#),
        ));
        let prolog = &source[..root.range().start];
        if !prolog.to_ascii_lowercase().contains("<!doctype") {
            // After a byte-order mark, which the HTML decoder strips only at the very start.
            let start = if source.starts_with('\u{feff}') { 3 } else { 0 };
            edits.push((start..start, String::from("<!DOCTYPE html>\n")));
        }
    }
    edits.sort_by_key(|(range, _)| (range.start, range.end));
    let mut result = String::with_capacity(source.len());
    let mut offset = 0;
    for (range, replacement) in edits {
        if range.start >= offset {
            result.push_str(&source[offset..range.start]);
            result.push_str(&replacement);
            offset = range.end;
        }
    }
    result.push_str(&source[offset..]);
    Some(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restricted_reader_serves_placeholder_without_mutating_epub_or_chapters() {
        let bytes = content_restrictions::cover_fixture(
            "https://standardebooks.org/ebooks/apuleius/the-golden-ass/william-adlington",
        );
        let book = epub::parse(&bytes).unwrap();
        let placeholder =
            b"<svg xmlns='http://www.w3.org/2000/svg'><text>Fixture notice</text></svg>";
        let hidden = content_restrictions::metadata_restricted(&book.metadata)
            .then_some(placeholder.as_slice());
        for path in ["images/cover.jpg", "images/cover.svg"] {
            let response = book_response(&book, path, hidden);
            assert_eq!(response.status, 200);
            if hidden.is_some() {
                assert_eq!(response.mime, "image/svg+xml");
                assert_eq!(response.body.as_ref(), placeholder);
            } else {
                assert_ne!(response.body.as_ref(), placeholder);
            }
        }
        assert_eq!(
            book.resource("images/cover.jpg").unwrap().as_ref(),
            b"original synthetic cover bytes"
        );
        let chapter = book_response(&book, "chapter.xhtml", hidden);
        assert!(
            std::str::from_utf8(&chapter.body)
                .unwrap()
                .contains("Preserved fixture text")
        );
        assert_eq!(book_response(&book, "missing.jpg", hidden).status, 404);
        let mut unrelated = book.clone();
        unrelated.metadata.identifiers.clear();
        assert_eq!(
            book_response(&unrelated, "images/cover.jpg", None)
                .body
                .as_ref(),
            b"original synthetic cover bytes"
        );
    }
    #[test]
    fn svg_images_keep_relative_references_but_not_executable_content() {
        let input=br#"<svg xmlns="http://www.w3.org/2000/svg" onload="bad()"><script>bad()</script><image href="../images/a.png"/><a href="javascript:bad()"><text>Fixture</text></a><set attributeName="href" to="javascript:bad()"/></svg>"#;
        let out = sanitize(input).unwrap();
        assert!(!out.contains("bad()"));
        assert!(out.contains("../images/a.png"));
        assert!(roxmltree::Document::parse(&out).is_ok());
    }
    #[test]
    fn sanitizes_active_content_without_rewriting_resources() {
        let source=br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><base href="https://evil.invalid/"/><link rel="stylesheet" href="../styles/a.css"/><script>bad()</script></head><body onload="bad()"><img src="../images/a.png" srcset="../images/b.png 2x"/><p>Fixture</p></body></html>"#;
        let out = sanitize(source).unwrap();
        assert!(!out.contains("bad()"));
        assert!(!out.contains("<base"));
        assert!(out.contains("../styles/a.css"));
        assert!(out.contains("srcset="));
        assert!(out.contains(CONTENT_POLICY));
    }
    #[test]
    fn xhtml_parses_as_written_in_standards_mode() {
        // Project Gutenberg's chapter shape: an XML declaration, no doctype, an empty anchor.
        let source = "<?xml version='1.0' encoding='utf-8'?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>T</title></head><body><h2><a id=\"chap01\"/>CHAPTER I.<br/>\nDown the Rabbit-Hole</h2><div class=\"x\"/><p>After</p><svg xmlns=\"http://www.w3.org/2000/svg\"><rect/></svg></body></html>";
        let out = sanitize(source.as_bytes()).unwrap();
        assert!(out.starts_with("<!DOCTYPE html>\n<?xml"));
        assert!(out.contains(r#"<a id="chap01"></a>CHAPTER I.<br/>"#));
        assert!(out.contains(r#"<div class="x"></div><p>After</p>"#));
        // Void elements and foreign content keep their self-closed form.
        assert!(out.contains("<rect/>"));
        // A publisher's own doctype stays the only one.
        let declared =
            "<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head/><body/></html>";
        let out = sanitize(declared.as_bytes()).unwrap();
        assert_eq!(out.matches("<!DOCTYPE").count(), 1);
        // SVG documents are served as SVG, with no doctype added.
        let svg = sanitize(br#"<svg xmlns="http://www.w3.org/2000/svg"><g/></svg>"#).unwrap();
        assert!(!svg.contains("DOCTYPE"));
    }
}
