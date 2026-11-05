# Dear Us — Architecture & Build Plan

> Status: **v1.0.0 released** (2026-11-05). The live Supabase test was skipped at the owner's request; the couple's first pairing is the first real run. Releases are built by GitHub Actions with signed auto-updates; ffmpeg is a one-click download in Settings.
> Visual reference: `dear_us_relationship_app_mockup.html` (and the Cupid concept sheet).
> Last updated: 2026-11-03

---

## 0. Agreed decisions (summary)

| Topic | Decision |
|---|---|
| Platform | Windows desktop first (Tauri 2). Mobile out of scope for v1. |
| Sidebar | 9 items: Home, Journal, Timeline, Bucket List, Date Planner, Love Letters, Questions, Special Dates, Settings & Sync |
| Placement | On This Day → Home. Scrapbook → memory editor + Settings. Reactions/comments → Journal. Backups → Settings. |
| Themes | Paper, Rose, Lavender, Sage (from mockup) + Cozy Dark |
| Cloud relay | Supabase only (no Cloudflare, no credit card). Relay layer is swappable. |
| Accounts | **Bring-your-own.** No developer account, URL or key ships in the app. Each couple creates their own free Supabase project; each user adds their own Gemini key. The app works 100% offline/local with zero accounts. |
| Time capsules | Real lock via **drand tlock** (time-lock encryption, no server of ours). |
| AI | **Cupid** — optional floating companion (hooded design, **artwork made by the user**), Gemini via each user's own key. Off by default. |
| Scrapbook v1 | Themes + 4 automatic layouts + cover choice + a small set of frames. No free-form sticker canvas in v1. |
| Backup encryption | Not in v1 (lost-password risk). |
| Direct PC-to-PC | Deferred to Phase 6, optional. |

---

## 1. High-level architecture

```
┌──────────────────────── Windows PC (each partner) ────────────────────────┐
│                                                                            │
│  React UI (WebView2)                                                       │
│   ├─ screens, design system, Cupid character                              │
│   └─ typed API client  ──invoke()──┐                                       │
│                                     ▼                                      │
│  Rust core (Tauri 2)                                                       │
│   ├─ Repository layer ── every write = entity change + op-log row          │
│   │                       in ONE SQLite transaction                        │
│   ├─ SQLite (dear_us.db) ── permanent local archive                        │
│   ├─ Media store ── content-addressed files, thumbnails, previews          │
│   ├─ Sync engine ── HLC clock, outbox/inbox, merge, conflicts              │
│   │     └─ Relay trait ── SupabaseRelay (v1) | DirectRelay (Phase 6)       │
│   ├─ Crypto ── XChaCha20-Poly1305 space key, X25519 device keys, tlock     │
│   ├─ Backup ── backup folder mirror + single-file export, verify, restore  │
│   ├─ Cupid ── Gemini client (key never touches JS)                         │
│   └─ Secrets ── Windows Credential Manager                                 │
└───────────────────────────────┬────────────────────────────────────────────┘
                                │ HTTPS (only encrypted blobs + minimal metadata)
                       ┌────────▼─────────┐
                       │ Couple's OWN      │  Auth · Postgres (envelopes, acks,
                       │ Supabase project  │  devices) · Realtime · Storage
                       └──────────────────┘  (encrypted media chunks, temporary)
```

**Rules of the architecture**
1. The UI never talks to SQLite, the filesystem, Supabase or Gemini directly — only through Rust commands.
2. The local database is the source of truth. The cloud is a mailbox.
3. Every mutation is recorded as an operation, so sync, conflicts, undo and the recovery bin all share one mechanism.

---

## 2. Tech stack

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri 2 (Rust) | Native installer, small, filesystem access |
| UI | React 19 + TypeScript + Vite | Standard, fast |
| Styling | Tailwind CSS v4 + CSS variables for theme tokens | Themes = swapping tokens |
| UI state | Zustand | Light |
| Data fetching | TanStack Query over Tauri commands | Caching, invalidation on sync events |
| Rich text | TipTap (bold, italic, headings, lists only) | Stored as ProseMirror JSON |
| Animation | Motion (framer-motion), used sparingly | Cupid + subtle transitions |
| Icons | lucide-react | |
| Fonts | Bundled locally (offline): serif heading font, Inter, one handwritten accent font | Mockup uses Georgia/Inter; we bundle a refined serif |
| Database | SQLite via `rusqlite` (bundled) + versioned migrations | Transactions in Rust |
| IDs | UUIDv7 | Sortable, globally unique |
| Hashing | BLAKE3 | Fast integrity checks |
| Crypto | `chacha20poly1305`, `x25519-dalek`, `argon2` (none for v1 backups), tlock crate | |
| Media | `image` crate for JPEG/PNG/WebP; **bundled ffmpeg sidecar** for HEIC, video thumbnails and H.264 preview proxies | See §8 |
| Secrets | `keyring` crate → Windows Credential Manager | |
| Notifications / tray / autostart / updater | Tauri plugins | Updater via GitHub Releases (free) |

