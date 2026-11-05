//! Two-PC sync tests: two separate archives talking through an in-memory relay.

use super::crypto::SpaceKey;
use super::engine::{counts, Ctx, Report};
use super::relay::memory::MemoryRelay;
use super::relay::{Device, Relay};
use crate::db::repo::journal::{self, EntryInput, EntryMediaInput, EntryQuery};
use crate::db::repo::profile::{self, ProfileInput};
use crate::media::Store;
use crate::sync::oplog::Writer;
use rusqlite::Connection;
use std::sync::Mutex;
use tempfile::TempDir;

struct Peer {
    dir: TempDir,
    db: Mutex<Connection>,
    store: Store,
    device: String,
}

impl Peer {
    fn new(me: &str, partner: &str) -> Peer {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open(&dir.path().join("t.db")).unwrap();
        let device = super::device_id(&conn).unwrap();
        profile::save(
            &mut conn,
            &ProfileInput { my_name: me.into(), partner_name: Some(partner.into()), space_name: "Us".into(), start_date: Some("2024-07-03".into()) },
        )
        .unwrap();
        super::identity::record_profile_ops(&mut conn).unwrap();
        let store = Store::new(dir.path()).unwrap();
        Peer { dir, db: Mutex::new(conn), store, device }
    }
    fn me(&self) -> String {
        super::identity::my_user_id(&self.db.lock().unwrap()).unwrap().unwrap()
    }
    fn writer(&self) -> Writer {
        Writer { device_id: self.device.clone() }
    }
    fn sync(&self, relay: &MemoryRelay, key: &SpaceKey, chunk: i64, cap: i64) -> Report {
        Ctx { db: &self.db, direct: None, store: &self.store, relay, key, device: self.device.clone(), chunk_size: chunk, quota_cap: cap }
            .sync_once()
            .unwrap()
    }
    fn s(&self, relay: &MemoryRelay, key: &SpaceKey) -> Report {
        self.sync(relay, key, 1024 * 1024, i64::MAX)
    }
    fn entry(&self, title: &str, body: &str, media: Vec<String>) -> String {
        let mut i = journal::tests::input(title, "2025-07-12");
        i.body_json = doc(body);
        i.media = media.into_iter().map(|id| EntryMediaInput { id, caption: None }).collect();
        let me = self.me();
        journal::create(&mut self.db.lock().unwrap(), &self.writer(), &me, &i).unwrap().summary.id
    }
    fn edit(&self, id: &str, f: impl FnOnce(&mut EntryInput)) {
        let d = journal::get(&self.db.lock().unwrap(), id).unwrap();
        let mut i = EntryInput {
            title: d.summary.title.clone(),
            body_json: d.body_json.clone(),
            entry_date: d.summary.entry_date.clone(),
            entry_time: d.summary.entry_time.clone(),
            location: d.summary.location.clone(),
            tags: d.summary.tags.clone(),
            media: d.media.iter().map(|m| EntryMediaInput { id: m.media.id.clone(), caption: m.caption.clone() }).collect(),
            cover_media_id: d.cover_media_id.clone(),
            highlight: d.summary.highlight,
            ..journal::tests::input("x", "2025-01-01")
        };
        f(&mut i);
        journal::update(&mut self.db.lock().unwrap(), &self.writer(), id, &i).unwrap();
    }
    fn get(&self, id: &str) -> journal::EntryDetail {
        journal::get(&self.db.lock().unwrap(), id).unwrap()
    }
    fn titles(&self) -> Vec<String> {
        journal::list(&self.db.lock().unwrap(), &EntryQuery::default()).unwrap().into_iter().map(|e| e.title).collect()
    }
    fn photo(&self, name: &str, size: (u32, u32)) -> (String, Vec<u8>) {
        let p = self.dir.path().join(name);
        image::RgbImage::from_fn(size.0, size.1, |x, y| image::Rgb([(x * 7 % 255) as u8, (y * 3 % 255) as u8, 90])).save(&p).unwrap();
        let (m, _) = crate::media::import_one(&self.store, &self.db, &p, Some(self.me())).unwrap();
        (m.r.id, std::fs::read(&p).unwrap())
    }
    fn conflicts(&self) -> i64 {
        counts(&self.db.lock().unwrap()).unwrap().open_conflicts
    }
}

