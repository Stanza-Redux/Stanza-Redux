-- Private replication state. The trigger outbox shares the library writer's transaction.
CREATE TABLE IF NOT EXISTS stanza_sync_control (id INTEGER PRIMARY KEY CHECK(id=1), projecting INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS stanza_sync_pending (
    field TEXT NOT NULL, book TEXT NOT NULL, item TEXT NOT NULL, body TEXT,
    PRIMARY KEY(field,book,item)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS stanza_sync_values (
    object TEXT PRIMARY KEY, field TEXT NOT NULL, book TEXT NOT NULL, item TEXT NOT NULL, body TEXT NOT NULL
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS stanza_values_book ON stanza_sync_values(book,field);
CREATE TABLE IF NOT EXISTS stanza_sync_descriptions(book TEXT PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,hash TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS stanza_sync_uploads(book TEXT PRIMARY KEY) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS stanza_sync_remote_files(book TEXT PRIMARY KEY,body TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS stanza_sync_hidden(book TEXT PRIMARY KEY) WITHOUT ROWID;
-- A DELETE may finish after re-import, or before the process records its response.
-- Keep this independent of deletion intent, which re-import is allowed to cancel.
CREATE TABLE IF NOT EXISTS stanza_sync_file_repairs(book TEXT PRIMARY KEY) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS stanza_sync_deletions(book TEXT PRIMARY KEY,body TEXT NOT NULL) WITHOUT ROWID;
CREATE TRIGGER IF NOT EXISTS stanza_sync_restore_local AFTER INSERT ON books
WHEN (SELECT projecting FROM stanza_sync_control)=0 BEGIN
    DELETE FROM stanza_sync_hidden WHERE book=NEW.id;
    DELETE FROM stanza_sync_deletions WHERE book=NEW.id;
END;


-- Seed in the same transaction that installs capture. Running this after a restart must
-- never manufacture fresh edits for earlier local or remote data.
INSERT OR IGNORE INTO stanza_sync_pending SELECT 'catalog',id,'',json_object('Catalog',json_object('title',title,'author',author,'added',added)) FROM books WHERE NOT EXISTS(SELECT 1 FROM stanza_sync_control);
INSERT OR IGNORE INTO stanza_sync_uploads SELECT id FROM books WHERE NOT EXISTS(SELECT 1 FROM stanza_sync_control);
INSERT OR IGNORE INTO stanza_sync_pending SELECT 'position',id,'',json_object('Position',json_object('chapter',chapter,'millionths',CAST(round(progress*1000000) AS INTEGER))) FROM books WHERE (chapter!=0 OR progress!=0) AND NOT EXISTS(SELECT 1 FROM stanza_sync_control);
INSERT OR IGNORE INTO stanza_sync_pending SELECT 'bookmark',book,id,json_object('Bookmark',json_object('chapter',chapter,'millionths',CAST(round(progress*1000000) AS INTEGER),'title',title)) FROM bookmarks WHERE NOT EXISTS(SELECT 1 FROM stanza_sync_control);
INSERT OR IGNORE INTO stanza_sync_pending SELECT 'opened',id,'',json_object('Opened',opened) FROM books WHERE opened>0 AND NOT EXISTS(SELECT 1 FROM stanza_sync_control);
INSERT OR IGNORE INTO stanza_sync_control VALUES(1,0);

CREATE TRIGGER IF NOT EXISTS stanza_sync_book_add AFTER INSERT ON books WHEN (SELECT projecting FROM stanza_sync_control)=0 BEGIN
    INSERT INTO stanza_sync_pending VALUES('catalog',NEW.id,'',json_object('Catalog',json_object('title',NEW.title,'author',NEW.author,'added',NEW.added))) ON CONFLICT DO UPDATE SET body=excluded.body;
    INSERT INTO stanza_sync_uploads VALUES(NEW.id) ON CONFLICT DO NOTHING;
    INSERT INTO stanza_sync_pending SELECT 'position',NEW.id,'',json_object('Position',json_object('chapter',NEW.chapter,'millionths',CAST(round(NEW.progress*1000000) AS INTEGER))) WHERE NEW.chapter!=0 OR NEW.progress!=0 ON CONFLICT DO UPDATE SET body=excluded.body;
END;
CREATE TRIGGER IF NOT EXISTS stanza_sync_book_edit AFTER UPDATE OF title,author ON books WHEN (SELECT projecting FROM stanza_sync_control)=0 AND (OLD.title!=NEW.title OR OLD.author!=NEW.author) BEGIN
    INSERT INTO stanza_sync_pending VALUES('catalog',NEW.id,'',json_object('Catalog',json_object('title',NEW.title,'author',NEW.author,'added',NEW.added))) ON CONFLICT DO UPDATE SET body=excluded.body;
END;
CREATE TRIGGER IF NOT EXISTS stanza_sync_opened AFTER UPDATE OF opened ON books WHEN (SELECT projecting FROM stanza_sync_control)=0 AND OLD.opened!=NEW.opened BEGIN
    INSERT INTO stanza_sync_pending VALUES('opened',NEW.id,'',json_object('Opened',NEW.opened)) ON CONFLICT DO UPDATE SET body=excluded.body;
END;
CREATE TRIGGER IF NOT EXISTS stanza_sync_position AFTER UPDATE OF chapter,progress ON books WHEN (SELECT projecting FROM stanza_sync_control)=0 AND (OLD.chapter!=NEW.chapter OR OLD.progress!=NEW.progress) BEGIN
    INSERT INTO stanza_sync_pending VALUES('position',NEW.id,'',json_object('Position',json_object('chapter',NEW.chapter,'millionths',CAST(round(NEW.progress*1000000) AS INTEGER)))) ON CONFLICT DO UPDATE SET body=excluded.body;
END;
CREATE TRIGGER IF NOT EXISTS stanza_sync_book_delete AFTER DELETE ON books WHEN (SELECT projecting FROM stanza_sync_control)=0 BEGIN
    INSERT INTO stanza_sync_pending VALUES('catalog',OLD.id,'',NULL),('position',OLD.id,'',NULL),('opened',OLD.id,'',NULL),('file',OLD.id,'',NULL) ON CONFLICT DO UPDATE SET body=NULL;
    DELETE FROM stanza_sync_uploads WHERE book=OLD.id;
END;
CREATE TRIGGER IF NOT EXISTS stanza_sync_mark_add AFTER INSERT ON bookmarks WHEN (SELECT projecting FROM stanza_sync_control)=0 BEGIN
    INSERT INTO stanza_sync_pending VALUES('bookmark',NEW.book,NEW.id,json_object('Bookmark',json_object('chapter',NEW.chapter,'millionths',CAST(round(NEW.progress*1000000) AS INTEGER),'title',NEW.title))) ON CONFLICT DO UPDATE SET body=excluded.body;
END;
CREATE TRIGGER IF NOT EXISTS stanza_sync_mark_edit AFTER UPDATE OF title ON bookmarks WHEN (SELECT projecting FROM stanza_sync_control)=0 AND OLD.title!=NEW.title BEGIN
    INSERT INTO stanza_sync_pending VALUES('bookmark',NEW.book,NEW.id,json_object('Bookmark',json_object('chapter',NEW.chapter,'millionths',CAST(round(NEW.progress*1000000) AS INTEGER),'title',NEW.title))) ON CONFLICT DO UPDATE SET body=excluded.body;
END;
CREATE TRIGGER IF NOT EXISTS stanza_sync_mark_delete AFTER DELETE ON bookmarks WHEN (SELECT projecting FROM stanza_sync_control)=0 BEGIN
    INSERT INTO stanza_sync_pending VALUES('bookmark',OLD.book,OLD.id,NULL) ON CONFLICT DO UPDATE SET body=NULL;
END;

-- A UI window can write through a separate SQLite connection. After the coordinated
-- freeze, refuse a new user intent in the same transaction as its originating edit.
CREATE TRIGGER IF NOT EXISTS stanza_sync_frozen BEFORE INSERT ON stanza_sync_pending
WHEN EXISTS(SELECT 1 FROM _day_sync_fence) OR EXISTS(SELECT 1 FROM _day_sync_config WHERE poisoned!=0)
BEGIN SELECT RAISE(ABORT,'stanza.sync.writer_fenced'); END;

-- Metadata can also be supplied after the initial EPUB import. Queue it with the
-- originating transaction, including changes made from a cloud-only library.
CREATE TRIGGER IF NOT EXISTS stanza_sync_metadata_add AFTER INSERT ON book_metadata
WHEN (SELECT projecting FROM stanza_sync_control)=0 BEGIN
    INSERT INTO stanza_sync_uploads VALUES(NEW.book) ON CONFLICT DO NOTHING;
END;
CREATE TRIGGER IF NOT EXISTS stanza_sync_metadata_edit AFTER UPDATE OF json ON book_metadata
WHEN (SELECT projecting FROM stanza_sync_control)=0 AND NEW.json!=OLD.json BEGIN
    INSERT INTO stanza_sync_uploads VALUES(NEW.book) ON CONFLICT DO NOTHING;
END;
CREATE TRIGGER IF NOT EXISTS stanza_sync_upload_frozen BEFORE INSERT ON stanza_sync_uploads
WHEN EXISTS(SELECT 1 FROM _day_sync_fence) OR EXISTS(SELECT 1 FROM _day_sync_config WHERE poisoned!=0)
BEGIN SELECT RAISE(ABORT,'stanza.sync.writer_fenced'); END;