Toolchain on this PC: Node 24, Rust 1.98 ✔. Still needed: MSVC Build Tools (C++ workload) if missing — checked in Phase 1.

---

## 3. Repository structure

```
dear-us/
├─ docs/                     PLAN.md, SYNC.md, SECURITY.md (split later)
├─ supabase/
│  └─ setup.sql              the one script couples paste into their project
├─ src/                      React app
│  ├─ app/                   router, providers, layout (sidebar + topbar)
│  ├─ design/                tokens.css, themes/, typography
│  ├─ components/ui/         Button, Card, Modal, ConfirmDialog, EmptyState,
│  │                         Toast, Polaroid, PhotoGrid, MediaViewer, SyncBadge…
│  ├─ features/
│  │  ├─ home/  journal/  timeline/  bucket/  planner/
│  │  ├─ letters/  questions/  special-dates/  settings/
│  │  └─ onboarding/
│  ├─ cupid/                 CupidSvg, expressions, behaviors, menu, chat
│  └─ lib/api/               typed wrappers around invoke()
├─ src-tauri/
│  ├─ src/
│  │  ├─ commands/           one module per feature (thin)
│  │  ├─ db/                 connection, migrations/, repo/
│  │  ├─ domain/             entity structs + validation
│  │  ├─ media/              import, hash, thumbs, ffmpeg, protocol handler
│  │  ├─ sync/               hlc, oplog, merge, conflicts, engine, relay/
│  │  ├─ crypto/             space key, device keys, envelopes, tlock
│  │  ├─ backup/
│  │  ├─ cupid/
│  │  └─ secrets.rs
│  └─ binaries/              ffmpeg sidecar
└─ dear_us_relationship_app_mockup.html
```

---

## 4. Local storage layout

Default root: `%APPDATA%\DearUs\` (user can move the media library in onboarding/settings).

```
DearUs/
├─ dear_us.db (+ -wal)       SQLite, WAL mode
├─ media/
│  ├─ originals/ab/cd/<blake3>.<ext>    untouched originals, content-addressed
│  ├─ thumbs/<media_id>_400.webp
│  ├─ previews/<media_id>_1600.webp | <media_id>.mp4 (H.264 proxy if needed)
│  └─ incoming/              partial downloads (resumable), never shown in UI
├─ imports/                  temp during import, cleaned on success/startup
├─ backups/                  automatic local snapshots (DB only, rotating)
└─ logs/
```

- Media is referenced by `media_id`; the file path is derived from the hash, never stored as a hard-coded absolute path.
- Identical files are stored once (dedup by hash).
- Originals are **never** recompressed. Thumbs/previews are disposable and can be regenerated.
- Media served to the UI via a custom `media://` protocol with HTTP range support (needed for video seeking).

---

## 5. Data model (SQLite)

Common columns on every **shared** table:
`id TEXT PK (uuidv7)`, `created_at`, `updated_at`, `created_by (user_id)`, `deleted_at NULL`, `field_clock TEXT (JSON: field → HLC)`.

