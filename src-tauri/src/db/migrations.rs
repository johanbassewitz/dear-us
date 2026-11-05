//! Forward-only schema migrations tracked with SQLite's `user_version`.
//! Never edit a migration that has shipped — always append a new one.

use crate::error::AppResult;
use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    // 1 — foundation: local settings + who we are
    r#"
    CREATE TABLE settings (
        key        TEXT PRIMARY KEY,
        value      TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE users (
        id              TEXT PRIMARY KEY,
        display_name    TEXT NOT NULL,
        is_self         INTEGER NOT NULL DEFAULT 0,
        avatar_media_id TEXT,
        created_at      TEXT NOT NULL,
        updated_at      TEXT NOT NULL
    );
    CREATE UNIQUE INDEX one_self_user ON users(is_self) WHERE is_self = 1;

    CREATE TABLE space (
        id         TEXT PRIMARY KEY,
        name       TEXT NOT NULL,
        start_date TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    "#,
    // 2 — journal, media, milestones, op-log (sync-ready from day one)
    r#"
    CREATE TABLE local_meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    -- Content-addressed and immutable. Garbage-collected only when nothing references it.
    CREATE TABLE media (
        id             TEXT PRIMARY KEY,
        hash           TEXT NOT NULL UNIQUE,
        kind           TEXT NOT NULL CHECK (kind IN ('photo', 'video')),
        mime           TEXT NOT NULL,
        ext            TEXT NOT NULL,
        size           INTEGER NOT NULL,
        width          INTEGER,
        height         INTEGER,
        duration_ms    INTEGER,
        taken_at       TEXT,
        original_name  TEXT,
        has_thumb      INTEGER NOT NULL DEFAULT 0,
        preview        TEXT NOT NULL DEFAULT 'none'
                       CHECK (preview IN ('none', 'needed', 'image', 'video', 'failed')),
        availability   TEXT NOT NULL DEFAULT 'local'
                       CHECK (availability IN ('local', 'pending', 'missing')),
        created_by     TEXT,
        created_at     TEXT NOT NULL
    );

    CREATE TABLE journal_entries (
        id             TEXT PRIMARY KEY,
        author_id      TEXT NOT NULL REFERENCES users(id),
        title          TEXT NOT NULL,
        body_json      TEXT NOT NULL DEFAULT '',
        body_text      TEXT NOT NULL DEFAULT '',
        entry_date     TEXT NOT NULL,
        entry_time     TEXT,
        location       TEXT,
        tags_json      TEXT NOT NULL DEFAULT '[]',
        cover_media_id TEXT REFERENCES media(id),
        highlight      INTEGER NOT NULL DEFAULT 0,
        created_at     TEXT NOT NULL,
        updated_at     TEXT NOT NULL,
        deleted_at     TEXT,
        field_clock    TEXT NOT NULL DEFAULT '{}'
    );
    CREATE INDEX journal_by_date ON journal_entries(deleted_at, entry_date DESC, entry_time DESC);

    CREATE TABLE entry_media (
        entry_id  TEXT NOT NULL REFERENCES journal_entries(id) ON DELETE CASCADE,
        media_id  TEXT NOT NULL REFERENCES media(id),
        position  INTEGER NOT NULL,
        caption   TEXT,
        PRIMARY KEY (entry_id, media_id)
    );
    CREATE INDEX entry_media_by_media ON entry_media(media_id);

    CREATE VIRTUAL TABLE journal_fts USING fts5(
        entry_id UNINDEXED, title, body, tags, location,
        tokenize = "unicode61 remove_diacritics 2"
    );

    CREATE TABLE milestones (
        id              TEXT PRIMARY KEY,
        title           TEXT NOT NULL,
        date            TEXT NOT NULL,
        description     TEXT NOT NULL DEFAULT '',
        cover_media_id  TEXT REFERENCES media(id),
        linked_entry_id TEXT REFERENCES journal_entries(id),
        created_by      TEXT,
        created_at      TEXT NOT NULL,
        updated_at      TEXT NOT NULL,
        deleted_at      TEXT,
        field_clock     TEXT NOT NULL DEFAULT '{}'
    );
    CREATE INDEX milestones_by_date ON milestones(deleted_at, date);

    -- Every shared change, in order. Phase 4 ships these to the partner's PC.
    CREATE TABLE oplog (
        op_id        TEXT PRIMARY KEY,
        hlc          TEXT NOT NULL,
        device_id    TEXT NOT NULL,
        entity       TEXT NOT NULL,
        entity_id    TEXT NOT NULL,
        kind         TEXT NOT NULL CHECK (kind IN ('upsert', 'delete', 'restore', 'purge')),
        changes_json TEXT NOT NULL,
        created_at   TEXT NOT NULL,
        state        TEXT NOT NULL DEFAULT 'pending'
    );
    CREATE INDEX oplog_by_state ON oplog(state, hlc);
    "#,
    // ---- Shared bucket list ----
    r#"
    CREATE TABLE bucket_items (
        id              TEXT PRIMARY KEY,
        title           TEXT NOT NULL,
        description     TEXT NOT NULL DEFAULT '',
        category        TEXT NOT NULL DEFAULT '',
        priority        TEXT NOT NULL DEFAULT 'medium' CHECK (priority IN ('low','medium','high')),
        status          TEXT NOT NULL DEFAULT 'want' CHECK (status IN ('want','planned','done')),
        target_date     TEXT,
        completed_at    TEXT,
        linked_entry_id TEXT REFERENCES journal_entries(id),
        cover_media_id  TEXT REFERENCES media(id),
        created_by      TEXT,
        created_at      TEXT NOT NULL,
        updated_at      TEXT NOT NULL,
        deleted_at      TEXT,
        field_clock     TEXT NOT NULL DEFAULT '{}'
    );
    CREATE INDEX bucket_by_status ON bucket_items(deleted_at, status);

    CREATE TABLE bucket_media (
        item_id   TEXT NOT NULL REFERENCES bucket_items(id) ON DELETE CASCADE,
        media_id  TEXT NOT NULL REFERENCES media(id),
        position  INTEGER NOT NULL,
        PRIMARY KEY (item_id, media_id)
    );
    CREATE INDEX bucket_media_by_media ON bucket_media(media_id);
    "#,
    r#"
    CREATE TABLE special_dates (
        id                 TEXT PRIMARY KEY,
        title              TEXT NOT NULL,
        date               TEXT NOT NULL,
        time               TEXT,
        description        TEXT NOT NULL DEFAULT '',
        recurrence         TEXT NOT NULL DEFAULT 'none' CHECK (recurrence IN ('none','yearly','monthly')),
        remind_days_before INTEGER CHECK (remind_days_before IS NULL OR (remind_days_before BETWEEN 0 AND 60)),
        linked_entry_id    TEXT REFERENCES journal_entries(id),
        created_by         TEXT,
        created_at         TEXT NOT NULL,
        updated_at         TEXT NOT NULL,
        deleted_at         TEXT,
        field_clock        TEXT NOT NULL DEFAULT '{}'
    );
    CREATE INDEX special_dates_by_date ON special_dates(deleted_at, date);
    "#,
    // ---- Date planner: plans and the couple's own date ideas ----
    r#"
    CREATE TABLE planned_dates (
        id              TEXT PRIMARY KEY,
        title           TEXT NOT NULL,
        description     TEXT NOT NULL DEFAULT '',
        date            TEXT,
        time            TEXT,
        location        TEXT NOT NULL DEFAULT '',
        budget_cents    INTEGER CHECK (budget_cents IS NULL OR budget_cents >= 0),
        currency        TEXT NOT NULL DEFAULT 'USD',
        status          TEXT NOT NULL DEFAULT 'idea' CHECK (status IN ('idea','planned','done','cancelled')),
        checklist_json  TEXT NOT NULL DEFAULT '[]',
        idea_ref        TEXT,
        linked_entry_id TEXT REFERENCES journal_entries(id),
        created_by      TEXT,
        created_at      TEXT NOT NULL,
        updated_at      TEXT NOT NULL,
        deleted_at      TEXT,
        field_clock     TEXT NOT NULL DEFAULT '{}'
    );
    CREATE INDEX planned_dates_by_date ON planned_dates(deleted_at, status, date);

    CREATE TABLE date_ideas (
        id          TEXT PRIMARY KEY,
        title       TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        category    TEXT NOT NULL,
        budget      TEXT NOT NULL,
        duration    TEXT NOT NULL,
        setting     TEXT NOT NULL,
        mood        TEXT NOT NULL,
        created_by  TEXT,
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL,
        deleted_at  TEXT,
        field_clock TEXT NOT NULL DEFAULT '{}'
    );
    "#,
    r#"
    CREATE TABLE custom_questions (
        id          TEXT PRIMARY KEY,
        text        TEXT NOT NULL,
        category    TEXT NOT NULL,
        created_by  TEXT,
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL,
        deleted_at  TEXT,
        field_clock TEXT NOT NULL DEFAULT '{}'
    );
    CREATE TABLE answers (
        id                TEXT PRIMARY KEY,
        question_id       TEXT NOT NULL,
        user_id           TEXT NOT NULL,
        body              TEXT NOT NULL,
        answered_for_date TEXT NOT NULL,
        created_at        TEXT NOT NULL,
        updated_at        TEXT NOT NULL,
        deleted_at        TEXT,
        field_clock       TEXT NOT NULL DEFAULT '{}'
    );
    CREATE INDEX idx_answers_question ON answers(question_id, answered_for_date);
    CREATE TABLE daily_questions (
        id               TEXT PRIMARY KEY,
        question_id      TEXT NOT NULL,
        picked_by_device TEXT NOT NULL,
        created_at       TEXT NOT NULL
    );
    -- Private to this device: never op-logged, never synced.
    CREATE TABLE moods (
        id         TEXT PRIMARY KEY,
        date       TEXT NOT NULL UNIQUE,
        mood       TEXT NOT NULL,
        note       TEXT NOT NULL DEFAULT '',
        created_at TEXT NOT NULL
    );
    "#,
    r#"
    -- Love letters and time capsules. Drafts stay on this device (never op-logged).
    -- unlocked_at is local state: set once a received capsule has been opened.
    CREATE TABLE letters (
        id               TEXT PRIMARY KEY,
        sender_id        TEXT NOT NULL,
        recipient_id     TEXT,
        title            TEXT NOT NULL DEFAULT '',
        body_json        TEXT NOT NULL DEFAULT '',
        kind             TEXT NOT NULL DEFAULT 'letter' CHECK (kind IN ('letter','capsule')),
        status           TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','sent')),
        sent_at          TEXT,
        unlock_at        TEXT,
        show_title       INTEGER NOT NULL DEFAULT 1,
        lock_kind        TEXT NOT NULL DEFAULT 'none' CHECK (lock_kind IN ('tlock','none')),
        tlock_ciphertext TEXT,
        unlocked_at      TEXT,
        created_at       TEXT NOT NULL,
        updated_at       TEXT NOT NULL,
        deleted_at       TEXT,
        field_clock      TEXT NOT NULL DEFAULT '{}'
    );
    CREATE INDEX letters_by_status ON letters(deleted_at, status, kind);
    CREATE TABLE letter_media (
        letter_id TEXT NOT NULL REFERENCES letters(id) ON DELETE CASCADE,
        media_id  TEXT NOT NULL REFERENCES media(id),
        position  INTEGER NOT NULL,
        PRIMARY KEY (letter_id, media_id)
    );
    CREATE INDEX letter_media_by_media ON letter_media(media_id);
    "#,
    r#"
    -- Reactions, comments and scrapbook styling for journal entries.
    ALTER TABLE journal_entries ADD COLUMN layout TEXT NOT NULL DEFAULT 'auto'
        CHECK (layout IN ('auto','polaroid','collage','magazine','minimal'));
    ALTER TABLE journal_entries ADD COLUMN frame TEXT NOT NULL DEFAULT 'none'
        CHECK (frame IN ('none','polaroid','tape','rounded','vintage'));
    ALTER TABLE journal_entries ADD COLUMN paper TEXT NOT NULL DEFAULT 'none'
        CHECK (paper IN ('none','dots','lines','kraft'));
    CREATE TABLE reactions (
        id          TEXT PRIMARY KEY,
        entry_id    TEXT NOT NULL,
        user_id     TEXT NOT NULL,
        type        TEXT NOT NULL CHECK (type IN ('love','emotional','funny','adorable','special')),
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL,
        deleted_at  TEXT,
        field_clock TEXT NOT NULL DEFAULT '{}'
    );
    CREATE INDEX reactions_by_entry ON reactions(entry_id, deleted_at);
    CREATE TABLE comments (
        id          TEXT PRIMARY KEY,
        entry_id    TEXT NOT NULL,
        author_id   TEXT NOT NULL,
        body        TEXT NOT NULL,
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL,
        deleted_at  TEXT,
        field_clock TEXT NOT NULL DEFAULT '{}'
    );
    CREATE INDEX comments_by_entry ON comments(entry_id, deleted_at);
    "#,
    // 9 — partner sync bookkeeping (Phase 4)
    r#"
    ALTER TABLE oplog ADD COLUMN envelope_id TEXT;

    -- Every remote op applied here, so a re-delivered envelope never applies twice.
    CREATE TABLE sync_applied (
        op_id      TEXT PRIMARY KEY,
        applied_at TEXT NOT NULL
    );

    -- Envelopes received but not yet confirmed back to the relay, and ones that failed to open.
    CREATE TABLE sync_inbox (
        envelope_id TEXT PRIMARY KEY,
        seq         INTEGER NOT NULL,
        state       TEXT NOT NULL CHECK (state IN ('applied', 'acked', 'failed')),
        error       TEXT,
        updated_at  TEXT NOT NULL
    );

    -- Envelopes this device sent, until every partner device has confirmed them.
    CREATE TABLE sync_outbox (
        envelope_id TEXT PRIMARY KEY,
        op_count    INTEGER NOT NULL,
        size        INTEGER NOT NULL,
        state       TEXT NOT NULL CHECK (state IN ('sent', 'delivered')),
        created_at  TEXT NOT NULL
    );

    -- Photo/video transfers (both directions), resumable chunk by chunk.
    CREATE TABLE sync_blobs (
        hash        TEXT NOT NULL,
        direction   TEXT NOT NULL CHECK (direction IN ('up', 'down')),
        media_id    TEXT NOT NULL,
        size        INTEGER NOT NULL,
        chunk_count INTEGER NOT NULL,
        chunks_done INTEGER NOT NULL DEFAULT 0,
        state       TEXT NOT NULL CHECK (state IN ('queued', 'active', 'done', 'delivered', 'failed')),
        attempts    INTEGER NOT NULL DEFAULT 0,
        last_error  TEXT,
        updated_at  TEXT NOT NULL,
        PRIMARY KEY (hash, direction)
    );

    -- Media the partner knows under a different id (both imported the same file).
    CREATE TABLE media_alias (
        remote_id TEXT PRIMARY KEY,
        local_id  TEXT NOT NULL
    );

    -- Both partners changed the same long text while apart: both versions kept.
    CREATE TABLE sync_conflicts (
        id            TEXT PRIMARY KEY,
        entity        TEXT NOT NULL,
        entity_id     TEXT NOT NULL,
        field         TEXT NOT NULL,
        kept_value    TEXT,
        other_value   TEXT,
        other_device  TEXT,
        created_at    TEXT NOT NULL,
        resolved_at   TEXT
    );
    "#,
    // 10 — direct PC-to-PC transfers (Phase 6)
    r#"
    ALTER TABLE sync_blobs ADD COLUMN direct_since TEXT;
    "#,
    // 11 — Cupid's chat with this person (local only, never synced or backed up)
    r#"
    CREATE TABLE cupid_chats (
        id         TEXT PRIMARY KEY,
        role       TEXT NOT NULL CHECK (role IN ('user', 'model')),
        content    TEXT NOT NULL,
        created_at TEXT NOT NULL
    );
    "#,
];

pub fn migrate(conn: &mut Connection) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
pub fn latest_version() -> i64 {
    MIGRATIONS.len() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_fresh_db_to_latest_and_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, latest_version());
    }
}
