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
    let source = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resource/assets")
            .join(res::assets::censorship_json.as_str()),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&source).unwrap();
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

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn launch_fetch_caches_valid_documents_and_keeps_them_after_failures() {
    use futures_lite::future::block_on;
    use std::io::{BufRead, Write};
    use std::net::TcpListener;
    if std::env::var_os("STANZA_POLICY_TEST_CHILD").is_none() {
        let dir = tempfile::tempdir().unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "content_restrictions::tests::launch_fetch_caches_valid_documents_and_keeps_them_after_failures"])
            .env("STANZA_POLICY_TEST_CHILD", "1")
            .env("DAY_DATA_DIR", dir.path())
            .output().unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    // Loopback fixture, no production server or personal preferences are touched.
    fn serve(response: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/censorship.json", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                headers.push_str(&line.to_ascii_lowercase());
            }
            assert!(headers.starts_with("get /censorship.json http/1.1"));
            assert!(headers.contains("cache-control: no-cache"));
            assert!(headers.contains("accept: application/json"));
            // Oversized bodies can be rejected before the peer finishes writing.
            let _ = stream.write_all(&response);
        });
        (endpoint, worker)
    }
    let good = br#"{"version":1,"enabled":false,"messages":{},"rules":[]}"#;
    let response = |status, body: &[u8]| {
        let mut bytes = format!(
            "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        bytes.extend_from_slice(body);
        bytes
    };
    let (endpoint, server) = serve(response("200 OK", good));
    block_on(refresh_from(&endpoint)).unwrap();
    server.join().unwrap();
    assert!(!policy().enabled);
    let saved = day::prefs::get(CACHE);
    assert_eq!(saved.as_deref(), Some(std::str::from_utf8(good).unwrap()));
    let revision = revision();
    let oversized = vec![b' '; LIMIT + 1];
    let mut chunked = format!(
        "HTTP/1.1 200 OK\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n",
        oversized.len()
    )
    .into_bytes();
    chunked.extend_from_slice(&oversized);
    chunked.extend_from_slice(b"\r\n0\r\n\r\n");
    for bad in [
        response("200 OK", b"<html>unavailable</html>"),
        response("404 Not Found", good),
        response("200 OK", &oversized),
        chunked,
    ] {
        let (endpoint, server) = serve(bad);
        assert!(block_on(refresh_from(&endpoint)).is_err());
        server.join().unwrap();
        assert!(!policy().enabled);
        assert_eq!(super::revision(), revision);
        assert_eq!(day::prefs::get(CACHE), saved);
    }
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/censorship.json", closed.local_addr().unwrap());
    drop(closed);
    assert!(block_on(refresh_from(&endpoint)).is_err());
    assert_eq!(day::prefs::get(CACHE), saved);
    assert!(block_on(fetch("http://publisher.test/censorship.json")).is_err());
}
