# Dear Us — Microsoft Store listing (ready to paste)

Partner Center → your app → **Store listings → English**. Character limits in brackets.

## Product name
Dear Us

## Short description [≤ 270]
A private journal for two. Keep your memories, love letters, date plans and time capsules on your own PCs, shared only with your partner and encrypted end to end.

## Description [≤ 10,000]
Dear Us is a private little home for the two of you.

Write memories with photos and videos, send love letters, seal time capsules that open on a day you choose, plan dates, keep a bucket list, answer a question together every day and never miss a special date. Everything is saved on your own PCs, works offline, and is shared only with your partner.

PRIVATE BY DESIGN
• Encrypted end to end: your memories are locked with a key that exists only on your devices. Not even we can read them.
• The cloud is only a mailbox: updates wait there just until your partner's PC has them, then they're deleted.
• Daily backups to your own OneDrive, Google Drive, iCloud Drive or Dropbox, encrypted too.
• No ads. No tracking. No public profiles. Just you two.

MADE FOR TWO
• Journal and timeline with photos, videos and scrapbook layouts
• Love letters, and time capsules sealed until their moment
• Date planner with ideas, checklists and budgets
• Shared bucket list and special dates with reminders
• A daily question for both of you
• Cupid, a small companion for date ideas and sweet words (he never sees your journal)
• Use it on more than one PC with the same account

NEVER HELD HOSTAGE
Start with 30 days of everything, from the moment your partner joins. Then choose Dear Us Premium for both of you, or stay free: your memories are never deleted or locked, they keep syncing, photos travel freely over your home Wi-Fi (plus 100 MB a month through the cloud), and Cupid has 10 arrows a month.

Dear Us Premium covers both partners on all their devices:
• Unlimited cloud sync for photos and videos, anywhere
• 100 Cupid arrows every month
• Unlimited sealed time capsules

## What's new in this version [≤ 1,500]
First release on the Microsoft Store.

## Product features [up to 20, ≤ 200 each]
1. Private journal for couples with photos and videos
2. End-to-end encrypted sync between your PCs
3. Love letters and time capsules that open on a chosen day
4. Date planner, bucket list and special-date reminders
5. A daily question to answer together
6. Encrypted daily backups to OneDrive, Google Drive, iCloud Drive or Dropbox
7. Works offline; everything is saved on your own PC
8. Cupid, a gentle companion for date ideas
9. Five themes, including a cozy dark mode
10. One subscription covers both partners

## Search terms [up to 7, ≤ 30 each, ≤ 21 words total]
couple journal · relationship · love letters · private diary · date planner · time capsule · memories

## Screenshots (1366×768 or larger, up to 10)
Take them from the real app with sample memories (never real private ones):
1. Home with "On this day" and the daily question
2. Journal entry with a photo scrapbook layout
3. Timeline
4. Love letters with a sealed time capsule
5. Date planner with an idea and checklist
6. Settings → Your plan + Partner sync (shows encryption)
7. Cupid chat
8. Cozy Dark theme

## Store logos
Partner Center → Store listings → Store logos: upload `src-tauri/icons/Square310x310Logo.scale-200.png`
(620×620) as the 1:1 logo; the 2:3 poster is optional.

## URLs
- Privacy policy: `https://<your-site>/privacy.html` (see `site/README.md`)
- Website: `https://<your-site>/`
- Support contact: `https://<your-site>/support.html` or your email

## Properties
- Category: **Lifestyle**
- Privacy: "This product accesses, collects or transmits personal information" → **Yes** (account email; encrypted content)
- System requirements: Windows 10 version 1809 or later, x64
- Product declarations: "This app has been tested to meet accessibility guidelines" only if you test it

## Age rating questionnaire (IARC) — suggested answers
- Violence, fear, sexuality, gambling, drugs, crude humor: **No**
- Does the app let users interact or exchange content? **Yes**, but only privately between two people who invite each other (no public sharing, no strangers)
- Shares the user's location? **No**
- Digital purchases? **Yes** (subscription and consumables)
Expected result: suitable for teens/adults (e.g. PEGI 12 / ESRB Teen due to user interaction).

## Pricing and availability
- Base price: **Free**
- Markets: all (or start with your main ones)
- Visibility: **Private audience** for the first build (testing purchases with promo codes), then public

## Add-ons (see STORE_RELEASE.md §1.2)
| Name shown | Product ID | Type | Price |
|---|---|---|---|
| Dear Us Premium (monthly) | premium_monthly | Subscription, 1 month | $3.99 |
| Dear Us Premium (yearly) | premium_yearly | Subscription, 1 year | $29.99 |
| 100 Cupid arrows | arrows_100 | Developer-managed consumable | $1.99 |
| 300 Cupid arrows | arrows_300 | Developer-managed consumable | $3.99 |

Add-on description (Premium): "Unlimited cloud sync for photos and videos, 100 Cupid arrows a month and unlimited sealed time capsules, for both of you on all your devices."
Add-on description (arrows): "Extra Cupid arrows for chats and date ideas. They never expire."

## Notes for certification
> Dear Us is a private journal shared between two people; it's fully usable on one PC.
> To test syncing, use two PCs (or two Windows user accounts):
> 1. PC 1: Create our space → Settings & Sync → Partner sync → create an account (any email you
>    can read) → "Create our shared space" → note the recovery phrase → "Create invite code".
> 2. PC 2: "Join my partner" → paste the invite code → create a second account.
> Memories, letters and photos then sync both ways. Pairing starts a 30-day trial with everything
> unlocked. In-app purchases use Microsoft Store commerce. The app needs the `runFullTrust`
> capability because it is a desktop (WebView2) app packaged as MSIX.
