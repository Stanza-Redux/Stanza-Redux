// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
use crate::{error::AppError, res};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Link {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub url: String,
    pub mime: String,
    pub rel: String,
    /// Items behind a navigation or facet link (`numberOfItems`, Atom `thr:count`).
    #[serde(default)]
    pub count: Option<u64>,
    /// The facet that produced this feed (OPDS 2 `self`, Atom `opds:activeFacet`).
    #[serde(default)]
    pub active: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Publication {
    pub title: String,
    pub author: String,
    pub description: String,
    #[serde(default)]
    pub metadata: crate::metadata::Metadata,
    pub cover: String,
    pub links: Vec<Link>,
    /// The complete entry, when a feed lists only summaries (OPDS 2 publication or Atom entry).
    #[serde(default)]
    pub detail: String,
    /// Feeds about this book: its authors, subjects and related titles.
    #[serde(default)]
    pub browse: Vec<Link>,
    /// The catalog's web page for this book.
    #[serde(default)]
    pub web: String,
}
/// A titled shelf of an OPDS 2 grouped feed, with a link to its complete feed.
#[derive(Clone, Debug, Default)]
pub struct Group {
    pub title: String,
    pub total: Option<u64>,
    pub more: Option<String>,
    pub entries: Vec<Publication>,
}
/// One facet dimension, such as sort order or language. At most one link is active.
#[derive(Clone, Debug, Default)]
pub struct FacetGroup {
    pub title: String,
    pub links: Vec<Link>,
}
#[derive(Clone, Debug, Default)]
pub struct Feed {
    pub title: String,
    pub icon: String,
    pub description: String,
    pub updated: String,
    pub is_catalog: bool,
    /// Size of the complete result set, when the catalog reports it.
    pub total: Option<u64>,
    pub entries: Vec<Publication>,
    pub navigation: Vec<Link>,
    pub groups: Vec<Group>,
    pub facets: Vec<FacetGroup>,
    pub next: Option<String>,
    pub search: Option<Link>,
}
impl Feed {
    pub fn book_count(&self) -> usize {
        self.entries.len() + self.groups.iter().map(|g| g.entries.len()).sum::<usize>()
    }
    pub fn link_count(&self) -> usize {
        self.navigation.len()
    }
}
/// Use the catalog's own origin; never disclose subscriptions to an icon aggregator.
pub fn favicon_url(address: &str) -> String {
    let Ok(mut url) = Url::parse(address) else {
        return String::new();
    };
    if !matches!(url.scheme(), "http" | "https") {
        return String::new();
    }
    let path = match url.host_str() {
        Some("www.gutenberg.org" | "gutenberg.org") => "/gutenberg/favicon.ico",
        Some("standardebooks.org" | "www.standardebooks.org") => {
            "/images/favicons/favicon-64x64.png"
        }
        _ => "/favicon.ico",
    };
    url.set_path(path);
    url.set_query(None);
    url.set_fragment(None);
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.into()
}
pub fn catalog_host(address: &str) -> String {
    Url::parse(address)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_default()
}
fn text(v: &Value) -> String {
    v.as_str()
        .or_else(|| v.get("name").and_then(Value::as_str))
        .unwrap_or_default()
        .into()
}
fn many(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        Value::Null => vec![],
        _ => vec![v],
    }
}
pub fn resolve(base: &str, href: &str) -> Result<String, AppError> {
    // URL parsing would encode a template's braces and split `{?query}` at its `?`.
    if let Some(i) = href.find('{') {
        return Ok(resolve(base, &href[..i])? + &href[i..]);
    }
    let u = Url::parse(base)
        .and_then(|u| u.join(href))
        .map_err(|e| AppError::detail(res::str::catalog_invalid, e))?;
    if !matches!(u.scheme(), "https" | "http") {
        return Err(AppError::new(res::str::catalog_http_only));
    }
    Ok(u.into())
}
fn json_link(v: &Value, base: &str) -> Option<Link> {
    let rel = many(&v["rel"])
        .iter()
        .map(|v| text(v))
        .collect::<Vec<_>>()
        .join(" ");
    Some(Link {
        title: text(&v["title"]),
        description: crate::metadata::plain(&text(&v["description"])),
        url: resolve(base, v["href"].as_str()?).ok()?,
        mime: text(&v["type"]),
        count: v["properties"]["numberOfItems"].as_u64(),
        active: rel.split_whitespace().any(|r| r == "self"),
        rel,
    })
}
fn is_feed(mime: &str) -> bool {
    mime.contains("opds+json") || mime.contains("profile=opds-catalog")
}
/// Contributor and subject objects may link to feeds of their other works.
fn json_browse(m: &Value, base: &str) -> Vec<Link> {
    let mut links = Vec::new();
    for (key, rel) in [
        ("author", "author"),
        ("translator", "author"),
        ("illustrator", "author"),
        ("subject", "subject"),
    ] {
        for v in many(&m[key]) {
            let name = text(v);
            if let Some(mut link) = many(&v["links"])
                .into_iter()
                .filter_map(|l| json_link(l, base))
                .find(|l| is_feed(&l.mime) || l.mime.is_empty())
            {
                if name.is_empty() || links.iter().any(|l: &Link| l.url == link.url) {
                    continue;
                }
                link.title = name;
                link.rel = rel.into();
                links.push(link);
            }
        }
    }
    links
}
fn web_page(links: &[Link], identifiers: &[String]) -> String {
    links
        .iter()
        .find(|l| rel(l, "alternate") && l.mime.starts_with("text/html"))
        .map(|l| l.url.clone())
        .or_else(|| {
            identifiers
                .iter()
                .find(|i| i.starts_with("https://") || i.starts_with("http://"))
                .cloned()
        })
        .unwrap_or_default()
}
/// Cover lists are rendered at up to 3x on 46–96 point rows; prefer the smallest image of at
/// least 180 pixels, then a thumbnail, then any image.
fn cover(images: &[(Option<u64>, Link)]) -> String {
    images
        .iter()
        .filter(|(w, _)| w.is_some_and(|w| w >= 180))
        .min_by_key(|(w, _)| *w)
        .or_else(|| {
            images
                .iter()
                .min_by_key(|(w, l)| (!l.rel.contains("thumbnail"), w.unwrap_or(u64::MAX)))
        })
        .map(|(_, l)| l.url.clone())
        .unwrap_or_default()
}
fn json_publication(v: &Value, base: &str) -> Publication {
    let m = &v["metadata"];
    let links: Vec<Link> = many(&v["links"])
        .iter()
        .filter_map(|v| json_link(v, base))
        .collect();
    let identifiers: Vec<String> = many(&m["identifier"]).iter().map(|v| text(v)).collect();
    let mut browse = json_browse(m, base);
    browse.extend(
        links
            .iter()
            .filter(|l| rel(l, "related") && is_feed(&l.mime) && !l.title.is_empty())
            .cloned(),
    );
    Publication {
        detail: links
            .iter()
            .find(|l| rel(l, "self") && l.mime.contains("opds-publication+json"))
            .map(|l| l.url.clone())
            .unwrap_or_default(),
        web: web_page(&links, &identifiers),
        browse,
        title: text(&m["title"]),
        author: many(&m["author"])
            .iter()
            .map(|v| text(v))
            .collect::<Vec<_>>()
            .join(", "),
        description: crate::metadata::plain(&text(&m["description"])),
        metadata: crate::metadata::Metadata {
            authors: many(&m["author"])
                .iter()
                .map(|v| text(v))
                .filter(|s| !s.is_empty())
                .collect(),
            subtitle: text(&m["subtitle"]),
            subjects: many(&m["subject"])
                .iter()
                .map(|v| text(v))
                .filter(|s| !s.is_empty())
                .collect(),
            publisher: many(&m["publisher"])
                .iter()
                .map(|v| text(v))
                .collect::<Vec<_>>()
                .join(", "),
            published: text(&m["published"]),
            modified: text(&m["modified"]),
            languages: many(&m["language"]).iter().map(|v| text(v)).collect(),
            identifiers,
            contributors: ["translator", "editor", "illustrator", "contributor"]
                .iter()
                .flat_map(|k| many(&m[*k]).into_iter().map(|v| text(v)))
                .collect(),
            rights: text(&m["rights"]),
            pages: m["numberOfPages"].as_u64(),
            series: many(&m["belongsTo"]["series"])
                .iter()
                .map(|v| text(v))
                .collect::<Vec<_>>()
                .join(", "),
            ..Default::default()
        },
        cover: cover(
            &many(&v["images"])
                .iter()
                .filter_map(|v| json_link(v, base).map(|link| (v["width"].as_u64(), link)))
                .collect::<Vec<_>>(),
        ),
        links,
    }
}
fn rel(l: &Link, r: &str) -> bool {
    l.rel.split_whitespace().any(|x| x == r)
}
impl Publication {
    pub fn epub(&self) -> Option<&Link> {
        self.links.iter().find(|l| {
            l.mime.split(';').next() == Some("application/epub+zip")
                && (l.rel.is_empty()
                    || l.rel == "enclosure"
                    || l.rel.contains("opds-spec.org/acquisition"))
                && !["borrow", "buy", "subscribe"]
                    .iter()
                    .any(|k| l.rel.contains(&format!("acquisition/{k}")))
        })
    }
    /// The Internet Archive's feed advertises only the PDF of an open-access scan, though many
    /// scans also have an EPUB. Returns the item identifier; [`archive_epub`] finds the file.
    pub fn archive_item(&self) -> Option<String> {
        self.links.iter().find_map(|l| {
            if !l.rel.contains("acquisition/open-access") || !l.mime.starts_with("application/pdf")
            {
                return None;
            }
            let url = Url::parse(&l.url).ok()?;
            if url.host_str() != Some("archive.org") {
                return None;
            }
            let mut path = url.path_segments()?;
            let item = match (path.next(), path.next(), path.next(), path.next()) {
                (Some("services"), Some("opds"), Some("book"), Some(item)) => item,
                _ => return None,
            };
            item.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                .then(|| item.to_owned())
        })
    }
    /// Why no EPUB can be downloaded, from the acquisition kinds the catalog does offer.
    pub fn unavailable(&self) -> Unavailable {
        let acquisitions: Vec<&Link> = self
            .links
            .iter()
            .filter(|l| l.rel.contains("opds-spec.org/acquisition") && !l.rel.contains("sample"))
            .collect();
        if acquisitions.iter().any(|l| {
            ["borrow", "buy", "subscribe"]
                .iter()
                .any(|k| l.rel.contains(&format!("acquisition/{k}")))
        }) {
            Unavailable::Account
        } else if acquisitions
            .iter()
            .any(|l| l.mime.starts_with("application/pdf"))
        {
            Unavailable::Pdf
        } else if acquisitions
            .iter()
            .any(|l| l.mime.contains("audiobook") || l.mime.starts_with("audio/"))
        {
            Unavailable::Audio
        } else {
            Unavailable::Other
        }
    }
    /// Fill what a feed's summary lacked from the complete entry; the summary's values win.
    pub fn supplement(&mut self, full: Publication) {
        if self.description.is_empty() {
            self.description = full.description;
        }
        if self.cover.is_empty() {
            self.cover = full.cover;
        }
        if self.web.is_empty() {
            self.web = full.web;
        }
        self.metadata.supplement(&full.metadata);
        for l in full.links {
            if !self.links.iter().any(|o| o.url == l.url) {
                self.links.push(l)
            }
        }
        for l in full.browse {
            if !self.browse.iter().any(|o| o.url == l.url) {
                self.browse.push(l)
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unavailable {
    /// Borrowing or buying needs an account.
    Account,
    Pdf,
    Audio,
    Other,
}
fn top_link(f: &mut Feed, l: Link) {
    if rel(&l, "icon") {
        f.icon = l.url.clone();
    }
    if rel(&l, "next") {
        f.next = Some(l.url.clone())
    }
    if rel(&l, "search") {
        f.search = Some(l.clone())
    }
}
fn base_at(n: roxmltree::Node, base: &str) -> String {
    let mut b = base.to_owned();
    let chain: Vec<_> = n.ancestors().filter(|a| a.is_element()).collect();
    for a in chain.into_iter().rev() {
        if let Some(h) = a.attribute(("http://www.w3.org/XML/1998/namespace", "base")) {
            if let Ok(u) = resolve(&b, h) {
                b = u
            }
        }
    }
    b
}
// Feeds disagree on the OPDS namespace prefix and URI; match the attribute's local name.
fn opds_attribute<'a>(n: roxmltree::Node<'a, '_>, name: &str) -> Option<&'a str> {
    n.attributes().find(|a| a.name() == name).map(|a| a.value())
}
fn xml_link(n: roxmltree::Node, base: &str) -> Option<Link> {
    Some(Link {
        title: n.attribute("title").unwrap_or_default().into(),
        description: String::new(),
        url: resolve(&base_at(n, base), n.attribute("href")?).ok()?,
        mime: n.attribute("type").unwrap_or_default().into(),
        rel: n.attribute("rel").unwrap_or_default().into(),
        count: n
            .attribute(("http://purl.org/syndication/thread/1.0", "count"))
            .and_then(|c| c.trim().parse().ok()),
        active: opds_attribute(n, "activeFacet") == Some("true"),
    })
}
fn child(n: roxmltree::Node, name: &str) -> String {
    n.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
        .map(|n| {
            n.descendants()
                .filter_map(|n| n.text().filter(|_| n.is_text()))
                .collect()
        })
        .unwrap_or_default()
}
pub fn parse(bytes: &[u8], base: &str) -> Result<Feed, AppError> {
    if let Ok(v) = serde_json::from_slice::<Value>(bytes) {
        let mut f = Feed {
            title: text(&v["metadata"]["title"]),
            icon: favicon_url(base),
            description: crate::metadata::plain(&text(&v["metadata"]["description"])),
            updated: text(&v["metadata"]["modified"]),
            is_catalog: ["publications", "navigation", "groups"]
                .iter()
                .any(|k| v[*k].is_array()),
            ..Default::default()
        };
        if v.get("metadata").is_none() {
            return Err(AppError::new(res::str::catalog_invalid));
        }
        f.total = v["metadata"]["numberOfItems"].as_u64();
        for l in many(&v["links"]).iter().filter_map(|v| json_link(v, base)) {
            top_link(&mut f, l)
        }
        f.entries = many(&v["publications"])
            .iter()
            .map(|v| json_publication(v, base))
            .collect();
        f.navigation = many(&v["navigation"])
            .iter()
            .filter_map(|v| json_link(v, base))
            .collect();
        for group in many(&v["groups"]) {
            f.navigation.extend(
                many(&group["navigation"])
                    .iter()
                    .filter_map(|v| json_link(v, base)),
            );
            let entries: Vec<_> = many(&group["publications"])
                .iter()
                .map(|p| json_publication(p, base))
                .collect();
            if entries.is_empty() {
                continue;
            }
            let links: Vec<_> = many(&group["links"])
                .iter()
                .filter_map(|v| json_link(v, base))
                .collect();
            f.groups.push(Group {
                title: text(&group["metadata"]["title"]),
                total: group["metadata"]["numberOfItems"].as_u64(),
                more: links
                    .iter()
                    .find(|l| rel(l, "self"))
                    .or(links.first())
                    .map(|l| l.url.clone()),
                entries,
            });
        }
        for facet in many(&v["facets"]) {
            let links: Vec<_> = many(&facet["links"])
                .iter()
                .filter_map(|v| json_link(v, base))
                .collect();
            if !links.is_empty() {
                f.facets.push(FacetGroup {
                    title: text(&facet["metadata"]["title"]),
                    links,
                });
            }
        }
        // A publication document (OPDS 2 `application/opds-publication+json`), not a feed.
        if !f.is_catalog && v["links"].is_array() {
            f.entries.push(json_publication(&v, base))
        }
        return Ok(f);
    }
    let s =
        std::str::from_utf8(bytes).map_err(|e| AppError::detail(res::str::catalog_invalid, e))?;
    let d = roxmltree::Document::parse(s)
        .map_err(|e| AppError::detail(res::str::catalog_invalid, e))?;
    let root = d.root_element();
    if !matches!(root.tag_name().name(), "feed" | "entry") {
        return Err(AppError::new(res::str::catalog_invalid));
    }
    let mut f = Feed {
        title: child(root, "title"),
        icon: {
            let icon = child(root, "icon");
            if icon.is_empty() {
                favicon_url(base)
            } else {
                resolve(&base_at(root, base), &icon).unwrap_or_else(|_| favicon_url(base))
            }
        },
        description: crate::metadata::plain(&child(root, "subtitle")),
        updated: child(root, "updated"),
        is_catalog: root.tag_name().name() == "feed",
        total: child(root, "totalResults").trim().parse().ok(),
        ..Default::default()
    };
    for n in root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "link")
    {
        if let Some(l) = xml_link(n, base) {
            if l.rel.contains("facet") {
                let group = opds_attribute(n, "facetGroup").unwrap_or_default();
                match f.facets.iter_mut().find(|g| g.title == group) {
                    Some(g) => g.links.push(l),
                    None => f.facets.push(FacetGroup {
                        title: group.into(),
                        links: vec![l],
                    }),
                }
            } else {
                top_link(&mut f, l)
            }
        }
    }
    let entries: Vec<_> = if root.tag_name().name() == "entry" {
        vec![root]
    } else {
        root.children()
            .filter(|n| n.is_element() && n.tag_name().name() == "entry")
            .collect()
    };
    for n in entries {
        let title = child(n, "title");
        let links: Vec<_> = n
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "link")
            .filter_map(|n| xml_link(n, base))
            .collect();
        let author = n
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "author")
            .map(|n| child(n, "name"))
            .collect::<Vec<_>>()
            .join(", ");
        let cover = links
            .iter()
            .find(|l| l.rel.contains("image/thumbnail"))
            .or_else(|| links.iter().find(|l| l.rel.contains("image")))
            .map(|l| l.url.clone())
            .unwrap_or_default();
        let description = {
            let s = child(n, "summary");
            if s.is_empty() { child(n, "content") } else { s }
        };
        let identifiers = vec![child(n, "id")];
        let p = Publication {
            detail: links
                .iter()
                .find(|l| rel(l, "alternate") && l.mime.contains("type=entry"))
                .map(|l| l.url.clone())
                .unwrap_or_default(),
            web: web_page(&links, &identifiers),
            browse: links
                .iter()
                .filter(|l| rel(l, "related") && is_feed(&l.mime) && !l.title.is_empty())
                .cloned()
                .collect(),
            title: title.clone(),
            author,
            description: crate::metadata::plain(&description),
            metadata: crate::metadata::Metadata {
                authors: n
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "author")
                    .map(|n| child(n, "name"))
                    .collect(),
                subjects: n
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "category")
                    .filter_map(|n| n.attribute("label").or_else(|| n.attribute("term")))
                    .map(str::to_owned)
                    .collect(),
                publisher: child(n, "publisher"),
                published: child(n, "issued"),
                modified: child(n, "updated"),
                languages: n
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "language")
                    .filter_map(|n| n.text())
                    .map(str::to_owned)
                    .collect(),
                identifiers,
                rights: child(n, "rights"),
                contributors: n
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "contributor")
                    .map(|n| child(n, "name"))
                    .collect(),
                ..Default::default()
            },
            cover,
            links: links.clone(),
        };
        if p.epub().is_some() || links.iter().any(|l| l.rel.contains("acquisition")) {
            f.entries.push(p)
        } else {
            for mut l in links.into_iter().filter(|l| {
                l.mime.contains("atom") || l.mime.contains("opds") || l.rel == "subsection"
            }) {
                l.title = title.clone();
                l.description = p.description.clone();
                f.navigation.push(l)
            }
        }
    }
    Ok(f)
}
/// Expands a search template for `query`: OpenSearch parameters (`{searchTerms}`) and the
/// RFC 6570 expressions OPDS 2 catalogs publish (`{?query,title,author}`, `{&query}`, `{query}`).
/// The search terms fill `searchTerms`, `query` or `q`; other form-style variables are omitted.
pub fn search_url(template: &str, query: &str) -> Result<String, AppError> {
    let invalid = || AppError::new(res::str::search_invalid);
    let template = template
        .replace("%7B", "{")
        .replace("%7D", "}")
        .replace("%7b", "{")
        .replace("%7d", "}");
    let form: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let strict = percent_encoding::utf8_percent_encode(query, UNRESERVED).to_string();
    let mut out = String::new();
    let mut filled = false;
    let mut rest = template.as_str();
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let end = start + rest[start..].find('}').ok_or_else(invalid)?;
        let expression = &rest[start + 1..end];
        rest = &rest[end + 1..];
        let terms = |name: &str| matches!(name, "searchTerms" | "query" | "q");
        match expression.chars().next() {
            Some(op @ ('?' | '&')) => {
                let mut separator = op;
                for name in expression[1..].split(',') {
                    let name = name.split([':', '*']).next().unwrap_or_default();
                    if terms(name) {
                        out.push(separator);
                        out.push_str(name);
                        out.push('=');
                        out.push_str(&strict);
                        separator = '&';
                        filled = true;
                    }
                }
            }
            Some(c) if c.is_ascii_alphanumeric() || c == '_' => {
                let (name, optional) = match expression.strip_suffix('?') {
                    Some(name) => (name, true),
                    None => (expression, false),
                };
                // OpenSearch names may carry a namespace prefix (`{opds:searchTerms}`).
                let name = name.rsplit(':').next().unwrap_or(name);
                filled |= terms(name);
                match name {
                    "searchTerms" => out.push_str(&form),
                    "query" | "q" => out.push_str(&strict),
                    "count" if optional => out.push_str("30"),
                    "startIndex" if optional => out.push('0'),
                    "startPage" if optional => out.push('1'),
                    "language" if optional => out.push('*'),
                    _ if optional => {}
                    _ => return Err(invalid()),
                }
            }
            _ => return Err(invalid()),
        }
    }
    out.push_str(rest);
    if !filled {
        return Err(invalid());
    }
    resolve(&out, "")
}
/// RFC 3986 unreserved characters stay literal; RFC 6570 percent-encodes everything else.
const UNRESERVED: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');
/// The Internet Archive's file list for an item (its metadata API).
pub fn archive_files_url(item: &str) -> String {
    format!("https://archive.org/metadata/{item}/files")
}
/// The open EPUB in an Internet Archive file list. Encrypted loan copies are skipped.
pub fn archive_epub(item: &str, files: &[u8]) -> Option<Link> {
    let v: Value = serde_json::from_slice(files).ok()?;
    let name = many(&v["result"]).into_iter().find_map(|f| {
        let name = f["name"].as_str()?;
        (f["format"].as_str() == Some("EPUB")
            && name.ends_with(".epub")
            && !name.contains("_encrypted")
            && !name.contains("_lcp"))
        .then_some(name)
    })?;
    let mut url = Url::parse("https://archive.org/download/").ok()?;
    url.path_segments_mut()
        .ok()?
        .pop_if_empty()
        .push(item)
        .push(name);
    Some(Link {
        url: url.into(),
        mime: "application/epub+zip".into(),
        rel: "http://opds-spec.org/acquisition/open-access".into(),
        ..Default::default()
    })
}
pub fn open_search(bytes: &[u8], base: &str) -> Result<String, AppError> {
    let s =
        std::str::from_utf8(bytes).map_err(|e| AppError::detail(res::str::catalog_invalid, e))?;
    let d = roxmltree::Document::parse(s)
        .map_err(|e| AppError::detail(res::str::catalog_invalid, e))?;
    let n = d
        .descendants()
        .find(|n| {
            n.is_element()
                && n.tag_name().name() == "Url"
                && n.attribute("type")
                    .is_some_and(|s| s.contains("atom") || s.contains("opds"))
        })
        .ok_or_else(|| AppError::new(res::str::search_missing))?;
    resolve(
        base,
        n.attribute("template")
            .ok_or_else(|| AppError::new(res::str::search_missing))?,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    // Synthetic fixtures matching the public Gutenberg navigation/detail shapes.
    #[test]
    fn catalog_icons_and_navigation_descriptions() {
        let feed = parse(br#"<feed xmlns='http://www.w3.org/2005/Atom' xml:base='/catalog/'>
          <title>Fixture catalog</title><icon>icons/site.ico</icon>
          <link rel='search' type='application/opensearchdescription+xml' href='search.xml'/>
          <link rel='next' href='?page=2'/>
          <entry><title>Fixture book</title><content type='text'>Fixture author</content>
          <link rel='subsection' type='application/atom+xml;profile=opds-catalog' href='book.opds'/></entry>
        </feed>"#, "https://example.org/root").unwrap();
        assert_eq!(feed.icon, "https://example.org/catalog/icons/site.ico");
        assert_eq!(feed.navigation[0].description, "Fixture author");
        assert_eq!(
            feed.navigation[0].url,
            "https://example.org/catalog/book.opds"
        );
        assert_eq!(
            feed.next.as_deref(),
            Some("https://example.org/catalog/?page=2")
        );
        assert!(feed.search.is_some());
        let json = parse(br#"{"metadata":{"title":"Fixture"},"links":[{"rel":"icon","href":"icon.png"}],"navigation":[{"title":"Category","description":"<p>Fixture summary</p>","href":"books"}]}"#, "https://example.org/").unwrap();
        assert_eq!(json.icon, "https://example.org/icon.png");
        assert_eq!(json.navigation[0].description, "Fixture summary");
    }
    #[test]
    fn favicons_stay_on_origin_without_credentials_or_query() {
        assert_eq!(
            favicon_url("https://reader:secret@example.org:8443/opds?token=private#shelf"),
            "https://example.org:8443/favicon.ico"
        );
        assert_eq!(
            catalog_host("https://reader:secret@example.org/opds?token=private"),
            "example.org"
        );
        assert_eq!(
            favicon_url("https://www.gutenberg.org/ebooks/search.opds/"),
            "https://www.gutenberg.org/gutenberg/favicon.ico"
        );
        assert!(favicon_url("file:///tmp/catalog").is_empty());
        assert!(favicon_url("invalid").is_empty());
    }
    #[test]
    fn atom_relative_enclosure() {
        let f=parse(br#"<feed xmlns='http://www.w3.org/2005/Atom' xml:base='/books/'><title>Books</title><link rel='next' href='?page=2'/><entry><title>Alice</title><author><name>Carroll</name></author><link rel='enclosure' type='application/epub+zip' href='alice.epub'/></entry></feed>"#,"https://example.org/catalog").unwrap();
        assert_eq!(
            f.entries[0].epub().unwrap().url,
            "https://example.org/books/alice.epub"
        );
        assert_eq!(f.next.as_deref(), Some("https://example.org/books/?page=2"));
    }
    #[test]
    fn groups_facets() {
        let f=parse(br#"{"metadata":{"title":"Books"},"groups":[{"metadata":{"title":"New"},"publications":[{"metadata":{"title":"A","author":[{"name":"B"}]},"links":[{"href":"a.epub","type":"application/epub+zip","rel":"http://opds-spec.org/acquisition/open-access"}]}]}],"facets":[{"metadata":{"title":"Language"},"links":[{"title":"French","href":"fr"}]}]}"#,"https://example.org/").unwrap();
        assert!(f.entries.is_empty());
        assert_eq!(f.groups[0].title, "New");
        assert_eq!(f.groups[0].entries[0].author, "B");
        assert_eq!(f.facets[0].title, "Language");
        assert_eq!(f.facets[0].links[0].title, "French");
    }
    #[test]
    fn publication_metadata_and_sharp_cover() {
        let feed = parse(br#"{"metadata":{"title":"Catalog"},"publications":[{"metadata":{"title":"Long title","author":[{"name":"Ada"},{"name":"Paul"}],"subject":[{"name":"Fiction"}],"publisher":{"name":"Press"},"language":["en","fr"],"numberOfPages":123,"identifier":"isbn:123","description":"<p>A &amp; B</p>"},"images":[{"href":"large.jpg","width":1000},{"href":"medium.jpg","width":200},{"href":"small.jpg","width":100}],"links":[]}]}"#, "https://example.org/catalog").unwrap();
        let p = &feed.entries[0];
        assert_eq!(p.author, "Ada, Paul");
        assert_eq!(p.metadata.subjects, ["Fiction"]);
        assert_eq!(p.metadata.publisher, "Press");
        assert_eq!(p.metadata.languages, ["en", "fr"]);
        assert_eq!(p.metadata.pages, Some(123));
        assert_eq!(p.description, "A & B");
        assert_eq!(p.cover, "https://example.org/medium.jpg");
    }
    #[test]
    fn search_encoding() {
        assert_eq!(
            search_url("https://x/?q={searchTerms}", "a & é").unwrap(),
            "https://x/?q=a+%26+%C3%A9"
        );
        assert_eq!(
            search_url("https://x/?q=%7BsearchTerms%7D", "a & é").unwrap(),
            "https://x/?q=a+%26+%C3%A9"
        );
        assert!(resolve("https://x/", "javascript:alert(1)").is_err());
        assert!(search_url("https://x/?q={title}", "a").is_err());
        assert!(search_url("https://x/{?title,author}", "a").is_err());
        assert_eq!(
            search_url("https://x/?q={searchTerms}&n={count?}&l={lang?}", "a").unwrap(),
            "https://x/?q=a&n=30&l="
        );
    }
    // Trimmed live responses; see tests/fixtures/opds/README.md.
    const GUTENBERG_ROOT: &str = include_str!("../tests/fixtures/opds/gutenberg-root.json");
    const GUTENBERG_SEARCH: &str = include_str!("../tests/fixtures/opds/gutenberg-search.json");
    const GUTENBERG_PUBLICATION: &str =
        include_str!("../tests/fixtures/opds/gutenberg-publication.json");
    const ARCHIVE_ROOT: &str = include_str!("../tests/fixtures/opds/archive-root.json");
    const ARCHIVE_SEARCH: &str = include_str!("../tests/fixtures/opds/archive-search.json");
    const GUTENBERG: &str = "https://opds-test.pglaf.org/opds/";
    #[test]
    fn gutenberg_grouped_root() {
        let f = parse(GUTENBERG_ROOT.as_bytes(), GUTENBERG).unwrap();
        assert!(f.is_catalog);
        let titles: Vec<_> = f.navigation.iter().map(|l| l.title.as_str()).collect();
        assert_eq!(titles, ["Browse Bookshelves", "Browse Subjects"]);
        assert!(f.entries.is_empty());
        let shelves: Vec<_> = f.groups.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(shelves, ["Recently Added", "Most Popular"]);
        let popular = &f.groups[1];
        assert_eq!(popular.total, Some(78648));
        assert_eq!(
            popular.more.as_deref(),
            Some("https://opds-test.pglaf.org/opds/search?sort=downloads&sort_order=desc")
        );
        let book = &popular.entries[0];
        assert_eq!(book.title, "Pride and Prejudice");
        assert_eq!(book.author, "Austen, Jane");
        assert_eq!(
            book.epub().unwrap().url,
            "https://www.gutenberg.org/cache/epub/1342/pg1342-images-3.epub"
        );
        assert_eq!(
            book.cover,
            "https://www.gutenberg.org/cache/epub/1342/pg1342.cover.medium.jpg"
        );
        assert_eq!(
            book.detail,
            "https://opds-test.pglaf.org/opds/publications?id=1342"
        );
        assert_eq!(book.web, "https://www.gutenberg.org/ebooks/1342");
        let search = f.search.unwrap();
        assert_eq!(
            search_url(&search.url, "tale of two").unwrap(),
            "https://opds-test.pglaf.org/opds/search?query=tale%20of%20two"
        );
    }
    #[test]
    fn gutenberg_search_facets_and_paging() {
        let f = parse(GUTENBERG_SEARCH.as_bytes(), GUTENBERG).unwrap();
        assert_eq!(f.total, Some(277));
        assert_eq!(f.entries.len(), 2);
        assert_eq!(
            f.next.as_deref(),
            Some("https://opds-test.pglaf.org/opds/search?limit=25&query=dickens&page=2")
        );
        let groups: Vec<_> = f.facets.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(groups, ["Sort By", "Top Subjects in Results", "Language"]);
        let sort = &f.facets[0].links;
        assert!(sort[0].active && !sort[1].active);
        assert_eq!(sort[1].title, "Newest");
        let language = &f.facets[2].links;
        assert_eq!(language[1].title, "English");
        assert_eq!(language[1].count, Some(227));
    }
    #[test]
    fn gutenberg_publication_document() {
        let f = parse(
            GUTENBERG_PUBLICATION.as_bytes(),
            "https://opds-test.pglaf.org/opds/publications?id=79774",
        )
        .unwrap();
        assert!(!f.is_catalog);
        let p = &f.entries[0];
        assert_eq!(p.title, "The flaw");
        assert!(p.description.starts_with("Creators: Helen R. Hull"));
        assert!(p.metadata.subjects.iter().any(|s| s == "Short stories"));
        assert_eq!(p.metadata.publisher, "New York: Harper & Brothers, 1919.");
        let browse: Vec<_> = p
            .browse
            .iter()
            .map(|l| (l.rel.as_str(), l.title.as_str()))
            .collect();
        assert_eq!(browse[0], ("author", "Hull, Helen R. (Helen Rose)"));
        assert_eq!(browse[1], ("author", "Biggs, Walter"));
        assert!(browse.contains(&("subject", "Courtship -- Fiction")));
        assert_eq!(browse.last(), Some(&("related", "Readers also downloaded")));
        assert_eq!(
            p.browse[0].url,
            "https://opds-test.pglaf.org/opds/search?author_id=52995"
        );
        let mut summary = Publication {
            title: p.title.clone(),
            ..Default::default()
        };
        summary.supplement(p.clone());
        assert_eq!(summary.description, p.description);
        assert_eq!(summary.browse.len(), p.browse.len());
    }
    #[test]
    fn archive_root_and_search() {
        let f = parse(ARCHIVE_ROOT.as_bytes(), "https://archive.org/services/opds").unwrap();
        let titles: Vec<_> = f.navigation.iter().map(|l| l.title.as_str()).collect();
        assert_eq!(titles, ["eBooks", "Audiobooks"]);
        assert_eq!(f.groups.len(), 2);
        assert_eq!(f.groups[0].total, Some(3666721));
        let audiobook = &f.groups[1].entries[0];
        assert!(audiobook.epub().is_none() && audiobook.archive_item().is_none());
        assert_eq!(audiobook.unavailable(), Unavailable::Audio);
        assert_eq!(
            search_url(&f.search.unwrap().url, "jane austen").unwrap(),
            "https://archive.org/services/opds/catalog?query=jane%20austen&type=search"
        );

        let f = parse(
            ARCHIVE_SEARCH.as_bytes(),
            "https://archive.org/services/opds/catalog?type=search",
        )
        .unwrap();
        assert_eq!(f.total, Some(858));
        assert_eq!(f.facets[0].title, "Browse by Language");
        let loan = f
            .entries
            .iter()
            .find(|p| p.web.ends_with("prideprejudiceba00jane"))
            .unwrap();
        assert!(loan.epub().is_none() && loan.archive_item().is_none());
        assert_eq!(loan.unavailable(), Unavailable::Account);
        let scan = f
            .entries
            .iter()
            .find(|p| p.web.ends_with("prideprejudiceby0000jane"))
            .unwrap();
        assert!(scan.epub().is_none());
        assert_eq!(scan.unavailable(), Unavailable::Pdf);
        let item = scan.archive_item().unwrap();
        assert_eq!(item, "prideprejudiceby0000jane");
        assert_eq!(
            archive_files_url(&item),
            "https://archive.org/metadata/prideprejudiceby0000jane/files"
        );
        // Shape of the metadata API's file list.
        let files = br#"{"result":[{"name":"x.pdf","format":"Text PDF"},{"name":"x_lcp.epub","format":"EPUB"},{"name":"E07124 Pride.epub","format":"EPUB"}]}"#;
        let epub = archive_epub("dli.ministry.18962", files).unwrap();
        assert_eq!(
            epub.url,
            "https://archive.org/download/dli.ministry.18962/E07124%20Pride.epub"
        );
        assert!(
            archive_epub("x", br#"{"result":[{"name":"x.pdf","format":"Text PDF"}]}"#).is_none()
        );
        assert_eq!(epub.mime, "application/epub+zip");
    }
}
