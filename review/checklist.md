# Review checklist

Passes run in this order. Rules and their sources are in `~/.claude/skills/beedle/reference/`.
Skip anything the brief lists as *Handed to a person* or *Still open*.

## 1. Crawlable

- Run `audit.mjs`. Confirm each error. It covers blank images and dead external links, so don't
  repeat that work by hand.
- Fetch or read the raw HTML with no JavaScript. Are the H1, body copy, CTA and links all there?
  This approximates crawlers that don't run JS.
- Is any marketing content only in client-rendered code?

## 2. Ten seconds

Read only the first screen at 1440 and 390 wide, as a stranger.

- Can you say what it is and who it's for?
- Is the primary CTA visible, and does its wording say what happens?
- Is the real product shown?
- Does the page match its type's primary CTA in `page-types.md`?

## 3. Less is more

- List the sections. Does each do a job the five blocks or a named objection in the brief requires?
  Name the ones to cut.
- More than one competing goal on the page?
- Is anything repeated without adding information?
- Don't punish a page for being short if it does its job, and don't cut a section that answers a
  real objection.

## 4. Truth

The most important pass. Check each claim against evidence, not against how plausible it sounds.

- Every number, quote, comparison and superlative: is it in the brief's claims table, with a real
  source? Open the source when you can.
- Apply the README rule below to every row that cites the project's own README.
- Do testimonials name real people, and are insiders or incentives disclosed?
- Does structured data claim ratings or reviews that don't exist?
- Do privacy, cookie and tracking statements match the scripts actually loaded on every page,
  including the privacy page itself?
- Does availability wording match reality? No store badges or downloads for an unreleased product.
- Are "screenshots" real? The audit only catches blank ones. Look at each for a mock-up, stock
  picture or a different product. A capture of the real app with example data is real if the page
  says it's an example.
- Are there placeholder IDs, links or dates?

### The project's own README

It isn't independent evidence. It has the same author as the page, often the same session: in the
first trial, the README called a blank image a real screenshot.

- **It can support** a plain description of what the product is and does. Mark that `README only`
  and Likely, and check it against the code if you have it.
- **It never proves** numbers, dates, versions, comparisons, superlatives, availability, licence,
  price, security, privacy, notarisation, ratings or quotes. Those need a primary source: a LICENSE
  file, a release page, the code, a measurement, or the owner's confirmation dated in the claims
  table.
- A claims row whose only source is the README counts as having no source for those. That makes
  the claim unsupported, not untrue: rate it Major, or Blocker when people act on it, such as price,
  licence, safety or availability.
- A README that contradicts the page is a finding either way.

## 5. Search

- Does the title and H1 use words the audience actually searches for, per the brief?
- Are titles and descriptions specific, or generic ("Home", "Features")?
- Is structured data right for the type? Validate the required properties by hand, and read
  `seo.md`. `Organization` with no `logo` or `sameAs` is fine when the product has none.
- Is the share image a real 1200x630 image, not a logo?
- Are `robots.txt` and the sitemap correct, and are search crawlers allowed?
- Do `noindex` pages deserve it, and is anything important accidentally excluded?

## 6. Words

- Roughly grade 5-7, or suited to the audience?
- Specific rather than generic?
- Developer pages: command (or what `message.md` says leads when there's none), code, docs, GitHub,
  pricing and privacy all findable? A missing command is a finding only if the tool has one.
- Stacked generated tells from `message.md`? One tell isn't a finding.

## 7. Required pages

- Privacy, terms, support and deletion pages per `page-types.md`. Do they work, and do they say
  something real?
- Do footer links resolve to pages with content?
- The optional pages in `page-types.md`, such as Download and Changelog, are never a finding for
  being absent.
- Do app links and `.well-known` files exist where the type needs them?

## 8. Performance, links and accessibility

Serve the build locally, such as `python3 -m http.server --directory <dir> 8123`, then run:

```sh
npx --yes lighthouse http://localhost:8123 --quiet --chrome-flags="--headless" \
  --only-categories=performance,accessibility,best-practices,seo --output=json \
  --output-path=<tmp>/lh.json
npx --yes pa11y http://localhost:8123
npx --yes linkinator http://localhost:8123 --recurse --skip '^https?://(?!localhost)'
```

Linkinator covers internal links only, because the audit already checked the external ones. Report
failures rather than scores. Lighthouse's SEO score is shallow: it gave 0.92 to a test page
with a broken link and missing alt text, so don't treat a high score as a pass. A local run says
nothing about real-user Core Web Vitals, so list those under *Not checked*.

## 9. Layout

Check for horizontal overflow and an unreachable CTA at 320, 390, 768 and 1440 wide, if you can
drive a browser. Tab through to the primary CTA. Honour reduced motion. For a full inspection of
interactive sections, recommend `ultrahand:review` in the report.

## Always under Not checked

- Rich Results Test, which has no API.
- Real-user Core Web Vitals, and how the site ranks.
- CDN or firewall blocks on crawlers, unless tested on the live site.
