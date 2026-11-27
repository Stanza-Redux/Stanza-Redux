// SPDX-License-Identifier: GPL-2.0-or-later
//! The default catalog section of the shared website configuration, so the catalogs
//! a new reader starts with can change without an app release.
//!
//! Catalog-section example (the full document also requires `censorship`):
//! ```json
//! {
//!   "version": 1,
//!   "catalogs": [
//!     {
//!       "id": "gutenberg-ru",
//!       "url": "https://www.gutenberg.org/ebooks/search.opds/?query=l.ru",
//!       "name": { "en": "Project Gutenberg in Russian", "ru": "Проект «Гутенберг» на русском" },
//!       "description": { "en": "Public-domain books in Russian." },
//!       "locales": ["ru"]
//!     }
//!   ]
//! }
//! ```
//!
//! - `version`: the format's major version. Unsupported versions are rejected whole.
//!   Unknown catalog-entry fields are ignored.
//! - `catalogs`: the defaults, in the order they are listed in the app.
//! - `id`: stable for the life of the catalog. The app keys its copy on it, so a later document
//!   can rename the catalog or move its `url`, and a catalog the reader removed stays removed.
//! - `url`: an OPDS 1 or 2 feed, HTTP or HTTPS.
//! - `name`, `description`: one string for every language, or a map from BCP 47 tags to strings.
//!   The reader's first preferred language with an entry wins, then `en`, then any entry.
//! - `locales`: optional BCP 47 language ranges. When present, the catalog is listed only for a
//!   reader with a matching preferred language; `ru` matches `ru` and `ru-RU`, in any position
//!   of the device's language list. When absent, the catalog is listed for everyone.
//!
//! - `enabled`: defaults to true. False withdraws a managed default until re-enabled.
//!
//! This is the catalog section of `catalog-config.json`; `catalog_config` owns its single
//! launch fetch, cache, fallback and the accompanying content restrictions.
use serde::Deserialize;
use std::collections::BTreeMap;

const VERSION: u32 = 1;

#[derive(Deserialize)]
struct Document {
    version: u32,
    catalogs: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    #[serde(default = "enabled_by_default")]
    enabled: bool,
    id: String,
    url: String,
    name: Localized,
    #[serde(default)]
    description: Localized,
    #[serde(default)]
    locales: Vec<String>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Localized {
    Plain(String),
    Map(BTreeMap<String, String>),
}
impl Default for Localized {
    fn default() -> Self {
        Self::Plain(String::new())
    }
}
/// One default catalog, as it applies to this reader.
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultCatalog {
    pub id: String,
    pub url: String,
    pub title: String,
    pub description: String,
}

fn enabled_by_default() -> bool {
    true
}

/// Whether `tag` falls in the language range `range` (RFC 4647 basic filtering).
fn in_range(range: &str, tag: &str) -> bool {
    let (range, tag) = (range.to_ascii_lowercase(), tag.to_ascii_lowercase());
    range == "*" || tag == range || tag.starts_with(&format!("{range}-"))
}
impl Localized {
    fn resolve(&self, preferred: &[String]) -> String {
        let map = match self {
            Self::Plain(text) => return text.clone(),
            Self::Map(map) => map,
        };
        let exact = |tag: &str| {
            map.iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(tag))
                .map(|(_, text)| text)
        };
        for tag in preferred {
            // `pt-BR` reads its own entry, then `pt`; `pt` alone accepts a `pt-BR` entry.
            let mut parent = tag.as_str();
            loop {
                if let Some(text) = exact(parent) {
                    return text.clone();
                }
                match parent.rsplit_once('-') {
                    Some((shorter, _)) => parent = shorter,
                    None => break,
                }
            }
            if let Some((_, text)) = map.iter().find(|(key, _)| in_range(tag, key)) {
                return text.clone();
            }
        }
        exact("en")
            .or_else(|| map.values().next())
            .cloned()
            .unwrap_or_default()
    }
}

