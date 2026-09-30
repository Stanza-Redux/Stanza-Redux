// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
use crate::{error::AppError, res};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Seek, SeekFrom},
    sync::{Arc, Mutex},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chapter {
    pub path: String,
    pub title: String,
}
/// Only this small manifest is sent to the reader. ZIP contents never serialize with it.
#[derive(Clone, Debug, Serialize)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: String,
    pub language: String,
    #[serde(skip_serializing)]
    pub metadata: crate::metadata::Metadata,
    pub rtl: bool,
    pub cover: Option<String>,
    pub chapters: Vec<Chapter>,
    pub toc: Vec<Chapter>,
    #[serde(skip_serializing)]
    pub resources: BTreeMap<String, String>,
    #[serde(skip)]
    source: Arc<Archive>,
}
#[derive(Debug)]
struct Archive {
    zip: Mutex<zip::ZipArchive<Source>>,
    fonts: BTreeMap<String, (Vec<u8>, usize)>,
    // Keep only the last inflated resource,
    // never an inflated copy/base64 representation of every entry in the publication.
    cache: Mutex<Option<(String, Arc<[u8]>)>>,
}
#[derive(Debug)]
enum Source {
    Memory(Cursor<Arc<[u8]>>),
    #[cfg(not(target_arch = "wasm32"))]
    File(std::fs::File),
}
impl Read for Source {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Memory(source) => source.read(bytes),
            #[cfg(not(target_arch = "wasm32"))]
            Self::File(source) => source.read(bytes),
        }
    }
}
impl Seek for Source {
    fn seek(&mut self, offset: SeekFrom) -> std::io::Result<u64> {
        match self {
            Self::Memory(source) => source.seek(offset),
            #[cfg(not(target_arch = "wasm32"))]
            Self::File(source) => source.seek(offset),
        }
    }
}
/// Library files are immutable and named by the content hash computed at import.
/// Native readers seek directly in the ZIP; web keeps compressed bytes because Day's
/// browser file store currently exposes whole-file reads, not a seekable handle.
pub async fn open_saved(id: &str) -> Result<Book, AppError> {
    if id.len() != 64 || !id.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::new(res::str::epub_unsafe_path));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let id = id.to_owned();
        crate::covers::background(move || {
            let file = day_part_fs::open_read(&format!("books/{id}.epub"))
                .map_err(|e| AppError::from(e.to_string()))?;
            let len = file
                .metadata()
                .map_err(|e| AppError::from(e.to_string()))?
                .len();
            parse_source(Source::File(file), id, len)
        })
        .await
        .map_err(AppError::from)?
    }
    #[cfg(target_arch = "wasm32")]
    {
        let bytes = day_part_fs::read_future(&format!("books/{id}.epub"))
            .await
            .map_err(|e| AppError::from(e.to_string()))?;
        let len = bytes.len() as u64;
        parse_source(
            Source::Memory(Cursor::new(Arc::from(bytes))),
            id.to_owned(),
            len,
        )
    }
}
pub const RESOURCE_LIMIT: u64 = 32 * 1024 * 1024;
fn read_entry<R: Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    path: &str,
) -> Result<Vec<u8>, AppError> {
    let mut entry = zip
        .by_name(path)
        .map_err(|e| AppError::detail(res::str::epub_resource_missing, e))?;
    if entry.size() > RESOURCE_LIMIT {
        return Err(AppError::new(res::str::epub_size_limit));
    }
    let mut bytes = Vec::new();
    entry
        .by_ref()
        .take(RESOURCE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| AppError::detail(res::str::epub_invalid, e))?;
    if bytes.len() as u64 > RESOURCE_LIMIT {
        return Err(AppError::new(res::str::epub_size_limit));
    }
    Ok(bytes)
}
impl Book {
    pub fn resource(&self, path: &str) -> Result<Arc<[u8]>, AppError> {
        if !self.resources.contains_key(path) {
            return Err(AppError::detail(res::str::epub_resource_missing, path));
        }
        let mut cache = self.source.cache.lock().unwrap();
        if let Some((key, bytes)) = &*cache {
            if key == path {
                return Ok(bytes.clone());
            }
        }
        let mut bytes = read_entry(&mut self.source.zip.lock().unwrap(), path)?;
        if let Some((key, length)) = self.source.fonts.get(path) {
            for (i, b) in bytes.iter_mut().take(*length).enumerate() {
                *b ^= key[i % key.len()];
            }
        }
        let bytes: Arc<[u8]> = Arc::from(bytes);
        *cache = Some((path.to_owned(), bytes.clone()));
        Ok(bytes)
    }
}
pub fn resolve(base: &str, relative: &str) -> Option<String> {
    let u = url::Url::parse(&format!("https://epub.invalid/{base}"))
        .ok()?
        .join(relative)
        .ok()?;
    if u.host_str() != Some("epub.invalid") {
        return None;
    }
    Some(
        percent_encoding::percent_decode_str(u.path().trim_start_matches('/'))
            .decode_utf8()
            .ok()?
            .into_owned()
            + &u.fragment().map(|f| format!("#{f}")).unwrap_or_default(),
    )
}
pub fn parse(bytes: &[u8]) -> Result<Book, AppError> {
    use sha2::{Digest, Sha256};
    parse_source(
        Source::Memory(Cursor::new(Arc::from(bytes))),
        format!("{:x}", Sha256::digest(bytes)),
        bytes.len() as u64,
    )
}
fn parse_source(source: Source, id: String, compressed_len: u64) -> Result<Book, AppError> {
    let mut z =
        zip::ZipArchive::new(source).map_err(|e| AppError::detail(res::str::epub_invalid, e))?;
    if z.len() > 10000 {
        return Err(AppError::new(res::str::epub_resources_limit));
    }
    let mut names = std::collections::BTreeSet::new();
    let mut total = 0u64;
    for i in 0..z.len() {
        let f = z
            .by_index(i)
            .map_err(|e| AppError::detail(res::str::epub_invalid, e))?;
        if f.is_dir() {
            continue;
        }
        if f.enclosed_name().is_none()
            || f.name().contains('\\')
            || !names.insert(f.name().to_owned())
        {
            return Err(AppError::new(res::str::epub_unsafe_path));
        }
        total = total
            .checked_add(f.size())
            .ok_or_else(|| AppError::new(res::str::epub_size_limit))?;
        if total > 256 * 1024 * 1024 || f.size() > RESOURCE_LIMIT {
            return Err(AppError::new(res::str::epub_size_limit));
        }
    }
    // Only package, navigation and encryption metadata are inflated at open.
    let mut xml = |path: &str| -> Result<String, AppError> {
        String::from_utf8(read_entry(&mut z, path)?)
            .map_err(|e| AppError::detail(res::str::epub_invalid, e))
    };
    let container = xml("META-INF/container.xml")?;
    let d = roxmltree::Document::parse(&container)
        .map_err(|e| AppError::detail(res::str::epub_invalid, e))?;
    let opf = d
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "rootfile")
        .and_then(|n| n.attribute("full-path"))
        .ok_or_else(|| AppError::new(res::str::epub_package_missing))?;
    let package = xml(opf)?;
    let d = roxmltree::Document::parse(&package)
        .map_err(|e| AppError::detail(res::str::epub_invalid, e))?;
    let meta = |name: &str| {
        d.descendants()
            .find(|n| n.is_element() && n.tag_name().name() == name)
            .and_then(|n| n.text())
            .unwrap_or_default()
            .to_owned()
    };
    let unique = d
        .root_element()
        .attribute("unique-identifier")
        .unwrap_or_default();
    let identifier = d
        .descendants()
        .find(|n| n.attribute("id") == Some(unique))
        .and_then(|n| n.text())
        .unwrap_or_default()
        .to_owned();
    let encryption = if names.contains("META-INF/encryption.xml") {
        Some(xml("META-INF/encryption.xml")?)
    } else {
        None
    };
    let mut manifest = BTreeMap::new();
    let mut mime = BTreeMap::new();
    let mut cover = None;
    let legacy_cover = d
        .descendants()
        .find(|n| n.attribute("name") == Some("cover"))
        .and_then(|n| n.attribute("content"));
    let mut nav = None;
    let mut ncx = None;
    for n in d
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "item")
    {
        if let (Some(id), Some(href)) = (n.attribute("id"), n.attribute("href")) {
            if let Some(path) = resolve(opf, href) {
                let typ = n
                    .attribute("media-type")
                    .unwrap_or("application/octet-stream");
                if typ.starts_with("image/")
                    && (legacy_cover == Some(id)
                        || n.attribute("properties")
                            .is_some_and(|p| p.split_whitespace().any(|p| p == "cover-image")))
                {
                    cover = Some(path.clone());
                }
                manifest.insert(id.to_owned(), path.clone());
                mime.insert(path.clone(), typ.to_owned());
                if n.attribute("properties")
                    .is_some_and(|p| p.split_whitespace().any(|p| p == "nav"))
                {
                    nav = Some(path.clone())
                }
                if typ == "application/x-dtbncx+xml" {
                    ncx = Some(path)
                }
            }
        }
    }
    let mut toc = Vec::new();
    if let Some(path) = nav {
        if let Ok(s) = xml(&path) {
            if let Ok(d) = roxmltree::Document::parse(&s) {
                for n in d
                    .descendants()
                    .filter(|n| n.is_element() && n.tag_name().name() == "a")
                {
                    if let Some(p) = n.attribute("href").and_then(|h| resolve(&path, h)) {
                        let title = n
                            .descendants()
                            .filter(|n| n.is_text())
                            .filter_map(|n| n.text())
                            .collect::<String>();
                        toc.push(Chapter { path: p, title })
                    }
                }
            }
        }
    }
    if toc.is_empty() {
        if let Some(path) = ncx {
            if let Ok(s) = xml(&path) {
                if let Ok(d) = roxmltree::Document::parse(&s) {
                    for n in d
                        .descendants()
                        .filter(|n| n.is_element() && n.tag_name().name() == "navPoint")
                    {
                        let title = n
                            .descendants()
                            .find(|n| n.is_element() && n.tag_name().name() == "text")
                            .and_then(|n| n.text())
                            .unwrap_or_default()
                            .into();
                        if let Some(p) = n
                            .children()
                            .find(|n| n.is_element() && n.tag_name().name() == "content")
                            .and_then(|n| n.attribute("src"))
                            .and_then(|h| resolve(&path, h))
                        {
                            toc.push(Chapter { path: p, title })
                        }
                    }
                }
            }
        }
    }
    let mut chapters = Vec::new();
    for n in d.descendants().filter(|n| {
        n.is_element() && n.tag_name().name() == "itemref" && n.attribute("linear") != Some("no")
    }) {
        if let Some(p) = n.attribute("idref").and_then(|id| manifest.get(id)) {
            if !names.contains(p) {
                return Err(AppError::detail(res::str::epub_chapter_missing, p));
            }
            let title = toc
                .iter()
                .find(|c| c.path.split('#').next() == Some(p.as_str()))
                .map(|c| c.title.clone())
                .unwrap_or_else(|| format!("{}", chapters.len() + 1));
            chapters.push(Chapter {
                path: p.clone(),
                title,
            })
        }
    }
    if chapters.is_empty() {
        return Err(AppError::new(res::str::epub_no_chapters));
    }
    if toc.is_empty() {
        toc = chapters.clone()
    }
    let fonts = encryption
        .as_deref()
        .map(|xml| font_keys(xml, &identifier))
        .transpose()?
        .unwrap_or_default();
    if fonts.keys().any(|path| !names.contains(path)) {
        return Err(AppError::new(res::str::epub_font_invalid));
    }
    let all_meta = |name: &str| {
        d.descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == name)
            .filter_map(|n| n.text())
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let metadata = crate::metadata::Metadata {
        authors: all_meta("creator"),
        description: crate::metadata::plain(&meta("description")),
        subjects: all_meta("subject"),
        publisher: meta("publisher"),
        published: meta("date"),
        languages: all_meta("language"),
        identifiers: all_meta("identifier"),
        contributors: all_meta("contributor"),
        rights: meta("rights"),
        bytes: Some(compressed_len),
        chapters: Some(chapters.len() as u64),
        modified: d
            .descendants()
            .find(|n| n.attribute("property") == Some("dcterms:modified"))
            .and_then(|n| n.text())
            .unwrap_or_default()
            .into(),
        ..Default::default()
    };
    let source = Arc::new(Archive {
        zip: Mutex::new(z),
        fonts,
        cache: Mutex::new(None),
    });
    Ok(Book {
        id,
        metadata,
        title: meta("title"),
        author: all_meta("creator").join(", "),
        language: meta("language"),
        rtl: d
            .descendants()
            .any(|n| n.attribute("page-progression-direction") == Some("rtl")),
        cover,
        chapters,
        toc,
        resources: mime,
        source,
    })
}
// EPUB font obfuscation is not DRM. Decode the two standard font schemes; reject
// unknown encryption instead of silently rendering a damaged document.
fn font_keys(xml: &str, identifier: &str) -> Result<BTreeMap<String, (Vec<u8>, usize)>, AppError> {
    let mut keys = BTreeMap::new();
    use sha1::Digest;
    let d =
        roxmltree::Document::parse(xml).map_err(|e| AppError::detail(res::str::epub_invalid, e))?;
    let id: String = identifier
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n'))
        .collect();
    for n in d
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "EncryptedData")
    {
        let algorithm = n
            .descendants()
            .find(|n| n.has_tag_name(("http://www.w3.org/2001/04/xmlenc#", "EncryptionMethod")))
            .and_then(|n| n.attribute("Algorithm"))
            .ok_or_else(|| AppError::new(res::str::epub_encryption_invalid))?;
        let path = n
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "CipherReference")
            .and_then(|n| n.attribute("URI"))
            .and_then(|p| resolve("", p))
            .ok_or_else(|| AppError::new(res::str::epub_encryption_invalid))?;
        let (key, length) = match algorithm {
            "http://www.idpf.org/2008/embedding" if !id.is_empty() => {
                (sha1::Sha1::digest(id.as_bytes()).to_vec(), 1040)
            }
            "http://ns.adobe.com/pdf/enc#RC" => {
                let hex = id.trim_start_matches("urn:uuid:").replace('-', "");
                if hex.len() != 32 || !hex.is_ascii() {
                    return Err(AppError::new(res::str::epub_font_invalid));
                }
                let key = (0..32)
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| AppError::detail(res::str::epub_font_invalid, e))?;
                (key, 1024)
            }
            _ => return Err(AppError::new(res::str::epub_drm)),
        };
        keys.insert(path, (key, length));
    }
    Ok(keys)
}
#[cfg(test)]
fn deobfuscate(
    files: &mut BTreeMap<String, Vec<u8>>,
    xml: &str,
    identifier: &str,
) -> Result<(), AppError> {
    for (path, (key, length)) in font_keys(xml, identifier)? {
        let bytes = files
            .get_mut(&path)
            .ok_or_else(|| AppError::new(res::str::epub_font_invalid))?;
        for (i, b) in bytes.iter_mut().take(length).enumerate() {
            *b ^= key[i % key.len()];
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn fixture(chapter: &str) -> Book {
    use std::io::Write;
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, data) in [
        (
            "META-INF/container.xml",
            "<container><rootfiles><rootfile full-path='package.opf'/></rootfiles></container>",
        ),
        (
            "package.opf",
            "<package><metadata><title>Synthetic book</title></metadata><manifest><item id='c' href='chapter.xhtml' media-type='application/xhtml+xml'/></manifest><spine><itemref idref='c'/></spine></package>",
        ),
        ("chapter.xhtml", chapter),
        (
            "unreferenced.bin",
            "This fixture must remain compressed until explicitly requested",
        ),
    ] {
        archive
            .start_file(
                path,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        archive.write_all(data.as_bytes()).unwrap();
    }
    parse(&archive.finish().unwrap().into_inner()).unwrap()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoded_paths_and_font_obfuscation() {
        assert_eq!(
            resolve("OPS/book.opf", "chapter%201.xhtml").as_deref(),
            Some("OPS/chapter 1.xhtml")
        );
        let original = vec![42u8; 2000];
        let mut files = BTreeMap::from([("Fonts/a.otf".into(), original.clone())]);
        let xml = r#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><EncryptedData xmlns="http://www.w3.org/2001/04/xmlenc#"><EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/><CipherData><CipherReference URI="Fonts/a.otf"/></CipherData></EncryptedData></encryption>"#;
        deobfuscate(&mut files, xml, "urn:example:test").unwrap();
        assert_ne!(&files["Fonts/a.otf"][..1040], &original[..1040]);
        assert_eq!(&files["Fonts/a.otf"][1040..], &original[1040..]);
        deobfuscate(&mut files, xml, "urn:example:test").unwrap();
        assert_eq!(files["Fonts/a.otf"], original);
        assert!(
            deobfuscate(
                &mut files,
                &xml.replace("http://www.idpf.org/2008/embedding", "unknown:drm"),
                "id"
            )
            .is_err()
        );
    }
    #[test]
    fn bundled_alice() {
        let book = parse(crate::sample_book().unwrap().as_slice()).unwrap();
        assert!(book.title.contains("Alice"));
        assert!(book.chapters.len() > 4);
        assert!(!book.toc.is_empty());
        assert!(
            book.cover
                .as_ref()
                .is_some_and(|p| book.resources[p] == "image/jpeg")
        );
    }
    #[test]
    fn opening_keeps_resources_lazy_and_serialization_contains_only_the_manifest() {
        let b = fixture("<html><body>Chapter fixture</body></html>");
        assert!(b.source.cache.lock().unwrap().is_none());
        let json = serde_json::to_value(&b).unwrap();
        assert!(json.get("assets").is_none());
        assert!(json.get("resources").is_none());
        assert!(!json.to_string().contains("Chapter fixture"));
        assert!(!json.to_string().contains("unreferenced"));
        assert!(b.resource("../chapter.xhtml").is_err());
        assert!(b.resource("unreferenced.bin").is_err());
        assert_eq!(
            b.resource("chapter.xhtml").unwrap().as_ref(),
            b"<html><body>Chapter fixture</body></html>"
        );
    }
    #[test]
    fn large_section_preserves_exact_utf8_bytes() {
        let text = "é".repeat(192 * 1024 * 2);
        let b = fixture(&text);
        let bytes = b.resource("chapter.xhtml").unwrap();
        assert_eq!(bytes.as_ref(), text.as_bytes());
    }
    #[test]
    fn path_resolution() {
        assert_eq!(
            resolve("OPS/Text/a.xhtml", "../Images/a.png"),
            Some("OPS/Images/a.png".into())
        );
        assert!(resolve("a.xhtml", "https://remote.test/x").is_none());
    }
}