```
-- identity
users(id, display_name, avatar_media_id)
space(id, name, start_date, settings_json)          -- the relationship (one row)
devices(id, user_id, name, public_key, registered_at, revoked_at, is_self)

-- content
journal_entries(id, author_id, title, body_json, entry_date, entry_time,
                location, cover_media_id, layout, frame, …common)
tags(id, name) ; entry_tags(entry_id, tag_id)
media(id, hash, mime, kind[photo|video], size, width, height, duration_ms,
      taken_at, original_name, availability[local|pending|missing], …common)
entry_media(entry_id, media_id, position, caption)
reactions(id, entry_id, user_id, type)              -- unique(entry_id,user_id,type)
comments(id, entry_id, author_id, body, …common)

bucket_items(id, title, description, category, priority, status[want|planned|done],
             target_date, completed_at, linked_entry_id, …common)
bucket_media(item_id, media_id, position)

milestones(id, title, date, description, kind, cover_media_id, linked_entry_id, is_major, …)

letters(id, sender_id, recipient_id, title, body_json, status[draft|sent],
        sent_at, kind[letter|capsule], …common)
letter_media(letter_id, media_id)
capsules(letter_id PK, unlock_at, show_title, tlock_ciphertext BLOB,
         state[locked|unlocked], unlocked_at)

planned_dates(id, title, description, scheduled_at, location, budget_cents,
              currency, status[idea|planned|done|cancelled], linked_entry_id, …)
date_checklist(id, date_id, text, done, position)
date_ideas(id, title, description, category, budget[free|low|mid|high],
           duration, setting[indoor|outdoor|either], mood, source[library|custom|cupid])

questions(id, text, category, source[library|custom|cupid])
answers(id, question_id, user_id, body, answered_at, …common)
daily_questions(id = 'YYYY-MM-DD', question_id, picked_by_device, …)  -- deterministic id
moods(id, user_id, date, mood, note)   -- PRIVATE, never synced

special_dates(id, title, date, time, description, recurrence[none|yearly|monthly],
              remind_days_before, linked_entry_id, …common)
dismissed_on_this_day(entry_id, year)  -- local only

-- sync / ops
hlc_state(device_id, last_hlc)
oplog(op_id PK, device_id, hlc, entity, entity_id, kind[upsert|delete|restore],
      changes_json, base_json, state[pending|sent|acked], envelope_id)
inbox_applied(op_id PK, applied_at)               -- idempotency
conflicts(id, entity, entity_id, field, local_value, remote_value,
          remote_device, created_at, resolved_at, resolution)
blob_transfers(media_id, direction[up|down], chunk_count, chunks_done_json,
               remote_path, state, attempts, last_error)
sync_state(key, value)                            -- cursors, last_sync_at…

-- local-only
settings(key, value)          -- theme, cupid position, notifications…
backups(id, created_at, kind, path, size, manifest_hash, verified_at, status)
cupid_chats(id, role, content, created_at)        -- never synced
```

Shared vs local:
- **Synced:** everything under "content", plus `space`, `users`, `devices` (via relay).
- **Never synced:** settings, moods, cupid_chats, dismissed_on_this_day, drafts (`letters.status='draft'` stays local until sent), API keys.

Timeline is a **view**, not a table: `milestones UNION journal_entries` (optionally marked "major"), so there are no duplicate records.

---

## 6. Sync protocol

### 6.1 Clock and operations
- Each device keeps a **Hybrid Logical Clock** (HLC = wall time + counter + device_id) so ordering is safe even if PC clocks drift.
- Every local write runs in one transaction: update entity, bump `field_clock` for each changed field, insert an `oplog` row with `changes` (the new field values) and `base` (the field HLCs it was edited on top of).

### 6.2 Merge rules
| Situation | Rule |
|---|---|
| Different fields of the same record changed | Both kept (per-field merge) |
| Same **short** field (title, status, date, priority…) changed on both sides | Higher HLC wins; loser value kept in op history |
| Same **long text** field (journal body, letter body, comment, answer) changed on both sides concurrently | **Conflict**: both versions saved, record flagged, user picks/merges in a conflict screen |
| Delete vs concurrent edit | Record goes to recovery bin **with the edit applied**, and the user is notified. Nothing silently lost. |
| Same op received twice | Ignored (`inbox_applied`) |
| `daily_questions` same date picked by both | First picker wins (lowest HLC) — deterministic on both PCs |
| Reaction toggled | Unique by (entry, user, type): set semantics, safe |

"Concurrent" = the incoming op's `base` HLC for that field is older than the local field's HLC and the local change came from a different device.

### 6.3 Relay (Supabase) — envelopes and acks
1. Outbox batches pending ops (≤ 256 ops or 512 KB) → JSON → zstd → encrypt with space key → **envelope** row (`ciphertext bytea`).
2. Media: original file encrypted in **8 MB chunks** → Storage bucket `relay` at `space_id/blob_id/n`. The op that references the media is sent normally; media downloads separately.
3. Recipient device fetches envelopes newer than its cursor (Realtime push + poll every 5 min + on app start/focus).
4. Decrypt → verify → apply in one SQLite transaction → **then** insert an `ack(envelope_id, device_id)`.
5. Blobs: download chunks to `media/incoming/` → verify each chunk's AEAD tag + final BLAKE3 of the whole file → atomic move into `originals/` → fsync → ack the blob.
6. **Deletion:** after acking, the device checks whether every active non-revoked device in the space (except the sender) has acked. If yes, it deletes the envelope row / storage chunks **through the Storage API**. Any device also runs a cleanup sweep on start for fully-acked leftovers. RLS only allows deleting fully-acked items.
7. Never delete an un-acked payload. If a recipient device is revoked, it's removed from the "must ack" set.

### 6.4 Storage quota handling (1 GB free)
- The client tracks bytes it has in the relay. Soft cap **700 MB in flight** (configurable).
- If the cap is reached, uploads queue locally ("waiting for space — your partner's PC needs to pick up earlier files"). Originals stay safe on the sender's PC.
- Videos bigger than the cap are streamed: upload chunks, wait for acks/deletes, continue.

