// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Disposable libraries and bundled sample bytes. No personal libraries or credentials.
use super::*;
use crate::Saved;
use futures_lite::future::block_on;

async fn two_devices(dav: &dyn Store) {
    let temp = tempfile::tempdir().unwrap();
    let (ap, bp) = (temp.path().join("a.db"), temp.path().join("b.db"));
    let (af, bf) = (temp.path().join("files-a"), temp.path().join("files-b"));
    std::fs::create_dir(&af).unwrap();
    let bytes = crate::sample_book().unwrap();
    let parsed = crate::epub::parse(&bytes).unwrap();
    let id = &parsed.id;
    let file = format!("{id}.epub");
    std::fs::write(af.join(&file), &bytes).unwrap();
    let a = Library::with_driver(Sqlite::at(&ap)).unwrap();
    let b = Library::with_driver(Sqlite::at(&bp)).unwrap();
    let row = Saved {
        id: id.clone(),
        title: "Synthetic library edition".into(),
        author: "Fixture".into(),
        cover: None,
    };
    // A populated library enrolls with prior bookmarks and progress.
    a.put(&row).unwrap();
    a.save_metadata(
        id,
        &crate::metadata::Metadata {
            subjects: vec!["Synthetic subject".into()],
            ..Default::default()
        },
    )
    .unwrap();
    a.position(id, 2, 0.375).unwrap();
    a.opened(id, 123456).unwrap();
    a.add_bookmark(id, 1, 0.25, "Fixture mark".into()).unwrap();
    let space = descriptor(dav, None).await.unwrap();
    assert_eq!(descriptor(dav, None).await.unwrap(), space);
    let ai = Identity::join(space.clone()).unwrap();
    let bi = Identity::join(space.clone()).unwrap();
    let ar = open(&ap, &ai).await.unwrap();
    let br = open(&bp, &bi).await.unwrap();
    metadata(&ar, dav).await.unwrap();
    metadata(&br, dav).await.unwrap();
    b.check_external().unwrap();
    let book = b.book(id).unwrap();
    assert_eq!(book.opened, 123456);
    assert_eq!(
        (book.title, book.chapter, book.progress),
        (row.title.clone(), 2, 0.375)
    );
    assert_eq!(b.bookmark_query(id).try_collect().unwrap().len(), 1);
    assert!(!bf.join(&file).exists());
    assert!(
        materialize(&b, dav, &space.id, id, &bf, false)
            .await
            .is_err()
    );
    upload_book(&ar, dav, id, &af).await.unwrap();
    metadata(&ar, dav).await.unwrap();
    metadata(&br, dav).await.unwrap();
    descriptions(&br, dav, temp.path()).await.unwrap();
    b.check_external().unwrap();
    assert_eq!(b.metadata(id).unwrap().subjects, vec!["Synthetic subject"]);
    assert!(!bf.join(&file).exists());
    // Catalog-only devices can contribute later metadata without possessing the EPUB.
    let mut amended = b.metadata(id).unwrap();
    amended.publisher = "Synthetic publisher".into();
    b.save_metadata(id, &amended).unwrap();
    descriptions(&br, dav, temp.path()).await.unwrap();
    upload_book(&br, dav, id, &bf).await.unwrap();
    assert!(!bf.exists());
    metadata(&br, dav).await.unwrap();
    metadata(&ar, dav).await.unwrap();
    descriptions(&ar, dav, temp.path()).await.unwrap();
    a.check_external().unwrap();
    assert_eq!(a.metadata(id).unwrap().publisher, amended.publisher);
    assert!(
        a.db.try_with_connection(|c| rows(c, "SELECT book FROM stanza_sync_uploads", &[]))
            .unwrap()
            .is_empty()
    );
    materialize(&b, dav, &space.id, id, &bf, false)
        .await
        .unwrap();
    let named = "Synthetic library edition.epub";
    assert_eq!(
        dav.get(&["books".into()], named).await.unwrap().unwrap(),
        bytes
    );
    for dir in [&af, &bf] {
        assert_eq!(crate::shelf::path(dir, id).unwrap(), dir.join(named));
        assert_eq!(std::fs::read(dir.join(named)).unwrap(), bytes);
        assert!(!dir.join(&file).exists());
    }
    let large: Vec<u8> = (0..assets::CHUNK_BYTES * 2 + 137)
        .map(|n| (n % 253) as u8)
        .collect();
    let receipts = temp.path().join("resume.sqlite");
    let first = assets::upload_with_journal(
        dav,
        &space.id,
        large.as_slice(),
        Some(&assets::UploadJournal::open(&receipts).unwrap()),
    )
    .await
    .unwrap();
    let resumed = assets::upload_with_journal(
        dav,
        &space.id,
        large.as_slice(),
        Some(&assets::UploadJournal::open(&receipts).unwrap()),
    )
    .await
    .unwrap();
    assert_eq!(first, resumed);
    let mut staged = tempfile::tempfile().unwrap();
    std::io::Write::write_all(&mut staged, &large[..assets::CHUNK_BYTES + 9]).unwrap();
    assets::download(dav, &space.id, &first, &mut staged)
        .await
        .unwrap();
    std::io::Seek::rewind(&mut staged).unwrap();
    let mut recovered = Vec::new();
    std::io::Read::read_to_end(&mut staged, &mut recovered).unwrap();
    assert_eq!(large, recovered);
    // Unsent changes on both devices, including a deliberate backward move.
    a.add_bookmark(id, 2, 0.5, "Second fixture".into()).unwrap();
    b.position(id, 0, 0.125).unwrap();
    metadata(&ar, dav).await.unwrap();
    // An uninstrumented UI writer's pending edit must be captured first.
    assert!(br.sync(dav).await.is_err());
    metadata(&br, dav).await.unwrap();
    metadata(&ar, dav).await.unwrap();
    a.check_external().unwrap();
    b.check_external().unwrap();
    assert_eq!(a.book(id).unwrap().progress, 0.125);
    assert_eq!(b.bookmark_query(id).try_collect().unwrap().len(), 2);
    // Concurrent progress offers both exact locations. A stale dialog cannot overwrite
    // another device's newer choice.
    a.position(id, 3, 0.75).unwrap();
    b.position(id, 1, 0.5).unwrap();
    metadata(&ar, dav).await.unwrap();
    metadata(&br, dav).await.unwrap();
    metadata(&ar, dav).await.unwrap();
    let key = day_sync::RecordKey {
        model: PROFILE.into(),
        field: "position".into(),
        key: id.clone(),
    };
    let object = key.change::<Field>(None).unwrap().id;
    let choices = ar.candidates(object.clone()).await.unwrap();
    assert_eq!(choices.len(), 2);
    assert!(ar.choose_candidate(choices.clone(), 0).await.unwrap());
    assert!(!ar.choose_candidate(choices, 1).await.unwrap());
    metadata(&ar, dav).await.unwrap();
    metadata(&br, dav).await.unwrap();
    assert_eq!(br.candidates(object).await.unwrap().len(), 1);
    let mark = b.bookmark_query(id).try_collect().unwrap()[0].id.clone();
    b.remove_bookmark(id, &mark).unwrap();
    br.close().await.unwrap();
    let br = open(&bp, &bi).await.unwrap();
    metadata(&br, dav).await.unwrap();
    metadata(&ar, dav).await.unwrap();
    a.check_external().unwrap();
    assert_eq!(a.bookmark_query(id).try_collect().unwrap().len(), 1);
    a.remove(id).unwrap();
    metadata(&ar, dav).await.unwrap();
    metadata(&br, dav).await.unwrap();
    b.check_external().unwrap();
    assert!(b.book(id).is_none());
    assert!(b.bookmark_query(id).try_collect().unwrap().is_empty());
    // Compacted history also bootstraps a fresh, empty library after deletion.
    ar.compact(dav).await.unwrap();
    let cp = temp.path().join("fresh.db");
    let ci = Identity::join(space).unwrap();
    let cr = open(&cp, &ci).await.unwrap();
    metadata(&cr, dav).await.unwrap();
    assert!(
        Library::with_driver(Sqlite::at(cp))
            .unwrap()
            .book(id)
            .is_none()
    );
    cr.close().await.unwrap();
    ar.close().await.unwrap();
    br.close().await.unwrap();
}
#[test]
fn sync_two_libraries_and_resume_offline() {
    block_on(two_devices(&day_sync::MemoryStore::default()));
}
#[test]
#[ignore = "requires a disposable DAV collection and synthetic account"]
fn dav_library_roundtrip() {
    let url = std::env::var("STANZA_TEST_DAV").unwrap();
    let password = std::env::var("STANZA_TEST_PASSWORD").unwrap();
    let server = url::Url::parse(&url).unwrap();
    assert_eq!(server.host_str(), Some("127.0.0.1"));
    assert!(
        server
            .path()
            .trim_end_matches('/')
            .split('/')
            .next_back()
            .unwrap()
            .starts_with("stanza-fixture-")
    );
    let user = std::env::var("STANZA_TEST_USER").unwrap();
    let dav = client(&url, user.clone(), password, Policy::default()).unwrap();
    block_on(async {
        dav.client().check_writable_collection().await.unwrap();
        let invalid = client(
            &url,
            user,
            "synthetic-invalid-password".into(),
            Policy::default(),
        )
        .unwrap();
        assert!(invalid.client().check_writable_collection().await.is_err());
        let root = dav;
        let dav = layout(root.clone(), true).unwrap();
        two_devices(&dav).await;
        let descriptor_bytes = root
            .get(&[".sync".into(), "stanza-redux-v1".into()], "library.json")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            SyncSpace::from_bytes(&descriptor_bytes).unwrap(),
            descriptor(&dav, None).await.unwrap()
        );
        assert_eq!(
            root.get(&["books".into()], "Synthetic library edition.epub")
                .await
                .unwrap()
                .unwrap(),
            crate::sample_book().unwrap()
        );
        claim_colliding_names(&dav).await;
        // Exercise a whole file larger than the regular DAV buffer cap using a real server.
        use sha2::{Digest, Sha256};
        use std::io::{Seek as _, Write as _};
        let mut upload = tempfile::NamedTempFile::new().unwrap();
        let chunk = vec![0x51; 1024 * 1024];
        let mut digest = Sha256::new();
        for _ in 0..12 {
            upload.write_all(&chunk).unwrap();
            digest.update(&chunk);
        }
        upload.as_file().sync_all().unwrap();
        let hash = format!("{:x}", digest.finalize());
        let file = assets::AssetFile {
            directory: "books".into(),
            name: "Large test édition.epub".into(),
        };
        let reference = assets::upload_file(&dav, file.clone(), upload.path(), &hash, |_| {})
            .await
            .unwrap()
            .unwrap();
        let mut copy = tempfile::tempfile().unwrap();
        let space = descriptor(&dav, None).await.unwrap();
        assets::download(&dav, &space.id, &reference, &mut copy)
            .await
            .unwrap();
        copy.rewind().unwrap();
        assert_eq!(crate::shelf::digest(&mut copy).unwrap(), hash);
        assert!(
            dav.list(&["books".into()])
                .await
                .unwrap()
                .contains(&file.name)
        );
        let mut other = tempfile::NamedTempFile::new().unwrap();
        other.write_all(b"second edition").unwrap();
        let other_hash = format!("{:x}", Sha256::digest(b"second edition"));
        assert!(
            assets::upload_file(&dav, file, other.path(), &other_hash, |_| {})
                .await
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn checkpoint_verifies_media_before_reclaim_and_resumes() {
    block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let store = day_sync::MemoryStore::new();
        let backup = day_sync::MemoryStore::new();
        let a = Identity::create_space_for(PROFILE).unwrap();
        let b = Identity::create_space_for(PROFILE).unwrap();
        let dbfile = dir.path().join("source.db");
        let lib = Library::with_driver(Sqlite::at(&dbfile)).unwrap();
        let r = open(&dbfile, &a).await.unwrap();
        let target = open(&dir.path().join("target.db"), &b).await.unwrap();
        let bytes = crate::sample_book().unwrap();
        let book = crate::epub::parse(&bytes).unwrap();
        let id = book.id;
        lib.put(&Saved {
            id: id.clone(),
            title: "Checkpoint fixture".into(),
            author: String::new(),
            cover: None,
        })
        .unwrap();
        std::fs::write(dir.path().join(format!("{id}.epub")), &bytes).unwrap();
        metadata(&r, &store).await.unwrap();
        assert!(r.freeze(a.actor()).await.is_err());
        upload_book(&r, &store, &id, dir.path()).await.unwrap();
        metadata(&r, &store).await.unwrap();
        r.freeze(a.actor()).await.unwrap();
        assert!(lib.position(&id, 4, 0.5).is_err());
        r.sync(&store).await.unwrap();
        let pkg = r
            .prepare_checkpoint(&target, vec![r.attest().await.unwrap()], vec![])
            .await
            .unwrap();
        let pin = pkg.pin().unwrap();
        assert!(
            r.collect_checkpoint_assets(&store, &backup, &pin, 99, dir.path())
                .await
                .is_err()
        );
        pkg.publish(&store, &pin).await.unwrap();
        assert!(
            r.collect_checkpoint_assets(&store, &store, &pin, 99, dir.path())
                .await
                .is_err()
        );
        let shelf = ["books".into()];
        let filename = "Checkpoint fixture.epub";
        assert!(store.corrupt(&shelf, filename, b"synthetic damage".to_vec()));
        assert!(
            r.collect_checkpoint_assets(&store, &backup, &pin, 99, dir.path())
                .await
                .is_err()
        );
        assert!(backup.get(&shelf, filename).await.unwrap().is_none());
        store.put(&shelf, filename, bytes.clone()).await.unwrap();
        // A legacy chunk still needs an independent backup before reclamation.
        let old = assets::upload(&store, a.space(), b"legacy orphan".as_slice())
            .await
            .unwrap();
        let shard = [
            format!("day-assets-v1-{}", a.space()),
            "objects".into(),
            old.sha256[..2].into(),
        ];
        assert_eq!(
            r.collect_checkpoint_assets(&store, &backup, &pin, 1, dir.path())
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            r.collect_checkpoint_assets(&store, &backup, &pin, 100, dir.path())
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            backup.get(&shard, &old.sha256).await.unwrap(),
            Some(b"legacy orphan".to_vec())
        );
        assert_eq!(store.get(&shard, &old.sha256).await.unwrap(), None);
        assert_eq!(
            backup.get(&shelf, filename).await.unwrap(),
            Some(bytes.clone())
        );
        assert_eq!(store.get(&shelf, filename).await.unwrap(), Some(bytes));
        target.close().await.unwrap();
        r.close().await.unwrap();
    });
}

#[test]
fn an_existing_chunked_book_migrates_without_a_local_epub() {
    block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let store = day_sync::MemoryStore::new();
        let dbfile = temp.path().join("library.db");
        let db = Library::with_driver(Sqlite::at(&dbfile)).unwrap();
        let bytes = crate::sample_book().unwrap();
        let id = crate::epub::parse(&bytes).unwrap().id;
        let identity = Identity::create_space_for(PROFILE).unwrap();
        db.put(&Saved {
            id: id.clone(),
            title: "Legacy edition".into(),
            author: String::new(),
            cover: None,
        })
        .unwrap();
        let replica = open(&dbfile, &identity).await.unwrap();
        metadata(&replica, &store).await.unwrap();
        let old_epub = assets::upload(&store, identity.space(), bytes.as_slice())
            .await
            .unwrap();
        let old_metadata = assets::upload(
            &store,
            identity.space(),
            br#"{"publisher":"Fixture archive"}"#.as_slice(),
        )
        .await
        .unwrap();
        let body = serde_json::to_string(&Field::File {
            epub: old_epub,
            metadata: old_metadata,
        })
        .unwrap();
        let key = id.clone();
        replica
            .worker()
            .write(move |db| {
                db.try_with_connection(|c| {
                    c.execute(
                        "INSERT INTO stanza_sync_pending VALUES('file',?,'',?)",
                        &[Value::Text(key), Value::Text(body)],
                    )?;
                    c.execute("DELETE FROM stanza_sync_uploads", &[])?;
                    Ok(())
                })
            })
            .await
            .unwrap();
        metadata(&replica, &store).await.unwrap();
        let offline = temp.path().join("no-local-epub");
        upload_book(&replica, &store, &id, &offline).await.unwrap();
        metadata(&replica, &store).await.unwrap();
        assert!(!offline.exists());
        descriptions(&replica, &store, temp.path()).await.unwrap();
        db.check_external().unwrap();
        assert_eq!(db.metadata(&id).unwrap().publisher, "Fixture archive");
        assert_eq!(
            store
                .get(&["books".into()], "Legacy edition.epub")
                .await
                .unwrap(),
            Some(bytes.clone())
        );
        materialize(&db, &store, identity.space(), &id, &offline, false)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(offline.join("Legacy edition.epub")).unwrap(),
            bytes
        );
        let previous = offline.join(format!("{id}.epub"));
        std::fs::rename(offline.join("Legacy edition.epub"), &previous).unwrap();
        std::fs::remove_file(offline.join(".stanza").join(&id)).unwrap();
        reconcile_names(&db, &offline).await.unwrap();
        assert!(!previous.exists());
        assert_eq!(
            std::fs::read(offline.join("Legacy edition.epub")).unwrap(),
            bytes
        );
        replica.close().await.unwrap();
    });
}

