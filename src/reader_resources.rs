// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! One resource namespace per reader. The engine resolves all EPUB-relative URLs.
use crate::*;
use day_piece_webview::{ResourceProvider, ResourceResponse, web_view_resources};
const CONTENT_POLICY: &str = "default-src 'none'; script-src 'none'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; media-src 'self' data:; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";
pub fn view(a: App) -> day_piece_webview::WebView {
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
                        serde_json::to_string(rows).unwrap()
                    )),
                    Err(error) => a.error(error.clone()),
                }
            },
        );
    }
    let provider = ResourceProvider::with_site(res::assets::reader, move |request| {
        let Some(book) = &book else {
            return ResourceResponse::not_found();
        };
        let Some(path) = request.path.strip_prefix("book/") else {
            return ResourceResponse::not_found();
        };
        let Some(mime) = book.resources.get(path) else {
            return ResourceResponse::not_found();
        };
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
        response
            .headers
            .push(("Content-Security-Policy".into(), CONTENT_POLICY.into()));
        response
    });
    web_view_resources(provider, "__day_assets/index.html")
        .on_external_link(move |url| a.on_link(url))
}
// EPUB content is XHTML. Strip active elements before the browser sees it; CSP independently
// denies scripts and remote requests. Keep image/CSS/font URLs exactly as the publisher wrote them.
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
}
