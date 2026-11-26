// SPDX-License-Identifier: GPL-2.0-or-later
//! What day-build does before this project compiles: today, the typed `res::` constants
//! generated from `resource/` (https://daybrite.dev/docs/resources).
fn main() {
    println!("cargo:rerun-if-env-changed=STANZA_STOREFRONT");
    let storefront = std::env::var("STANZA_STOREFRONT").unwrap_or_else(|_| {
        match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
            Ok("android") => "google-play".into(),
            Ok("ios") => "apple-app-store".into(),
            _ => "unrestricted".into(),
        }
    });
    assert!(
        matches!(
            storefront.as_str(),
            "google-play" | "apple-app-store" | "unrestricted"
        ),
        "STANZA_STOREFRONT must be google-play, apple-app-store, or unrestricted"
    );
    println!("cargo:rustc-env=STANZA_STOREFRONT={storefront}");
    println!("cargo:rerun-if-env-changed=STANZA_CENSORSHIP_URL");
    day_build::prebuild_project().expect("day-build: prebuild");
}
