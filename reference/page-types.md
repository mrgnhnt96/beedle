# Page types

Pick the type by what the reader does next, not by what the product is. The section lists are
judgment drawn from the sources and from the owner's sites, not measured rules.

## Launch state

Each type has two states. Pre-launch swaps the primary CTA for one email field and removes
anything that implies the product is available: no store badges, no download buttons, no promised
dates. Live swaps the waitlist for the real action.

## Types

| Type | Primary CTA | Must have | Leave out |
| --- | --- | --- | --- |
| Desktop app | Download for Mac | Real window screenshot, what it does, system requirements, install steps | Sign-up gate before the download |
| Open-source dev tool | Install command, Get started, or View on GitHub | Copyable command, or what leads instead when there's none (`message.md`), a code sample, GitHub and docs links, license | Sales forms, hype, stock photos |
| SaaS | Start, or join the waitlist | Product UI, pricing or a pricing link, security and privacy, proof | Feature dumps, hidden prices |
| Consumer mobile app | Store badges, or email before launch | Real screenshots, ratings if real, privacy and support links | Email gate on a live app, full-page interstitials |
| Local service | Call, book or inquire | Service area, address or area, hours, real reviews, contact in one tap | Stock teams, long history |
| Personal | Contact or résumé | Who you are, 3-6 pieces of work with outcomes, contact | Skill bars, content only JS can show |

Developer pages usually carry two equal actions: try it, and read the docs. That's fine.

## Supporting pages

Add only what the site needs. Each is short and has one job. These are needed whenever the case
applies:

| Page | When | What it needs |
| --- | --- | --- |
| Privacy | Any analytics, form, account or app | Must match what actually loads; collection, third parties, retention, how to withdraw consent |
| Terms | Paid products and accounts | Plain terms |
| Support | Any store app | Real contact details; Apple requires a support URL |
| Delete account | Apps with accounts | Names the app, works without reinstalling, explains steps. Apple needs deletion to start in the app as well |
| Invite fallback | Deep links | `noindex`, app links, store fallback, and `.well-known` files served with no redirects and the right content type |

These are optional. A site is complete without them, so add one only when it earns its place:

| Page | Worth adding when | What it needs |
| --- | --- | --- |
| Download | A desktop app has several builds to choose from. Otherwise the CTA can link straight to the release | Resolves the latest build, with a noscript fallback link |
| Changelog | There are real dated releases to list and people check what's new | Dated entries |
| Pricing | The plans need more room than a home page section. A paid product must still show a price, range or example somewhere | Tiers, limits, a short plan FAQ |
| Comparison | People search for a named rival | Dated, sourced, says where the rival wins. A few honest pages, never templated variants |
| Use-case or feature | There's a distinct search phrase | One job with one example, not a product tour |

## Conventions from existing sites

- One component per section.
- Marketing and docs live apart, such as `revali.dev` and `docs.revali.dev`.
- GitHub Pages has no rewrites. Use Cloudflare Pages when routes like `/i/<code>` need them.
- Commit `CNAME`, and have the deploy fail if a required file is missing or empty.
- Version media filenames, because a reused URL can stick in browser caches.
