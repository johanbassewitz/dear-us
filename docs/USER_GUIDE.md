# Dear Us — User guide

Dear Us is a private home for your memories, made for two. Everything is saved permanently
**on your own PC**. Syncing with your partner and Cupid's AI are optional extras that use
**your own free accounts**. The app has no accounts built in.

---

## 1. Install

1. Download the installer: **[DearUs-Setup.exe](https://github.com/johanbassewitz/dear-us/releases/latest/download/DearUs-Setup.exe)**
   (it's always the newest version), then open it. It installs for you only, so no admin password is needed.
2. Windows may say *"Windows protected your PC"* because the app isn't signed by a big
   company. Click **More info → Run anyway**.
3. Open **Dear Us** from the Start menu or your desktop.

On the first start, choose one:
- **Create our space**: you're the first of you two.
- **Join my partner**: your partner already uses Dear Us and sent you an invite code.
- **Restore from a backup**: you're moving to a new PC (see §5).

---

## 2. Everyday use

| Where | What |
|---|---|
| **Journal** | Memories with photos, videos, tags and places. Drag photos in, or click *Choose files*. Search everything with **Ctrl+K**. |
| **Timeline** | Your story by year and month. Add milestones like "the day we met". |
| **Bucket list** | Dreams → plans → done together. Finishing one lets you write the memory. |
| **Date planner** | Plans with checklists and budgets, and 119 ideas with a *Surprise us* shuffle. |
| **Love letters** | Letters and **time capsules** that truly can't be opened before their date. |
| **Questions** | A gentle daily question, a weekly check-in, and a private mood check-in. |
| **Special dates** | Countdowns for birthdays, anniversaries and trips. |

**Deleted something by mistake?** Settings → **Recovery bin**. Everything waits there for 30 days.

**Cupid** floats in the corner. Drag him anywhere, or hide him in Settings → Look & feel.

---

## 3. Sync with your partner (optional)

Memories travel **encrypted** through a free Supabase project that **you** create. Only your
two PCs hold the key, so Supabase can't read anything. Files are deleted from the cloud as soon
as both PCs have them.

**The first of you** (one time, about 10 minutes):
1. Create a free account and project at **supabase.com**.
2. In Dear Us: **Settings → Partner sync**, then follow the 4 steps:
   - paste the project's **URL** and **anon / public key** (never the *service_role* key);
   - **Copy setup script**, then paste it in Supabase → **SQL Editor** → **Run**;
   - **Check connection**;
   - create your sign-in (email + password).
     Tip: in Supabase → Authentication → Sign In / Providers → Email you can switch off
     *Confirm email* for your private project.
3. Click **Create our shared space**. **Write the 24-word recovery phrase on paper.**
4. Click **Create invite code** and give it to your partner privately (in person is best).
   It works once and expires after 48 hours.

**Your partner**: install Dear Us, choose **Join my partner**, paste the code, and pick an
email and password. Everything you've saved so far arrives automatically.

**Good to know**
- Sync runs every ~45 seconds while the app is open, plus **Sync now** in Settings.
- **Offline?** Keep going. Everything is saved and syncs later.
- **Both edited the same text while apart?** Nothing is lost: Settings shows both versions so
  you can choose.
- **Same Wi-Fi?** Photos and videos go straight between your PCs (Settings →
  *Direct PC-to-PC transfers*). Windows may ask once to allow it on private networks.
- **"Your Supabase project is paused"**: free projects sleep after a quiet week. Open
  supabase.com and click *Restore*. Your memories are safe on your PCs meanwhile.
- **Cloud full?** Big files wait safely on the sender's PC until space frees up.

---

## 4. Cupid's brain (optional)

Cupid can chat, choose the daily question, and suggest date ideas, using **your own free Gemini key**.

1. Settings → **Cupid's brain** → read the notice → *I understand*.
2. Get a free key at **aistudio.google.com → Get API key** (with your own Google account).
3. Paste it, then click **Save & test key**. The key stays on this PC (Windows Credential Manager).

**Privacy:** on Gemini's free tier, Google may use what you send to improve its products. Cupid
only sees what you type to him, a shortlist of questions, or your date-idea filters (and open
bucket-list titles, only if you allow it). Never your journal, letters, capsules or photos.

---

## 5. Backups & moving to a new PC

**Set up backups once:** Settings → **Backups & recovery** → **Choose folder**. Pick an external
drive, or your own OneDrive or Google Drive folder. Dear Us then backs up **every day** while
it's open: photos are copied once, and every backup is checked file by file.

- **Verify backup** checks the backup; **Verify this PC's archive** checks your own files.
- Dear Us also keeps a small daily safety copy of its database (last 7 days).

**New PC:** install Dear Us, choose **Restore from a backup**, and pick the `DearUsBackup`
folder. Restoring only **adds** what's missing; it never overwrites newer memories. To reconnect
sync afterwards, ask your partner for a new invite code.

---

## 6. iPhone photos & videos (HEIC / HEVC)

These need the free converter **ffmpeg** on each PC. Go to Settings → **Photo & video converter**
→ **Download converter**. It's about 110 MB, takes a minute and needs no restart.
Photos and videos you added before get their previews afterwards.

Without it, those files are still saved in full quality. They just can't be previewed yet.

---

## 7. Updates

Dear Us checks for a new version every time it opens. When there is one, a small card appears
in the bottom-right corner: click **Update now** and the app restarts on the new version a
minute later. Your memories, settings and sync are never touched by an update.
You can also check yourself: Settings → **Updates** → **Check for updates**.

---

## 8. Where things live

- Memories, photos and videos: `%APPDATA%\app.dearus.journal\`
  (Settings → Your archive → **Open**)
- Sign-ins, the space key and the Gemini key: Windows Credential Manager (never in files or backups)

## 9. Known limitations (v1.0)

- Sync has been tested between two simulated PCs, but this is its first release on a real
  Supabase project. **Make a backup before pairing** (Settings → Backups), and check your
  first shared memory arrives on both PCs.
- Direct transfers only work on the same network; otherwise files go through the cloud.
- Desktop reminders only appear while Dear Us is open.
- Removing a lost device stops new updates reaching it, but can't wipe what's already on it.