/// The catalogs `json` lists for a reader with these languages, most preferred first.
pub fn parse(json: &[u8], preferred: &[String]) -> Result<Vec<DefaultCatalog>, String> {
    let document: Document = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    if document.version != VERSION {
        return Err(format!(
            "unsupported catalog list version {}; expected {VERSION}",
            document.version
        ));
    }
    let mut catalogs = Vec::<DefaultCatalog>::new();
    for entry in document.catalogs {
        let listed = entry.locales.is_empty()
            || entry
                .locales
                .iter()
                .any(|range| preferred.iter().any(|tag| in_range(range, tag)));
        let title = entry.name.resolve(preferred);
        let web = entry.url.starts_with("https://") || entry.url.starts_with("http://");
        // One bad entry does not cost the reader the rest of the list.
        if !entry.enabled
            || !listed
            || !web
            || entry.id.is_empty()
            || title.is_empty()
            || catalogs
                .iter()
                .any(|c| c.id == entry.id || c.url == entry.url)
        {
            continue;
        }
        catalogs.push(DefaultCatalog {
            id: entry.id,
            url: entry.url,
            title,
            description: entry.description.resolve(preferred),
        });
    }
    Ok(catalogs)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tags(tags: &[&str]) -> Vec<String> {
        tags.iter().map(|t| (*t).to_owned()).collect()
    }
    const LIST: &str = r#"{"version":1,"future":true,"catalogs":[
        {"id":"ru","url":"https://ru.test/opds","name":{"en":"Russian","ru":"Русский"},"locales":["ru"]},
        {"id":"all","url":"https://all.test/opds","name":"Everyone",
         "description":{"en":"English","pt-BR":"Brasil","zh-Hant":"繁體"}},
        {"id":"all","url":"https://duplicate.test/opds","name":"Duplicate id"},
        {"id":"file","url":"file:///etc/passwd","name":"Not a web address"}]}"#;

    #[test]
    fn locale_gated_catalogs_follow_any_preferred_language() {
        let ids = |preferred: &[&str]| {
            parse(LIST.as_bytes(), &tags(preferred))
                .unwrap()
                .into_iter()
                .map(|c| c.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&["en-US"]), ["all"]);
        assert_eq!(ids(&["en-US", "ru-RU"]), ["ru", "all"]);
        assert_eq!(ids(&["RU"]), ["ru", "all"]);
        // A range is whole subtags: `ru` is not a prefix match for another language.
        assert_eq!(ids(&["rup"]), ["all"]);
    }
    #[test]
    fn text_resolves_in_preference_order_with_fallbacks() {
        let first =
            |preferred: &[&str]| parse(LIST.as_bytes(), &tags(preferred)).unwrap().remove(0);
        assert_eq!(first(&["en", "ru"]).title, "Russian");
        assert_eq!(first(&["ru-RU", "en"]).title, "Русский");
        assert_eq!(first(&["pt-PT"]).description, "English");
        assert_eq!(first(&["pt"]).description, "Brasil");
        assert_eq!(first(&["zh-Hant-TW"]).description, "繁體");
        assert_eq!(first(&["de"]).title, "Everyone");
        assert_eq!(first(&["ru"]).description, "");
    }
    #[test]
    fn later_versions_and_malformed_documents_are_refused() {
        assert!(parse(br#"{"version":2,"catalogs":[]}"#, &[]).is_err());
        assert!(parse(br#"{"catalogs":[]}"#, &[]).is_err());
        assert!(parse(b"<html>", &[]).is_err());
    }
    #[test]
    fn bundled_list_is_the_published_one() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let bundled = std::fs::read(
            root.join("resource/assets")
                .join(crate::res::assets::catalog_config_json.as_str()),
        )
        .unwrap();
        let published = std::fs::read(root.join("website/public/catalog-config.json")).unwrap();
        assert!(
            bundled == published,
            "copy website/public/catalog-config.json to resource/assets/catalog-config.json"
        );
        let everyone = parse(&bundled, &tags(&["en"])).unwrap();
        assert!(!everyone.is_empty());
        assert!(everyone.iter().all(|c| !c.description.is_empty()));
        let russian = parse(&bundled, &tags(&["en", "ru"])).unwrap();
        assert!(russian.len() >= everyone.len());
    }
}
