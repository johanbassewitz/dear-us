# Dear Us — Commercialization & Monetization Plan

> Saved: 2026-11-05
> Status: Planning & Go-To-Market Blueprint

---

## 1. Executive Summary

Dear Us transitions from an open-source/hobby desktop app into a cross-platform consumer subscription product sold across the **Microsoft Store**, **Apple App Store**, **Google Play Store**, and **Direct Web**.

- **Pricing Model**: Hybrid Trial-to-Freemium Funnel:
  * **Day 1–30**: Full All-Access Free Trial upon pairing.
  * **Day 31+ (Subscribed)**: Premium Couple Tier ($3.99/mo or $29.99/yr for both partners).
  * **Day 31+ (Unsubscribed)**: Graceful fallback to Capped Free Tier (50 memories, 100 MB media, 10 Cupid AI quota).
- **Core Principle**: One subscription covers **both partners across all devices**. Nobody is ever locked out of their saved memories.

---

## 2. The Core Onboarding & Retention Funnel

```
[ Download App (Solo Free) ]
             │
             ▼
[ Pair with Partner ]
             │
             ▼
✨ 30-Day All-Access Free Trial
   • Unlimited Cloud Sync
   • 100 Cupid AI Credits
   • Full 4K Photos & Video Sync
   • Time-Lock Capsules
             │
      Day 31 Milestone
             ├──▶ [ Subscribed ] ──▶ 🌟 Dear Us Premium ($3.99/mo or $29.99/yr)
             │                        • Unlimited Cloud Sync & 4K Media
             │                        • 100 Cupid Chats/month
             │                        • Automated Cloud Snapshots & Backup
             │
             └──▶ [ Not Yet ] ─────▶ 🍃 Graceful Capped Free Tier
                                      • All existing memories safe forever
                                      • Cloud Sync active up to 50 memories & 100 MB
                                      • Unlimited Local Wi-Fi Sync (free at home)
                                      • 10 Cupid Chats/month
                                      • Upgrade anytime banner
```

---

## 3. Product Tier Breakdown

### Phase 1: 30-Day All-Access Free Trial (Upon Pairing)
* Activated immediately when the couple pairs their devices.
* **Full Cloud Sync**: Instant sync anywhere in the world.
* **Unlimited entries**: Write memories, scrapbook layouts, date plans, bucket list.
* **Full Cupid AI Quota**: 100 Cupid interactions to try companion chat and date ideas.
* **Full Media**: Upload and sync photos and videos without quality caps.
* **Time Capsules**: Create and seal drand time-locked letters.

---

### Phase 2: Post-Trial Paths (Day 31+)

| Feature | Capped Free Tier (Unsubscribed) | Premium Tier ($3.99/mo or $29.99/yr) |
|---|---|---|
| **Coverage** | Both partners (1 shared space) | **Both partners across all devices** |
| **Existing Memories** | **100% preserved forever** (never deleted) | 100% preserved |
| **Cloud Sync Cap** | **Up to 50 memories & 100 MB media** | **Unlimited cloud sync** |
| **Home Wi-Fi Sync** | **Unlimited peer-to-peer local sync** | Unlimited peer-to-peer local sync |
| **Cupid AI Companion** | **10 interactions / month** | **100 interactions / month** |
| **Cupid Arrow Top-ups** | $1.99 for 100 arrows | $1.99 for 100 arrows (with subscriber discounts) |
| **Photos & Videos** | Standard resolution (within 100 MB cap) | **Full 4K resolution + video streaming** |
| **Time Capsules** | View existing unlocked capsules | **Create new cryptographic time capsules** |
| **Cloud Backups** | Manual local file backup | **Automated daily cloud snapshots** |

---

## 4. Why This Hybrid Strategy Wins