### 6.5 New or rebuilt device (initial sync)
The relay never holds full history, so a new device fills up from:
- **A. Partner's PC (snapshot):** when a device joins, an online device produces a snapshot (all entities as ops + all media) streamed through the relay under the quota cap. Progress shown. The partner's PC must be online during this.
- **B. A backup:** restore a backup first, then sync only the newer changes.

### 6.6 Sync states shown to users
Up to date · Syncing · Offline · Waiting for partner · Pending upload · Pending download · Waiting for cloud space · Paused · Error · Conflict.
Copy example: *"Your memories are saved safely on this computer. 3 updates are waiting to reach your partner."*

### 6.7 Supabase project paused (free tier inactivity)
Detected as a specific error → banner: "Your sync project is sleeping. Open Supabase → Restore project." Local use is unaffected.

---

## 7. Security & encryption

### 7.1 Keys
| Key | Where | Purpose |
|---|---|---|
| **Space key** (256-bit) | Credential Manager on each device | Encrypts all relay payloads (end-to-end). Supabase never sees it. |
| **Device key pair** (X25519) | Credential Manager; public key in `devices` | Re-sharing a new space key after a device is revoked |
| Supabase session | Credential Manager | Auth |
| Gemini key | Credential Manager, per user, never synced | Cupid |

### 7.2 Pairing
1. Creator makes the space → space key generated → shown once as a **recovery phrase (24 words)** with "I wrote it down" confirmation (and a printable card).
2. Creator generates an **invite code** (and QR): contains Supabase URL, anon key, space id, one-time invite secret, and the space key. Expires in 48 h and is single-use (enforced server-side via hashed secret).
3. Partner pastes it → creates their login in the couple's project → joins → device registered.
- Trade-off, stated honestly: whoever sees an unused invite code could join. The code must be shared privately and is useless once used or expired.

### 7.3 Server-side protection (in `setup.sql`)
- RLS on every table: a user can only see rows of a space they are a member of. Max 2 members per space.
- Storage bucket `relay` is private; policies restrict paths to `space_id/…` of members.
- No service-role key ever in the app.
- Supabase only ever sees: emails, device names, sizes, timestamps, and ciphertext.

