// SPDX-License-Identifier: GPL-2.0-or-later
//! Remotely maintained, edition-specific presentation rules. Never rewrites stored EPUBs.
use crate::{epub, metadata::Metadata, opds, res};
use day::prelude::*;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub const HIDDEN_COVER: &str = "stanza-restricted:";
const LIMIT: usize = 256 * 1024;
const CACHE: &str = "stanza.censorship.v1";
const PUBLISHED: &str = match option_env!("STANZA_CENSORSHIP_URL") {
    Some(url) => url,
    None => "https://stanza-redux.app/censorship.json",
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Rules {
    version: u32,
    enabled: bool,
    messages: BTreeMap<String, BTreeMap<String, String>>,
    rules: Vec<Rule>,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Rule {
    id: String,
    enabled: bool,
    storefronts: Vec<String>,
    #[serde(rename = "match")]
    matcher: Matcher,
    #[serde(default)]
    cover_message: Option<String>,
    #[serde(default)]
    hide_from_catalogs: bool,
    #[serde(default)]
    case: String,
    #[serde(default)]
    evidence: String,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Matcher {
    #[serde(default)]
    identifiers: Vec<String>,
    #[serde(default)]
    url_prefixes: Vec<String>,
}
fn web_url(value: &str) -> Option<url::Url> {
    let mut url = url::Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    url.set_query(None);
    url.set_fragment(None);
    // Publisher IDs historically use both http and https, with or without www.
    url.set_scheme("https").ok()?;
    if let Some(host) = url
        .host_str()
        .and_then(|h| h.strip_prefix("www."))
        .map(str::to_owned)
    {
        url.set_host(Some(&host)).ok()?;
    }
    Some(url)
}
impl Matcher {
    fn matches(&self, value: &str) -> bool {
        let url = web_url(value);
        self.identifiers.iter().any(|id| {
            id == value
                || url
                    .as_ref()
                    .zip(web_url(id).as_ref())
                    .is_some_and(|(a, b)| {
                        a.origin() == b.origin()
                            && a.path().trim_end_matches('/') == b.path().trim_end_matches('/')
                    })
        }) || url.is_some_and(|url| {
            self.url_prefixes
                .iter()
                .filter_map(|p| web_url(p))
                .any(|prefix| {
                    url.origin() == prefix.origin() && url.path().starts_with(prefix.path())
                })
        })
    }
    fn metadata(&self, m: &Metadata) -> bool {
        m.identifiers.iter().any(|id| self.matches(id)) || self.matches(&m.acquisition)
    }
    fn publication(&self, p: &opds::Publication) -> bool {
        self.metadata(&p.metadata)
            || self.matches(&p.web)
            || self.matches(&p.cover)
            || self.matches(&p.detail)
            || p.links.iter().any(|link| self.matches(&link.url))
    }
}
impl Rules {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > LIMIT {
            return Err("censorship.document_too_large".into());
        }
        let rules: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if rules.version != 1 || rules.rules.len() > 512 || rules.messages.len() > 512 {
            return Err("censorship.unsupported_document".into());
        }
        let mut ids = BTreeSet::new();
        for translations in rules.messages.values() {
            if translations.get("en").is_none_or(|s| s.trim().is_empty())
                || translations.iter().any(|(tag, text)| {
                    tag.is_empty()
                        || tag.len() > 64
                        || !tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                        || text.trim().is_empty()
                        || text.chars().count() > 240
                        || text.chars().any(|c| c.is_control() && c != '\n')
                })
            {
                return Err("censorship.invalid_message".into());
            }
        }
        for rule in &rules.rules {
            if rule.id.trim().is_empty()
                || !ids.insert(&rule.id)
                || rule.storefronts.is_empty()
                || rule
                    .storefronts
                    .iter()
                    .any(|s| !matches!(s.as_str(), "google-play" | "apple-app-store"))
                || (rule.cover_message.is_none() && !rule.hide_from_catalogs)
                || rule
                    .cover_message
                    .as_ref()
                    .is_some_and(|key| !rules.messages.contains_key(key))
                || (rule.matcher.identifiers.is_empty() && rule.matcher.url_prefixes.is_empty())
                || rule
                    .matcher
                    .identifiers
                    .iter()
                    .any(|s| s.trim().is_empty() || s.len() > 2048)
                || rule.matcher.url_prefixes.iter().any(|s| {
                    s.len() > 2048
                        || !s.ends_with('/')
                        || web_url(s).is_none_or(|u| u.path() == "/")
                })
            {
                return Err("censorship.invalid_rule".into());
            }
        }
        Ok(rules)
    }
    fn active<'a>(&'a self, storefront: &'a str) -> impl Iterator<Item = &'a Rule> {
        self.rules.iter().filter(move |r| {
            self.enabled && r.enabled && r.storefronts.iter().any(|s| s == storefront)
        })
    }
    fn hidden(&self, p: &opds::Publication, storefront: &str) -> bool {
        self.active(storefront)
            .any(|r| r.hide_from_catalogs && r.matcher.publication(p))
    }
    fn message(&self, key: &str, locales: &[String]) -> Option<String> {
        let map = self.messages.get(key)?;
        for tag in locales {
            let mut parent = tag.as_str();
            loop {
                if let Some((_, text)) =
                    map.iter().find(|(key, _)| key.eq_ignore_ascii_case(parent))
                {
                    return Some(text.clone());
                }
                match parent.rsplit_once('-') {
                    Some((shorter, _)) => parent = shorter,
                    None => break,
                }
            }
            if let Some((_, text)) = map.iter().find(|(key, _)| {
                key.to_ascii_lowercase()
                    .starts_with(&format!("{}-", tag.to_ascii_lowercase()))
            }) {
                return Some(text.clone());
            }
        }
        map.get("en").cloned()
    }
}
fn bundled() -> Rules {
    #[cfg(not(test))]
    let bytes = day::resource(res::assets::censorship_json)
        .expect("bundled censorship registry")
        .to_vec();
    #[cfg(test)]
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resource/assets")
            .join(res::assets::censorship_json.as_str()),
    )
    .expect("bundled censorship registry");
    Rules::parse(&bytes).expect("valid bundled censorship registry")
}
thread_local! {
    static POLICY: Signal<Arc<Rules>> = Signal::global(Arc::new(bundled()));
    static REVISION: Signal<u64> = Signal::global(0);
    static STARTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FETCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn policy() -> Arc<Rules> {
    POLICY.with(|s| s.get())
}
pub fn revision() -> u64 {
    REVISION.with(|s| s.get())
}
pub fn enabled() -> bool {
    env!("STANZA_STOREFRONT") != "unrestricted"
}
fn install(rules: Rules) {
    if POLICY.with(|s| *s.get_untracked() == rules) {
        return;
    }
    crate::covers::clear_cache();
    POLICY.with(|s| s.set(Arc::new(rules)));
    REVISION.with(|s| s.update(|n| *n = n.wrapping_add(1)));
}
async fn fetch(endpoint: &str) -> Result<Vec<u8>, String> {
    let url = url::Url::parse(endpoint).map_err(|e| e.to_string())?;
    if url.scheme() != "https"
        && !(url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
    {
        return Err("censorship.insecure_endpoint".into());
    }
    let response = crate::http_client()
        .send_future(
            day_part_http::Request::get(endpoint)
                .header("Accept", "application/json")
                .header("Cache-Control", "no-cache"),
        )
        .await
        .map_err(|e| e.to_string())?;
    if response.status() != 200 || response.expected_length().is_some_and(|n| n > LIMIT as u64) {
        return Err("censorship.invalid_response".into());
    }
    let mut body = response.into_body();
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        if bytes.len() + chunk.len() > LIMIT {
            return Err("censorship.document_too_large".into());
        }
        bytes.extend(chunk);
    }
    Ok(bytes)
}
fn refresh() {
    if !enabled() || FETCHING.with(|f| f.replace(true)) {
        return;
    }
    day::task(async {
        struct Fetching;
        impl Drop for Fetching {
            fn drop(&mut self) {
                FETCHING.with(|f| f.set(false));
            }
        }
        let _fetching = Fetching;
        if let Err(error) = refresh_from(PUBLISHED).await {
            day::warn!("Censorship registry not refreshed: {error}");
        }
    });
}
async fn refresh_from(endpoint: &str) -> Result<(), String> {
    let bytes = fetch(endpoint).await?;
    let rules = Rules::parse(&bytes)?;
    // Only validated responses replace the cache. Empty/disabled documents are valid;
    // errors and 404s preserve the last policy rather than accidentally lifting it.
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    day::prefs::set(CACHE, &text);
    install(rules);
    Ok(())
}
pub fn start() {
    if !enabled() || STARTED.with(|s| s.replace(true)) {
        return;
    }
    if let Some(cached) = day::prefs::get(CACHE) {
        match Rules::parse(cached.as_bytes()) {
            Ok(rules) => install(rules),
            Err(e) => day::warn!("Ignoring invalid cached censorship registry: {e}"),
        }
    }
    refresh();
}
pub fn publication_hidden(p: &opds::Publication) -> bool {
    policy().hidden(p, env!("STANZA_STOREFRONT"))
}
pub fn metadata_message(m: &Metadata) -> Option<String> {
    policy()
        .active(env!("STANZA_STOREFRONT"))
        .find(|r| r.cover_message.is_some() && r.matcher.metadata(m))
        .and_then(|r| r.cover_message.clone())
}
#[cfg(test)]
pub fn metadata_restricted(m: &Metadata) -> bool {
    metadata_message(m).is_some()
}
pub fn cover_message(url: &str) -> Option<String> {
    let rules = policy();
    if !rules.enabled || !enabled() {
        return None;
    }
    if let Some(encoded) = url.strip_prefix(HIDDEN_COVER) {
        let (rule_id, _): (String, String) = serde_json::from_str(encoded).ok()?;
        return rules
            .active(env!("STANZA_STOREFRONT"))
            .find(|r| r.id == rule_id)
            .and_then(|r| r.cover_message.clone());
    }
    rules
        .active(env!("STANZA_STOREFRONT"))
        .find(|r| r.cover_message.is_some() && r.matcher.matches(url))
        .and_then(|r| r.cover_message.clone())
}

pub fn publication_cover(p: &opds::Publication) -> String {
    if let Some(r) = policy()
        .active(env!("STANZA_STOREFRONT"))
        .find(|r| r.cover_message.is_some() && r.matcher.publication(p))
    {
        return format!(
            "{HIDDEN_COVER}{}",
            serde_json::to_string(&(&r.id, &p.cover)).expect("cover request strings")
        );
    }
    p.cover.clone()
}
pub fn message(key: &str) -> day::LocalizedText {
    let mut locales = vec![day::locale().get()];
    locales.extend(day::launch_locales());
    // Remote editorial text stays plain text and passes through a generated UI accessor.
    res::str::cover_restriction_message(policy().message(key, &locales).unwrap_or_default())
}

pub fn cover_path(cover: Option<&str>, path: &str) -> bool {
    let Some(cover) = cover else { return false };
    if path == cover {
        return true;
    }
    let directory = cover.rsplit_once('/').map_or("", |(dir, _)| dir);
    let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
    parent == directory && matches!(name, "cover.svg" | "cover.jpg" | "cover.jpeg" | "cover.png")
}
pub fn reader_placeholder(book: &epub::Book) -> Option<Vec<u8>> {
    metadata_message(&book.metadata)
        .map(|key| placeholder_svg(&message(&key).format()).into_bytes())
}
fn placeholder_svg(message: &str) -> String {
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let lines = message
        .chars()
        .collect::<Vec<_>>()
        .chunks(24)
        .map(|line| escape(&line.iter().collect::<String>()))
        .collect::<Vec<_>>();
    let mut text = String::new();
    for (i, line) in lines.iter().enumerate() {
        text.push_str(&format!(
            "<tspan x=\"300\" y=\"{}\">{line}</tspan>",
            450isize - (lines.len() as isize * 16) + (i as isize * 32)
        ));
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="600" height="900" viewBox="0 0 600 900"><title>{}</title><rect width="600" height="900" fill="#eee8da"/><text text-anchor="middle" font-family="sans-serif" font-size="24" fill="#333">{text}</text></svg>"##,
        escape(message)
    )
}
#[cfg(test)]
pub(crate) fn cover_fixture(uid: &str) -> Vec<u8> {
    // Synthetic EPUB with a caller-supplied ID; no publication text or artwork.
    use std::io::{Cursor, Write};
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, data) in [
        (
            "META-INF/container.xml",
            "<container><rootfiles><rootfile full-path='package.opf'/></rootfiles></container>",
        ),
        (
            "package.opf",
            &format!(
                "<package><metadata><title>Policy fixture</title><identifier>{uid}</identifier></metadata><manifest><item id='c' href='chapter.xhtml' media-type='application/xhtml+xml'/><item id='cover' href='images/cover.jpg' properties='cover-image' media-type='image/jpeg'/><item id='wrapper' href='images/cover.svg' media-type='image/svg+xml'/></manifest><spine><itemref idref='c'/></spine></package>"
            ),
        ),
        (
            "chapter.xhtml",
            "<html xmlns='http://www.w3.org/1999/xhtml'><head><title>Fixture</title></head><body><p>Preserved fixture text</p><img src='images/cover.svg'/></body></html>",
        ),
        ("images/cover.jpg", "original synthetic cover bytes"),
        (
            "images/cover.svg",
            "<svg xmlns='http://www.w3.org/2000/svg'><image href='cover.jpg'/></svg>",
        ),
    ] {
        zip.start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(data.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[cfg(test)]
mod tests;
