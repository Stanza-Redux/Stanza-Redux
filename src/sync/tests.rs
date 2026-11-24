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
    assert_eq!(std::fs::read(bf.join(&file)).unwrap(), bytes);
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
    let dav = client(&url, user.clone(), password).unwrap();
    block_on(async {
        dav.client().check_writable_collection().await.unwrap();
        let invalid = client(&url, user, "synthetic-invalid-password".into()).unwrap();
        assert!(invalid.client().check_writable_collection().await.is_err());
        two_devices(&dav).await;
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
            r.collect_checkpoint_assets(&store, &backup, &pin, 99)
                .await
                .is_err()
        );
        pkg.publish(&store, &pin).await.unwrap();
        assert!(
            r.collect_checkpoint_assets(&store, &store, &pin, 99)
                .await
                .is_err()
        );
        let shard = [
            format!("day-assets-v1-{}", a.space()),
            "objects".into(),
            id[..2].to_owned(),
        ];
        assert!(store.corrupt(&shard, &id, b"synthetic damage".to_vec()));
        assert!(
            r.collect_checkpoint_assets(&store, &backup, &pin, 99)
                .await
                .is_err()
        );
        assert!(store.get(&shard, &id).await.unwrap().is_some());
        store.put(&shard, &id, bytes.clone()).await.unwrap();
        assert_eq!(
            r.collect_checkpoint_assets(&store, &backup, &pin, 1)
                .await
                .unwrap(),
            1
        );
        assert!(
            r.collect_checkpoint_assets(&store, &backup, &pin, 100)
                .await
                .unwrap()
                > 0
        );
        assert_eq!(
            r.collect_checkpoint_assets(&store, &backup, &pin, 100)
                .await
                .unwrap(),
            0
        );
        assert_eq!(backup.get(&shard, &id).await.unwrap(), Some(bytes.clone()));
        assert_eq!(store.get(&shard, &id).await.unwrap(), None);
        let target_shard = [
            format!("day-assets-v1-{}", b.space()),
            "objects".into(),
            id[..2].to_owned(),
        ];
        assert_eq!(store.get(&target_shard, &id).await.unwrap(), Some(bytes));
        target.close().await.unwrap();
        r.close().await.unwrap();
    });
}
