# Dear Us — App Store & Google Play plan

> Saved: 2026-11-05 · Status: **plan only** (Microsoft Store edition ships first)

## 1. What carries over unchanged
- **The cloud** (`cloud/`): accounts, the encrypted mailbox, plans and Cupid arrows are already
  per couple and store-agnostic. A phone is just another device in the space.
- **The sync engine, crypto and SQLite archive** (`src-tauri/src/sync`, `db`): plain Rust, compiles
  for iOS/Android. The local-first rule stays: the phone keeps its own full copy.
- **The React UI**: Tauri 2 mobile renders the same frontend in WKWebView / Android WebView.

## 2. Work needed per area

| Area | Desktop today | Mobile change |
|---|---|---|
| Shell | Tauri 2 desktop | `tauri ios init` / `tauri android init` (Tauri 2 supports both); new `src-tauri/gen/{apple,android}` projects |
| Secrets (space key, refresh token) | `keyring` → Windows Credential Manager | iOS **Keychain** / Android **Keystore**: a small Tauri plugin (or `tauri-plugin-stronghold`) behind `secrets.rs` |
| Layout | 1320×860 window, sidebar | responsive layout: bottom tab bar, single-column journal, touch-sized controls (the biggest frontend job) |
| Photos | file picker + `image` crate | system photo picker (PHPicker / Android Photo Picker) via plugin; HEIC decodes natively on iOS |
| Video previews | ffmpeg sidecar | **no ffmpeg on mobile**: phones play their own H.264/HEVC; for the partner's desktop-only formats show the original or a poster |
| Background sync | thread every 45 s while open | iOS: sync on open + `BGAppRefreshTask` (minutes-to-hours, not guaranteed); Android: `WorkManager` periodic job. Push "something new" via APNs/FCM sent by the cloud (contains no content) |
| Wi-Fi direct sync | LAN HTTP server on the PC | iOS can't keep a server running in the background → phones use the cloud; desktop↔desktop Wi-Fi stays. On the capped free tier, phone media counts toward the 100 MB/month |
| Notifications / reminders | Tauri notification plugin | same plugin (mobile supported); scheduled reminders need the OS scheduler |
| Time capsules (drand tlock) | `tlock_age` + HTTP | same crate; check binary size |
| Updates | Store / GitHub | App Store / Play handle it |
| Cupid | cloud gateway | identical (already server-side) |

## 3. Purchases: one plan for both partners, everywhere
- **iOS:** StoreKit 2 subscriptions + consumable arrows (Small Business Program: 15%).
- **Android:** Google Play Billing (15% on subscriptions; 15% on the first $1M with the reduced fee).
- **Unify with RevenueCat** (Apple + Google) → its webhook calls a new `cloud/src/billing/revenuecat.rs`
  that updates the same `spaces.premium_until` / `cupid_bonus` as Microsoft Store purchases.
  RevenueCat doesn't cover the Microsoft Store, which is why Windows is verified directly.
- Buying on any platform → Premium for the couple on every device (the server is the source of truth).
- **Payouts in Lebanon:** confirm at sign-up that Apple (App Store Connect → Agreements, Tax, and
  Banking) and Google (Play Console → Payments profile) pay out to a Lebanese bank account in USD.
  If either doesn't, the usual fallback is registering a company where payouts are supported.

## 4. Store requirements to plan for
- **Apple:** a Mac with Xcode for builds (or a CI service with macOS runners), Apple Developer
  Program ($99/yr), Sign in with Apple **only if** we add other social logins (email+password is
  fine), privacy nutrition labels, account deletion inside the app (also a Google requirement)
  → add `DELETE /v1/account` on the server first.
- **Google:** Play Console ($25 once), Data safety form, target the current API level, closed
  testing with 12+ testers for 14 days before production (new personal accounts).

## 5. Suggested order (after the Windows launch is stable)
1. **Server prep** (1 week): account deletion endpoint, push notifications (APNs/FCM), RevenueCat webhook.
2. **Android first** (3–4 weeks): easier to iterate, no Mac needed; responsive layout, Keystore
   secrets, photo picker, WorkManager sync, Play Billing via RevenueCat.
3. **iOS** (2–3 weeks on top): Keychain, BGAppRefresh, StoreKit via RevenueCat, App Review.
4. **macOS** (optional, small): the desktop build mostly works already; Mac App Store needs sandboxing.

## 6. Risks
- Background sync on iOS is opportunistic: set expectations ("opens and syncs in a second").
- Large videos over mobile data on the capped tier: respect a "Wi-Fi only for big files" setting.
- WebView performance for long journals on low-end Android: virtualize lists.
