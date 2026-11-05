# Dear Us — Microsoft Store release guide

> Saved: 2026-11-05 · Edition: **Store** (`--no-default-features --features store`)
> The **personal edition** (your own Supabase + Gemini key, GitHub auto-updates) is untouched and
> keeps building from `.github/workflows/release.yml` exactly as before.

## 0. The two editions at a glance

| | Personal (default build) | Microsoft Store |
|---|---|---|
| Sync relay | couple's own Supabase | **Dear Us Cloud** (`cloud/`, our server) |
| Accounts | Supabase sign-in in their project | Dear Us account (email + password) |
| Cupid | own Gemini key | our Gemini key via the cloud, **arrows** per couple |
| Plans | none | 30-day trial → Premium or capped Free |
| Updates | `tauri-plugin-updater` (GitHub) | **the Store** (updater not compiled in) |
| ffmpeg | one-click download (GPL build) | **LGPL build inside the package**, never downloaded |
| App id / data folder | `app.dearus.journal` | `app.dearus.store` (separate, no mixing) |
| Invite codes | work only with personal | work only with Store |

## 1. One-time setup

### 1.1 Partner Center (Microsoft Store account)
1. Register as an app developer at <https://partner.microsoft.com/dashboard> (individual or company).
   Choose **Lebanon** as your country; you'll add a Lebanese bank account that receives **USD wires**
   (open a *fresh dollars* account at your bank). Payout threshold: USD 50.
   Then check that bank-transfer payouts are offered for Lebanon in **Payout and tax profiles**
   (Microsoft's payout table lists Lebanon, but verify it for your account).
2. Complete the **tax profile** (W-8BEN for an individual outside the US).
3. **Reserve the name** "Dear Us" (Apps and games → New product → MSIX or PWA app).
4. Product identity: copy *Package/Identity/Name*, *Package/Identity/Publisher* (`CN=…`) and
   *Publisher display name* into `packaging/msix/identity.json`
   (or into the repo variables `MS_IDENTITY_NAME`, `MS_PUBLISHER`, `MS_PUBLISHER_DISPLAY_NAME`).

### 1.2 Add-ons (in-app products) — Partner Center → your app → Add-ons
| Product id (yours) | Type | Price tier | Server env var |
|---|---|---|---|
| `premium_monthly` | **Subscription**, 1 month | $3.99 | `MS_PRODUCT_PREMIUM_MONTHLY` = its **Store ID** (9N…) |
| `premium_yearly` | **Subscription**, 1 year | $29.99 | `MS_PRODUCT_PREMIUM_YEARLY` |
| `arrows_100` | **Developer-managed consumable** | $1.99 | `MS_PRODUCT_ARROWS_100` |
| `arrows_300` (optional) | Developer-managed consumable | $3.99 | `MS_PRODUCT_ARROWS_300` |

Optionally give the subscriptions a free trial of 0 days: our own 30-day trial starts at pairing.

### 1.3 Azure AD app for purchase checks
1. Azure portal → Microsoft Entra ID → App registrations → New (single tenant). Note **Tenant ID**
   and **Client ID**; create a **client secret**.
2. Partner Center → Account settings → User management → Azure AD applications → **Add** this app
   and associate it with Dear Us (Product management → Product identity → "Associate Azure AD").
3. Put `MS_TENANT_ID`, `MS_CLIENT_ID`, `MS_CLIENT_SECRET` and the four Store IDs in the server's `.env`.

### 1.4 The cloud server
Follow `cloud/README.md` → *Deploy*: a ~5 EUR VPS, `api.dearus.app` DNS, R2 bucket with a 30-day
lifecycle rule, `GEMINI_API_KEY` (Google AI Studio, **paid** tier, so prompts aren't used for training),
`docker compose up -d --build`, then check `https://api.dearus.app/v1/health`.

> The app has the server address built in at build time (`DEARUS_CLOUD_URL`, default
> `https://api.dearus.app`). If you use another domain, set the repo variable `DEARUS_CLOUD_URL`.

## 2. Build the package
- **CI (recommended):** Actions → *Microsoft Store package* → Run workflow → download the
  `DearUs-msix` artifact.
- **Locally on Windows** (Windows SDK installed):
  ```powershell
  ./packaging/msix/build-msix.ps1 -CloudUrl https://api.dearus.app
  ```

The script builds the Store edition, bundles LGPL ffmpeg (refuses GPL builds), adds tiles and
licence notices and runs `makeappx`. **Before the first submission, pin ffmpeg** in
`packaging/msix/ffmpeg-lgpl.json` (a dated BtbN `win64-lgpl-shared` URL + its SHA-256; run once with
`-AllowUnpinnedFfmpeg` to print the hash).

To install a local test build, sign it with a self-signed certificate whose subject equals the
publisher (`-CertPfx`, `-CertPassword`) and trust that certificate on the test PC.

## 3. Test before going public
Store purchases only work for an app installed **from the Store**, so:
1. Submit the first build with **Visibility → Private audience** (only the Microsoft accounts you
   list can see it) and wait for certification.
2. Create **promo codes** for the add-ons (Partner Center → Add-ons → Promo codes) so test
   purchases cost nothing, and install the app from the Store link on two PCs.
3. Check, end to end, on two PCs:
   - [ ] create account → create space → invite → partner joins → **trial** shows 30 days
   - [ ] a memory with photos syncs both ways; the cloud empties (server `select count(*) from envelopes`)
   - [ ] Cupid chat works; arrows go down by one; an error doesn't cost an arrow
   - [ ] buy **Premium monthly** → plan shows Premium on **both** PCs
   - [ ] buy **100 arrows** → +100; buying again works (it was fulfilled)
   - [ ] **Restore purchases** on a second PC signed in with the same Microsoft account
   - [ ] set `trial_ends_at` in the past on the server → Free: 10 arrows, 100 MB/month; photos over Wi-Fi still sync
   - [ ] HEIC photo and an iPhone .mov show previews (ffmpeg from the package, H.264 via `h264_mf`)
   - [ ] no update prompt ever appears
4. Run the **Windows App Certification Kit** on the .msix.

When everything passes, switch visibility to public. **Verify during this private test, because it couldn't be tested here:** the Microsoft Store B2B
endpoints and response fields in `cloud/src/billing/microsoft.rs` (recurrence query, collections
query/consume), and that `RequestPurchaseAsync` shows its dialog when called from a background
thread (`src-tauri/src/billing.rs`; if not, start it on the main thread).

## 4. Store listing & certification notes
- **Category:** Lifestyle. **Age rating:** questionnaire (no user-to-user public content; private two-person sharing).
- **Privacy policy URL:** required (account email, encrypted relay, Gemini processing, no ads/tracking).
- **Restricted capability `runFullTrust`:** justify as "Desktop app (Tauri/WebView2) packaged as MSIX".
- **Notes for certification:** give testers a demo account pair (two emails) with an existing
  paired space, and explain that sync needs both accounts.
- **Pricing:** the app is **free**; revenue comes from the add-ons.
- **Policy 10.8.x (in-app purchases):** all digital purchases use Store commerce. Done.
- **Policy 10.2.x (no downloaded code):** the Store edition never downloads ffmpeg or updates.

## 5. Not built yet (before or right after launch)
- **Password reset / email verification** (needs an email provider) — see `cloud/README.md`.
- **Automated cloud snapshots** (listed as a Premium perk in the commercial plan): breaks the
  "the cloud keeps nothing" property and is the one cost that grows with users; decide
  size/retention first. Today Premium = unlimited cloud media + 100 arrows.
- **Free-tier limits on 4K media and creating time capsules** (in the commercial plan's table):
  not enforced; only the cloud-media and Cupid limits are.
- Localized prices in the plan card (the Store dialog already shows the real price).
