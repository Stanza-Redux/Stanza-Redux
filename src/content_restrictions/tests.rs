// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn fixture() -> Rules {
    Rules::parse(br#"{"version":1,"enabled":true,"messages":{"notice":{"en":"Synthetic notice","fr":"Avis de test","pt-BR":"Teste"}},"rules":[{"id":"fixture","enabled":true,"storefronts":["google-play"],"match":{"identifiers":["https://publisher.test/books/edition","urn:stanza:fixture"],"url_prefixes":["https://publisher.test/books/edition/downloads/","https://publisher.test/images/edition/"]},"cover_message":"notice","hide_from_catalogs":true}]}"#).unwrap()
}
fn publication() -> opds::Publication {
    let mut p = opds::Publication {
        cover: "https://proxy.test/fixture.jpg".into(),
        ..Default::default()
    };
    p.metadata.identifiers.push("urn:stanza:fixture".into());
    p
}
#[test]
fn rules_are_edition_specific_and_storefront_specific() {
    let rules = fixture();
    let rule = &rules.rules[0];
    for value in [
        "https://publisher.test/books/edition",
        "http://www.publisher.test/books/edition/?q=1#x",
        "https://publisher.test/books/edition/downloads/cover.jpg",
        "https://publisher.test/images/edition/revision/cover@2x.jpg",
    ] {
        assert!(rule.matcher.matches(value), "{value}");
    }
    for value in [
        "https://publisher.test/books/edition-2",
        "https://publisher.test/images/edition-2/cover.jpg",
        "https://publisher.test.evil.test/books/edition",
        "https://publisher.test@evil.test/books/edition",
        "https://unrelated.test/images/edition/cover.jpg",
        "Synthetic title",
    ] {
        assert!(!rule.matcher.matches(value), "{value}");
    }
    assert!(rules.hidden(&publication(), "google-play"));
    assert!(!rules.hidden(&publication(), "apple-app-store"));
    assert!(!rules.hidden(&publication(), "unrestricted"));
    let mut apple = rules;
    apple.rules[0].storefronts = vec!["apple-app-store".into()];
    assert!(apple.hidden(&publication(), "apple-app-store"));
    assert!(!apple.hidden(&publication(), "google-play"));
}
#[test]
fn disabling_and_hiding_are_independent_of_cover_replacement() {
    let mut rules = fixture();
    rules.rules[0].hide_from_catalogs = false;
    assert!(!rules.hidden(&publication(), "google-play"));
    assert!(
        rules
            .active("google-play")
            .next()
            .unwrap()
            .cover_message
            .is_some()
    );
    rules.rules[0].hide_from_catalogs = true;
    rules.rules[0].cover_message = None;
    assert!(rules.hidden(&publication(), "google-play"));
    rules.rules[0].enabled = false;
    assert!(!rules.hidden(&publication(), "google-play"));
    rules.rules[0].enabled = true;
    rules.enabled = false;
    assert!(!rules.hidden(&publication(), "google-play"));
    rules.enabled = true;
    rules.rules.clear();
    assert!(!rules.hidden(&publication(), "google-play"));
}
#[test]
fn localized_remote_messages_and_xml_are_plain_text() {
    let rules = fixture();
    for (locale, expected) in [
        ("fr-CA", "Avis de test"),
        ("PT-br", "Teste"),
        ("pt", "Teste"),
        ("xx", "Synthetic notice"),
    ] {
        assert_eq!(
            rules.message("notice", &[locale.into()]).as_deref(),
            Some(expected)
        );
    }
    let message = "<script> & a very long synthetic localized censorship message — 日本語";
    let svg = placeholder_svg(message);
    let parsed = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        parsed
            .descendants()
            .find(|n| n.tag_name().name() == "title")
            .unwrap()
            .text(),
        Some(message)
    );
    assert!(
        !parsed
            .descendants()
            .any(|n| n.tag_name().name() == "script")
    );
    assert!(
        parsed
            .descendants()
            .filter(|n| n.tag_name().name() == "tspan")
            .count()
            > 1
    );
}
#[test]
fn malformed_documents_are_rejected_whole_but_explicit_deactivation_is_valid() {
    let source = crate::catalog_config::bundled_bytes();
    let document: serde_json::Value = serde_json::from_slice(&source).unwrap();
    let value = document["censorship"].clone();
    for broken in [
        {
            let mut v = value.clone();
            v["version"] = 2.into();
            v
        },
        {
            let mut v = value.clone();
            v["rules"][0]["cover_message"] = "missing".into();
            v
        },
        {
            let mut v = value.clone();
            v["rules"][0]["storefronts"] = serde_json::json!(["unknown-store"]);
            v
        },
        {
            let mut v = value.clone();
            v["rules"][0]["match"]["url_prefixes"] = serde_json::json!(["https://publisher.test/"]);
            v
        },
        {
            let mut v = value.clone();
            v["rules"][0]["match"]["url_prefixes"] =
                serde_json::json!(["https://publisher.test/book"]);
            v
        },
        {
            let mut v = value.clone();
            v["messages"]["google-cover"]["en"] = "".into();
            v
        },
        {
            let mut v = value.clone();
            v["rules"][0]["enabeld"] = true.into();
            v
        },
        {
            let mut v = value.clone();
            v["rules"][1]["id"] = v["rules"][0]["id"].clone();
            v
        },
    ] {
        assert!(Rules::parse(&serde_json::to_vec(&broken).unwrap()).is_err());
    }
    assert!(Rules::parse(&vec![b' '; LIMIT + 1]).is_err());
    assert!(Rules::parse(br#"{"version":1,"enabled":false,"messages":{},"rules":[]}"#).is_ok());
}
#[test]
fn accepted_updates_invalidate_presentation_and_can_restore_publications() {
    let before = policy();
    let mut rules = fixture();
    rules.rules[0].storefronts = vec![env!("STANZA_STOREFRONT").into()];
    // Exercise the presentation engine even in unrestricted host builds; production
    // validation never permits a remote rule to target an unrestricted distribution.
    let old_revision = revision();
    install(rules.clone());
    assert!(publication_hidden(&publication()));
    assert!(revision() > old_revision);
    let rev = revision();
    install(rules.clone());
    assert_eq!(revision(), rev);
    rules.rules[0].enabled = false;
    install(rules);
    assert!(!publication_hidden(&publication()));
    assert_eq!(publication_cover(&publication()), publication().cover);
    install((*before).clone());
}
#[test]
fn bundled_registry_preserves_confirmed_and_precautionary_evidence() {
    let rules = bundled();
    assert!(
        rules
            .rules
            .iter()
            .any(|r| r.id == "google-zeno" && r.evidence.contains("confirmed"))
    );
    assert!(
        rules
            .rules
            .iter()
            .any(|r| r.id == "google-chink-in-armour" && r.evidence.contains("not cited"))
    );
    for rule in &rules.rules {
        assert!(rule.enabled);
        assert_eq!(rule.storefronts, ["google-play"]);
        for id in &rule.matcher.identifiers {
            assert!(rule.matcher.matches(id));
        }
    }
}

#[test]
fn published_registry_passes_the_apps_validator() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("website/public/censorship.json"),
    )
    .unwrap();
    Rules::parse(&bytes).unwrap();
}
