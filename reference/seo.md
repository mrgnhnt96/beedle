# SEO and discoverability

Google's own docs outrank SEO blogs here. Tags in [sources.md](sources.md) say how strong each
claim is.

## Picking a stack

Everything a reader or crawler needs must be in the HTML the server sends. Google can render
JavaScript, but many AI crawlers appear not to, and Flutter's own docs say Flutter web isn't suited
to landing pages.

- Dart or Flutter project: Jaspr in static mode, with the app kept separate.
- Multi-page site or blog: Astro.
- One page: hand-written HTML is fine.
- Keep what a project already uses, as long as its output is static.

Client-side extras such as islands are fine when the words and links are already in the HTML.

## Every indexable page

- `<html lang>` and a viewport tag.
- A unique, descriptive `<title>`, briefly branded, matching the visible H1. Google sets no length
  limit, so write for the reader rather than a character count.
- A unique meta description that summarises the page. Google may ignore it.
- An absolute `rel="canonical"` in the initial HTML.
- Exactly one H1, then headings in order.
- `og:title`, `og:description`, `og:type`, `og:url` and a 1200x630 `og:image` that isn't a logo or
  a square icon, plus `twitter:card` set to `summary_large_image`.
- Real `<a href>` links with readable hyphenated URLs. No fragment routing.
- Descriptive `alt` on meaningful images, with `width` and `height` set.
- Use the phrases from the brief where they fit naturally. Don't stuff them in.

## The home page

- `WebSite` JSON-LD for the site name, and `Organization` with a logo of at least 112px and
  `sameAs`, or `Person` for a personal site.
- No logo? Use the real app icon if it's at least 112px. If there's no icon either, leave `logo`
  out and keep `name` and `url`: Google requires no property here, so a shorter `Organization` is
  valid. Never draw a logo to fill the field, or point it at the share image. Leave `sameAs` out
  too if there are no real profiles.
- No company behind the product? Use `Person` for its maintainer, if they want to be named.
- A square favicon larger than 48px, as PNG or ICO. With no logo, a plain letter on a solid colour
  will do: it stands in for nothing.

## Structured data by type

| Page | Type | Note |
| --- | --- | --- |
| Software | `SoftwareApplication` or `MobileApplication` | A rich result needs a price plus real ratings. Add the type without ratings, and never invent them |
| Game | `["VideoGame", "MobileApplication"]` | Must be co-typed |
| Local business | `LocalBusiness` subtype | Real address, hours and phone |
| Physical product | `Product` | Not for SaaS |
| Demo video | `VideoObject` | If embedded |
| Deep pages | `BreadcrumbList` | Two or more levels only |

Don't add `FAQPage` or `HowTo`, which no longer produce rich results.

## Site files

- `sitemap.xml` with an accurate `lastmod`. Skip `priority` and `changefreq`, which Google ignores.
- `robots.txt` with a `Sitemap:` line. Allow Googlebot, Bingbot, OAI-SearchBot, Claude-SearchBot,
  Claude-User and PerplexityBot. Allowing training crawlers (GPTBot, ClaudeBot, Google-Extended)
  is the default for these products; `Google-Extended` doesn't affect Search. Also check that the
  CDN or firewall doesn't block these bots.
- A real 404 page. If the host has an SPA fallback, unknown paths must still return 404.
- `noindex` on auth, reset and invite pages.
- App links when there's an app: `apple-itunes-app` Smart App Banner, `assetlinks.json` and
  `apple-app-site-association`.
- `llms.txt`: only for developer tools, and only because agents like Claude Code fetch it. It does
  nothing for search.
- Translated pages only: reciprocal, self-referencing `hreflang`.

## Performance

Thresholds at the 75th percentile: LCP 2.5s or less, INP 200ms or less, CLS 0.1 or less. Fix
failures rather than chasing 100, because relevance matters more.

- Never lazy-load the hero image. Keep it in the HTML with `fetchpriority="high"`.
- Starting budgets (judgment, not platform rules): hero 150KB or less as AVIF or WebP with
  `srcset`, two WOFF2 files at most with `font-display: swap`, and 50KB gzipped JS or less.
- Respect `prefers-reduced-motion`.

## AI search

Inclusion in Google's AI features needs only normal indexing and snippet eligibility. There's no
special file or schema. Write original, first-hand, specific content with clear headings. Stating
facts, sources and tables plainly is sensible, but the evidence for citation gains is weak, so
don't contort the page for it, and don't quote GEO scores.

## After launch

- Verify Google Search Console and Bing Webmaster Tools; both show AI performance data. Submit the
  sitemap.
- Run the Rich Results Test on the live URL, since it has no API.
- Complete store listings, and set the homepage link, GitHub topics and pub.dev topics. It's cheap
  and helps discovery.
- IndexNow is cheap and reaches Bing, but not Google.
- Avoid paid links and mass directories, which are link spam.
