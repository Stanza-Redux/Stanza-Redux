// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
use day_persistence::{DbError, RowChange, SqliteConnection, Value};
use day_sync::{Application, BookmarkValue, Change, RecordKey, assets::AssetRef};
use serde::{Deserialize, Serialize};

pub const PROFILE: &str = "stanza-redux:library:1";
pub struct LibraryProfile;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) enum Field {
    Opened(i64),
    Catalog {
        title: String,
        author: String,
        added: i64,
    },
    Position {
        chapter: u32,
        millionths: u32,
    },
    Bookmark {
        chapter: u32,
        millionths: u32,
        title: String,
    },
    File {
        epub: AssetRef,
        metadata: AssetRef,
    },
}

pub(super) fn rows(
    c: &mut dyn SqliteConnection,
    sql: &str,
    params: &[Value],
) -> Result<Vec<Vec<Value>>, DbError> {
    let mut out = Vec::new();
    c.query(sql, params, &mut |r| {
        out.push((0..r.len()).map(|i| r.get(i)).collect())
    })?;
    Ok(out)
}
fn bad() -> DbError {
    DbError::driver("stanza.sync.invalid_record")
}
fn text(s: &str) -> Value {
    Value::Text(s.into())
}
pub(super) fn book_id(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn key(field: &str, book: &str, item: &str) -> RecordKey {
    RecordKey {
        model: PROFILE.into(),
        field: field.into(),
        key: if field == "bookmark" {
            item.into()
        } else {
            book.into()
        },
    }
}
fn coordinates(key: &RecordKey, value: &Field) -> Result<(String, String), DbError> {
    if key.model != PROFILE {
        return Err(bad());
    }
    let mut book = key.key.as_str();
    let mut item = "";
    let field = match value {
        Field::Opened(time) if *time >= 0 => "opened",
        Field::Catalog { .. } => "catalog",
        Field::Position { millionths, .. } if *millionths <= 1_000_000 => "position",
        Field::Bookmark {
            chapter,
            millionths,
            ..
        } if *millionths <= 1_000_000 => {
            book = key.key.split_once(':').ok_or_else(bad)?.0;
            if key.key != format!("{book}:{chapter}:{:.6}", f64::from(*millionths) / 1e6) {
                return Err(bad());
            }
            item = &key.key;
            "bookmark"
        }
        Field::File { epub, metadata }
            if epub.sha256 == book
                && epub.validate().is_ok()
                && metadata.validate().is_ok()
                && metadata.bytes <= crate::epub::RESOURCE_LIMIT =>
        {
            "file"
        }
        _ => return Err(bad()),
    };
    if !book_id(book) || key.field != field {
        return Err(bad());
    }
    Ok((book.into(), item.into()))
}

impl Application for LibraryProfile {
    fn profile(&self) -> &'static str {
        PROFILE
    }
    fn install(&self, c: &mut dyn SqliteConnection) -> Result<(), DbError> {
        c.execute_batch(include_str!("library.sql"))
    }
    fn before_import(&self, c: &mut dyn SqliteConnection) -> Result<(), DbError> {
        if !rows(c, "SELECT 1 FROM stanza_sync_pending LIMIT 1", &[])?.is_empty() {
            return Err(DbError::new(
                day_persistence::DbErrorKind::Busy,
                "stanza.sync.pending_local_edits",
            ));
        }
        Ok(())
    }
    fn before_freeze(&self, c: &mut dyn SqliteConnection) -> Result<(), DbError> {
        self.before_import(c)?;
        if rows(c, "SELECT 1 FROM stanza_sync_uploads LIMIT 1", &[])?.is_empty() {
            Ok(())
        } else {
            Err(DbError::driver("stanza.sync.pending_uploads"))
        }
    }
    fn asset_references(&self, change: &Change) -> Result<Vec<AssetRef>, DbError> {
        self.validate(change)?;
        if let Some(v) = &change.value
            && let (_, Field::File { epub, metadata }) =
                RecordKey::decode(&change.id, v).map_err(|_| bad())?
        {
            return Ok(vec![epub, metadata]);
        }
        Ok(vec![])
    }
    fn capture(
        &self,
        c: &mut dyn SqliteConnection,
        _: &[RowChange],
    ) -> Result<Vec<Change>, DbError> {
        let mut changes = Vec::new();
        for r in rows(
            c,
            "SELECT field,book,item,body FROM stanza_sync_pending",
            &[],
        )? {
            let key = key(r[0].as_text()?, r[1].as_text()?, r[2].as_text()?);
            let value: Option<Field> = if r[3] == Value::Null {
                None
            } else {
                Some(serde_json::from_str(r[3].as_text()?).map_err(|_| bad())?)
            };
            changes.push(key.change(value.as_ref()).map_err(|_| bad())?);
        }
        c.execute_batch("DELETE FROM stanza_sync_pending")?;
        Ok(changes)
    }
    fn validate(&self, change: &Change) -> Result<(), DbError> {
        if let Some(v) = &change.value {
            let (key, body) = RecordKey::decode(&change.id, v).map_err(|_| bad())?;
            coordinates(&key, &body)?;
        }
        Ok(())
    }
    fn project(
        &self,
        c: &mut dyn SqliteConnection,
        id: &str,
        value: Option<&BookmarkValue>,
    ) -> Result<(), DbError> {
        c.execute_batch("UPDATE stanza_sync_control SET projecting=1")?;
        match value {
            Some(v) => {
                let (key, body) = RecordKey::decode(id, v).map_err(|_| bad())?;
                let (book, item) = coordinates(&key, &body)?;
                c.execute("INSERT INTO stanza_sync_values VALUES(?,?,?,?,?) ON CONFLICT(object) DO UPDATE SET body=excluded.body",&[text(id),text(&key.field),text(&book),text(&item),text(&v.title)])?;
                apply(c, &book, &item, &body)?;
                if matches!(body, Field::Catalog { .. }) {
                    for r in rows(
                        c,
                        "SELECT item,body FROM stanza_sync_values WHERE book=? AND field IN ('position','opened','bookmark')",
                        &[text(&book)],
                    )? {
                        apply(
                            c,
                            &book,
                            r[0].as_text()?,
                            &serde_json::from_str(r[1].as_text()?).map_err(|_| bad())?,
                        )?;
                    }
                }
            }
            None => {
                for r in rows(
                    c,
                    "SELECT field,book,item FROM stanza_sync_values WHERE object=?",
                    &[text(id)],
                )? {
                    match r[0].as_text()? {
                        "catalog" => {
                            c.execute("DELETE FROM books WHERE id=?", &r[1..2])?;
                            c.execute("DELETE FROM stanza_sync_uploads WHERE book=?", &r[1..2])?;
                        }
                        "opened" => {
                            c.execute("UPDATE books SET opened=0 WHERE id=?", &r[1..2])?;
                        }
                        "position" => {
                            c.execute(
                                "UPDATE books SET chapter=0,progress=0 WHERE id=?",
                                &r[1..2],
                            )?;
                        }
                        "bookmark" => {
                            c.execute("DELETE FROM bookmarks WHERE id=?", &r[2..3])?;
                        }
                        _ => {}
                    }
                }
                c.execute("DELETE FROM stanza_sync_values WHERE object=?", &[text(id)])?;
            }
        }
        c.execute_batch("UPDATE stanza_sync_control SET projecting=0")
    }
}
fn apply(
    c: &mut dyn SqliteConnection,
    book: &str,
    item: &str,
    body: &Field,
) -> Result<(), DbError> {
    match body {
        Field::Opened(time) => {
            c.execute(
                "UPDATE books SET opened=? WHERE id=?",
                &[Value::Int(*time), text(book)],
            )?;
        }
        Field::Catalog {
            title,
            author,
            added,
        } => {
            c.execute("INSERT INTO books(id,title,author,cover,added,chapter,progress,opened,genres,description,indexed) VALUES(?,?,?,NULL,?,0,0,0,'','',0) ON CONFLICT(id) DO UPDATE SET title=excluded.title,author=excluded.author,added=excluded.added",&[text(book),text(title),text(author),Value::Int(*added)])?;
        }
        Field::Position {
            chapter,
            millionths,
        } => {
            c.execute(
                "UPDATE books SET chapter=?,progress=? WHERE id=?",
                &[
                    Value::Int(i64::from(*chapter)),
                    Value::Real(f64::from(*millionths) / 1e6),
                    text(book),
                ],
            )?;
        }
        Field::Bookmark {
            chapter,
            millionths,
            title,
        } => {
            c.execute("INSERT INTO bookmarks(id,book,chapter,progress,title) SELECT ?,?,?,?,? WHERE EXISTS(SELECT 1 FROM books WHERE id=?) ON CONFLICT(id) DO UPDATE SET title=excluded.title",&[text(item),text(book),Value::Int(i64::from(*chapter)),Value::Real(f64::from(*millionths)/1e6),text(title),text(book)])?;
        }
        Field::File { .. } => {}
    }
    Ok(())
}
