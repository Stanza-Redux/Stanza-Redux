// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! The default catalog list: a static JSON document published with the website, so the catalogs
//! a new reader starts with can change without an app release.
//!
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
//! - `version`: the format's major version. A document with a later one is ignored whole, so a
//!   breaking change cannot be half-read by this build. Unknown fields are ignored.
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
//! The source is `website/public/catalogs.json`; `resource/assets/catalogs.json` is the same
//! file, bundled for a first launch with no network.
use crate::{App, res, storage::Library};
use day::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Where the list is published. `STANZA_CATALOGS_URL` at build time points a development build
/// at a local preview of the website.
const PUBLISHED: &str = match option_env!("STANZA_CATALOGS_URL") {
    Some(url) => url,
    None => "https://stanza-redux.app/catalogs.json",
};
const VERSION: u32 = 1;
const LIMIT: usize = 256 * 1024;
/// The last document fetched, so a launch with no network keeps what the last one showed.
const CACHE: &str = "stanza.catalogs.published";

#[derive(Deserialize)]
struct Document {
    version: u32,
    catalogs: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
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
    if document.version > VERSION {
        return Err(format!(
            "catalog list version {} is newer than {VERSION}",
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
        if !listed
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

/// The app's language, then every language the device lists.
fn preferred() -> Vec<String> {
    let mut tags = vec![day::locale().get()];
    for tag in day::launch_locales() {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}
fn bundled() -> Option<Vec<u8>> {
    day::resource(res::assets::catalogs_json).map(|r| r.to_vec())
}
fn apply(db: &Library, json: &[u8]) -> Result<(), String> {
    db.sync_defaults(&parse(json, &preferred())?)
        .map_err(|e| e.to_string())
}
/// The list this install last saw, or the bundled one before its first fetch.
fn apply_known(db: &Library) {
    let known = day::prefs::get(CACHE)
        .map(String::into_bytes)
        .or_else(bundled);
    if let Some(json) = known
        && let Err(e) = apply(db, &json)
    {
        day::warn!("Default catalogs not applied: {e}");
    }
}

/// List the known defaults now, refresh them from the website, and follow the app's language.
pub fn start(a: App) {
    let Some(db) = a.database.get_untracked() else {
        return;
    };
    // Tracks the locale: a reader who switches language sees the names in it, and the catalogs
    // listed for it.
    let known = db.clone();
    watch(move || day::locale().get(), move |_, _| apply_known(&known));
    apply_known(&db);
    day::task(async move {
        let json = match crate::fetch_bytes(PUBLISHED, LIMIT, None).await {
            Ok((json, _)) => json,
            Err(e) => return day::warn!("Default catalogs not refreshed: {e}"),
        };
        match apply(&db, &json) {
            // Cached only once it has parsed, so a bad deployment never replaces a good list.
            Ok(()) => {
                if let Ok(text) = String::from_utf8(json) {
                    day::prefs::set(CACHE, &text);
                }
            }
            Err(e) => day::warn!("Published default catalogs ignored: {e}"),
        }
    });
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
                .join(res::assets::catalogs_json.as_str()),
        )
        .unwrap();
        let published = std::fs::read(root.join("website/public/catalogs.json")).unwrap();
        assert!(
            bundled == published,
            "copy website/public/catalogs.json to resource/assets/catalogs.json"
        );
        let everyone = parse(&bundled, &tags(&["en"])).unwrap();
        assert!(!everyone.is_empty());
        assert!(everyone.iter().all(|c| !c.description.is_empty()));
        let russian = parse(&bundled, &tags(&["en", "ru"])).unwrap();
        assert!(russian.len() > everyone.len());
    }
}