#[test]
fn missing_remote_metadata_is_republished_from_an_identical_local_copy() {
    block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let store = day_sync::MemoryStore::new();
        let dbfile = temp.path().join("library.db");
        let db = Library::with_driver(Sqlite::at(&dbfile)).unwrap();
        let bytes = crate::sample_book().unwrap();
        let id = crate::epub::parse(&bytes).unwrap().id;
        let identity = Identity::create_space_for(PROFILE).unwrap();
        db.put(&Saved {
            id: id.clone(),
            title: "Recovery fixture".into(),
            ..Default::default()
        })
        .unwrap();
        db.save_metadata(
            &id,
            &crate::metadata::Metadata {
                publisher: "Fixture archive".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let details = db
            .db
            .try_with_connection(|c| {
                Ok(rows(
                    c,
                    "SELECT json FROM book_metadata WHERE book=?",
                    &[Value::Text(id.clone())],
                )?[0][0]
                    .as_text()?
                    .to_owned())
            })
            .unwrap();
        let replica = open(&dbfile, &identity).await.unwrap();
        metadata(&replica, &store).await.unwrap();
        let old_epub = assets::upload(&store, identity.space(), bytes.as_slice())
            .await
            .unwrap();
        let old_metadata = assets::upload(&store, identity.space(), details.as_bytes())
            .await
            .unwrap();
        let chunk_dir = vec![
            format!("day-assets-v1-{}", identity.space()),
            "objects".into(),
            old_metadata.sha256[..2].into(),
        ];
        for name in store.list(&chunk_dir).await.unwrap() {
            store.delete(&chunk_dir, &name).await.unwrap();
        }
        assert!(
            assets::download(
                &store,
                identity.space(),
                &old_metadata,
                &mut tempfile::tempfile().unwrap()
            )
            .await
            .is_err()
        );
        let body = serde_json::to_string(&Field::File {
            epub: old_epub,
            metadata: old_metadata,
        })
        .unwrap();
        let key = id.clone();
        replica
            .worker()
            .write(move |db| {
                db.try_with_connection(|c| {
                    c.execute(
                        "INSERT INTO stanza_sync_pending VALUES('file',?,'',?)",
                        &[Value::Text(key), Value::Text(body)],
                    )?;
                    c.execute("DELETE FROM stanza_sync_uploads", &[])?;
                    Ok(())
                })
            })
            .await
            .unwrap();
        metadata(&replica, &store).await.unwrap();
        let local = temp.path().join("books");
        std::fs::create_dir(&local).unwrap();
        std::fs::write(local.join(format!("{id}.epub")), &bytes).unwrap();
        upload_book(&replica, &store, &id, &local).await.unwrap();
        metadata(&replica, &store).await.unwrap();
        descriptions(&replica, &store, temp.path()).await.unwrap();
        db.check_external().unwrap();
        assert_eq!(db.metadata(&id).unwrap().publisher, "Fixture archive");
        assert_eq!(
            store
                .get(&["books".into()], "Recovery fixture.epub")
                .await
                .unwrap(),
            Some(bytes)
        );
        // A matching local description needs no remote GET, even if that object is absent.
        use sha2::{Digest, Sha256};
        let name = format!("{:x}.json", Sha256::digest(details.as_bytes()));
        assert!(
            store
                .delete(&["stanza-redux-v1".into()], &name)
                .await
                .unwrap()
        );
        replica
            .worker()
            .write(|db| {
                db.try_with_connection(|c| c.execute_batch("DELETE FROM stanza_sync_descriptions"))
            })
            .await
            .unwrap();
        descriptions(&replica, &store, temp.path()).await.unwrap();
        // A different local description cannot stand in for the referenced server object.
        replica.worker().write(|db| db.try_with_connection(|c| c.execute_batch(
            "UPDATE stanza_sync_control SET projecting=1; UPDATE book_metadata SET json='{}'; UPDATE stanza_sync_control SET projecting=0; DELETE FROM stanza_sync_descriptions;"
        ))).await.unwrap();
        assert!(descriptions(&replica, &store, temp.path()).await.is_err());
        replica.close().await.unwrap();
    });
}

#[test]
fn filename_claims_converge_across_filesystem_case_and_normalization_rules() {
    block_on(claim_colliding_names(&day_sync::MemoryStore::new()));
}
async fn claim_colliding_names(dav: &dyn Store) {
    let first = "a".repeat(64);
    let second = "b".repeat(64);
    // Simulate independent writers on a case-sensitive store.
    let composed = "Straße Étude.epub";
    let decomposed = "STRASSE E\u{301}TUDE.epub";
    let (a, b) = futures_lite::future::zip(
        claim_name(dav, composed.into(), &first),
        claim_name(dav, decomposed.into(), &second),
    )
    .await;
    let (winner, loser, spelling) = match (a.unwrap(), b.unwrap()) {
        (Some(name), None) => (&first, &second, name),
        (None, Some(name)) => (&second, &first, name),
        _ => panic!("exactly one edition must own a portable filename"),
    };
    assert_eq!(
        claim_name(dav, decomposed.into(), winner).await.unwrap(),
        Some(spelling)
    );
    let suffix = crate::shelf::candidates(decomposed, loser)[1].clone();
    assert_eq!(
        claim_name(dav, suffix.clone(), loser).await.unwrap(),
        Some(suffix)
    );
}
