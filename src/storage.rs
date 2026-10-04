// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Library models and operations. Legacy formats are translated by `legacy`.
use crate::{Saved, opds};
use day::persistence::{DbError, DbErrorKind, Fetch, Many, One, Query};
use day::prelude::*;

pub const SEARCH_INDEX_VERSION: u32 = 2;
/// Searchable chapter text as (content id, text) pairs, built off the UI thread.
type ChapterText = Vec<(String, String)>;

#[derive(Model, Clone, Default, PartialEq)]
#[model(
    table = "books",
    fts(
        "title",
        "author",
        "genres",
        "description",
        tokenize = "unicode61 remove_diacritics 2"
    )
)]
pub struct BookRecord {
    #[model(id)]
    pub id: String,
    #[model(index)]
    pub title: String,
    pub author: String,
    pub cover: Option<String>,
    pub added: i64,
    pub chapter: u32,
    pub progress: f64,
    #[model(index)]
    pub opened: i64,
    pub genres: String,
    pub description: String,
    pub indexed: u32,
    #[model(relation(target = FolderRecord, join = "library_memberships", delete = "cascade"))]
    pub folders: Many<FolderRecord>,
    #[model(relation(target = ContentRecord, inverse = "book", delete = "cascade"))]
    contents: Many<ContentRecord>,
    #[model(relation(target = MetadataRecord, inverse = "book", delete = "cascade"))]
    metadata: Many<MetadataRecord>,
    #[model(relation(target = BookmarkRecord, inverse = "book", delete = "cascade"))]
    bookmarks: Many<BookmarkRecord>,
}
#[derive(Model, Clone, Default, PartialEq)]
#[model(table = "library_folders")]
pub struct FolderRecord {
    #[model(id)]
    pub id: String,
    #[model(index)]
    pub kind: String,
    pub name: String,
    pub sort_name: String,
    #[model(relation(target = BookRecord, join = "library_memberships"))]
    books: Many<BookRecord>,
}
#[derive(Model, Clone, Default, PartialEq)]
#[model(
    table = "book_contents",
    fts("text", tokenize = "unicode61 remove_diacritics 2")
)]
struct ContentRecord {
    #[model(id)]
    id: String,
    book: One<BookRecord>,
    text: String,
}
#[derive(Model, Clone, Default, PartialEq)]
#[model(table = "catalogs")]
pub struct CatalogRecord {
    #[model(id)]
    pub url: String,
    pub title: String,
    pub rank: u32,
}
#[derive(Model, Clone, Default, PartialEq)]
#[model(table = "imports")]
struct ImportRecord {
    #[model(id)]
    pub key: String,
}
#[derive(Model, Clone, Default, PartialEq)]
#[model(table = "book_metadata")]
struct MetadataRecord {
    #[model(id)]
    id: String,
    book: One<BookRecord>,
    // Keep the existing column name/JSON representation for on-disk compatibility.
    #[model(json)]
    json: crate::metadata::Metadata,
}
#[derive(Model, Clone, Default, PartialEq, serde::Serialize)]
#[model(table = "bookmarks")]
pub struct BookmarkRecord {
    #[model(id)]
    pub id: String,
    #[serde(skip)]
    pub book: One<BookRecord>,
    pub chapter: u32,
    pub progress: f64,
    pub title: String,
}
#[derive(Clone)]
pub struct Library {
    db: ModelContainer,
}
impl Library {
    pub fn open() -> Result<Self, DbError> {
        #[cfg(not(target_arch = "wasm32"))]
        let driver = Sqlite::app_data("stanza-library.sqlite")?;
        #[cfg(target_arch = "wasm32")]
        let driver = Sqlite::at("stanza-library.sqlite");
        Self::with_driver(driver)
    }
    fn with_driver(driver: Sqlite) -> Result<Self, DbError> {
        // Existing libraries have no foreign key in metadata and keep recency separately.
        // Transform owned data before Day installs the new constraints and FTS shadows.
        let plan = day::persistence::MigrationPlan::new().custom(0, 1, |conn| {
            let mut tables = Vec::new();
            conn.query("SELECT name FROM sqlite_master WHERE type = 'table'", &[], &mut |row| {
                if let Ok(name) = row.get(0).as_text() { tables.push(name.to_owned()); }
            })?;
            if tables.iter().any(|t| t == "book_metadata") {
                conn.execute_batch("CREATE TABLE book_metadata_v2(id TEXT NOT NULL PRIMARY KEY, book TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED, json TEXT NOT NULL); INSERT INTO book_metadata_v2 SELECT id, id, json FROM book_metadata WHERE id IN (SELECT id FROM books); DROP TABLE book_metadata; ALTER TABLE book_metadata_v2 RENAME TO book_metadata;")?;
            }
            if tables.iter().any(|t| t == "books") {
                conn.execute_batch("ALTER TABLE books ADD COLUMN opened INTEGER NOT NULL DEFAULT 0;")?;
                if tables.iter().any(|t| t == "reading_history") {
                    conn.execute_batch("UPDATE books SET opened = COALESCE((SELECT opened FROM reading_history WHERE book = books.id), 0); DROP TABLE reading_history;")?;
                }
            }
            if tables.iter().any(|t| t == "book_facets") { conn.execute_batch("DROP TABLE book_facets;")?; }
            Ok(())
        });
        let db = ModelContainer::open_with(
            driver,
            schema![
                BookRecord,
                FolderRecord,
                ContentRecord,
                CatalogRecord,
                ImportRecord,
                MetadataRecord,
                BookmarkRecord
            ],
            plan,
        )?;
        db.set_autosave(false);
        let library = Self { db };
        if !library.imported("library-relations-v2")? {
            for b in library.try_books()? {
                let metadata = library.try_metadata(&b.id)?.unwrap_or_default();
                library.stage_metadata(&b.id, &metadata)?;
            }
            library.mark_imported("library-relations-v2")?;
        }
        Ok(library)
    }
    pub fn error(&self) -> Signal<Option<String>> {
        self.db.last_error()
    }
    pub fn check_external(&self) -> Result<bool, DbError> {
        self.db.check_external()
    }
    pub fn metadata(&self, id: &str) -> Option<crate::metadata::Metadata> {
        Some(self.db.get::<MetadataRecord>(id.to_owned())?.json().read())
    }
    fn try_metadata(&self, id: &str) -> Result<Option<crate::metadata::Metadata>, DbError> {
        Ok(self
            .db
            .try_get::<MetadataRecord>(id.to_owned())?
            .map(|r| r.json().read()))
    }
    fn stage_metadata(
        &self,
        id: &str,
        metadata: &crate::metadata::Metadata,
    ) -> Result<(), DbError> {
        let Some(book) = self.db.try_get::<BookRecord>(id.to_owned())? else {
            return Ok(());
        };
        book.genres().write(metadata.subjects.join(" · "));
        book.description().write(metadata.description.clone());
        let authors = if metadata.authors.is_empty() {
            vec![book.author().read()]
        } else {
            metadata.authors.clone()
        };
        let mut wanted = Vec::new();
        for (kind, names) in [("author", authors), ("genre", metadata.subjects.clone())] {
            let mut names = names
                .into_iter()
                .map(|n| n.trim().to_owned())
                .filter(|n| !n.is_empty())
                .collect::<Vec<_>>();
            names.sort();
            names.dedup();
            if names.is_empty() {
                names.push(String::new());
            }
            for name in names {
                let key = serde_json::to_string(&(kind, &name)).expect("string tuple");
                if self.db.try_get::<FolderRecord>(key.clone())?.is_none() {
                    self.db.insert(FolderRecord {
                        id: key.clone(),
                        kind: kind.into(),
                        sort_name: name.to_lowercase(),
                        name,
                        ..Default::default()
                    });
                }
                wanted.push(day::model::ModelId::<FolderRecord>::of(key));
            }
        }
        for old in book.folders().ids() {
            if !wanted.contains(&old) {
                book.folders().remove(old);
            }
        }
        for key in wanted {
            book.folders().add(key);
        }
        self.db.insert(MetadataRecord {
            id: id.into(),
            book: One::to(id.to_owned()),
            json: metadata.clone(),
        });
        Ok(())
    }
    pub fn save_metadata(
        &self,
        id: &str,
        metadata: &crate::metadata::Metadata,
    ) -> Result<(), DbError> {
        self.stage_metadata(id, metadata)?;
        self.db.save()
    }
    pub fn folder_query(&self, kind: &str) -> Query<FolderRecord> {
        self.db
            .query::<FolderRecord>()
            .filter(FolderRecord::kind().eq(kind.to_owned()) & !FolderRecord::books().is_empty())
            .sort(FolderRecord::sort_name().asc())
            .sort(FolderRecord::id().asc())
            .live()
    }
    pub fn book_query(
        &self,
        facet: Option<(String, String)>,
        recent: Option<usize>,
        search: impl Fn() -> String + 'static,
    ) -> Query<BookRecord> {
        self.db.query_fn(move || {
            let text = search();
            let mut fetch = Fetch::new().filter(
                BookRecord::fts().search(&text)
                    | BookRecord::contents().any(ContentRecord::fts().search(&text)),
            );
            if let Some((kind, name)) = &facet {
                fetch = fetch.filter(BookRecord::folders().any(
                    FolderRecord::kind().eq(kind.clone()) & FolderRecord::name().eq(name.clone()),
                ));
            }
            if let Some(limit) = recent {
                fetch
                    .filter(BookRecord::opened().gt(0_i64))
                    .sort(BookRecord::opened().desc())
                    .sort(BookRecord::id().asc())
                    .limit(limit)
            } else {
                fetch
                    .sort(BookRecord::title().asc())
                    .sort(BookRecord::id().asc())
            }
        })
    }
    pub fn catalog_query(&self) -> Query<CatalogRecord> {
        self.db
            .query::<CatalogRecord>()
            .sort(CatalogRecord::rank().asc())
            .live()
    }
    // Snapshot helpers are for import/export and commands; list views bind Query directly.
    #[cfg(test)]
    pub fn folders(&self, kind: &str) -> Vec<String> {
        self.folder_query(kind)
            .ids()
            .into_iter()
            .filter_map(|id| Some(self.db.get::<FolderRecord>(id)?.name().read()))
            .collect()
    }
    #[cfg(test)]
    pub fn titles(&self, facet: Option<(&str, &str)>, search: &str) -> Vec<Saved> {
        let search = search.to_owned();
        self.book_query(facet.map(|(k, n)| (k.into(), n.into())), None, move || {
            search.clone()
        })
        .ids()
        .into_iter()
        .filter_map(|id| {
            self.db
                .get::<BookRecord>(id)?
                .with_value_untracked(|b| b.cloned())
                .map(Saved::from)
        })
        .collect()
    }
    #[cfg(test)]
    pub fn recents(&self, limit: usize) -> Vec<Saved> {
        self.book_query(None, Some(limit), String::new)
            .ids()
            .into_iter()
            .filter_map(|id| {
                self.db
                    .get::<BookRecord>(id)?
                    .with_value_untracked(|b| b.cloned())
                    .map(Saved::from)
            })
            .collect()
    }
    pub fn opened(&self, id: &str, timestamp: i64) -> Result<(), DbError> {
        if let Some(book) = self.db.try_get::<BookRecord>(id.to_owned())? {
            book.opened().write(timestamp);
            self.db.save()?;
        }
        Ok(())
    }
    pub fn book(&self, id: &str) -> Option<BookRecord> {
        self.db
            .get::<BookRecord>(id.to_owned())?
            .with_value_untracked(|b| b.cloned())
    }
    pub fn try_book(&self, id: &str) -> Result<Option<BookRecord>, DbError> {
        Ok(self
            .db
            .try_get::<BookRecord>(id.to_owned())?
            .and_then(|row| row.with_value_untracked(|b| b.cloned())))
    }
    pub fn try_books(&self) -> Result<Vec<Saved>, DbError> {
        self.db
            .query::<BookRecord>()
            .sort(BookRecord::added().desc())
            .live()
            .try_collect()
            .map(|rows| rows.into_iter().map(Saved::from).collect())
    }
    #[cfg(test)]
    pub fn books(&self) -> Vec<Saved> {
        self.try_books().unwrap()
    }
    fn stage_book(&self, b: &Saved) -> Result<(), DbError> {
        if let Some(old) = self.db.try_get::<BookRecord>(b.id.clone())? {
            old.title().write(b.title.clone());
            old.author().write(b.author.clone());
            old.cover().write(b.cover.clone());
        } else {
            self.db.insert(BookRecord {
                id: b.id.clone(),
                title: b.title.clone(),
                author: b.author.clone(),
                cover: b.cover.clone(),
                added: crate::now_ms() as i64,
                ..Default::default()
            });
            self.stage_metadata(&b.id, &Default::default())?;
        }
        Ok(())
    }
    pub fn put(&self, b: &Saved) -> Result<(), DbError> {
        self.stage_book(b)?;
        self.db.save()
    }
    #[cfg(any(not(target_arch = "wasm32"), test))]
    pub fn import_book(
        &self,
        b: &Saved,
        chapter: u32,
        progress: f64,
        marker: &str,
    ) -> Result<(), DbError> {
        if self.db.try_get::<BookRecord>(b.id.clone())?.is_none() {
            self.stage_book(b)?;
            if let Some(book) = self.db.try_get::<BookRecord>(b.id.clone())? {
                book.chapter().write(chapter);
                book.progress().write(if progress.is_finite() {
                    progress.clamp(0., 1.)
                } else {
                    0.
                });
            }
        }
        self.db.insert(ImportRecord { key: marker.into() });
        self.db.save()
    }
    pub fn bookmark_query(&self, book: &str) -> Query<BookmarkRecord> {
        self.db
            .query::<BookmarkRecord>()
            .filter(BookmarkRecord::book().eq(One::to(book.to_owned())))
            .sort(BookmarkRecord::chapter().asc())
            .sort(BookmarkRecord::progress().asc())
            .live()
    }
    pub fn add_bookmark(
        &self,
        book: &str,
        chapter: u32,
        progress: f64,
        title: String,
    ) -> Result<(), DbError> {
        if !progress.is_finite() || self.db.try_get::<BookRecord>(book.to_owned())?.is_none() {
            return Ok(());
        }
        let progress = (progress.clamp(0., 1.) * 1_000_000.).round() / 1_000_000.;
        let id = format!("{book}:{chapter}:{progress:.6}");
        if self.db.try_get::<BookmarkRecord>(id.clone())?.is_none() {
            self.db.insert(BookmarkRecord {
                id,
                book: One::to(book.to_owned()),
                chapter,
                progress,
                title,
            });
            self.db.save()?;
        }
        Ok(())
    }
    pub fn remove_bookmark(&self, book: &str, id: &str) -> Result<(), DbError> {
        if let Some(row) = self.db.try_get::<BookmarkRecord>(id.to_owned())? {
            if row.book().read() == One::to(book.to_owned()) {
                self.db.delete::<BookmarkRecord>(id.to_owned())?;
                self.db.save()?;
            }
        }
        Ok(())
    }
    pub fn position(&self, id: &str, chapter: u32, progress: f64) -> Result<(), DbError> {
        if let Some(book) = self.db.try_get::<BookRecord>(id.to_owned())? {
            book.chapter().write(chapter);
            book.progress().write(if progress.is_finite() {
                progress.clamp(0., 1.)
            } else {
                0.
            });
            self.db.save()?;
        }
        Ok(())
    }
    pub fn remove(&self, id: &str) -> Result<(), DbError> {
        self.db.delete::<BookRecord>(id.to_owned())?;
        self.db.save()
    }
    pub fn try_catalogs(&self) -> Result<Vec<opds::Link>, DbError> {
        self.catalog_query().try_collect().map(|rows| {
            rows.into_iter()
                .map(|c| opds::Link {
                    title: c.title,
                    url: c.url,
                    ..Default::default()
                })
                .collect()
        })
    }
    #[cfg(test)]
    pub fn catalogs(&self) -> Vec<opds::Link> {
        self.try_catalogs().unwrap()
    }
    pub fn save_catalogs(&self, catalogs: &[opds::Link]) -> Result<(), DbError> {
        for id in self.catalog_query().try_ids()? {
            if let Some(old) = self.db.try_get::<CatalogRecord>(id)? {
                if !catalogs.iter().any(|c| c.url == old.url().read()) {
                    self.db.delete::<CatalogRecord>(id)?;
                }
            }
        }
        for (rank, c) in catalogs.iter().enumerate() {
            if let Some(old) = self.db.try_get::<CatalogRecord>(c.url.clone())? {
                old.title().write(c.title.clone());
                old.rank().write(rank as u32);
            } else {
                self.db.insert(CatalogRecord {
                    url: c.url.clone(),
                    title: c.title.clone(),
                    rank: rank as u32,
                });
            }
        }
        self.db.save()
    }
    pub fn imported(&self, key: &str) -> Result<bool, DbError> {
        Ok(self.db.try_get::<ImportRecord>(key.to_owned())?.is_some())
    }
    pub fn mark_imported(&self, key: &str) -> Result<(), DbError> {
        self.db.insert(ImportRecord { key: key.into() });
        self.db.save()
    }
    /// Each chapter's searchable text, keyed by its content id. Pure, so it runs on a worker
    /// thread: inflating and parsing every chapter of a long book takes seconds on a phone.
    fn chapter_text(b: &crate::epub::Book) -> Result<ChapterText, DbError> {
        b.chapters
            .iter()
            .map(|chapter| {
                let bytes = b
                    .resource(&chapter.path)
                    .map_err(|e| DbError::new(DbErrorKind::Decode, e.to_string()))?;
                let html = std::str::from_utf8(&bytes)
                    .map_err(|e| DbError::new(DbErrorKind::Decode, e.to_string()))?;
                let doc = roxmltree::Document::parse_with_options(
                    html,
                    roxmltree::ParsingOptions {
                        allow_dtd: true,
                        ..Default::default()
                    },
                )
                .map_err(|e| DbError::new(DbErrorKind::Decode, e.to_string()))?;
                let text = doc
                    .descendants()
                    .filter(|n| {
                        n.is_text()
                            && !n
                                .ancestors()
                                .any(|p| matches!(p.tag_name().name(), "script" | "style" | "head"))
                    })
                    .filter_map(|n| n.text())
                    .collect::<Vec<_>>()
                    .join(" ");
                Ok((
                    serde_json::to_string(&(&b.id, &chapter.path)).expect("string tuple"),
                    text,
                ))
            })
            .collect()
    }
    fn stage_index(&self, b: &crate::epub::Book, contents: ChapterText) -> Result<(), DbError> {
        self.stage_metadata(&b.id, &b.metadata)?;
        for (id, text) in contents {
            self.db.insert(ContentRecord {
                id,
                book: One::to(b.id.clone()),
                text,
            });
        }
        if let Some(book) = self.db.try_get::<BookRecord>(b.id.clone())? {
            book.indexed().write(SEARCH_INDEX_VERSION);
        }
        Ok(())
    }
    fn save_index(&self, b: &crate::epub::Book, contents: ChapterText) -> Result<(), DbError> {
        self.stage_index(b, contents)?;
        self.db.save()
    }
    fn install_with(
        &self,
        row: &Saved,
        b: &crate::epub::Book,
        contents: ChapterText,
    ) -> Result<(), DbError> {
        self.stage_book(row)?;
        self.stage_index(b, contents)?;
        self.db.save()
    }
    #[cfg(test)]
    pub fn install(&self, row: &Saved, b: &crate::epub::Book) -> Result<(), DbError> {
        self.install_with(row, b, Self::chapter_text(b)?)
    }
    /// One-time bridge from the first Day prototype. Keep old prefs intact for recovery.
    pub fn import_preferences(&self) -> Result<(), DbError> {
        if self.imported("day-prefs-v1")? {
            return Ok(());
        }
        for b in crate::saved::<Vec<Saved>>("stanza.library")
            .into_iter()
            .rev()
        {
            if self.try_book(&b.id)?.is_none() {
                self.put(&b)?;
                let p: serde_json::Value = crate::saved(&format!("stanza.position.{}", b.id));
                self.position(
                    &b.id,
                    p["chapter"].as_u64().unwrap_or(0) as u32,
                    p["progress"].as_f64().unwrap_or(0.),
                )?;
            }
        }
        let catalogs = day::prefs::get("stanza.catalogs")
            .and_then(|v| serde_json::from_str::<Vec<opds::Link>>(&v).ok())
            .unwrap_or_else(crate::defaults);
        self.save_catalogs(&catalogs)?;
        self.mark_imported("day-prefs-v1")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bookmarks_are_live_scoped_deduplicated_and_cascade_after_reopen() {
        let path =
            std::env::temp_dir().join(format!("stanza-bookmarks-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let db = Library::with_driver(Sqlite::at(&path)).unwrap();
            for id in ["a", "b"] {
                db.put(&Saved {
                    id: id.into(),
                    title: "Bookmark fixture".into(),
                    ..Default::default()
                })
                .unwrap();
            }
            let query = db.bookmark_query("a");
            let count = Signal::new(0usize);
            let observed = query.clone();
            let _watch = watch(move || observed.count(), move |n, _| count.set(*n));
            db.add_bookmark("a", 3, 0.6, "Later fixture".into())
                .unwrap();
            db.add_bookmark("a", 1, 0.2, "Earlier fixture".into())
                .unwrap();
            db.add_bookmark("a", 1, 0.2, "Duplicate fixture".into())
                .unwrap();
            db.add_bookmark("b", 1, 0.2, "Other book fixture".into())
                .unwrap();
            db.add_bookmark("missing", 0, 0.0, String::new()).unwrap();
            db.add_bookmark("a", 0, f64::NAN, String::new()).unwrap();
            day::reactive::flush_sync();
            assert_eq!(count.get(), 2);
            let rows = query.try_collect().unwrap();
            assert_eq!(
                rows.iter().map(|b| b.chapter).collect::<Vec<_>>(),
                vec![1, 3]
            );
            db.remove_bookmark("b", &rows[0].id).unwrap();
            assert_eq!(query.count(), 2); // cannot remove another book's bookmark
            assert_eq!(db.book("a").unwrap().progress, 0.0); // saving is not navigation
        }
        {
            let db = Library::with_driver(Sqlite::at(&path)).unwrap();
            let query = db.bookmark_query("a");
            let rows = query.try_collect().unwrap();
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].progress, 0.2);
            db.remove_bookmark("a", &rows[0].id).unwrap();
            assert_eq!(query.count(), 1);
            db.remove("a").unwrap();
            assert_eq!(query.count(), 0);
            assert_eq!(db.bookmark_query("b").count(), 1);
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn live_queries_follow_fts_membership_edits_and_recency_without_refresh_signals() {
        let db = Library::with_driver(Sqlite::memory()).unwrap();
        let titles = db.book_query(None, None, || "constellation".into());
        let authors = db.folder_query("author");
        let recents = db.book_query(None, Some(1), String::new);
        db.put(&Saved {
            id: "fixture".into(),
            title: "Constellation".into(),
            author: "Synthetic author".into(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(titles.count(), 1);
        assert_eq!(authors.count(), 1);
        assert_eq!(recents.count(), 0);
        db.opened("fixture", 123).unwrap();
        assert_eq!(recents.count(), 1);
        db.save_metadata(
            "fixture",
            &crate::metadata::Metadata {
                authors: vec!["Different creator".into()],
                subjects: vec!["Astronomy".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(db.folders("author"), ["Different creator"]);
        assert_eq!(db.titles(None, "astronomy").len(), 1);
        db.remove("fixture").unwrap();
        assert_eq!(
            (titles.count(), authors.count(), recents.count()),
            (0, 0, 0)
        );
    }
    #[test]
    fn chapters_are_searchable_without_faulting_their_text_into_list_rows() {
        let db = Library::with_driver(Sqlite::memory()).unwrap();
        let b = crate::epub::fixture(
            "<!DOCTYPE html><html><head><style>hiddenword</style></head><body><p>Extraordinary constellations.</p></body></html>",
        );
        db.install(
            &Saved {
                id: b.id.clone(),
                title: b.title.clone(),
                ..Default::default()
            },
            &b,
        )
        .unwrap();
        assert_eq!(db.titles(None, "constellations").len(), 1);
        assert!(db.titles(None, "hiddenword").is_empty());
        db.remove(&b.id).unwrap();
        assert_eq!(db.db.table_count::<ContentRecord>().unwrap(), 0);
    }
    #[test]
    fn previous_library_schema_migrates_without_losing_position_metadata_or_recency() {
        use day::persistence::SqliteConnection;
        use day::persistence::SqliteDriver;
        let path =
            std::env::temp_dir().join(format!("stanza-old-schema-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let mut conn = Sqlite::at(&path).open().unwrap();
            conn.execute_batch("CREATE TABLE books(id TEXT PRIMARY KEY, title TEXT, author TEXT, cover TEXT, added INTEGER, chapter INTEGER, progress REAL); INSERT INTO books VALUES ('legacy-fixture','Preserved title','Creator',NULL,10,3,0.5); CREATE TABLE book_metadata(id TEXT PRIMARY KEY,json TEXT); INSERT INTO book_metadata VALUES ('legacy-fixture','{\"authors\":[\"Creator\"],\"subjects\":[\"Astronomy\"]}'); CREATE TABLE reading_history(book TEXT PRIMARY KEY,opened INTEGER); INSERT INTO reading_history VALUES ('legacy-fixture',100); CREATE TABLE book_facets(id TEXT PRIMARY KEY,book TEXT,kind TEXT,name TEXT);").unwrap();
        }
        {
            let db = Library::with_driver(Sqlite::at(&path)).unwrap();
            let book = db.book("legacy-fixture").unwrap();
            assert_eq!((book.chapter, book.progress, book.opened), (3, 0.5, 100));
            assert_eq!(db.metadata(&book.id).unwrap().subjects, ["Astronomy"]);
            assert_eq!(db.folders("author"), ["Creator"]);
            assert_eq!(db.titles(None, "Astronomy").len(), 1);
        }
        let db = Library::with_driver(Sqlite::at(&path)).unwrap();
        assert_eq!(db.recents(1)[0].id, "legacy-fixture");
        drop(db);
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn starter_book_deletion_keeps_the_first_launch_marker() {
        let db = Library::with_driver(Sqlite::memory()).unwrap();
        let row = Saved {
            id: "starter-fixture".into(),
            title: "Starter fixture".into(),
            ..Default::default()
        };
        db.put(&row).unwrap();
        db.mark_imported("starter-book-v1").unwrap();
        db.position(&row.id, 2, 0.6).unwrap();
        db.put(&row).unwrap(); // Reimport preserves the same record and reading position.
        assert_eq!(db.books().len(), 1);
        assert_eq!(db.book(&row.id).unwrap().chapter, 2);
        db.remove(&row.id).unwrap();
        assert!(db.books().is_empty());
        assert!(db.imported("starter-book-v1").unwrap());
    }
    #[test]
    fn library_facets_history_and_deletion_survive_reopen() {
        let path =
            std::env::temp_dir().join(format!("stanza-facets-{}.sqlite", std::process::id()));
        {
            let db = Library::with_driver(Sqlite::at(&path)).unwrap();
            // Synthetic publications; commas in author names are never split.
            for (id, title, authors, subjects) in [
                (
                    "a",
                    "Alpha",
                    vec!["Doe, Jane", "Roe"],
                    vec!["Fiction", "Adventure"],
                ),
                ("b", "Beta", vec!["Roe"], vec!["Fiction"]),
                ("c", "Gamma", vec![], vec![]),
            ] {
                db.put(&Saved {
                    id: id.into(),
                    title: title.into(),
                    author: authors.join(", "),
                    ..Default::default()
                })
                .unwrap();
                db.save_metadata(
                    id,
                    &crate::metadata::Metadata {
                        authors: authors.into_iter().map(str::to_owned).collect(),
                        subjects: subjects.into_iter().map(str::to_owned).collect(),
                        ..Default::default()
                    },
                )
                .unwrap();
            }
            assert!(db.recents(50).is_empty()); // Adding/downloading alone is not reading.
            db.opened("a", 100).unwrap();
            db.opened("b", 200).unwrap();
            db.opened("a", 300).unwrap();
            db.position("a", 3, 0.4).unwrap();
        }
        {
            let db = Library::with_driver(Sqlite::at(&path)).unwrap();
            assert_eq!(db.folders("author"), ["", "Doe, Jane", "Roe"]);
            assert_eq!(db.titles(Some(("author", "Roe")), "").len(), 2);
            assert_eq!(db.titles(Some(("author", "Doe, Jane")), "")[0].id, "a");
            assert_eq!(db.titles(Some(("genre", "Fiction")), "beta")[0].id, "b");
            assert_eq!(db.titles(Some(("genre", "")), "")[0].id, "c");
            assert_eq!(db.recents(1)[0].id, "a");
            assert_eq!(
                db.recents(50)
                    .iter()
                    .map(|b| b.id.as_str())
                    .collect::<Vec<_>>(),
                ["a", "b"]
            );
            assert!(db.recents(0).is_empty());
            assert_eq!(db.book("a").unwrap().chapter, 3);
            db.save_metadata(
                "a",
                &crate::metadata::Metadata {
                    authors: vec!["Roe".into()],
                    subjects: vec!["Poetry".into()],
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(!db.folders("author").contains(&"Doe, Jane".into()));
            db.remove("a").unwrap();
            assert!(db.metadata("a").is_none());
            assert!(!db.folders("genre").contains(&"Poetry".into()));
            assert_eq!(db.recents(50)[0].id, "b");
        }
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn imports_preserve_existing_positions_and_survive_reopen() {
        let path =
            std::env::temp_dir().join(format!("stanza-import-test-{}.sqlite", std::process::id()));
        let row = Saved {
            id: "imported".into(),
            title: "Legacy title".into(),
            ..Default::default()
        };
        {
            let l = Library::with_driver(Sqlite::at(&path)).unwrap();
            l.import_book(&row, 2, 0.4, "v1:1").unwrap();
            l.position(&row.id, 5, 0.7).unwrap();
            l.import_book(&row, 0, 0., "v1:duplicate").unwrap();
        }
        {
            let l = Library::with_driver(Sqlite::at(&path)).unwrap();
            assert!(l.imported("v1:1").unwrap());
            assert!(l.imported("v1:duplicate").unwrap());
            assert_eq!(l.books().len(), 1);
            let b = l.book(&row.id).unwrap();
            assert_eq!((b.chapter, b.progress), (5, 0.7));
        }
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn metadata_survives_reopen_without_changing_position() {
        let path = std::env::temp_dir().join(format!(
            "stanza-metadata-test-{}.sqlite",
            std::process::id()
        ));
        {
            let db = Library::with_driver(Sqlite::at(&path)).unwrap();
            db.put(&Saved {
                id: "metadata".into(),
                title: "Book".into(),
                ..Default::default()
            })
            .unwrap();
            db.position("metadata", 3, 0.42).unwrap();
            db.save_metadata(
                "metadata",
                &crate::metadata::Metadata {
                    publisher: "Test press".into(),
                    subjects: vec!["Fiction".into()],
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let db = Library::with_driver(Sqlite::at(&path)).unwrap();
        assert_eq!(db.metadata("metadata").unwrap().publisher, "Test press");
        assert_eq!(db.book("metadata").unwrap().progress, 0.42);
        assert_eq!(db.book("metadata").unwrap().chapter, 3);
        drop(db);
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn database_round_trip_and_empty_catalogs() {
        let path =
            std::env::temp_dir().join(format!("stanza-db-test-{}.sqlite", std::process::id()));
        {
            let l = Library::with_driver(Sqlite::at(&path)).unwrap();
            let b = Saved {
                id: "one".into(),
                title: "Book".into(),
                ..Default::default()
            };
            l.put(&b).unwrap();
            l.position("one", 3, 0.4).unwrap();
            l.put(&b).unwrap();
            assert_eq!(l.book("one").unwrap().chapter, 3);
            l.save_catalogs(&crate::defaults()).unwrap();
            let mut c = l.catalogs();
            c.swap(0, 2);
            l.save_catalogs(&c).unwrap();
            assert_eq!(l.catalogs()[0].url, c[0].url);
            l.save_catalogs(&[]).unwrap();
            l.mark_imported("done").unwrap();
        }
        {
            let l = Library::with_driver(Sqlite::at(&path)).unwrap();
            assert_eq!(l.books().len(), 1);
            assert_eq!(l.book("one").unwrap().progress, 0.4);
            assert!(l.catalogs().is_empty());
            assert!(l.imported("done").unwrap());
            l.remove("one").unwrap();
            assert!(l.books().is_empty());
        }
        let _ = std::fs::remove_file(path);
    }
}

/// Extract a book's chapter text on a worker thread; only the database writes stay on the UI
/// thread. Indexing a long book on the UI thread froze the app at launch.
async fn extract_text(b: &crate::epub::Book) -> Result<ChapterText, String> {
    let b = b.clone();
    crate::covers::background(move || Library::chapter_text(&b))
        .await?
        .map_err(|e| e.to_string())
}
/// Write assets before committing metadata. Interrupted writes leave only replaceable orphans.
pub async fn store_book(db: &Library, b: &crate::epub::Book, bytes: Vec<u8>) -> Result<(), String> {
    let row = write_assets(b, bytes).await?;
    let contents = extract_text(b).await?;
    db.install_with(&row, b, contents)
        .map_err(|e| e.to_string())
}
/// Rebuild a saved book's metadata and search text with the current parser.
pub async fn index_book(db: &Library, b: &crate::epub::Book) -> Result<(), String> {
    let contents = extract_text(b).await?;
    db.save_index(b, contents).map_err(|e| e.to_string())
}
pub async fn write_assets(b: &crate::epub::Book, bytes: Vec<u8>) -> Result<Saved, String> {
    day_part_fs::write_future(&format!("books/{}.epub", b.id), bytes)
        .await
        .map_err(|e| e.to_string())?;
    let cover = if let Some(cover) = &b.cover {
        let data = b.resource(cover).map_err(|e| e.to_string())?;
        let path = format!("books/{}.cover", b.id);
        day_part_fs::write_future(&path, data.to_vec())
            .await
            .map_err(|e| e.to_string())?;
        Some(path)
    } else {
        None
    };
    Ok(Saved {
        id: b.id.clone(),
        title: b.title.clone(),
        author: b.author.clone(),
        cover,
    })
}

impl From<BookRecord> for Saved {
    fn from(b: BookRecord) -> Self {
        Self {
            id: b.id,
            title: b.title,
            author: b.author,
            cover: b.cover,
        }
    }
}
