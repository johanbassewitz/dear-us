# Dear Us — Microsoft Store launch checklist

Free download from the Microsoft Store; Premium and Cupid arrows are paid through **Paddle**
(checkout in the browser). Microsoft Store purchases are not used.

Do the parts in order. ⭐ = send the result to Claude. 🔒 = secret: never paste it in chat; it
goes straight into the server's `.env` file. Menu names in Microsoft/Cloudflare/Google screens
can differ slightly from what's written here.

---

## Part 1 — Partner Center (free, ~20 min)

**1.1 Reserve the app name**
1. partner.microsoft.com/dashboard → **Apps and games** → **+ New product** → **MSIX or PWA app**.
2. Name: `Dear Us` → **Check availability** → **Reserve product name**.
   If it's taken, try `Dear Us – Couple Journal`. ⭐ Tell Claude the name you got.

**1.2 Copy the app's identity** ⭐
Your app → **Product management → Product identity**. Send Claude these 4 lines (not secret):
- Package/Identity/Name
- Package/Identity/Publisher (starts with `CN=`)
- Package/Properties/PublisherDisplayName
- Store ID (starts with `9N`)

**1.3 Payouts:** nothing to do in Partner Center (the app is free there; money comes through
Paddle, Part 6.4).

✅ Done: name "Dear Us", identity values, Store ID 9PP0J9L5P20C, owner Anthony Chaaya ("axiant.").

---

## Part 2 — Domain + email on Cloudflare (~$12/year, ~15 min)

**2.1 Buy the domain**
1. Create an account at dash.cloudflare.com.
2. **Domain Registration → Register Domains** → search `axianthq.com` → buy it (card needed).
   ⭐ If you buy a different name, tell Claude.

**2.2 Support email (free forwarding)**
1. Cloudflare → **axianthq.com → Email → Email Routing** → **Enable**.
2. **Routing rules → Create address**: `support` → destination: your personal email → Save.
3. Open the verification email Cloudflare sends to your inbox and confirm.
   Mail to dearus@axianthq.com now reaches you. Reply from your own email.

---

## Part 3 — GitHub (~5 min)

1. Tell Claude **"open the PR"** → you get a link → **Merge pull request** → **Confirm merge**.
   (The build workflow and the website only work from the `main` branch.)
2. Claude then fills in your name, Store ID and identity values and pushes them; merge that too.

---

## Part 4 — Website (free, ~10 min)

1. Cloudflare → **Workers & Pages → Create → Import a repository** → GitHub → `dear-us`.
2. Project → **Settings → Build**: Root directory **`site`** · Build command: *(empty)* ·
   Deploy command: `npx wrangler deploy` · branch `main`.
3. **Settings → Domains & Routes → Add → Custom domain** → `dearus.axianthq.com`.
4. Check: `https://dearus.axianthq.com` shows the Dear Us page (not the app), and
   `https://dearus.axianthq.com/privacy` opens.

---

## Part 5 — The server (~€5/month, ~30 min)

**5.1 Rent it**
1. Sign up at hetzner.com/cloud (they may ask for ID verification; can take a few hours).
2. Cloud console → **New project** "Dear Us" → **Add server**:
   - Location: Falkenstein, Nuremberg or Helsinki
   - Image: **Ubuntu 24.04**
   - Type: **CX22** (Shared vCPU, x86)
   - Networking: keep **Public IPv4** on
   - SSH key: optional; without one, Hetzner emails you the root password
   - Name: `dearus-1` → **Create & Buy now**
3. Copy the server's **IPv4 address**.

**5.2 Point the domain at it**
Cloudflare → axianthq.com → **DNS → Records → Add record**:
Type **A** · Name `dearus-api` · IPv4 = the server's address · Proxy status: **DNS only (grey cloud)** → Save.

**5.3 Install Dear Us Cloud** (from your Windows PC, in PowerShell)
1. GitHub → the repository → **Code → Download ZIP** → extract it (e.g. to `Downloads\dear-us-main`).
2. Copy the server folder up (replace `IP`):
   ```powershell
   scp -r $HOME\Downloads\dear-us-main\cloud root@IP:/opt/dearus
   ```
3. Log in and run the setup:
   ```powershell
   ssh root@IP
   ```
   ```bash
   cd /opt/dearus && bash deploy/setup-vps.sh
   ```
   It asks for your API domain: type `dearus-api.axianthq.com`. It creates the settings file
   `/opt/dearus/.env` with fresh secrets and stops. Leave the window open for Part 6.

---

