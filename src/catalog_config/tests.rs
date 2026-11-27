// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

#[test]
fn combined_documents_are_validated_and_invalid_cache_falls_back() {
    let source = bundled_bytes();
    let good = Configuration::parse(&source).unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(&source).unwrap();
    json["censorship"]["version"] = 2.into();
    assert!(Configuration::parse(&serde_json::to_vec(&json).unwrap()).is_err());
    json = serde_json::from_slice(&source).unwrap();
    json["catalogs"][0]["enabled"] = "false".into();
    assert!(Configuration::parse(&serde_json::to_vec(&json).unwrap()).is_err());
    assert!(Configuration::parse(br#"{"version":1,"catalogs":[]}"#).is_err());
    assert_eq!(
        select_known(Some("<html>maintenance</html>"), None).rules,
        good.rules
    );
    let legacy = r#"{"version":1,"enabled":false,"messages":{},"rules":[]}"#;
    assert_eq!(
        select_known(None, Some(legacy)).rules,
        content_restrictions::Rules::parse(legacy.as_bytes()).unwrap()
    );
    assert_eq!(
        select_known(Some(std::str::from_utf8(&source).unwrap()), Some(legacy)).rules,
        good.rules
    );
}

#[test]
fn disabling_and_reenabling_defaults_preserves_reader_choices() {
    let mut document: serde_json::Value = serde_json::from_slice(&bundled_bytes()).unwrap();
    let parse = |value: &serde_json::Value| {
        default_catalogs::parse(&serde_json::to_vec(value).unwrap(), &["en".into()]).unwrap()
    };
    for catalog in document["catalogs"].as_array_mut().unwrap() {
        if catalog["url"].as_str().unwrap().contains("gutenberg.org") {
            catalog["enabled"] = false.into();
        }
    }
    let disabled = parse(&document);
    assert!(disabled.iter().all(|c| !c.url.contains("gutenberg.org")));
    for catalog in document["catalogs"].as_array_mut().unwrap() {
        catalog["enabled"] = true.into();
    }
    let enabled = parse(&document);
    let db = Library::with_driver(day_persistence::Sqlite::memory()).unwrap();
    db.sync_defaults(&enabled).unwrap();
    let gutenberg = "https://www.gutenberg.org/ebooks/search.opds/";
    assert!(db.catalogs().iter().any(|c| c.url == gutenberg));
    let mut personal = db.catalogs();
    personal.push(crate::opds::Link {
        title: "Synthetic personal catalog".into(),
        url: "https://www.gutenberg.org/ebooks/search.opds/?query=custom".into(),
        ..Default::default()
    });
    db.save_catalogs(&personal).unwrap();
    db.sync_defaults(&disabled).unwrap();
    assert!(!db.catalogs().iter().any(|c| c.url == gutenberg));
    assert!(
        db.catalogs()
            .iter()
            .any(|c| c.title == "Synthetic personal catalog")
    );
    db.sync_defaults(&enabled).unwrap();
    assert!(db.catalogs().iter().any(|c| c.url == gutenberg));
    let mut personal = db.catalogs();
    personal.retain(|c| c.url != gutenberg);
    db.save_catalogs(&personal).unwrap();
    db.sync_defaults(&disabled).unwrap();
    db.sync_defaults(&enabled).unwrap();
    assert!(!db.catalogs().iter().any(|c| c.url == gutenberg));
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
            .args(["--exact", "catalog_config::tests::launch_fetch_caches_valid_documents_and_keeps_them_after_failures"])
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
        let endpoint = format!(
            "http://{}/catalog-config.json",
            listener.local_addr().unwrap()
        );
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
            assert!(headers.starts_with("get /catalog-config.json http/1.1"));
            assert!(headers.contains("cache-control: no-cache"));
            assert!(headers.contains("accept: application/json"));
            // Oversized bodies can be rejected before the peer finishes writing.
            let _ = stream.write_all(&response);
        });
        (endpoint, worker)
    }
    let good = br#"{"version":1,"catalogs":[{"id":"fixture","url":"https://fixture.test/opds","name":"Synthetic catalog"}],"censorship":{"version":1,"enabled":false,"messages":{},"rules":[]}}"#;
    let db = Library::with_driver(day_persistence::Sqlite::memory()).unwrap();
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
    let before_revision = content_restrictions::revision();
    block_on(refresh_from(&endpoint, Some(&db))).unwrap();
    server.join().unwrap();
    assert_eq!(db.catalogs().len(), 1);
    assert!(content_restrictions::revision() > before_revision);
    let saved = day::prefs::get(CACHE);
    assert_eq!(saved.as_deref(), Some(std::str::from_utf8(good).unwrap()));
    let revision = content_restrictions::revision();
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
        // A valid catalog withdrawal must not apply alongside malformed restrictions.
        response("200 OK", br#"{"version":1,"catalogs":[],"censorship":{}}"#),
        response("404 Not Found", good),
        response("200 OK", &oversized),
        chunked,
    ] {
        let (endpoint, server) = serve(bad);
        assert!(block_on(refresh_from(&endpoint, Some(&db))).is_err());
        server.join().unwrap();
        assert_eq!(db.catalogs().len(), 1);
        assert_eq!(content_restrictions::revision(), revision);
        assert_eq!(day::prefs::get(CACHE), saved);
    }
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!(
        "http://{}/catalog-config.json",
        closed.local_addr().unwrap()
    );
    drop(closed);
    assert!(block_on(refresh_from(&endpoint, Some(&db))).is_err());
    assert_eq!(day::prefs::get(CACHE), saved);
    assert!(block_on(fetch("http://publisher.test/catalog-config.json")).is_err());
}