fn doc(t: &str) -> String {
    format!(r#"{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"{t}"}}]}}]}}"#)
}

fn pair() -> (MemoryRelay, SpaceKey, Peer, Peer) {
    let relay = MemoryRelay::default();
    let a = Peer::new("Jo", "Sam");
    let b = Peer::new("Sam", "Jo");
    {
        let mut s = relay.0.lock().unwrap();
        s.devices.push(Device { id: a.device.clone(), user_id: "ua".into(), name: "A".into(), revoked: false, endpoint: None });
        s.devices.push(Device { id: b.device.clone(), user_id: "ub".into(), name: "B".into(), revoked: false, endpoint: None });
    }
    (relay, SpaceKey::generate(), a, b)
}

fn relay_empty(r: &MemoryRelay) -> bool {
    let s = r.0.lock().unwrap();
    s.envelopes.is_empty() && s.chunks.is_empty() && s.blobs.is_empty()
}

#[test]
fn memory_with_photo_travels_a_to_b_and_relay_is_emptied_after_confirmation() {
    let (relay, key, a, b) = pair();
    let (pid, bytes) = a.photo("beach.png", (400, 300));
    let id = a.entry("Beach day", "Sunset and ice cream", vec![pid.clone()]);

    let ra = a.s(&relay, &key);
    assert!(ra.sent_ops > 0 && ra.uploaded_files == 1);
    // Uploaded, but nobody confirmed yet: the relay must keep it.
    assert!(!relay_empty(&relay));
    let first = relay.0.lock().unwrap().envelopes[0].0.id.clone();
    assert!(!relay.cleanup_envelope(&first).unwrap());

    let rb = b.s(&relay, &key);
    assert!(rb.received_ops > 0 && rb.downloaded_files == 1, "{rb:?}");
    let e = b.get(&id);
    assert_eq!(e.summary.title, "Beach day");
    assert_eq!(e.summary.excerpt, "Sunset and ice cream");
    assert_eq!(e.summary.author_name.as_deref(), Some("Jo"), "author is the real partner, not the placeholder");
    // the photo arrived byte-identical and is now part of B's archive
    let m = &e.media[0];
    assert_eq!(m.media.availability, "local");
    assert!(m.media.has_thumb);
    let row = crate::db::repo::media::get(&b.db.lock().unwrap(), &pid).unwrap().unwrap();
    assert_eq!(std::fs::read(b.store.original(&row.hash, &row.ext)).unwrap(), bytes);
    // search works on B too
    assert_eq!(journal::list(&b.db.lock().unwrap(), &EntryQuery { text: Some("ice".into()), ..Default::default() }).unwrap().len(), 1);

    a.s(&relay, &key); // A sees the confirmation and cleans up its side
    b.s(&relay, &key);
    assert!(relay_empty(&relay), "relay holds nothing once both PCs have everything");
    let ca = counts(&a.db.lock().unwrap()).unwrap();
    assert_eq!((ca.pending_ops, ca.awaiting_delivery), (0, 0));
}

#[test]
fn both_directions_and_partner_placeholder_is_replaced() {
    let (relay, key, a, b) = pair();
    a.entry("From A", "a", vec![]);
    b.entry("From B", "b", vec![]);
    for _ in 0..2 {
        a.s(&relay, &key);
        b.s(&relay, &key);
    }
    let mut ta = a.titles();
    let mut tb = b.titles();
    ta.sort();
    tb.sort();
    assert_eq!(ta, vec!["From A", "From B"]);
    assert_eq!(ta, tb);
    // B's "partner" is now A's real user id, with one partner row only
    let conn = b.db.lock().unwrap();
    let partners: Vec<(String, String)> = conn
        .prepare("SELECT id, display_name FROM users WHERE is_self = 0")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(partners, vec![(a.me(), "Jo".to_string())]);
}

#[test]
fn offline_edits_to_different_fields_merge() {
    let (relay, key, a, b) = pair();
    let id = a.entry("Picnic", "text", vec![]);
    a.s(&relay, &key);
    b.s(&relay, &key);
    a.edit(&id, |i| i.title = "Picnic ♡".into());
    b.edit(&id, |i| i.location = Some("Batroun".into()));
    for _ in 0..2 {
        a.s(&relay, &key);
        b.s(&relay, &key);
    }
    for p in [&a, &b] {
        let e = p.get(&id);
        assert_eq!(e.summary.title, "Picnic ♡");
        assert_eq!(e.summary.location.as_deref(), Some("Batroun"));
    }
    assert_eq!(a.conflicts() + b.conflicts(), 0);
}

#[test]
fn concurrent_text_edits_keep_both_versions() {
    let (relay, key, a, b) = pair();
    let id = a.entry("Trip", "original", vec![]);
    a.s(&relay, &key);
    b.s(&relay, &key);
    a.s(&relay, &key);
    a.edit(&id, |i| i.body_json = doc("Jo's version"));
    b.edit(&id, |i| i.body_json = doc("Sam's version"));
    for _ in 0..2 {
        a.s(&relay, &key);
        b.s(&relay, &key);
    }
    let (ea, eb) = (a.get(&id), b.get(&id));
    assert_eq!(ea.body_json, eb.body_json, "both PCs converge on one current text");
    assert!(a.conflicts() + b.conflicts() >= 1, "the other version is kept, not silently lost");
    let both: Vec<String> = [&a, &b]
        .iter()
        .flat_map(|p| {
            let c = p.db.lock().unwrap();
            let v: Vec<String> = c
                .prepare("SELECT kept_value || ' / ' || other_value FROM sync_conflicts")
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            v
        })
        .collect();
    let joined = both.join(" ");
    assert!(joined.contains("Jo's version") && joined.contains("Sam's version"), "{joined}");
}

#[test]
fn delete_on_one_pc_while_editing_on_the_other_loses_nothing() {
    let (relay, key, a, b) = pair();
    let id = a.entry("Fragile", "text", vec![]);
    a.s(&relay, &key);
    b.s(&relay, &key);
    journal::delete(&mut a.db.lock().unwrap(), &a.writer(), &id).unwrap();
    b.edit(&id, |i| i.title = "Fragile but edited".into());
    for _ in 0..2 {
        a.s(&relay, &key);
        b.s(&relay, &key);
    }
    for p in [&a, &b] {
        let trash = journal::trash(&p.db.lock().unwrap()).unwrap();
        assert_eq!(trash.len(), 1, "in the recovery bin, restorable");
        assert_eq!(trash[0].title, "Fragile but edited", "the edit survived");
    }
}

#[test]
fn applying_the_same_ops_twice_changes_nothing() {
    let (_relay, _key, a, b) = pair();
    a.entry("Once", "x", vec![]);
    let ops: Vec<super::apply::Op> = {
        let c = a.db.lock().unwrap();
        let mut s = c.prepare("SELECT op_id, hlc, device_id, entity, entity_id, kind, changes_json FROM oplog").unwrap();
        s.query_map([], |r| {
            Ok(super::apply::Op {
                op_id: r.get(0)?,
                hlc: r.get(1)?,
                device_id: r.get(2)?,
                entity: r.get(3)?,
                entity_id: r.get(4)?,
                kind: r.get(5)?,
                changes: serde_json::from_str(&r.get::<_, String>(6)?).unwrap(),
            })
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    };
    let c = b.db.lock().unwrap();
    let first = super::apply::apply_batch(&c, &ops, &b.device).unwrap();
    let second = super::apply::apply_batch(&c, &ops, &b.device).unwrap();
    assert!(first.applied > 0 && first.failed.is_empty(), "{:?}", first.failed);
    assert_eq!((second.applied, second.skipped_duplicates), (0, ops.len()));
    drop(c);
    assert_eq!(b.titles(), vec!["Once"]);
}

#[test]
fn damaged_or_foreign_envelopes_are_never_confirmed_or_deleted() {
    let (relay, key, _a, b) = pair();
    let other_couple = SpaceKey::generate();
    let ct = other_couple.seal_b64(b"{\"v\":1,\"ops\":[]}", b"env-x");
    relay.put_envelope("env-x", "someone", &[b.device.clone()], &ct).unwrap();
    let r = b.s(&relay, &key);
    assert!(!r.errors.is_empty());
    assert_eq!(counts(&b.db.lock().unwrap()).unwrap().failed_updates, 1);
    assert!(!relay.0.lock().unwrap().envelopes.is_empty(), "kept on the relay, not confirmed");
}

#[test]
fn multi_chunk_file_arrives_byte_identical() {
    let (relay, key, a, b) = pair();
    let (pid, bytes) = a.photo("big.png", (900, 700)); // many 4 KB chunks
    a.entry("Big one", "x", vec![pid.clone()]);
    a.sync(&relay, &key, 4096, i64::MAX);
    assert!(relay.0.lock().unwrap().chunks.len() > 3);
    let rb = b.sync(&relay, &key, 4096, i64::MAX);
    assert_eq!(rb.downloaded_files, 1, "{rb:?}");
    let row = crate::db::repo::media::get(&b.db.lock().unwrap(), &pid).unwrap().unwrap();
    assert_eq!(std::fs::read(b.store.original(&row.hash, &row.ext)).unwrap(), bytes);
}

#[test]
fn upload_interrupted_mid_file_resumes_where_it_stopped() {
    let (relay, key, a, b) = pair();
    let (pid, bytes) = a.photo("resume.png", (900, 700));
    a.entry("Resume", "x", vec![pid.clone()]);
    let chunk = 4096;
    // push ops + meta (calls: devices, put_envelope, delivered checks...), then fail during chunks
    let ctx = Ctx { db: &a.db, direct: None, store: &a.store, relay: &relay, key: &key, device: a.device.clone(), chunk_size: chunk, quota_cap: i64::MAX };
    let total_chunks = (bytes.len() as i64 + chunk - 1) / chunk;
    assert!(total_chunks > 3, "test needs a multi-chunk file");
    ctx.sync_once().unwrap();
    // simulate: the relay lost the last chunk and the PC crashed before finishing
    {
        let mut s = relay.0.lock().unwrap();
        let hash = s.blobs.keys().next().unwrap().clone();
        s.chunks.remove(&(hash.clone(), total_chunks - 1));
    }
    {
        let c = a.db.lock().unwrap();
        c.execute("UPDATE sync_blobs SET state = 'active', chunks_done = ?1 WHERE direction = 'up'", [total_chunks - 1]).unwrap();
    }
    // B tries first: file incomplete → waits (nothing broken)
    let r_wait = b.sync(&relay, &key, chunk, i64::MAX);
    assert_eq!(r_wait.downloaded_files, 0);
    // A resumes from the missing chunk only
    let r_resume = ctx.sync_once().unwrap();
    assert_eq!(r_resume.uploaded_files, 1);
    let rb = b.sync(&relay, &key, chunk, i64::MAX);
    assert_eq!(rb.downloaded_files, 1, "{rb:?}");
    let row = crate::db::repo::media::get(&b.db.lock().unwrap(), &pid).unwrap().unwrap();
    assert_eq!(std::fs::read(b.store.original(&row.hash, &row.ext)).unwrap(), bytes, "byte-identical after resume");
}

#[test]
fn full_relay_queues_files_instead_of_failing() {
    let (relay, key, a, b) = pair();
    let (pid, _) = a.photo("p.png", (300, 300));
    a.entry("Space", "x", vec![pid]);
    let r = a.sync(&relay, &key, 1 << 20, 10); // a 10-byte cap: nothing fits
    assert!(r.waiting_for_space);
    assert_eq!(r.uploaded_files, 0);
    assert_eq!(counts(&a.db.lock().unwrap()).unwrap().uploads_pending, 1, "still queued, original safe on this PC");
    // the memory itself still reaches B; the photo shows as "on its way"
    b.s(&relay, &key);
    let e = b.titles();
    assert_eq!(e, vec!["Space"]);
    // space frees up → it goes through
    a.s(&relay, &key);
    assert_eq!(b.s(&relay, &key).downloaded_files, 1);
}

#[test]
fn nothing_is_sent_until_the_partner_device_exists() {
    let relay = MemoryRelay::default();
    let key = SpaceKey::generate();
    let a = Peer::new("Jo", "Sam");
    relay.0.lock().unwrap().devices.push(Device { id: a.device.clone(), user_id: "ua".into(), name: "A".into(), revoked: false, endpoint: None });
    a.entry("Early", "x", vec![]);
    let r = a.s(&relay, &key);
    assert!(r.waiting_for_partner_device);
    assert!(counts(&a.db.lock().unwrap()).unwrap().pending_ops > 0, "kept for when the partner joins");
}

fn from<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn live_count(p: &Peer, table: &str) -> i64 {
    let c = p.db.lock().unwrap();
    let has_deleted = c.prepare(&format!("SELECT deleted_at FROM {table} LIMIT 0")).is_ok();
    let sql = if has_deleted { format!("SELECT COUNT(*) FROM {table} WHERE deleted_at IS NULL") } else { format!("SELECT COUNT(*) FROM {table}") };
    c.query_row(&sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn every_kind_of_record_reaches_the_partner() {
    use crate::db::repo::{bucket, engagement, letters, milestones, planner, questions, special};
    use serde_json::json;
    let (relay, key, a, b) = pair();
    let me = a.me();
    let w = a.writer();
    let entry = a.entry("Picnic", "x", vec![]);
    {
        let mut c = a.db.lock().unwrap();
        bucket::create(&mut c, &w, &me, &from(json!({"title":"Northern lights","priority":"high","status":"want","targetDate":null,"coverMediaId":null}))).unwrap();
        special::create(&mut c, &w, &me, &from(json!({"title":"Sam's birthday","date":"1998-11-20","time":null,"recurrence":"yearly","remindDaysBefore":3,"linkedEntryId":null}))).unwrap();
        planner::create(&mut c, &w, &me, &from(json!({"title":"Pottery class","date":"2026-12-01","time":null,"budgetCents":8000,"currency":"USD","status":"planned","ideaRef":null,"checklist":[{"id":"c1","text":"Book","done":false}]}))).unwrap();
        planner::idea_create(&mut c, &w, &me, &from(json!({"title":"Our own idea","category":"at-home","budget":"free","duration":"short","setting":"indoor","mood":"cozy"}))).unwrap();
        milestones::create(&mut c, &w, &me, &from(json!({"title":"We met","date":"2023-05-14","coverMediaId":null,"linkedEntryId":null}))).unwrap();
        engagement::toggle_reaction(&mut c, &w, &entry, &me, "love").unwrap();
        engagement::add_comment(&mut c, &w, &entry, &me, "best day").unwrap();
        questions::answer_save(&mut c, &w, &me, "q-favorite-memory", "the beach", "2026-11-04").unwrap();
        questions::daily_ensure(&mut c, &w, "2026-11-04", "q-from-a").unwrap();
        let lid = letters::save_draft(&mut c, &me, None, &from(json!({"title":"Hi love","bodyJson":doc("You make every day lighter."),"kind":"letter","unlockAt":null}))).unwrap();
        letters::send(&mut c, &w, &me, &lid).unwrap();
        // a draft is private: it must NOT travel
        letters::save_draft(&mut c, &me, None, &from(json!({"title":"Secret draft","bodyJson":doc("not yet"),"kind":"letter","unlockAt":null}))).unwrap();
    }
    {
        // B picked a different daily question first? No — A picked first, B later: A's wins.
        let mut c = b.db.lock().unwrap();
        questions::daily_ensure(&mut c, &b.writer(), "2026-11-04", "q-from-b").unwrap();
    }
    for _ in 0..2 {
        a.s(&relay, &key);
        b.s(&relay, &key);
    }
    for t in ["journal_entries", "bucket_items", "special_dates", "planned_dates", "date_ideas", "milestones", "reactions", "comments", "answers"] {
        assert_eq!(live_count(&a, t), live_count(&b, t), "{t} differs");
        assert!(live_count(&b, t) >= 1, "{t} didn't arrive");
    }
    let c = b.db.lock().unwrap();
    let (title, recipient): (String, String) = c.query_row("SELECT title, recipient_id FROM letters WHERE status = 'sent'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(title, "Hi love");
    assert_eq!(Some(recipient), super::identity::my_user_id(&c).unwrap(), "the letter is for B");
    let drafts: i64 = c.query_row("SELECT COUNT(*) FROM letters WHERE title = 'Secret draft'", [], |r| r.get(0)).unwrap();
    assert_eq!(drafts, 0, "drafts stay private");
    let checklist: String = c.query_row("SELECT checklist_json FROM planned_dates", [], |r| r.get(0)).unwrap();
    assert!(checklist.contains("Book"));
    drop(c);
    let qa: String = a.db.lock().unwrap().query_row("SELECT question_id FROM daily_questions", [], |r| r.get(0)).unwrap();
    let qb: String = b.db.lock().unwrap().query_row("SELECT question_id FROM daily_questions", [], |r| r.get(0)).unwrap();
    assert_eq!(qa, qb, "both PCs settle on the same question of the day");
    assert_eq!(qa, "q-from-a", "the first pick wins");
}

/// Runs a direct-transfer server for a peer, reading its archive through a second
/// connection to the same database file (as the app's server thread would).
fn serve_for(p: &Peer, key: &SpaceKey, chunk: i64) -> String {
    let db: &'static Mutex<Connection> = Box::leak(Box::new(Mutex::new(crate::db::open(&p.dir.path().join("t.db")).unwrap())));
    let store: &'static Store = Box::leak(Box::new(p.store.clone()));
    let k = key.clone();
    let provider: super::direct::ChunkProvider = std::sync::Arc::new(move |h: &str, n: i64| super::engine::serve_chunk(db, store, &k, h, n, chunk));
    let port = super::direct::serve("127.0.0.1:0", key.clone(), provider).unwrap();
    format!("127.0.0.1:{port}")
}

#[test]
fn same_network_files_go_direct_and_never_touch_the_cloud() {
    let (relay, key, a, b) = pair();
    let (pid, bytes) = a.photo("direct.png", (700, 500));
    a.entry("Direct", "x", vec![pid.clone()]);
    let chunk = 4096;
    let a_ep = serve_for(&a, &key, chunk);
    let b_ep = serve_for(&b, &key, chunk);
    let to_b = super::direct::DirectClient::new(key.clone(), [(b.device.clone(), b_ep)].into_iter().collect());
    let to_a = super::direct::DirectClient::new(key.clone(), [(a.device.clone(), a_ep)].into_iter().collect());
    let ctx_a = Ctx { db: &a.db, direct: Some(&to_b), store: &a.store, relay: &relay, key: &key, device: a.device.clone(), chunk_size: chunk, quota_cap: i64::MAX };
    let ctx_b = Ctx { db: &b.db, direct: Some(&to_a), store: &b.store, relay: &relay, key: &key, device: b.device.clone(), chunk_size: chunk, quota_cap: i64::MAX };

    ctx_a.sync_once().unwrap();
    assert!(relay.0.lock().unwrap().chunks.is_empty(), "no file bytes uploaded to the cloud");
    let rb = ctx_b.sync_once().unwrap();
    assert_eq!(rb.downloaded_files, 1, "{rb:?}");
    let row = crate::db::repo::media::get(&b.db.lock().unwrap(), &pid).unwrap().unwrap();
    assert_eq!(std::fs::read(b.store.original(&row.hash, &row.ext)).unwrap(), bytes, "byte-identical");
    ctx_a.sync_once().unwrap(); // sees the confirmation, cleans the tiny record up
    ctx_b.sync_once().unwrap();
    assert!(relay_empty(&relay));
}

#[test]
fn if_the_partner_never_shows_up_directly_the_cloud_takes_over() {
    let (relay, key, a, b) = pair();
    let (pid, bytes) = a.photo("fallback.png", (300, 200));
    a.entry("Fallback", "x", vec![pid.clone()]);
    let b_ep = serve_for(&b, &key, 1 << 20);
    let to_b = super::direct::DirectClient::new(key.clone(), [(b.device.clone(), b_ep)].into_iter().collect());
    let ctx_a = Ctx { db: &a.db, direct: Some(&to_b), store: &a.store, relay: &relay, key: &key, device: a.device.clone(), chunk_size: 1 << 20, quota_cap: i64::MAX };
    ctx_a.sync_once().unwrap();
    assert!(relay.0.lock().unwrap().chunks.is_empty(), "waiting for a direct transfer first");
    // B is away from this network and the grace period passes
    a.db.lock().unwrap().execute("UPDATE sync_blobs SET direct_since = '2000-01-01T00:00:00Z'", []).unwrap();
    let ctx_a_cloud = Ctx { direct: None, ..ctx_a };
    ctx_a_cloud.sync_once().unwrap();
    assert!(!relay.0.lock().unwrap().chunks.is_empty(), "uploaded to the cloud after the grace period");
    assert_eq!(b.s(&relay, &key).downloaded_files, 1);
    let row = crate::db::repo::media::get(&b.db.lock().unwrap(), &pid).unwrap().unwrap();
    assert_eq!(std::fs::read(b.store.original(&row.hash, &row.ext)).unwrap(), bytes);
}

#[test]
fn when_the_cloud_allowance_is_used_up_files_still_go_over_the_home_network() {
    let (relay, key, a, b) = pair();
    let (pid, bytes) = a.photo("capped.png", (400, 300));
    a.entry("Capped", "x", vec![pid.clone()]);
    let chunk = 4096;
    let b_ep = serve_for(&b, &key, chunk);
    let a_ep = serve_for(&a, &key, chunk);
    let to_b = super::direct::DirectClient::new(key.clone(), [(b.device.clone(), b_ep)].into_iter().collect());
    let to_a = super::direct::DirectClient::new(key.clone(), [(a.device.clone(), a_ep)].into_iter().collect());
    // free tier, nothing left this month; the direct grace period is long over
    let ctx_a = Ctx { db: &a.db, direct: Some(&to_b), store: &a.store, relay: &relay, key: &key, device: a.device.clone(), chunk_size: chunk, quota_cap: 0 };
    let ctx_b = Ctx { db: &b.db, direct: Some(&to_a), store: &b.store, relay: &relay, key: &key, device: b.device.clone(), chunk_size: chunk, quota_cap: 0 };
    ctx_a.sync_once().unwrap();
    a.db.lock().unwrap().execute("UPDATE sync_blobs SET direct_since = '2000-01-01T00:00:00Z'", []).unwrap();
    let r = ctx_a.sync_once().unwrap();
    assert!(!r.waiting_for_space, "{r:?}");
    assert!(relay.0.lock().unwrap().chunks.is_empty(), "nothing uploaded to the cloud");
    assert_eq!(ctx_b.sync_once().unwrap().downloaded_files, 1);
    let row = crate::db::repo::media::get(&b.db.lock().unwrap(), &pid).unwrap().unwrap();
    assert_eq!(std::fs::read(b.store.original(&row.hash, &row.ext)).unwrap(), bytes);
    // away from home it waits (safely) instead
    let (pid2, _) = a.photo("later.png", (200, 100));
    a.entry("Later", "x", vec![pid2]);
    let away = Ctx { direct: None, ..ctx_a };
    away.sync_once().unwrap();
    a.db.lock().unwrap().execute("UPDATE sync_blobs SET direct_since = '2000-01-01T00:00:00Z' WHERE state = 'queued'", []).unwrap();
    assert!(away.sync_once().unwrap().waiting_for_space);
}

/// The real sync engine, two PCs, through a running Dear Us Cloud server (Store edition):
///   DEARUS_CLOUD_TEST_URL=http://127.0.0.1:8080 cargo test --no-default-features --features store cloud_engine -- --ignored
#[cfg(feature = "store")]
#[test]
#[ignore]
fn cloud_engine_memory_with_photo_travels_and_the_mailbox_empties() {
    use super::cloud::{redeem_invite, sign_up, CloudRelay};
    let url = std::env::var("DEARUS_CLOUD_TEST_URL").expect("DEARUS_CLOUD_TEST_URL");
    let a = Peer::new("Jo", "Sam");
    let b = Peer::new("Sam", "Jo");
    let key = SpaceKey::generate();
    let email = |n: &str| format!("{n}-{}@engine.dearus", uuid::Uuid::now_v7());
    let sa = sign_up(&url, "", &email("jo"), "password-jo").unwrap().unwrap();
    let sb = sign_up(&url, "", &email("sam"), "password-sam").unwrap().unwrap();
    let space = uuid::Uuid::now_v7().to_string();
    let ra = CloudRelay::new(&url, "", &space, sa, Box::new(|_| {}));
    ra.create_space().unwrap();
    ra.register_device(&a.device, "Jo's PC").unwrap();
    let secret = format!("engine-{space}");
    ra.create_invite(&super::crypto::sha256_hex(&secret)).unwrap();
    redeem_invite(&url, "", &sb, &secret).unwrap();
    let rb = CloudRelay::new(&url, "", &space, sb, Box::new(|_| {}));
    rb.register_device(&b.device, "Sam's PC").unwrap();

    let run = |p: &Peer, relay: &CloudRelay| {
        Ctx { db: &p.db, direct: None, store: &p.store, relay, key: &key, device: p.device.clone(), chunk_size: 64 * 1024, quota_cap: ra.plan().unwrap().engine_cap() }
            .sync_once()
            .unwrap()
    };
    let (pid, bytes) = a.photo("cloud.png", (900, 700));
    let id = a.entry("Through the cloud", "Sealed on Jo's PC", vec![pid.clone()]);
    let r1 = run(&a, &ra);
    assert!(r1.sent_ops > 0 && r1.uploaded_files == 1, "{r1:?}");
    let r2 = run(&b, &rb);
    assert!(r2.received_ops > 0 && r2.downloaded_files == 1, "{r2:?}");
    assert_eq!(b.get(&id).summary.title, "Through the cloud");
    let row = crate::db::repo::media::get(&b.db.lock().unwrap(), &pid).unwrap().unwrap();
    assert_eq!(std::fs::read(b.store.original(&row.hash, &row.ext)).unwrap(), bytes, "byte-identical");
    // confirmations travel back; both sides clean up
    run(&a, &ra);
    run(&b, &rb);
    run(&a, &ra);
    assert!(rb.fetch_envelopes(&b.device, 0, 100).unwrap().is_empty(), "Sam's mailbox emptied");
    assert!(ra.fetch_envelopes(&a.device, 0, 100).unwrap().is_empty(), "Jo's mailbox emptied");
    assert!(ra.blob_meta(&row.hash).unwrap().is_none(), "file removed from the cloud");
    assert_eq!(ra.plan().unwrap().tier, "trial");
}