## Part 6 — Service keys (free tiers + Gemini pay-as-you-go, ~45 min)

Open the settings file on the server with `nano /opt/dearus/.env` and fill in each 🔒 value as
you get it (save: Ctrl+O, Enter; exit: Ctrl+X).

**6.1 Cloudflare R2 (photo storage in transit)**
1. Cloudflare → **R2 Object Storage** → add a payment method if asked (free up to 10 GB).
2. **Create bucket** → name `dearus-relay` → Create.
3. The bucket → **Settings → Object lifecycle rules → Add rule** → delete objects **30 days** after upload → Save.
4. R2 → **Manage API tokens → Create API token** → permission **Object Read & Write**, only bucket
   `dearus-relay` → Create.
5. 🔒 Into `.env`: `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, and `R2_ENDPOINT` =
   `https://<your account id>.r2.cloudflarestorage.com` (shown on the same page).

**6.2 Google Gemini (Cupid)**
1. aistudio.google.com → **Get API key → Create API key** (in a new Google Cloud project).
2. Turn on billing for that project (AI Studio shows **Set up billing**; add your card). This
   makes it the paid tier, where Google doesn't use the messages to improve its products.
3. Google Cloud console → **Billing → Budgets & alerts** → budget $20/month with email alerts.
4. 🔒 Into `.env`: `GEMINI_API_KEY`.

