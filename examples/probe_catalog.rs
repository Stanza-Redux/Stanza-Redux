//! Read-only OPDS probe: cargo run --example probe_catalog -- [--acquire] https://…
use std::collections::{HashSet, VecDeque};
fn get(url: &str) -> day_part_http::Response {
    let response = day_part_http::fetch(
        &day_part_http::Request::get(url)
            .header("User-Agent", dayapp::USER_AGENT)
            .timeout(std::time::Duration::from_secs(45)),
    )
    .expect("HTTP response");
    assert!(
        (200..300).contains(&response.status),
        "{url}: HTTP {}",
        response.status
    );
    response
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let acquire = args.iter().any(|a| a == "--acquire");
    for url in args.iter().filter(|a| !a.starts_with("--")) {
        let mut queue = VecDeque::from([url.clone()]);
        let mut visited = HashSet::new();
        let mut downloaded = false;
        while let Some(url) = queue.pop_front() {
            if visited.len() >= 12 || !visited.insert(url.clone()) {
                continue;
            }
            let response = get(&url);
            let base = if response.url.is_empty() {
                &url
            } else {
                &response.url
            };
            let feed = dayapp::opds::parse(&response.body, base).expect("OPDS feed");
            println!(
                "{}: HTTP {} — {} books, {} navigation links ({url})",
                feed.title,
                response.status,
                feed.entries.len(),
                feed.navigation.len()
            );
            if !acquire {
                break;
            }
            if let Some(link) = feed.entries.iter().find_map(|p| p.epub()) {
                let response = get(&link.url);
                let book = dayapp::epub::parse(&response.body).expect("Acquired EPUB");
                println!(
                    "Acquired {} ({} bytes, {} spine items, {} contents entries)",
                    book.title,
                    response.body.len(),
                    book.chapters.len(),
                    book.toc.len()
                );
                downloaded = true;
                break;
            }
            queue.extend(feed.navigation.into_iter().map(|l| l.url));
        }
        assert!(
            !acquire || downloaded,
            "No direct EPUB found within twelve catalog pages"
        );
    }
}
