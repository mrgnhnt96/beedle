---
name: beedle
description: Build or change landing pages and marketing sites that people can find and that say one thing clearly. Use whenever the user wants a landing page, marketing page, product or app website, homepage, waitlist page, download page, pricing page, comparison page, or a site for a local business or personal brand, or asks to rewrite, restructure or improve the SEO of one, even if they only say "a site for this project". Do not use for docs sites, app screens or dashboards (use the ultrahand skills for app UI).
---

# Beedle

Build marketing pages that search engines and AI crawlers can read, that a stranger understands
in ten seconds, and that claim only what can be proved. Less is more: every section has to earn
its place. Every round ends with an independent review.

**Lessons:** before step 1, read the project's `design/LESSONS.md` if it has one and follow
anything about its marketing site. Those are the owner's corrections.

## 1. Read the project

Find out what already exists before you ask anything: the stack and host, any current site, the
product's real name, repository, store listings, pricing, screenshots, design tokens or theme,
analytics, and `docs/marketing/page-brief.md`.

Build in the stack the project already uses. For a new site, follow
[reference/seo.md](reference/seo.md) ("Picking a stack"). Marketing content is never Flutter web
or client-rendered, because crawlers may never see it.

Use the project's theme if it has one (`design/THEME.md`, `*.tokens.json`). If not, port the tokens
from the app. If there is no app to port from either, write the one small file in
[reference/small-theme.md](reference/small-theme.md), because one page doesn't need a design
system. Reach for the `ultrahand:theme` skill only when the user asks for a theme, or when several
sites or an app will have to share it.

## 2. Pick the type and launch state

Read [reference/page-types.md](reference/page-types.md). Choose one type and one state
(pre-launch or live), and note the supporting pages the site needs.

## 3. Write the brief

Save `docs/marketing/page-brief.md` in the project, using this template. It is the contract the
reviewer checks the page against, and the next session's starting point.

```markdown
# Page brief

- Type and state:
- Audience: (who, and what they already know)
- The page's one job:
- Primary CTA: (exact words and destination)
- Secondary action: (one, lighter, or "none")
- Search phrases: (3-5 things this audience actually types)
- Objections: (the real reasons someone would leave)
- Analytics and cookies: (what loads, or "none")

## Claims

| Claim on the page | Evidence |
| --- | --- |
| <every number, quote, comparison, "first", "fastest", ...> | <file, link, or measurement, with date> |
| <a plain description of what it is or does> | <file or link, or `README only` if that's all there is> |

## Waiting on a person

- <who: what they must do, or "nothing">
```

Ask the user only for what the project can't answer, usually the audience and the proof. Never
invent proof. A missing testimonial is a missing section, not a placeholder to fill in. Use
sentinels that fail loudly, such as `APP_STORE_ID_TODO`, rather than plausible fake values.

**When the README is all you have,** you can still build the page. It can say plainly what the
product is and does: mark those rows `README only`. It can't prove numbers, dates, versions,
licence, price, security, privacy or availability, so look for a primary source for each: the code,
a LICENSE file, a release page, a registry or store listing, a measurement, or the owner's dated
word. Cite that source. If there isn't one, leave the claim off the page.

**Anything only a person can supply** goes under *Waiting on a person* as soon as you know about
it: a real screenshot, a domain, a price to confirm. It stays there until they've done it.

## 4. Write the words first

Follow [reference/message.md](reference/message.md). Write the copy before any layout. Start from
the five blocks there. Add a section only when you can name the objection it answers, and record
that objection in the brief.

## 5. Build

Follow [reference/seo.md](reference/seo.md) for what goes in the page and alongside it. Use real
product screenshots or footage, and never label a recreation as a screenshot. A capture of the real
app running example data is real: say it's an example on the page, and say how you made it in the
claims table. With nothing real to show, leave the image out, or leave a sentinel if the page type
needs one. Avoid the design
tells listed in the message reference, since they make a page look generated before anyone has
read it.

## 6. Self-check

Build the site and run the audit on the built output, then fix every error you can:

```sh
beedle audit --dir <built site>
```

Add `--exclude docs` for each section that is not marketing (repeatable). If `beedle` isn't on the
PATH, tell the user the audit didn't run and that [INSTALL.md](INSTALL.md) says how to add it.

An error only a person can clear stays, such as a `DOMAIN_TODO` sentinel for a domain nobody has
bought. Never swap in a fake value to get a clean run. Make sure it's under *Waiting on a person*,
then go on to the review.

## 7. Review the round

Every round ends with a review by a fresh reviewer in its own session, before you tell the user
the page is ready. A **round** is the first build and every later prompt that changes the page,
so a one-line copy change is reviewed too. Skip it only when the user says to skip it this round,
and say that you skipped it.

Start it yourself, without being asked. You can't review your own page fairly: you see what you
meant to build. Follow [review/round.md](review/round.md). In short, write the brief, run
`beedle review --brief <brief> --add-dir <project>` in the background, then act on the findings. Don't publish, deploy or call the page done before this.

## 8. Report

Say what you built, the type and state, the verdict, and what the review found, fixed and left
open, with its cost.

If the verdict is *Can't ship until a person acts*, lead with that: the page isn't ready, who must
do what, and what the next round will check once they have. Never soften it to "nearly done".

Always list what's left for the owner after launch: Search Console and Bing Webmaster verification,
and the Rich Results Test on the deployed page.

Do not deploy unless the user asks.

## 9. Learn

When the owner corrects the result or states a lasting preference, add one line to the project's
`design/LESSONS.md` (create it if needed). If the correction shows these rules are wrong for every
project, fix the rule here too.