1. **Zero Friction Onboarding**: Couples experience the full magic of the app for an entire month without seeing paywalls or credit card prompts.
2. **Emotional Investment**: By Day 30, a couple typically has dozens of intimate memories, dates planned, and mutual answers stored.
3. **No Hostage Taking (Trust & Goodwill)**: If a couple chooses not to subscribe immediately, **their memories are never held hostage or deleted**. They simply drop into the 50-memory/100MB tier. If they already exceeded 50 memories during the trial, their existing memories remain fully readable, and they can continue syncing over local Wi-Fi.
4. **Natural Upgrade Triggers**: Adding their 51st memory, uploading high-res trip photos, or asking Cupid for a date idea triggers a warm, friendly prompt to join Premium.

---

## 5. Cupid AI Economy ("Cupid's Quiver")

* **Provider**: Google Gemini 1.5 / 2.5 Flash (Paid Tier API with data privacy guarantees).
* **Cost to you**: ~$0.0001 per turn (negligible).
* **Quotas**:
  * Free Trial: 100 interactions.
  * Post-Trial Free Tier: 10 interactions / month.
  * Premium: 100 interactions / month.
* **Consumable Arrow Packs (In-App Purchase)**:
  * 100 extra Cupid arrows: **$1.99**
  * 300 extra Cupid arrows: **$3.99**
  * Margin: **~99% profit** on top-ups.

---

## 6. Store Distribution & Tech Stack

| Platform | Channel | Fee | Billing Integration |
|---|---|---|---|
| **Windows** | Microsoft Store | 15% | Microsoft Store IAP / MSIX package |
| **iOS / Mac** | Apple App Store | 15% (Small Business) | Apple StoreKit (via RevenueCat) |
| **Android** | Google Play Store | 15% | Google Play Billing (via RevenueCat) |
| **Web** | `dearus.app` direct | ~5% | Stripe / Lemon Squeezy (via RevenueCat) |

### Technical Roadmap
1. **Unified Backend**: Migrate from user-supplied Supabase keys to your own managed Supabase/PostgreSQL instance.
2. **Space Quota System**: Each couple's space record tracks `trial_ends_at`, `subscription_status`, `memory_count`, `media_bytes_used`, and `cupid_monthly_turns`.
3. **Multi-Store IAP**: Integrate **RevenueCat** so purchasing Premium on an iPhone immediately upgrades the partner on Windows.

---

## 7. Implementation decisions (2026-11-05)

| Topic | Decision |
|---|---|
| Launch platform | **Microsoft Store first.** Apple/Google later: see `docs/MOBILE_PLAN.md`. |
| Payments | **Microsoft Store in-app purchases** (15%). Microsoft lists Lebanon for bank-transfer payouts in USD and is the merchant of record (handles VAT/tax). Stripe and PayPal can't pay out to Lebanon; Paddle (wire/Payoneer) is the fallback for web sales later. |
| Packaging | MSIX (Store purchases need package identity) built by `packaging/msix/build-msix.ps1`. |
| Backend | **Dear Us Cloud** (`cloud/`): one Rust service + Postgres on a small VPS, media chunks in Cloudflare R2. Stage 2 = managed Postgres, no code change. |
| Free-tier limits | **Text/memories sync without limit.** Cloud **media: 100 MB per calendar month** (counted when a file actually goes through the cloud; Wi-Fi transfers are free and unlimited). The "50 memories" cap was dropped: the server can't count memories (end-to-end encrypted) and text costs nothing to relay. Wi-Fi sync for text may come later. |
| Cupid | Our paid Gemini key behind the cloud. Arrows per couple: trial/premium 100/month, free/solo 10/month, top-ups never expire, failures refunded, the daily question pick is free. |
| Trial | 30 days, starts when the partner joins (pairing). |
| Existing users | The personal edition (own Supabase + Gemini key, GitHub updates) stays exactly as it is, as the default build. |
| Not yet built | Password reset (email provider), automated cloud snapshots, free-tier limits on 4K media / creating capsules. |

Release steps: `docs/STORE_RELEASE.md`. Server: `cloud/README.md`.
