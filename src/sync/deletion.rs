// SPDX-License-Identifier: GPL-2.0-or-later
//! Local hiding and explicit, durable deletion of a published server EPUB.
use super::*;

// Transfers run on the UI executor. Keep a per-replica/book gate alive for every
// outstanding operation, without accumulating entries for previously opened libraries.
type FileOperations =
    std::collections::BTreeMap<(String, String), std::sync::Weak<async_lock::Mutex<()>>>;
thread_local! {
    static FILE_OPERATIONS: RefCell<FileOperations> = RefCell::default();
}
pub(super) fn file_operation(r: &Replica, book: &str) -> Arc<async_lock::Mutex<()>> {
    FILE_OPERATIONS.with(|operations| {
        let mut operations = operations.borrow_mut();
        operations.retain(|_, gate| gate.strong_count() != 0);
        let key = (r.actor().to_owned(), book.to_owned());
        if let Some(gate) = operations.get(&key).and_then(std::sync::Weak::upgrade) {
            return gate;
        }
        let gate = Arc::new(async_lock::Mutex::new(()));
        operations.insert(key, Arc::downgrade(&gate));
        gate
    })
}

fn published(
    c: &mut dyn day_persistence::SqliteConnection,
    book: &str,
) -> std::result::Result<Option<String>, DbError> {
    rows(c, "SELECT v.body FROM stanza_sync_values v
        JOIN books b ON b.id=v.book JOIN stanza_sync_remote_files f ON f.book=v.book AND f.body=v.body
        WHERE v.field='file' AND v.book=? AND json_type(v.body,'$.File.epub.file')='object'
        AND NOT EXISTS(SELECT 1 FROM stanza_sync_file_repairs r WHERE r.book=v.book)
        AND NOT EXISTS(SELECT 1 FROM stanza_sync_uploads q WHERE q.book=v.book)
        AND NOT EXISTS(SELECT 1 FROM stanza_sync_pending p WHERE p.book=v.book AND p.field='file')",
        &[Value::Text(book.into())])?.first().map(|row| row[0].as_text().map(str::to_owned)).transpose()
}
pub(super) async fn available(book: &str) -> Result<bool> {
    let Some(account) = setting().get_untracked() else {
        return Ok(false);
    };
    let db = library()?;
    let r = replica(&db, &account).await?;
    let book = book.to_owned();
    Ok(r.worker()
        .read(move |db| db.try_with_connection(|c| Ok(published(c, &book)?.is_some())))
        .await?)
}
pub(super) async fn remove(book: &str, server: bool) -> Result<()> {
    let account = setting().get_untracked().ok_or(Error::WrongSpace)?;
    let db = library()?;
    let r = replica(&db, &account).await?;
    let book = book.to_owned();
    remove_from_replica(&r, book, server).await?;
    db.check_external()?;
    flush();
    Ok(())
}
pub(super) async fn remove_from_replica(r: &Replica, book: String, server: bool) -> Result<()> {
    r.worker().write(move |db| db.try_with_connection(|c| {
        let body = published(c, &book)?.ok_or_else(invalid)?;
        if server {
            c.execute("INSERT INTO stanza_sync_deletions VALUES(?,?) ON CONFLICT(book) DO UPDATE SET body=excluded.body", &[Value::Text(book.clone()), Value::Text(body)])?;
        } else {
            c.execute("INSERT OR IGNORE INTO stanza_sync_hidden VALUES(?)", &[Value::Text(book.clone())])?;
            c.execute_batch("UPDATE stanza_sync_control SET projecting=1")?;
        }
        c.execute("DELETE FROM books WHERE id=?", &[Value::Text(book)])?;
        if !server { c.execute_batch("UPDATE stanza_sync_control SET projecting=0")?; }
        Ok(())
    })).await?;
    Ok(())
}
/// Called only after a complete metadata exchange has published the deletion intent.
pub(super) async fn after_exchange(r: &Replica, dav: &dyn Store) -> Result<()> {
    let waiting = r.worker().write(|db| db.try_with_connection(|c| {
        // Edits can arrive after sync's final outbox read. Do not treat them as published.
        if !rows(c, "SELECT 1 FROM _day_sync_transactions WHERE local=1 AND published=0 UNION ALL SELECT 1 FROM stanza_sync_pending LIMIT 1", &[])?.is_empty() {
            return Ok(Vec::new());
        }
        c.execute_batch("INSERT INTO stanza_sync_remote_files SELECT book,body FROM stanza_sync_values WHERE field='file' ON CONFLICT(book) DO UPDATE SET body=excluded.body WHERE stanza_sync_remote_files.body IS NOT excluded.body;
            DELETE FROM stanza_sync_deletions WHERE book IN (SELECT id FROM books);")?;
        rows(c, "SELECT book,body FROM stanza_sync_deletions", &[])
    })).await?;
    for row in waiting {
        let book = row[0].as_text()?.to_owned();
        let body = row[1].as_text()?.to_owned();
        let gate = file_operation(r, &book);
        // during_transfer pauses an upload to exchange metadata. Waiting for that
        // upload here would deadlock; its next exchange will retry the deletion.
        let Some(_guard) = gate.try_lock() else {
            continue;
        };
        let Field::File { epub, .. } =
            serde_json::from_str(&body).map_err(|_| Error::InvalidWire)?
        else {
            return Err(Error::InvalidWire);
        };
        let file = epub.file.as_ref().ok_or(Error::InvalidWire)?;
        if epub.sha256 != book || file.directory != "books" || !crate::shelf::valid(&file.name) {
            return Err(Error::InvalidWire);
        }
        let active_book = book.clone();
        let active_body = body.clone();
        let active = r.worker().write(move |db| db.try_with_connection(|c| {
            let active = !rows(c, "SELECT 1 FROM stanza_sync_deletions WHERE book=? AND body=? AND NOT EXISTS(SELECT 1 FROM books WHERE id=?)", &[Value::Text(active_book.clone()), Value::Text(active_body), Value::Text(active_book.clone())])?.is_empty();
            if active {
                // Commit before any remote operation, including a GET that discovers
                // an already-missing file. Re-import may cancel intent, not recovery.
                c.execute("INSERT OR IGNORE INTO stanza_sync_file_repairs VALUES(?)", &[Value::Text(active_book)])?;
            }
            Ok(active)
        })).await?;
        if !active {
            continue;
        }
        // Never delete a different file that someone placed at the same server path.
        let mut verified = tempfile::tempfile_in(day::app_temp_dir()).map_err(io)?;
        match assets::download(dav, r.space(), &epub, &mut verified).await {
            Ok(()) => {
                let active_book = book.clone();
                let active_body = body.clone();
                let still_removed = r.worker().read(move |db| db.try_with_connection(|c| {
                    Ok(!rows(c, "SELECT 1 FROM stanza_sync_deletions WHERE book=? AND body=? AND NOT EXISTS(SELECT 1 FROM books WHERE id=?)", &[Value::Text(active_book.clone()), Value::Text(active_body), Value::Text(active_book)])?.is_empty())
                })).await?;
                if !still_removed {
                    continue;
                }
                dav.delete(std::slice::from_ref(&file.directory), &file.name)
                    .await?;
            }
            Err(Error::Store(error)) if error.kind == day_sync::StoreErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        r.worker()
            .write(move |db| {
                db.try_with_connection(|c| {
                    c.execute(
                        "DELETE FROM stanza_sync_deletions WHERE book=? AND body=?",
                        &[Value::Text(book.clone()), Value::Text(body)],
                    )?;
                    c.execute(
                        "DELETE FROM stanza_sync_remote_files WHERE book=?",
                        &[Value::Text(book)],
                    )?;
                    Ok(())
                })
            })
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Saved;

    #[test]
    fn local_removal_stays_local_and_server_removal_is_verified_and_retryable() {
        futures_lite::future::block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let store = day_sync::MemoryStore::new();
            let (ap, bp) = (temp.path().join("a.db"), temp.path().join("b.db"));
            let a = Library::with_driver(Sqlite::at(&ap)).unwrap();
            let b = Library::with_driver(Sqlite::at(&bp)).unwrap();
            let bytes = crate::sample_book().unwrap();
            let id = crate::epub::parse(&bytes).unwrap().id;
            let row = Saved {
                id: id.clone(),
                title: "Deletion fixture".into(),
                ..Default::default()
            };
            a.put(&row).unwrap();
            let ai = Identity::create_space_for(PROFILE).unwrap();
            let bi = Identity::join(ai.space.clone()).unwrap();
            let ar = open(&ap, &ai).await.unwrap();
            let br = open(&bp, &bi).await.unwrap();
            metadata(&ar, &store).await.unwrap();
            assert!(
                a.db.try_with_connection(|c| published(c, &id))
                    .unwrap()
                    .is_none()
            );
            let local = temp.path().join("books");
            std::fs::create_dir(&local).unwrap();
            std::fs::write(local.join(format!("{id}.epub")), &bytes).unwrap();
            upload_book(&ar, &store, &id, &local).await.unwrap();
            metadata(&ar, &store).await.unwrap();
            metadata(&br, &store).await.unwrap();
            a.check_external().unwrap();
            b.check_external().unwrap();
            assert!(
                a.db.try_with_connection(|c| published(c, &id))
                    .unwrap()
                    .is_some()
            );
            remove_from_replica(&ar, id.clone(), false).await.unwrap();
            metadata(&ar, &store).await.unwrap();
            metadata(&br, &store).await.unwrap();
            a.check_external().unwrap();
            b.check_external().unwrap();
            assert!(a.book(&id).is_none());
            assert!(b.book(&id).is_some());
            assert_eq!(
                store
                    .get(&["books".into()], "Deletion fixture.epub")
                    .await
                    .unwrap(),
                Some(bytes.clone())
            );
            // A later remote catalog update and a process restart must not unhide it.
            b.put(&Saved {
                title: "Renamed fixture".into(),
                ..row.clone()
            })
            .unwrap();
            metadata(&br, &store).await.unwrap();
            metadata(&ar, &store).await.unwrap();
            ar.close().await.unwrap();
            let ar = open(&ap, &ai).await.unwrap();
            metadata(&ar, &store).await.unwrap();
            a.check_external().unwrap();
            assert!(a.book(&id).is_none());
            // An explicit re-import restores the locally hidden book.
            a.put(&row).unwrap();
            metadata(&ar, &store).await.unwrap();
            upload_book(&ar, &store, &id, &local).await.unwrap();
            metadata(&ar, &store).await.unwrap();
            a.check_external().unwrap();
            assert!(a.book(&id).is_some());
            remove_from_replica(&ar, id.clone(), true).await.unwrap();
            after_exchange(&ar, &store).await.unwrap(); // Its tombstone is not published yet.

            assert!(
                store
                    .get(&["books".into()], "Deletion fixture.epub")
                    .await
                    .unwrap()
                    .is_some()
            );
            // An unrelated replacement at the same path is never removed.
            store
                .put(
                    &["books".into()],
                    "Deletion fixture.epub",
                    b"unrelated replacement".to_vec(),
                )
                .await
                .unwrap();
            assert!(metadata(&ar, &store).await.is_err());
            assert_eq!(
                store
                    .get(&["books".into()], "Deletion fixture.epub")
                    .await
                    .unwrap(),
                Some(b"unrelated replacement".to_vec())
            );
            ar.close().await.unwrap();
            let ar = open(&ap, &ai).await.unwrap();
            store
                .put(&["books".into()], "Deletion fixture.epub", bytes)
                .await
                .unwrap();
            metadata(&ar, &store).await.unwrap();
            metadata(&br, &store).await.unwrap();
            a.check_external().unwrap();
            b.check_external().unwrap();
            assert!(a.book(&id).is_none() && b.book(&id).is_none());
            assert!(
                store
                    .get(&["books".into()], "Deletion fixture.epub")
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(
                a.db.try_with_connection(|c| rows(
                    c,
                    "SELECT book FROM stanza_sync_deletions",
                    &[]
                ))
                .unwrap()
                .is_empty()
            );
            metadata(&ar, &store).await.unwrap();
            ar.close().await.unwrap();
            br.close().await.unwrap();
        });
    }
}

#[cfg(test)]
mod races {
    use super::*;
    use crate::Saved;
    use day_sync::{Created, MemoryStore, StoreError, StoreErrorKind, StoreFuture};
    use futures_channel::oneshot;
    use futures_lite::future::{block_on, poll_once, race, zip};
    use std::sync::Mutex;

    struct PausedDelete {
        inner: MemoryStore,
        replica: Replica,
        row: Saved,
        entered: Mutex<Option<oneshot::Sender<()>>>,
        release: Mutex<Option<oneshot::Receiver<()>>>,
        lose_response: bool,
        published_body: Option<String>,
    }
    impl Store for PausedDelete {
        fn endpoint(&self) -> String {
            self.inner.endpoint()
        }
        fn ensure_directory<'a>(&'a self, d: &'a [String]) -> StoreFuture<'a, ()> {
            self.inner.ensure_directory(d)
        }
        fn list<'a>(&'a self, d: &'a [String]) -> StoreFuture<'a, Vec<String>> {
            self.inner.list(d)
        }
        fn get<'a>(&'a self, d: &'a [String], n: &'a str) -> StoreFuture<'a, Option<Vec<u8>>> {
            self.inner.get(d, n)
        }
        fn put<'a>(&'a self, d: &'a [String], n: &'a str, b: Vec<u8>) -> StoreFuture<'a, ()> {
            self.inner.put(d, n, b)
        }
        fn create<'a>(
            &'a self,
            d: &'a [String],
            n: &'a str,
            b: Vec<u8>,
        ) -> StoreFuture<'a, Created> {
            self.inner.create(d, n, b)
        }
        fn delete<'a>(&'a self, d: &'a [String], n: &'a str) -> StoreFuture<'a, bool> {
            Box::pin(async move {
                if d == ["books"] {
                    let row = self.row.clone();
                    let body = self.published_body.clone();
                    self.replica.worker().write(move |db| {
                        Library { db: db.clone() }.put(&row)?;
                        if let Some(body) = body {
                            // A previously published reference can arrive from another
                            // device while DELETE is in flight. It does not prove that
                            // the bytes survived that request.
                            db.try_with_connection(|c| {
                                c.execute("INSERT INTO stanza_sync_pending VALUES('file',?,'',?) ON CONFLICT DO UPDATE SET body=excluded.body", &[Value::Text(row.id.clone()),Value::Text(body)])?;
                                c.execute("DELETE FROM stanza_sync_uploads WHERE book=?", &[Value::Text(row.id)])?;
                                Ok(())
                            })?;
                        }
                        Ok(())
                    }).await.map_err(|e| StoreError::with_source(StoreErrorKind::Other,e))?;
                    if let Some(entered) = self.entered.lock().unwrap().take() {
                        entered.send(()).unwrap();
                    }
                    let release = self.release.lock().unwrap().take();
                    if let Some(release) = release {
                        release.await.unwrap();
                    }
                }
                let removed = self.inner.delete(d, n).await?;
                if self.lose_response && d == ["books"] {
                    return Err(StoreError::new(StoreErrorKind::Unavailable));
                }
                Ok(removed)
            })
        }
    }

    async fn fixture(
        path: &Path,
        store: &MemoryStore,
    ) -> (Library, Identity, Replica, Saved, PathBuf, Vec<u8>, String) {
        let db = Library::with_driver(Sqlite::at(path)).unwrap();
        let bytes = crate::sample_book().unwrap();
        let id = crate::epub::parse(&bytes).unwrap().id;
        let row = Saved {
            id: id.clone(),
            title: "Synthetic race fixture".into(),
            ..Default::default()
        };
        db.put(&row).unwrap();
        let identity = Identity::create_space_for(PROFILE).unwrap();
        let replica = open(path, &identity).await.unwrap();
        metadata(&replica, store).await.unwrap();
        let root = path.parent().unwrap().join("books");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join(format!("{id}.epub")), &bytes).unwrap();
        upload_book(&replica, store, &id, &root).await.unwrap();
        metadata(&replica, store).await.unwrap();
        let body = db
            .db
            .try_with_connection(|c| published(c, &id))
            .unwrap()
            .unwrap();
        (db, identity, replica, row, root, bytes, body)
    }
    async fn assert_recovered(
        db: &Library,
        r: &Replica,
        store: &MemoryStore,
        row: &Saved,
        bytes: &[u8],
        dir: &Path,
    ) {
        metadata(r, store).await.unwrap();
        db.check_external().unwrap();
        assert!(db.book(&row.id).is_some());
        assert!(
            db.db
                .try_with_connection(|c| published(c, &row.id))
                .unwrap()
                .is_some()
        );
        let remote = store
            .get(&["books".into()], "Synthetic race fixture.epub")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(remote.as_slice(), bytes);
        assert!(db.db.try_with_connection(|c| rows(c, "SELECT book FROM stanza_sync_file_repairs UNION ALL SELECT book FROM stanza_sync_uploads", &[])).unwrap().is_empty());
        // A fresh peer must be able to download the re-published edition.
        let peer_path = dir.join("peer.db");
        let peer = Library::with_driver(Sqlite::at(&peer_path)).unwrap();
        let identity = Identity::join(r.space_descriptor()).unwrap();
        let replica = open(&peer_path, &identity).await.unwrap();
        metadata(&replica, store).await.unwrap();
        peer.check_external().unwrap();
        let downloads = dir.join("downloads");
        materialize(&peer, store, r.space(), &row.id, &downloads, false)
            .await
            .unwrap();
        let downloaded = std::fs::read(crate::shelf::path(&downloads, &row.id).unwrap()).unwrap();
        assert_eq!(downloaded.as_slice(), bytes);
        replica.close().await.unwrap();
    }

    #[test]
    fn reimport_upload_waits_for_inflight_delete_and_republishes_bytes() {
        block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("library.db");
            let store = MemoryStore::new();
            let (db, _, replica, row, root, bytes, _) = fixture(&path, &store).await;
            remove_from_replica(&replica, row.id.clone(), true)
                .await
                .unwrap();
            let (entered_tx, entered_rx) = oneshot::channel();
            let (release_tx, release_rx) = oneshot::channel();
            let paused = PausedDelete {
                inner: store.clone(),
                replica: replica.clone(),
                row: row.clone(),
                entered: Mutex::new(Some(entered_tx)),
                release: Mutex::new(Some(release_rx)),
                lose_response: false,
                published_body: None,
            };
            let mut deletion = std::pin::pin!(metadata(&replica, &paused));
            race(
                async {
                    (&mut deletion).await.unwrap();
                    panic!("DELETE must still be suspended")
                },
                async { entered_rx.await.unwrap() },
            )
            .await;
            let mut upload = std::pin::pin!(upload_book(&replica, &store, &row.id, &root));
            // Drain each worker roundtrip that an ungated upload could await. The
            // in-memory transport is immediately ready; no sleeps or timing assertions.
            for _ in 0..3 {
                assert!(
                    poll_once(&mut upload).await.is_none(),
                    "an upload must not finish before the older DELETE"
                );
                replica.worker().write(|_| Ok(())).await.unwrap();
            }
            release_tx.send(()).unwrap();
            let (deleted, uploaded) = zip(deletion, upload).await;
            deleted.unwrap();
            uploaded.unwrap();
            assert_recovered(&db, &replica, &store, &row, &bytes, temp.path()).await;
            replica.close().await.unwrap();
        });
    }

    #[test]
    fn lost_delete_response_and_restart_repair_an_existing_published_reference() {
        block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("library.db");
            let store = MemoryStore::new();
            let (db, identity, replica, row, root, bytes, body) = fixture(&path, &store).await;
            remove_from_replica(&replica, row.id.clone(), true)
                .await
                .unwrap();
            let failed = PausedDelete {
                inner: store.clone(),
                replica: replica.clone(),
                row: row.clone(),
                entered: Mutex::new(None),
                release: Mutex::new(None),
                lose_response: true,
                published_body: Some(body),
            };
            assert!(metadata(&replica, &failed).await.is_err());
            assert!(
                store
                    .get(&["books".into()], "Synthetic race fixture.epub")
                    .await
                    .unwrap()
                    .is_none()
            );
            replica.close().await.unwrap();
            let replica = open(&path, &identity).await.unwrap();
            metadata(&replica, &store).await.unwrap();
            assert!(
                db.db
                    .try_with_connection(|c| published(c, &row.id))
                    .unwrap()
                    .is_none(),
                "a stale reference is not a verified server file"
            );
            upload_book(&replica, &store, &row.id, &root).await.unwrap();
            assert_recovered(&db, &replica, &store, &row, &bytes, temp.path()).await;
            replica.close().await.unwrap();
        });
    }

    #[test]
    fn metadata_defers_deletion_while_a_transfer_owns_the_file() {
        block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let store = MemoryStore::new();
            let (_, _, replica, row, _, _, _) =
                fixture(&temp.path().join("library.db"), &store).await;
            remove_from_replica(&replica, row.id.clone(), true)
                .await
                .unwrap();
            let gate = file_operation(&replica, &row.id);
            let guard = gate.lock().await;
            metadata(&replica, &store).await.unwrap();
            assert!(
                store
                    .get(&["books".into()], "Synthetic race fixture.epub")
                    .await
                    .unwrap()
                    .is_some()
            );
            drop(guard);
            metadata(&replica, &store).await.unwrap();
            assert!(
                store
                    .get(&["books".into()], "Synthetic race fixture.epub")
                    .await
                    .unwrap()
                    .is_none()
            );
            replica.close().await.unwrap();
        });
    }
}