### 7.4 Device revocation
Settings → Devices → Revoke: the device is marked revoked (can't auth to the space), a new space key is generated and wrapped to the remaining devices' public keys, and future payloads use the new key.
Honest limit: a revoked PC keeps whatever was already saved on it locally — we can't remote-wipe.

### 7.5 Data at rest
Local DB and media are **not** encrypted by the app in v1 (they're protected by the Windows account; we recommend BitLocker / Device Encryption). Exception: locked time capsules (below).

### 7.6 Time capsules (tlock)
- Sender writes → body + attachments encrypted with **drand tlock** to the round at `unlock_at` (quicknet chain).
- Recipient's PC stores only the tlock ciphertext. Nobody (not even the recipient poking at files) can decrypt before that time.
- At/after unlock time, the app fetches the drand beacon (needs internet once), decrypts, stores plaintext permanently.
- Sender keeps their own plaintext copy (they wrote it). **Safety net:** if drand ever disappeared, the sender can "re-send unlocked". UI shows a countdown and title (if sender allowed) + sender name.

### 7.7 Malicious/corrupt payloads
AEAD authentication rejects tampered data; decoded ops are schema-validated (entity type whitelist, field types, size limits) before touching the DB; media MIME is sniffed from content, not the filename.

---

## 8. Media pipeline

1. Import: copy to `imports/` → BLAKE3 → if hash exists, reuse → move to `originals/` → insert `media` row.
2. Metadata: dimensions, EXIF date (for "taken at"), video duration.
3. Thumbnails/previews:
   - JPEG/PNG/WebP/GIF → `image` crate.
   - **HEIC/HEIF (iPhone photos)** → ffmpeg sidecar → WebP preview (WebView2 can't show HEIC).
   - **Video** → ffmpeg poster frame; if codec isn't H.264/VP9/AV1-playable in WebView2 (e.g. iPhone HEVC .mov) → generate an H.264 MP4 **preview proxy** in the background. Original kept untouched.
4. Media not yet downloaded shows a placeholder: "Still on its way from your partner's PC".
5. ffmpeg sidecar: LGPL build, ~80 MB added to the installer. Validated in a Phase 2 spike.

---

## 9. Backup & recovery

Two complementary mechanisms:

**A. Backup folder (automatic, incremental)** — user picks a folder (external drive, or their own OneDrive/Google Drive folder):
```
DearUsBackup/
├─ manifest.json        format version, app/schema version, list of files + BLAKE3 + size
├─ db/2026-11-03T21-00.db   (last N snapshots via SQLite backup API)
└─ media/<hash>.<ext>   copied once, never re-copied (content-addressed)
```
Runs daily (configurable) + after big imports. Fast because media is only copied once.

**B. Export archive (manual)** — single `.dearus` file (zip, store mode) with the same layout + manifest; for moving PCs or offline storage.

**Verify:** after writing, every file is re-read and its hash checked; the backup is only marked "complete" after verification passes. "Verify local archive" does the same against the live library.

**Restore:** on a fresh install or into an existing one. If the current library has newer changes, restore goes into a staging DB and merges via the normal op/merge rules (no blind overwrite). Includes the space id; the space key comes from the recovery phrase.

**Recovery bin:** deletes set `deleted_at`; items stay restorable for 30 days (configurable), then purge. Deletions sync as tombstones; restore syncs too.

**Automatic local DB snapshots:** small rotating DB copies in `backups/` (last 7) to undo DB corruption even without a backup folder configured.

---

## 10. Cupid

**Look:** user-supplied artwork: a chibi heart with wings, a heart halo, gold laurel, a red scarf and a gold heart. Sprites (happy, thinking, sleeping) are in `src/cupid/assets/` (converted from `cupid_transparent_sprites.zip`). **Done in Phase 1:** floating/breathing animation, drag anywhere (position remembered), dozes off after 2 min idle, click menu (Add memory, Remind me, Plan a date, Ask Cupid shown as "soon"), hide/show in Settings.

**Art the app expects** (dropped into `src/cupid/assets/`):
- Format: transparent **PNG or WebP**, at least **512×512**, same canvas size and same character position in every file (so swapping files doesn't make Cupid jump).
- One file per **expression**: `happy`, `excited`, `thinking`, `curious`, `helping`, `in-love`, `sleeping`.
- One file per **pose** (optional, nice to have): `sit`, `perch`, `nudge`, `celebrate`.
- Optional for nicer motion: **wings and halo as separate layers** (`wings-left.png`, `wings-right.png`, `halo.png`) so the app can flap/bob them; or short animated loops as **WebM with transparency (VP9 alpha)**, which WebView2 plays.
- The app adds the motion in code: floating, bobbing, glow pulse, drag, and switching between expression files.
**Expressions (eyes only):** happy, excited, thinking, curious, helping, in love, sleeping.
**Behaviors:** idle/float, move around, sit/rest, perch on a card/window edge, nudge (reminder), celebrate (bucket item done, capsule unlocked).
**Presence:** draggable anywhere inside the app window, remembers position, can be minimized to a small heart or hidden.

**Click →** speech bubble + menu: Add Memory · Remind Me · Plan Date · Ask Cupid.

| Capability | Without key | With Gemini key |
|---|---|---|
| Character + quick actions | ✔ | ✔ |
| Daily question | Picked from local library (same on both PCs: hash of date + space id) | Cupid picks/writes it; shared through sync (`daily_questions` id = date, first picker wins) |
| Date ideas | Curated local library + filters | Cupid suggests using bucket list, budget, mood; "add to planner" in 1 click |
| Chat (Ask Cupid) | Hidden | ✔ local-only history |
| Letter help / captions / monthly recap | Hidden | ✔ (user sees exactly what is sent) |

**Privacy rules:** off by default; first-time screen explains that Gemini's free tier may use prompts to improve Google products; never sends time-capsule contents, partner's letters, photos/videos; never gives relationship scores or judgments; key stored in Credential Manager and the Gemini call happens in Rust.
**Model** is a setting (default: the current Gemini Flash alias) since Google renames models.

---

## 11. Settings & Sync screen map

- **Profile** — name, avatar
- **Our space** — relationship name, start date, partner
- **Look & feel** — theme, Cupid on/off/position, fonts size
- **Cloud sync (Supabase)** — setup wizard (create project guide → paste URL + anon key → copy `setup.sql` → test), status dashboard, Sync now, transfer queue, retry failed, cloud usage
- **Pairing & devices** — create/accept invite, recovery phrase, devices list, revoke
- **Cupid (Gemini)** — guide to get key, paste/test, model, privacy notice, toggles (daily question, date ideas)
- **Backups** — backup folder, schedule, export, restore, verify archive, recovery bin
- **Notifications** — reminders, start with Windows / tray
- **Conflicts** (appears only when there are some)

---

## 12. Screen & component map

| Screen | Key pieces |
|---|---|
| Home | Greeting + date · stats (only real numbers, else empty states) · recent memories · On This Day · next date · bucket preview · nearest countdown · quick actions · daily question card |
| Journal | Feed/gallery toggle · search · tag/date filters · entry view (layouts, media viewer, reactions, comments) · editor (TipTap, media drop, layout/cover/frame picker) |
| Timeline | Vertical timeline · year navigation · all vs major filter · add milestone |
| Bucket List | Want / Planned / Done columns or tabs · categories · progress ring · complete → link/create memory |
| Date Planner | Upcoming plans · checklist · budget · idea generator (filters, shuffle, add to planner) · done → create memory |
| Love Letters | Inbox / sent / drafts · letter editor · capsule creation · capsule countdown cards |
| Questions | Daily question · answer (each partner independent; both shown once both answered) · library & custom · history · gentle mood check-in (private) |
| Special Dates | Upcoming countdowns · time-since milestones · recurrence · reminders |
| Onboarding | Welcome → Create/Join → Profile → (optional) Sync setup → Pairing → Storage location → Initial sync → Home |

Reusable components: Sidebar, TopBar, Card, Polaroid, PhotoGrid, MediaViewer, VideoPlayer, Modal, ConfirmDialog, EmptyState, Toast, SyncBadge, CountdownCard, TimelineMarker, LetterCard, BucketCard, PlanCard, TagInput, DatePicker, RichTextEditor, CupidCharacter.

---

## 13. Phases & acceptance criteria

### Phase 1 — Desktop foundation  *(built by: Opus, sequential)* ✅ DONE
- Tauri 2 + React + TS + Tailwind scaffold; window icon, title; Windows build works.
- Design tokens + 5 themes, bundled fonts, sidebar (9 items), top bar, routing, remembered last page.
- Component library (§12) with empty states.
- SQLite + migrations framework + settings table; Credential Manager wrapper.
- Home dashboard layout using **real (empty) data**.
- ✅ Accept: app installs/launches offline; theme + last page persist across restarts; no fake stats anywhere; `cargo test` + `npm test` pass.

### Phase 2 — Core journal  *(Opus, sequential)* ✅ DONE
Notes from building it:
- ffmpeg is found via `DEARUS_FFMPEG`, then next to the app, then PATH. **Phase 7 must bundle it** (the partner's PC won't have it). Without it, originals are still saved, but HEIC/HEVC show "can't preview on this PC".
- HEVC video → H.264 conversion was verified end to end in the real app. **HEIC was not verified** (this ffmpeg build can't create a test HEIC); it needs a manual test with a real iPhone photo.
- Milestones are soft-deleted and purged after 30 days, but the recovery bin UI lists journal entries only.
- Profile/space changes don't write op-log entries yet; add them in Phase 4 with pairing.
- Repository layer with op-log written in the same transaction (sync-ready from day one).
- Journal CRUD, TipTap editor, tags, search (SQLite FTS5), date filters.
- Media import, hashing, dedup, thumbnails, `media://` protocol, viewer, video playback.
- **Spike:** ffmpeg sidecar for HEIC + HEVC preview proxies.
- Timeline view + milestones. Recovery bin for entries.
- ✅ Accept: create/edit/delete/restore entries with 5 photos + 1 video; restart → all there; originals byte-identical (hash check); HEIC photo and iPhone .mov display/play; search finds entries by title/body/tag.

### Phase 3 — Relationship features  *(parallel Sonnet agents via workflow if you say "use a workflow"; Opus reviews/integrates)* ✅ DONE
Notes from building it (Sonnet agents, one at a time; checked afterwards by Opus):
- Time capsules use real drand quicknet tlock (src-tauri/src/timelock.rs), and the live network round-trip test passed. Capsule ops carry only ciphertext.
- Desktop reminders (tauri-plugin-notification) are off by default and only fire while the app is open on the reminder day. They haven't been seen on a real device yet.
- There's no recovery-bin UI for bucket items, special dates, plans, custom ideas or comments: deleting shows an Undo toast, and items are purged after 30 days. **TODO (Phase 5):** one combined recovery bin.
- Scrapbook layouts, frames and paper haven't been visually checked with real photos.
- Everything from the partner (answers, letters, reactions, comments) waits for Phase 4 sync, and the UI says so.
- Bucket list · Love letters + drafts · Time capsules (tlock) · Date planner + idea generator (curated library ~150 ideas) · Questions (library ~200) + mood · Special dates + notifications · Reactions & comments · On This Day · Scrapbook layouts/frames · **Cupid character (user-supplied art, expressions, behaviors, menu — AI parts stubbed)**.
- ✅ Accept: every feature works offline with persistence; capsule can't be decrypted before unlock (tested with a 2-minute capsule); bucket item → linked memory round trip; On This Day shows nothing when there's nothing.

### Phase 4 — Sync  *(Opus, sequential, + independent review agent)* ✅ BUILT (live test pending)
Notes from building it:
- **Engine** (src-tauri/src/sync/): op envelopes (≤300 ops) are encrypted with XChaCha20-Poly1305 (the space key, with the envelope id as AAD). The relay deletes an envelope only once every non-revoked recipient device has acknowledged it, and a device acknowledges only after its local commit. Files move in 8 MB encrypted chunks, are resumable, are verified by BLAKE3 before joining the archive, and are kept under a 700 MB relay cap (otherwise they queue).
- **Merge** (sync/apply.rs): table-driven per-field LWW via field_clock, with field names whitelisted against PRAGMA table_info. Long text (journal body, descriptions, answers, comments) records the losing version in sync_conflicts when the losing write was concurrent (local op not yet acked). Delete is soft; purge only removes rows that are already deleted. Idempotent via sync_applied. If a media hash already exists locally, the incoming id is aliased.
- **Identity**: "user" and "space" ops. The joiner adopts the creator's space id, and the local "partner" placeholder is re-pointed to the real user id across all user columns. Letters written before pairing get their recipient re-addressed on arrival.
- **Daily question**: first pick (lowest HLC) wins on both PCs.
- **Tests**: 12 two-PC scenarios in sync/tests.rs (MemoryRelay): photo A→B byte-identical with the relay emptied afterwards; both directions; field merge; concurrent text (both versions kept); delete vs edit; duplicate apply; foreign/damaged envelope never acked; multi-chunk; resume after a lost chunk; full relay queues; no partner device yet; every record type, including drafts staying private.
- **Supabase side**: supabase/setup.sql (RLS, RPCs, private 'relay' storage bucket). The app talks to it over plain REST (ureq). Polling runs every 45 s, plus Sync now. **Not yet run against a real Supabase project.**
- **Not done**: key rotation when a device is revoked (revoking only stops sending to that device and blocks it from posting); Realtime push; restoring the key from the recovery phrase (the function exists; the UI comes in Phase 5); initial sync for a *rebuilt* device (rebuild from backup in Phase 5).
- `setup.sql`, Supabase setup wizard, auth, space creation, invite/pairing, recovery phrase, devices.
- HLC, outbox/inbox, envelopes, encryption, chunked blobs, acks, safe deletion, quota cap, Realtime + polling, conflict detection + UI, sync dashboard, initial snapshot sync, revocation + key rotation.
- Cupid AI: Gemini client, daily question sync, date ideas, chat.
- ✅ Accept (two installs with separate data dirs on two PCs, or two profiles on one PC): A→B and B→A entries + media; offline edits on both → reconnect → consistent; concurrent body edit → conflict shown, both versions kept; delete-vs-edit → nothing lost; kill app mid-video-upload → resumes; relay emptied after both acks; payload never deleted before ack; duplicate envelope delivery → no duplicates; full relay → queued, no loss.

### Phase 5 — Backup & recovery  *(Opus)* ✅ DONE
Notes from building it:
- src-tauri/src/backup/mod.rs uses a folder format (DearUsBackup/: manifest.json, db snapshots via VACUUM INTO (last 10), content-addressed media copied once, thumbs). A backup counts as complete only after every file is re-read and hash-checked. Restore is a **merge**: INSERT OR IGNORE per table over the common columns, so it never overwrites or deletes, and older backups work. Damaged backup files are never imported, and their records are marked 'missing'. The search index is rebuilt and thumbs/previews regenerated afterwards.
- A daily automatic backup to the chosen folder (on by default once a folder is picked), and daily local DB snapshots in <data>/backups (last 7), from a scheduler thread.
- Verify backup and verify this PC's archive (PRAGMA integrity_check plus BLAKE3 of every original).
- One recovery bin (db/repo/bin.rs) for memories, milestones, bucket items, special dates, plans, custom ideas/questions, my answers, comments and letters. Restore and purge are op-logged.
- The welcome screen offers "Restore from a backup" for a new PC.
- Verified in the real app: backup on one install, restore on a fresh install, with the photo, video poster and data back and a repeat restore adding nothing.
- **Not done:** a single-file export archive (the folder is portable instead); restoring the space key from the recovery phrase (re-pairing via an invite covers the normal case); backup encryption (deliberately left out of v1).
- Backup folder, scheduled backups, export archive, verify, restore (incl. merge into newer data), device migration flow, corruption handling, local DB snapshots.
- ✅ Accept: back up → wipe app data → reinstall → restore → everything identical (hashes); corrupt one media file → verify reports it; restore older backup while newer changes exist → no newer data lost.

### Phase 6 — Direct PC-to-PC (optional) ✅ DONE (LAN)
Notes from building it:
- src-tauri/src/sync/direct.rs: a tiny HTTP server (std TcpListener, no new deps) on 0.0.0.0, a random port, and only while paired with the feature on. It serves only sealed chunks of files in sync_blobs 'up' that aren't delivered yet. Every request carries a BLAKE3 keyed MAC (key derived from the space key), bound to the path and a ±3-minute window. Chunks use the same sealing and AAD as cloud chunks.
- Each device publishes its LAN endpoint in devices.lan_endpoint (setup.sql adds the column). The sender stores only the small blob record in the cloud and waits up to 15 minutes (DIRECT_GRACE_MINUTES) for a direct fetch, then uploads normally. The receiver tries the direct path first for every chunk, then the cloud.
- Change envelopes always go through the cloud (tiny, and the reliable path).
- Tests: a same-network transfer moves zero file bytes through the cloud and arrives byte-identical; the grace-period fallback works; requests with the wrong key or expired tokens are refused.
- **Not done:** transfers between different networks (that would need NAT traversal, e.g. iroh). Windows shows a one-time firewall prompt the first time the server starts.
- Evaluate iroh (free relays, NAT traversal) behind the same Relay trait; LAN first.
- ✅ Accept: same Phase 4 test suite passes with direct transport on and off.

### Phase 7 — Polish & release ✅ DONE (v0.9.0)
Notes from building it:
- Cupid's brain (it had slipped from Phase 4): src-tauri/src/cupid + commands/cupid.rs. Gemini generateContent goes over REST with the person's own key (Credential Manager, never sent to the UI). It covers chat (local cupid_chats table, migration 11; excluded from backups and sync), the daily question (picks from a 10-question shortlist of the library and records it through daily_ensure, so the first pick wins on both PCs; falls back silently) and date ideas (sanitized against allowed values, saved as custom ideas). Off until a privacy acknowledgement and a key are given. It hasn't made a real Gemini call yet, because there's no key on this PC.
- **ffmpeg not bundled** (the full build is 445 MB and the user's disk is nearly full). Instead, Settings → Photo & video converter detects it and offers a copyable `winget install --id Gyan.FFmpeg -e`. Revisit if the couple wants a single installer.
- An ErrorBoundary around pages; journal paging ("Show more", 120 at a time); version 0.9.0; installer metadata.
- docs/USER_GUIDE.md written for the couple.
- Version stays below 1.0 until the live Supabase test passes.
- Accessibility (keyboard nav, focus states, contrast), performance (10k entries, 50 GB media), error copy, NSIS installer, auto-updater, user guide (incl. Supabase + Gemini setup with screenshots), known limitations.
- ✅ Accept: clean install on a second Windows PC from the installer only, following the guide.

---

## 14. Testing strategy
- **Rust unit tests:** HLC, merge rules (table-driven, every row of §6.2), envelope encrypt/decrypt, chunking/resume, backup manifest verification.
- **Property tests:** random op sequences applied in different orders on two simulated replicas → identical final state (or recorded conflicts).
- **Integration:** two app instances with different data dirs (`DEARUS_DATA_DIR`) against a **dev Supabase project you create** (or local Supabase via Docker) — never a project baked into the app.
- **Fault injection:** kill process mid-transaction / mid-upload / mid-download; drop acks; duplicate envelopes; corrupt chunks.
- **UI:** Vitest + Testing Library for components; manual QA checklist per phase.

---

## 15. Risks & spikes (resolve early)

| Risk | Plan |
|---|---|
| HEIC/HEVC support | Phase 2 spike with ffmpeg sidecar; fallback: Windows HEIF extension via WIC |
| tlock Rust crate maturity | Phase 3 spike; fallback: tlock-js inside the webview, or honest UI-only lock |
| Supabase free-tier limits change / project pauses | Quota-aware queue, clear "project sleeping" banner, swappable relay |
| Supabase email confirmation on sign-up | Setup guide covers it (confirm via email, or disable confirm for the private project) |
| Gemini free tier region availability / data use | Opt-in, clear notice, AI fully optional |
| Unsigned installer (SmartScreen) | Documented "More info → Run anyway" |
| Lost space key | Recovery phrase at creation; local data never depends on it |
| drand long-term availability | Sender keeps plaintext + "re-send unlocked" |

---

## 16. What you'll need to provide/do (not me)
- For development testing: a **dev Supabase project in your own account**, or Docker Desktop for a local Supabase. (The shipped app still contains no account.)
- Optional: your own Gemini key for testing Cupid.
- Install MSVC Build Tools if Phase 1 finds them missing.
