//! Export the bundled book for browser-level layout/security regression tests.
fn main() {
    let book = dayapp::epub::parse(
        &std::fs::read(std::env::args().nth(1).expect("EPUB fixture path")).unwrap(),
    )
    .unwrap();
    // Test harness fixture only: the app never sends these resources in stanza.load.
    use base64::Engine;
    let mut fixture = serde_json::to_value(&book).unwrap();
    fixture["assets"] = serde_json::Value::Object(book.resources.iter().map(|(path, mime)| {
        (path.clone(), serde_json::json!({"mime":mime,"data":base64::engine::general_purpose::STANDARD.encode(&*book.resource(path).unwrap())}))
    }).collect());
    println!("{fixture}");
}
