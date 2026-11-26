// SPDX-License-Identifier: GPL-2.0-or-later
//! The EPUB filename is presentation data. A private receipt maps the immutable book ID
//! to the file actually installed. Libraries written before receipts use the hash path.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

pub(crate) fn root() -> io::Result<PathBuf> {
    day_part_filesystem::data_dir()
        .map(|p| p.join("day-fs/books"))
        .map_err(io::Error::other)
}
fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn receipt(dir: &Path, id: &str) -> io::Result<PathBuf> {
    if !valid_id(id) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(dir.join(".stanza").join(id))
}
pub(crate) fn valid(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".epub") else {
        return false;
    };
    day_sync::assets::valid_file_component(name)
        && !stem.is_empty()
        && !stem.ends_with(['.', ' '])
        && !(stem.len() == 64 && stem.bytes().all(|b| b.is_ascii_hexdigit()))
}
pub(crate) fn name(title: &str, id: &str) -> String {
    let mut stem = String::new();
    for c in title.trim().trim_end_matches(".epub").chars() {
        if stem.len() + c.len_utf8() > 160 {
            break;
        }
        stem.push(if c.is_control() || "/\\:\"*?<>|".contains(c) {
            ' '
        } else {
            c
        });
    }
    let normalized = stem.split_whitespace().collect::<Vec<_>>().join(" ");
    let stem = normalized.trim_matches([' ', '.']);
    let name = format!("{stem}.epub");
    if stem.is_empty() {
        return format!("{}.epub", &id[..12]);
    }
    if valid(&name) {
        name
    } else {
        format!("_{name}")
    }
}
pub(crate) fn candidates(name: &str, id: &str) -> [String; 3] {
    // Keep room for the suffix even when a peer supplied a longer valid filename.
    let mut stem = name.trim_end_matches(".epub");
    while stem.len() > 160 {
        stem = &stem[..stem.char_indices().next_back().map_or(0, |v| v.0)];
    }
    [
        name.to_owned(),
        format!("{stem} ({}).epub", &id[..10]),
        format!("{stem} ({id}).epub"),
    ]
}
pub(crate) fn path(dir: &Path, id: &str) -> io::Result<PathBuf> {
    match fs::read_to_string(receipt(dir, id)?) {
        Ok(n) if valid(&n) => Ok(dir.join(n)),
        Ok(_) => Err(io::ErrorKind::InvalidData.into()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(dir.join(format!("{id}.epub"))),
        Err(e) => Err(e),
    }
}
pub(crate) fn digest(file: &mut fs::File) -> io::Result<String> {
    let mut h = Sha256::new();
    io::copy(file, &mut h)?;
    Ok(format!("{:x}", h.finalize()))
}
fn matches(path: &Path, id: &str) -> io::Result<bool> {
    if !fs::symlink_metadata(path)?.is_file() {
        return Ok(false);
    }
    Ok(digest(&mut fs::File::open(path)?)? == id)
}
pub(crate) fn install(
    dir: &Path,
    id: &str,
    preferred: &str,
    mut stage: NamedTempFile,
) -> io::Result<PathBuf> {
    let index = receipt(dir, id)?;
    if !valid(preferred) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let records = dir.join(".stanza");
    fs::create_dir_all(&records)?;
    stage.as_file().sync_all()?;
    for name in candidates(preferred, id) {
        let dest = dir.join(&name);
        match stage.persist_noclobber(&dest) {
            Ok(_) => {}
            Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => {
                stage = e.file;
                if !matches(&dest, id)? {
                    continue;
                }
            }
            Err(e) => return Err(e.error),
        }
        let mut record = NamedTempFile::new_in(&records)?;
        record.write_all(name.as_bytes())?;
        record.as_file().sync_all()?;
        record.persist(index).map_err(|e| e.error)?;
        #[cfg(unix)]
        {
            fs::File::open(dir)?.sync_all()?;
            fs::File::open(records)?.sync_all()?;
        }
        let legacy = dir.join(format!("{id}.epub"));
        match matches(&legacy, id) {
            Ok(true) => fs::remove_file(legacy)?,
            Ok(false) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        return Ok(dest);
    }
    Err(io::ErrorKind::AlreadyExists.into())
}
/// Install the verified bytes under the chosen name before retiring the old managed path.
pub(crate) fn adopt(dir: &Path, id: &str, preferred: &str) -> io::Result<PathBuf> {
    let old = path(dir, id)?;
    if old.file_name().and_then(|n| n.to_str()) == Some(preferred) {
        return Ok(old);
    }
    if !valid(preferred) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    // A user file or another edition can occupy the canonical name on this device.
    // Keep its existing disambiguated copy without recopying it on every sync.
    if let Some(current) = old.file_name().and_then(|n| n.to_str()) {
        for name in candidates(preferred, id) {
            if current == name {
                return Ok(old);
            }
            match matches(&dir.join(name), id) {
                Ok(false) => {}
                Ok(true) => break,
                Err(e) if e.kind() == io::ErrorKind::NotFound => break,
                Err(e) => return Err(e),
            }
        }
    }
    let mut input = fs::File::open(&old)?;
    let mut stage = NamedTempFile::new_in(dir)?;
    io::copy(&mut input, &mut stage)?;
    if !matches(stage.path(), id)? {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let dest = install(dir, id, preferred, stage)?;
    if old != dir.join(format!("{id}.epub"))
        && !same_file::is_same_file(&old, &dest)?
        && matches(&old, id)?
    {
        fs::remove_file(old)?;
    }
    Ok(dest)
}
pub(crate) fn remove(dir: &Path, id: &str) -> io::Result<()> {
    let path = path(dir, id)?;
    match matches(&path, id) {
        Ok(true) => fs::remove_file(path)?,
        Ok(false) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    match fs::remove_file(receipt(dir, id)?) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Convert only managed legacy hash paths. Existing receipts, absent (cloud-only) books,
/// and unrelated files are left alone. Each successful adoption verifies the content hash
/// and persists its receipt before removing the legacy file; retries are idempotent.
pub(crate) fn migrate_legacy_names(dir: &Path, books: &[(String, String)]) -> (usize, Vec<String>) {
    let mut renamed = 0;
    let mut errors = Vec::new();
    for (id, title) in books {
        let result = (|| -> io::Result<bool> {
            let current = path(dir, id)?;
            if current != dir.join(format!("{id}.epub")) || !current.try_exists()? {
                return Ok(false);
            }
            adopt(dir, id, &name(title, id))?;
            Ok(true)
        })();
        match result {
            Ok(true) => renamed += 1,
            Ok(false) => {}
            Err(error) => errors.push(format!("{title}: {error}")),
        }
    }
    (renamed, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_names_migrate_without_rewriting_bytes_and_retry_safely() {
        let dir = tempfile::tempdir().unwrap();
        let first = b"synthetic first edition";
        let second = b"synthetic second edition";
        let a = format!("{:x}", Sha256::digest(first));
        let b = format!("{:x}", Sha256::digest(second));
        let corrupt = "a".repeat(64);
        let absent = "b".repeat(64);
        fs::write(dir.path().join(format!("{a}.epub")), first).unwrap();
        fs::write(dir.path().join(format!("{b}.epub")), second).unwrap();
        fs::write(dir.path().join(format!("{corrupt}.epub")), b"edited bytes").unwrap();
        fs::write(dir.path().join("Shared title.epub"), b"unmanaged user file").unwrap();
        let books = vec![
            (a.clone(), "Shared title".into()),
            (b.clone(), "Shared title".into()),
            (corrupt.clone(), "Corrupt fixture".into()),
            (absent, "Cloud only".into()),
        ];
        let (renamed, errors) = migrate_legacy_names(dir.path(), &books);
        assert_eq!(renamed, 2);
        assert_eq!(errors.len(), 1);
        for (id, bytes) in [(&a, first.as_slice()), (&b, second.as_slice())] {
            let chosen = path(dir.path(), id).unwrap();
            assert_eq!(
                chosen.file_name().unwrap().to_str().unwrap(),
                format!("Shared title ({}).epub", &id[..10])
            );
            assert_eq!(fs::read(chosen).unwrap(), bytes);
            assert!(!dir.path().join(format!("{id}.epub")).exists());
        }
        assert_eq!(
            fs::read(dir.path().join("Shared title.epub")).unwrap(),
            b"unmanaged user file"
        );
        assert_eq!(
            fs::read(dir.path().join(format!("{corrupt}.epub"))).unwrap(),
            b"edited bytes"
        );
        let receipts: Vec<_> = [&a, &b]
            .iter()
            .map(|id| fs::read(receipt(dir.path(), id).unwrap()).unwrap())
            .collect();
        let (renamed, errors) = migrate_legacy_names(dir.path(), &books);
        assert_eq!((renamed, errors.len()), (0, 1));
        for (id, prior) in [&a, &b].iter().zip(receipts) {
            assert_eq!(fs::read(receipt(dir.path(), id).unwrap()).unwrap(), prior);
        }
    }

    #[test]
    fn portable_titles_keep_unicode_and_protect_other_editions_and_user_files() {
        let dir = tempfile::tempdir().unwrap();
        let first = b"synthetic first edition";
        let second = b"synthetic second edition";
        let a = format!("{:x}", Sha256::digest(first));
        let b = format!("{:x}", Sha256::digest(second));
        let title = name("  ../月: A / B ?  ", &a);
        assert_eq!(title, "月 A B.epub");
        for input in [
            "CON",
            "LPT1.extra",
            "..",
            "A|B",
            "book.epub",
            &"猫".repeat(130),
        ] {
            assert!(valid(&name(input, &a)));
        }
        fn stage(dir: &Path, bytes: &[u8]) -> NamedTempFile {
            let mut f = NamedTempFile::new_in(dir).unwrap();
            f.write_all(bytes).unwrap();
            f
        }
        let one = install(dir.path(), &a, &title, stage(dir.path(), first)).unwrap();
        let two = install(dir.path(), &b, &title, stage(dir.path(), second)).unwrap();
        assert_eq!(one, dir.path().join(&title));
        assert_eq!(two, dir.path().join(format!("月 A B ({}).epub", &b[..10])));
        assert_eq!(fs::read(&one).unwrap(), first);
        assert_eq!(fs::read(&two).unwrap(), second);
        assert_eq!(path(dir.path(), &b).unwrap(), two);
        fs::write(&one, b"edited outside Stanza").unwrap();
        remove(dir.path(), &a).unwrap();
        assert_eq!(fs::read(one).unwrap(), b"edited outside Stanza");
        assert_eq!(fs::read(two).unwrap(), second);
    }
}
