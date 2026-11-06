# Dear Us Cloud

The server behind the Microsoft Store edition of Dear Us: accounts, the encrypted ephemeral
mailbox, plans and quotas, the Cupid (Gemini) gateway and Microsoft Store purchase checks.

**It can't read anyone's memories.** Everything in the mailbox is sealed on the couple's PCs with
their space key (XChaCha20-Poly1305), which never leaves those PCs — invite codes carry it from
PC to PC directly. Items are deleted as soon as both PCs have them; leftovers after 30 days are
swept (the sender always keeps the original).

## What it stores

| Table | Contents |
|---|---|
| `users` | email, Argon2id password hash |
| `spaces`, `members`, `devices` | who is paired with whom, device names, LAN address for Wi-Fi transfers |
| `envelopes`, `blobs` (+ R2 chunks) | **ciphertext only**, until delivered |
| plan columns on `spaces`, `subscriptions`, `purchases` | trial end, Premium end, monthly counters, arrow top-ups |

## Plans (decided here, never by the app)

| | Solo (not paired) | Trial (30 days from pairing) | Premium | Free (after trial) |
|---|---|---|---|---|
| Text sync | unlimited | unlimited | unlimited | unlimited |
| Cloud media | 100 MB / month | unlimited | unlimited | 100 MB / month |
| Wi-Fi media | unlimited | unlimited | unlimited | unlimited |
| Cupid arrows | 10 / month | 100 / month | 100 / month | 10 / month |
| Time capsules sealed at once | 3 | unlimited | unlimited | 3 |

Top-up arrows never expire. A Gemini failure gives the arrow back.

## Run locally

```bash
docker run -d --name dearus-pg -e POSTGRES_PASSWORD=pw -p 5432:5432 postgres:16-alpine
DATABASE_URL=postgres://postgres:pw@localhost:5432/postgres \
PUBLIC_URL=http://localhost:8080 SERVER_SECRET=$(openssl rand -hex 32) \
cargo run
```

Without R2 settings, chunks are stored in `./data/chunks` with signed links (fine for dev).
Point a Store-edition build at it with `DEARUS_CLOUD_URL=http://localhost:8080` at build time.

## Tests

```bash
TEST_DATABASE_URL=postgres://postgres:pw@localhost:5432/postgres cargo test
```

## Deploy (Stage 1: one VPS, ~5 EUR/month)

1. A Hetzner CX22 (or similar) with Docker. Point `api.dearus.app` (A record) at it.
2. Create the R2 bucket `dearus-relay` and an API token with Object Read & Write on it.
   Add a lifecycle rule: delete objects 30 days after upload (a safety net for the sweeper).
3. `cp .env.example .env`, fill it in, then `docker compose up -d --build`.
4. Check `https://api.dearus.app/v1/health`.
5. Back up Postgres daily (`pg_dump` cron to R2, or the provider's snapshots). The mailbox is
   temporary, but accounts, pairings and purchases are not.

**Stage 2:** create a managed Postgres, `pg_dump | pg_restore` into it, set `DATABASE_URL`,
delete the `db` service. **Scaling the API:** it's stateless except for the sign-in brake
(in memory, per instance), so you can run several behind a load balancer.

## Accounts on several devices

One account works on any number of devices at once (PC, a second PC, later the phone). A new
device is linked from an existing one with a 15-minute code that only the same account can
use; the existing device then sends it one encrypted snapshot (free, not counted against the
photo allowance), and photos follow as normal files.

## Password reset

`POST /v1/auth/forgot` emails a 6-digit code (30 minutes, 5 tries); `POST /v1/auth/reset` sets
the new password and signs out every other session. Memories are unaffected: they're locked
with the couple's space key, not the password. Set `RESEND_API_KEY` and `MAIL_FROM`.

## Before launch

- Verify the Microsoft Store B2B endpoints against a sandbox purchase (`src/billing/microsoft.rs`).
