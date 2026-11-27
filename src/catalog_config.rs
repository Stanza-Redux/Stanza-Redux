// SPDX-License-Identifier: GPL-2.0-or-later
//! One launch-refreshed document for default catalogs and storefront content restrictions.
use crate::{App, content_restrictions, default_catalogs, res, storage::Library};
use day::prelude::*;
use serde::Deserialize;
use std::cell::{Cell, RefCell};

const LIMIT: usize = 512 * 1024;
const CACHE: &str = "stanza.catalog-config.v1";
const LEGACY_POLICY_CACHE: &str = "stanza.censorship.v1";
const PUBLISHED: &str = match option_env!("STANZA_CATALOG_CONFIG_URL") {
    Some(url) => url,
    None => "https://stanza-redux.app/catalog-config.json",
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    catalogs: Vec<serde_json::Value>,
    censorship: serde_json::Value,
}
#[derive(Clone)]
struct Configuration {
    json: Vec<u8>,
    rules: content_restrictions::Rules,
}
impl Configuration {
    fn parse(json: &[u8]) -> Result<Self, String> {
        if json.len() > LIMIT {
            return Err("catalog_config.document_too_large".into());
        }
        let doc: Document = serde_json::from_slice(json).map_err(|e| e.to_string())?;
        if doc.version != 1 || doc.catalogs.len() > 512 {
            return Err("catalog_config.unsupported_document".into());
        }
        // Validate both sections before replacing either current state or the cache.
        default_catalogs::parse(json, &[])?;
        let rules = content_restrictions::Rules::parse(
            &serde_json::to_vec(&doc.censorship).map_err(|e| e.to_string())?,
        )?;
        Ok(Self {
            json: json.to_vec(),
            rules,
        })
    }
    fn apply_catalogs(&self, db: &Library) -> Result<(), String> {
        db.sync_defaults(&default_catalogs::parse(&self.json, &preferred())?)
            .map_err(|e| e.to_string())
    }
}

/// The generated asset accessor is also used by filesystem-based unit tests.
pub(crate) fn bundled_bytes() -> Vec<u8> {
    #[cfg(not(test))]
    let bytes = day::resource(res::assets::catalog_config_json)
        .expect("bundled catalog configuration")
        .to_vec();
    #[cfg(test)]
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resource/assets")
            .join(res::assets::catalog_config_json.as_str()),
    )
    .expect("bundled catalog configuration");
    bytes
}
// Restrictions initialize from the same bundled document even before the app root mounts.
pub(crate) fn bundled_rules() -> content_restrictions::Rules {
    Configuration::parse(&bundled_bytes())
        .expect("valid bundled catalog configuration")
        .rules
}
fn select_known(cached: Option<&str>, legacy_policy: Option<&str>) -> Configuration {
    if let Some(config) = cached.and_then(|s| Configuration::parse(s.as_bytes()).ok()) {
        return config;
    }
    let bytes = bundled_bytes();
    // Preserve a valid policy learned by an older version on the first offline upgrade.
    // Catalogs come from the new bundle, so retired defaults do not survive in the old cache.
    if let Some(legacy) = legacy_policy
        && content_restrictions::Rules::parse(legacy.as_bytes()).is_ok()
        && let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&bytes)
        && let Ok(rules) = serde_json::from_str::<serde_json::Value>(legacy)
    {
        json["censorship"] = rules;
        if let Ok(bytes) = serde_json::to_vec(&json)
            && let Ok(config) = Configuration::parse(&bytes)
        {
            return config;
        }
    }
    Configuration::parse(&bytes).expect("valid bundled catalog configuration")
}
thread_local! {
    static CURRENT: RefCell<Option<Configuration>> = const { RefCell::new(None) };
    static INITIALIZED: Cell<bool> = const { Cell::new(false) };
    static STARTED: Cell<bool> = const { Cell::new(false) };
}
fn preferred() -> Vec<String> {
    let mut tags = vec![day::locale().get()];
    for tag in day::launch_locales() {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}
fn install(config: Configuration, db: Option<&Library>) -> Result<(), String> {
    if let Some(db) = db {
        config.apply_catalogs(db)?;
    }
    content_restrictions::install(config.rules.clone());
    CURRENT.with(|c| *c.borrow_mut() = Some(config));
    Ok(())
}
pub fn initialize() {
    if INITIALIZED.with(|s| s.replace(true)) {
        return;
    }
    let config = select_known(
        day::prefs::get(CACHE).as_deref(),
        day::prefs::get(LEGACY_POLICY_CACHE).as_deref(),
    );
    install(config, None).expect("configuration without a database cannot fail");
}
fn apply_known(db: &Library) {
    CURRENT.with(|current| {
        if let Some(config) = current.borrow().as_ref()
            && let Err(error) = config.apply_catalogs(db)
        {
            day::warn!("Default catalogs not applied: {error}");
        }
    });
}
/// All storefronts need the catalog list. Content rules still apply only to their storefront.
/// Locale changes reapply local data; foregrounding and catalog navigation never fetch it.
pub fn start(a: App) {
    initialize();
    let db = a.database.get_untracked();
    if let Some(db) = &db {
        apply_known(db);
        let db = db.clone();
        watch(move || day::locale().get(), move |_, _| apply_known(&db));
    }
    if STARTED.with(|s| s.replace(true)) {
        return;
    }
    day::task(async move {
        if let Err(error) = refresh_from(PUBLISHED, db.as_ref()).await {
            day::warn!("Catalog configuration not refreshed: {error}");
        }
    });
}
async fn fetch(endpoint: &str) -> Result<Vec<u8>, String> {
    let url = url::Url::parse(endpoint).map_err(|e| e.to_string())?;
    if url.scheme() != "https"
        && !(url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
    {
        return Err("catalog_config.insecure_endpoint".into());
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
        return Err("catalog_config.invalid_response".into());
    }
    let mut body = response.into_body();
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        if bytes.len() + chunk.len() > LIMIT {
            return Err("catalog_config.document_too_large".into());
        }
        bytes.extend(chunk);
    }
    Ok(bytes)
}
async fn refresh_from(endpoint: &str, db: Option<&Library>) -> Result<(), String> {
    let bytes = fetch(endpoint).await?;
    let config = Configuration::parse(&bytes)?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    install(config, db)?;
    day::prefs::set(CACHE, &text);
    Ok(())
}

#[cfg(test)]
mod tests;
