// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Read-only v1 adapter. No legacy tables or Readium JSON leak into the current schema.
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::{epub, storage};
    use rusqlite::{Connection, OpenFlags};
    use std::path::{Path, PathBuf};
    pub struct Source {
        pub database: PathBuf,
        pub documents: PathBuf,
    }
    pub struct LegacyBook {
        pub id: i64,
        pub path: String,
        pub title: String,
        pub author: String,
        pub chapter: u32,
        pub locator: Option<String>,
    }
    pub fn sources() -> Vec<Source> {
        let mut roots = Vec::new();
        // Explicit recovery/test source: an exported container, never modified.
        if let Some(p) = std::env::var_os("STANZA_LEGACY_DIR") {
            roots.push(PathBuf::from(p));
        }
        if let Some(p) = std::env::var_os("DAY_DATA_DIR") {
            roots.push(PathBuf::from(p));
        }
        if let Some(p) = std::env::var_os("HOME") {
            let home = PathBuf::from(p);
            roots.push(home.clone());
            #[cfg(target_os = "macos")]
            roots.push(home.join("Library/Containers/org.appfair.app.Stanza-Redux/Data"));
        }
        let mut result = Vec::new();
        for root in roots {
            for (database, documents) in [
                (root.join("library.sqlite"), root.clone()), // Android files directory
                (
                    root.join("Library/Application Support/library.sqlite"),
                    root.join("Documents"),
                ),
                (
                    root.join(
                        "Library/Application Support/org.appfair.app.Stanza-Redux/library.sqlite",
                    ),
                    root.join("Documents"),
                ),
            ] {
                if database.is_file() && !result.iter().any(|s: &Source| s.database == database) {
                    result.push(Source {
                        database,
                        documents,
                    });
                }
            }
        }
        result
    }
    pub fn records(source: &Source) -> Result<Vec<LegacyBook>, String> {
        let c = Connection::open_with_flags(&source.database, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;
        let cols = c
            .prepare("PRAGMA table_info(BOOK)")
            .map_err(|e| e.to_string())?
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        if !cols.iter().any(|c| c.eq_ignore_ascii_case("FILE_PATH")) {
            return Err("v1 BOOK.FILE_PATH is missing".into());
        }
        let column = |name: &str, fallback: &str| {
            if cols.iter().any(|c| c.eq_ignore_ascii_case(name)) {
                name.to_string()
            } else {
                fallback.into()
            }
        };
        let sql = format!(
            "SELECT ID,FILE_PATH,TITLE,AUTHOR,{},{} FROM BOOK ORDER BY ID",
            column("CURRENT_ITEM", "0"),
            column("LOCATOR_JSON", "NULL")
        );
        c.prepare(&sql)
            .map_err(|e| e.to_string())?
            .query_map([], |r| {
                Ok(LegacyBook {
                    id: r.get(0)?,
                    path: r.get(1)?,
                    title: r.get(2)?,
                    author: r.get(3)?,
                    chapter: r.get::<_, i64>(4)?.max(0) as u32,
                    locator: r.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    /// Resolve relative paths and old absolute container paths, rejecting escapes and symlinks.
    pub fn book_path(documents: &Path, stored: &str) -> Result<PathBuf, String> {
        let relative = if Path::new(stored).is_absolute() {
            if let Ok(p) = Path::new(stored).strip_prefix(documents) {
                p.to_path_buf()
            } else if let Some((_, p)) = stored.rsplit_once("/Documents/") {
                PathBuf::from(p)
            } else if let Some((_, p)) = stored.rsplit_once("/files/") {
                PathBuf::from(p)
            } else {
                return Err("Book path is outside the legacy documents directory".into());
            }
        } else {
            PathBuf::from(stored)
        };
        if relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("Unsafe legacy book path".into());
        }
        let root = documents.canonicalize().map_err(|e| e.to_string())?;
        let path = root
            .join(relative)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !path.starts_with(&root) {
            return Err("Legacy book symlink escapes documents directory".into());
        }
        Ok(path)
    }
    pub fn position(row: &LegacyBook, book: &epub::Book) -> (u32, f64) {
        let locator = row
            .locator
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .unwrap_or_default();
        let href = locator["href"]
            .as_str()
            .unwrap_or("")
            .split('#')
            .next()
            .unwrap_or("");
        let chapter = book
            .chapters
            .iter()
            .position(|c| c.path == href || c.path.ends_with(&format!("/{href}")))
            .map(|n| n as u32)
            .unwrap_or(row.chapter)
            .min(book.chapters.len().saturating_sub(1) as u32);
        let progress = locator["locations"]["progression"]
            .as_f64()
            .unwrap_or(0.)
            .clamp(0., 1.);
        (chapter, progress)
    }
    pub async fn migrate(db: storage::Library) -> Vec<String> {
        let mut errors = Vec::new();
        for source in sources() {
            let rows = match records(&source) {
                Ok(r) => r,
                Err(e) => {
                    errors.push(format!("{}: {e}", source.database.display()));
                    continue;
                }
            };
            for row in rows {
                let key = format!("stanza-v1:book:{}", row.id);
                let result = async {
                    if db.imported(&key).map_err(|e| e.to_string())? {
                        return Ok(());
                    }
                    let path = book_path(&source.documents, &row.path)?;
                    let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
                    if metadata.len() > 64 * 1024 * 1024 {
                        return Err("Legacy EPUB exceeds 64 MB".into());
                    }
                    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                    let mut b = epub::parse(&bytes).map_err(|e| e.to_string())?;
                    // Never overwrite a book or reading position that already exists in the new library.
                    if db.try_book(&b.id).map_err(|e| e.to_string())?.is_none() {
                        if !row.title.is_empty() {
                            b.title = row.title.clone();
                        }
                        if !row.author.is_empty() {
                            b.author = row.author.clone();
                        }
                        let saved = storage::write_assets(&b, bytes).await?;
                        let (chapter, progress) = position(&row, &b);
                        db.import_book(&saved, chapter, progress, &key)
                            .map_err(|e| e.to_string())?;
                    }
                    db.mark_imported(&key).map_err(|e| e.to_string())
                }
                .await;
                if let Err(e) = result {
                    errors.push(format!("{}: {e}", row.title));
                }
            }
        }
        errors
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn legacy_is_read_only_and_paths_are_confined() {
            let root = std::env::temp_dir().join(format!("stanza-legacy-{}", std::process::id()));
            std::fs::create_dir_all(root.join("Books")).unwrap();
            let db = root.join("library.sqlite");
            let c = Connection::open(&db).unwrap();
            c.execute_batch("CREATE TABLE BOOK(ID INTEGER, FILE_PATH TEXT,TITLE TEXT,AUTHOR TEXT,CURRENT_ITEM INTEGER);INSERT INTO BOOK VALUES(1,'Books/a.epub','Alice','Carroll',2);").unwrap();
            drop(c);
            std::fs::write(root.join("Books/a.epub"), b"fixture").unwrap();
            let before = std::fs::read(&db).unwrap();
            let rows = records(&Source {
                database: db.clone(),
                documents: root.clone(),
            })
            .unwrap();
            assert_eq!(rows.len(), 1);
            assert!(rows[0].locator.is_none());
            assert_eq!(std::fs::read(&db).unwrap(), before);
            assert!(book_path(&root, "../outside.epub").is_err());
            assert!(book_path(&root, "/old/container/Documents/Books/a.epub").is_ok());
            let b = epub::parse(crate::sample_book().unwrap().as_slice()).unwrap();
            let mut row = rows.into_iter().next().unwrap();
            row.locator = Some(
                serde_json::json!({"href":b.chapters[1].path,"locations":{"progression":0.25}})
                    .to_string(),
            );
            assert_eq!(position(&row, &b), (1, 0.25));
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub use native::migrate;
#[cfg(target_arch = "wasm32")]
pub async fn migrate(_: crate::storage::Library) -> Vec<String> {
    Vec::new()
}
