# Dear Us website

Landing page, privacy policy (required by the Microsoft Store) and help page. Plain HTML, no build step.

## Before publishing, replace the placeholders
`grep -n "{{" *.html` lists them:

| Placeholder | Example |
|---|---|
| `{{OPERATOR_NAME}}` | your full legal name (or company) |
| `{{OPERATOR_COUNTRY}}` | Lebanon |
| `{{CONTACT_EMAIL}}` | hello@dearus.app (or your own email until the domain exists) |
| `{{HOSTING_PROVIDER}}` | Hetzner Online GmbH (Germany) |
| `{{DATE}}` | the date you publish it |
| `{{STORE_LINK}}` | your app's Store link from Partner Center |

## Host it for free (works before you own a domain)
**Cloudflare Pages** (works with a private repository): dash.cloudflare.com → Workers & Pages → Create →
Pages → Connect to Git → this repository → build command: *(none)*, output directory: `site`.
You get `https://dear-us.pages.dev` right away; add `dearus.app` later under Custom domains.

Put `https://<your-site>/privacy.html` in Partner Center → Store listing → Privacy policy URL, and
`https://<your-site>/support.html` as the support contact / website.
