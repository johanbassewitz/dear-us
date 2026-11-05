# Dear Us

A private, local-first relationship journal for two. Windows desktop app built with Tauri 2, React and SQLite.
The full architecture and roadmap are in [`docs/PLAN.md`](docs/PLAN.md).

## Install

**[⬇ Download Dear Us for Windows](https://github.com/johanbassewitz/dear-us/releases/latest/download/DearUs-Setup.exe)** (always the newest version)

1. Open `DearUs-Setup.exe`. If Windows says *"Windows protected your PC"*, click **More info → Run anyway** (the app isn't signed by a big company).
2. Follow the welcome screen: **Create our space**, or **Join my partner** with an invite code.
3. Optional: connect sync and Cupid with your own free accounts. See the **[user guide](docs/USER_GUIDE.md)**.

Updates install themselves: the app checks GitHub on start and shows **Update now** when there's a new version.

## Status

**v1.0: all 7 phases built.** Journal with photos/videos, timeline, bucket list, date planner, letters & real time capsules, questions, special dates, reactions/comments, scrapbook styles, Cupid (with optional Gemini brain), encrypted partner sync through the couple's own Supabase, direct LAN transfers, verified backups/restore with a recovery bin, one-click ffmpeg download, and signed auto-updates.

**Not yet verified live:** sync against a real Supabase project. The engine passes its two-PC tests through an in-memory relay; first real use is the couple's own pairing.

The architecture and per-phase notes are in [docs/PLAN.md](docs/PLAN.md).

## Releasing a new version (maintainer)

1. Bump the version in `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`.
2. Commit, then `git tag v1.0.1 && git push origin main v1.0.1`.
3. GitHub Actions (`.github/workflows/release.yml`) builds the installer, signs the update and publishes the release. Installed apps pick it up automatically.

The update signing key lives in `~/.tauri/dear-us.key` (plus `dear-us.key.password`) and in the repo's Actions secrets. **Back it up:** without it, installed apps can't receive updates.

## Run it

Prerequisites: Node 20+, Rust (stable, MSVC), Visual Studio Build Tools (C++), WebView2 (included with Windows 11).

```bash
npm install
npm run tauri dev       # desktop app with hot reload
npm run tauri build     # installer → src-tauri/target/release/bundle/nsis/
```

`npm run dev` alone opens a **browser preview** with an in-memory stand-in backend (nothing is saved). It's for design work only.
Add `?space` to the URL to skip the welcome screen, plus `&theme=cozy-dark` or `&page=settings` if you like.

## Test

```bash
npm test                         # frontend (Vitest)
npx tsc -b                       # typecheck
cd src-tauri && cargo test       # Rust core (DB, migrations, settings, Credential Manager)
```

## Where data lives

- Database and (later) media: `%APPDATA%\app.dearus.journal\`
- Set `DEARUS_DATA_DIR` to use another folder. This lets two instances run side by side for sync testing.
- Secrets (later: sync session, keys, Gemini key) go to Windows Credential Manager, never into files.

## No built-in accounts

Dear Us ships with **no** cloud project, URL or API key. Each couple connects their own free Supabase project, and each person can add their own Gemini key for Cupid, both in Settings. Without any of that, the app works fully offline.