**6.3 Resend (password-reset emails)**
1. Sign up at resend.com → **Domains → Add domain** → `axianthq.com`.
2. Add the DNS records it shows in Cloudflare DNS (Resend can do it for you with "Sign in to
   Cloudflare"). Wait until the domain shows **Verified**.
3. **API Keys → Create API key** (Sending access).
4. 🔒 Into `.env`: `RESEND_API_KEY`. (`MAIL_FROM` is already set to `Dear Us <dearus-noreply@axianthq.com>`.)

**6.4 Paddle (payments)** — do it first in the **sandbox** (sandbox-vendors.paddle.com, a
separate sign-up, fake money), then repeat the same steps in the live account (vendors.paddle.com).
1. Sign up at paddle.com as a seller (individual/sole trader), complete verification, and add your
   payout method (bank wire to your IBAN; Payoneer only if Paddle can't wire to Lebanon).
   Paddle reviews your website: it must be live first (Part 4) — it has prices, terms, privacy and refunds.
2. **Catalog → Products → New product**:
   - "Dear Us Premium" → add 2 prices: **$3.99 / month** (recurring, monthly) and **$29.99 / year**
     (recurring, yearly), no trial.
   - "100 Cupid arrows" → 1 price: **$1.99**, one-time.
   - (optional) "300 Cupid arrows" → **$3.99**, one-time.
   Copy each **price ID** (`pri_…`).
3. **Checkout → Checkout settings → Default payment link**: `https://dearus.axianthq.com/checkout`.
   Also **Website approval**: add `dearus.axianthq.com` if Paddle asks.
4. **Developer tools → Authentication**:
   - **API key** (server) with write permissions for transactions and customer portal sessions.
   - **Client-side token** (starts `test_` in sandbox, `live_` in live) ⭐ send it to Claude: it's
     public, it goes on the checkout page.
5. **Developer tools → Notifications → New destination**:
   URL `https://dearus-api.axianthq.com/v1/billing/paddle/webhook`, type webhook, events:
   `subscription.created`, `subscription.updated`, `subscription.activated`, `subscription.canceled`,
   `subscription.past_due`, `subscription.paused`, `subscription.resumed`, `transaction.completed`.
   Save, then copy its **secret key**.
6. 🔒 Into `.env`: `PADDLE_ENV=sandbox` (later `production`), `PADDLE_API_KEY`,
   `PADDLE_WEBHOOK_SECRET`, `PADDLE_PRICE_PREMIUM_MONTHLY`, `PADDLE_PRICE_PREMIUM_YEARLY`,
   `PADDLE_PRICE_ARROWS_100`, `PADDLE_PRICE_ARROWS_300` (the price IDs aren't secret, but they
   live in the same file).

**6.5 Start the server**
On the server:
```bash
cd /opt/dearus && bash deploy/setup-vps.sh
```
It ends with **"Dear Us cloud is up ♡"**. Check `https://dearus-api.axianthq.com/v1/health` in a
browser. If it fails, copy the last lines of `docker compose logs api` to Claude (they contain no keys).

---

## Part 7 — Test build on your PCs (~1 hour + fixes)

1. GitHub → **Actions → Microsoft Store package → Run workflow** → keep **test build** ticked → Run.
   Wait ~20–30 min for a green tick. (If red, tell Claude: Claude can read the log.)
2. Open the run → **Artifacts → DearUs-test-build** → download and unzip.
3. On each test PC, once:
   double-click `DearUs-test-certificate.cer` → **Install Certificate** → **Local Machine** →
   **Place all certificates in the following store** → **Trusted People** → Finish.
4. Double-click the `.msix` → **Install**.
5. Test on **two PCs** (you and your partner, or two Windows user accounts), and note anything odd:
   - [ ] create account on PC 1 → create space → write down the recovery phrase → invite code
   - [ ] PC 2: Join my partner → second account → plan shows **Free trial, 30 days**
   - [ ] a memory with photos and a video arrives on the other PC
   - [ ] a love letter, and a time capsule set for tomorrow
   - [ ] Cupid chat (arrow count goes down), date ideas, Report button
   - [ ] Disconnect from my partner, then invite again
   - [ ] Settings → Backups → OneDrive (or Google Drive) → Back up now
   - [ ] Forgot your password → code by email → new password
   - [ ] Link my phone or another PC → on a third PC / Windows user: I already use Dear Us on another device
   - [ ] iPhone HEIC photo and a .mov video show previews
6. ⭐ Send Claude what went wrong (screenshots help). Claude fixes; repeat 1–5 until clean.

---

## Part 8 — Submit to the Store (~1 hour, then 1–3 days of review)

1. GitHub → Actions → **Run workflow** with **test build unticked** → download **DearUs-msix-for-store**.
2. Partner Center → your app → **Start your submission**:
   - **Pricing and availability:** Free · markets: all (or your choice) ·
     **Visibility: Private audience** → create an audience with your and your partner's Microsoft account emails.
   - **Properties:** Category **Lifestyle** · Privacy policy `https://dearus.axianthq.com/privacy.html` ·
     Website `https://dearus.axianthq.com/` · Support `dearus@axianthq.com` ·
     "accesses personal information": **Yes** · Additional license terms: `https://dearus.axianthq.com/terms.html`.
   - **Age ratings:** answer as in `docs/STORE_LISTING.md`.
   - **Packages:** upload the `.msix`.
   - **Store listings → English:** paste everything from `docs/STORE_LISTING.md`; upload 8
     screenshots from the test build (with made-up sample memories) and the logo.
   - **Submission options → Notes for certification:** paste the notes from `docs/STORE_LISTING.md`.
3. **Submit to the Store.** Microsoft reviews it in about 1–3 days. If it's rejected, send Claude
   the report: Claude fixes it.

---

## Part 9 — Test payments (sandbox: fake money)

With the server on `PADDLE_ENV=sandbox` and the sandbox client token on the checkout page:
1. Settings → Your plan → **Get Premium** → monthly → the checkout opens in your browser.
2. Pay with Paddle's test card `4242 4242 4242 4242`, any future date, any CVC.
   - [ ] back in Dear Us, **Premium shows on both PCs** within seconds
   - [ ] **Manage subscription** opens Paddle's page → cancel → at the end of the period it's back to free
   - [ ] **100 Cupid arrows** → +100; buy again → +100 again
3. ⭐ Send Claude the results. Then switch to **live**: in `.env` put the live API key, webhook
   secret, price IDs and `PADDLE_ENV=production`, run `bash deploy/setup-vps.sh` again, and send
   Claude the live client token (for the checkout page). Do one real $3.99 purchase and refund it
   from the Paddle dashboard.

## Part 10 — Go public 🎉

Partner Center → your app → **Update** → **Pricing and availability → Visibility: Public** → submit.

---

## After launch (10 minutes a week)

- On the server: `bash /opt/dearus/deploy/ai-reports.sh` → read Cupid reports → `… ai-reports.sh done`.
- Database backups are automatic (`/var/backups/dearus`, 14 days). Copy one off the server monthly.
- Paddle → **Payouts**: Paddle pays out monthly (once over its minimum) to your bank or Payoneer.
- Google Cloud billing alert emails: check Gemini costs stay small.

## Money summary

| | Cost |
|---|---|
| Domain axianthq.com | ~$12/year |
| Hetzner CX22 | ~€5/month |
| Gemini | cents per day at first |
| Cloudflare R2, Pages, Email Routing, Resend, GitHub | free |
| Microsoft Store | free (the app is free; payments don't go through Microsoft) |
| Paddle | 5% + $0.50 per sale |
