# Dear Us website (dearus.axianthq.com)

Landing page, privacy policy, terms of use and help, linked from every page's footer (the
Microsoft Store needs the privacy policy URL; the terms can be used as the app's license terms).
Plain HTML, no build step. Branding: "Dear Us by Axiant Labs, an independent project by <your
name>", because the Store account is an individual one.

## Before publishing, replace the placeholders
`grep -n "{{" *.html` lists them:

| Placeholder | Value |
|---|---|
| `{{OWNER_NAME}}` | your full legal name, exactly as on your Partner Center account |
| `{{STORE_ID}}` | your app's Store ID (Partner Center → Product identity → Store ID, e.g. 9NXXXXXXXXXX) |

These pages are a careful starting point, not legal advice. If you want certainty for your
markets, have a lawyer read them.

## Host it (free) on Cloudflare Pages
1. dash.cloudflare.com → Workers & Pages → Create → Pages → Connect to Git → this repository.
   Framework preset: none, build command: empty, output directory: `site`, branch: `main`.
2. Custom domains → add `dearus.axianthq.com` (the domain must be in the same Cloudflare account).
3. Email: Cloudflare → axianthq.com → Email → Email Routing → add `dearus@axianthq.com`
   forwarding to your own inbox (free). Replies go out from your own email.

URLs for Partner Center:
- Privacy policy: `https://dearus.axianthq.com/privacy.html`
- Website: `https://dearus.axianthq.com/`
- Support: `https://dearus.axianthq.com/support.html` (or dearus@axianthq.com)
- License terms (optional): `https://dearus.axianthq.com/terms.html`
