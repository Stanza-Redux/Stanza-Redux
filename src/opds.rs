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
}
#[derive(Clone, Debug, Default)]
pub struct Feed {
    pub title: String,
    pub icon: String,
    pub description: String,
    pub updated: String,
    pub is_catalog: bool,
    pub entries: Vec<Publication>,
    pub navigation: Vec<Link>,
    pub facets: Vec<Link>,
    pub next: Option<String>,
    pub search: Option<Link>,
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
    let u = Url::parse(base)
        .and_then(|u| u.join(href))
        .map_err(|e| AppError::detail(res::str::catalog_invalid, e))?;
    if !matches!(u.scheme(), "https" | "http") {
        return Err(AppError::new(res::str::catalog_http_only));
    }
    Ok(u.into())
}
fn json_link(v: &Value, base: &str) -> Option<Link> {
    Some(Link {
        title: text(&v["title"]),
        description: crate::metadata::plain(&text(&v["description"])),
        url: resolve(base, v["href"].as_str()?).ok()?,
        mime: text(&v["type"]),
        rel: many(&v["rel"])
            .iter()
            .map(|v| text(v))
            .collect::<Vec<_>>()
            .join(" "),
    })
}
fn json_publication(v: &Value, base: &str) -> Publication {
    let m = &v["metadata"];
    Publication {
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
            identifiers: many(&m["identifier"]).iter().map(|v| text(v)).collect(),
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
        cover: many(&v["images"])
            .iter()
            .filter_map(|v| json_link(v, base).map(|link| (v, link)))
            .min_by_key(|(v, link)| {
                (
                    !link.rel.contains("thumbnail"),
                    v["width"].as_u64().unwrap_or(u64::MAX),
                )
            })
            .map(|(_, link)| link)
            .map(|l| l.url)
            .unwrap_or_default(),
        links: many(&v["links"])
            .iter()
            .filter_map(|v| json_link(v, base))
            .collect(),
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
        })
    }
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
    if l.rel.contains("facet") {
        f.facets.push(l)
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
fn xml_link(n: roxmltree::Node, base: &str) -> Option<Link> {
    Some(Link {
        title: n.attribute("title").unwrap_or_default().into(),
        description: String::new(),
        url: resolve(&base_at(n, base), n.attribute("href")?).ok()?,
        mime: n.attribute("type").unwrap_or_default().into(),
        rel: n.attribute("rel").unwrap_or_default().into(),
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
            let name = text(&group["metadata"]["title"]);
            for p in many(&group["publications"]) {
                f.entries.push(json_publication(p, base))
            }
            for l in many(&group["navigation"])
                .into_iter()
                .chain(many(&group["links"]))
            {
                if let Some(mut l) = json_link(l, base) {
                    l.title = format!("{name} · {}", l.title);
                    f.navigation.push(l)
                }
            }
        }
        for facet in many(&v["facets"]) {
            let name = text(&facet["metadata"]["title"]);
            for l in many(&facet["links"]) {
                if let Some(mut l) = json_link(l, base) {
                    l.title = format!("{name} · {}", l.title);
                    f.facets.push(l)
                }
            }
        }
        if f.entries.is_empty() && f.navigation.is_empty() && v["links"].is_array() {
            let p = json_publication(&v, base);
            if p.epub().is_some() {
                f.entries.push(p)
            }
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
        ..Default::default()
    };
    for n in root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "link")
    {
        if let Some(mut l) = xml_link(n, base) {
            if l.rel.contains("facet") {
                if let Some(group) = n
                    .attributes()
                    .find(|a| a.name() == "facetGroup")
                    .map(|a| a.value())
                {
                    l.title = format!("{group} · {}", l.title);
                }
            }
            top_link(&mut f, l)
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
        let p = Publication {
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
                identifiers: vec![child(n, "id")],
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
pub fn search_url(template: &str, query: &str) -> Result<String, AppError> {
    let escaped: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let s = template
        .replace("%7B", "{")
        .replace("%7D", "}")
        .replace("%7b", "{")
        .replace("%7d", "}")
        .replace("{searchTerms}", &escaped)
        .replace("{searchTerms?}", &escaped)
        .replace("{count?}", "30")
        .replace("{startIndex?}", "0")
        .replace("{startPage?}", "1")
        .replace("{language?}", "*");
    if s.contains('{') {
        return Err(AppError::new(res::str::search_invalid));
    }
    Ok(s)
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
    .map(|s| s.replace("%7B", "{").replace("%7D", "}"))
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
        assert_eq!(f.entries[0].author, "B");
        assert_eq!(f.facets[0].title, "Language · French");
    }
    #[test]
    fn publication_metadata_and_smallest_cover() {
        let feed = parse(br#"{"metadata":{"title":"Catalog"},"publications":[{"metadata":{"title":"Long title","author":[{"name":"Ada"},{"name":"Paul"}],"subject":[{"name":"Fiction"}],"publisher":{"name":"Press"},"language":["en","fr"],"numberOfPages":123,"identifier":"isbn:123","description":"<p>A &amp; B</p>"},"images":[{"href":"large.jpg","width":1000},{"href":"small.jpg","width":100}],"links":[]}]}"#, "https://example.org/catalog").unwrap();
        let p = &feed.entries[0];
        assert_eq!(p.author, "Ada, Paul");
        assert_eq!(p.metadata.subjects, ["Fiction"]);
        assert_eq!(p.metadata.publisher, "Press");
        assert_eq!(p.metadata.languages, ["en", "fr"]);
        assert_eq!(p.metadata.pages, Some(123));
        assert_eq!(p.description, "A & B");
        assert_eq!(p.cover, "https://example.org/small.jpg");
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
    }
}
