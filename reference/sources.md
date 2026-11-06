# Sources

Researched 2026-11-06. Re-check platform rules before relying on them for long: they change.

**Strength:** Rule = binding or platform policy. Doc = documented by the platform. Measured = a
study. Vendor = vendor data, usually correlational. Opinion = expert or practitioner judgment.

## Content and conversion

| Claim | Strength | Source |
| --- | --- | --- |
| First screen gets 57% of viewing time | Measured, 120 users, 2018 | https://www.nngroup.com/articles/scrolling-and-attention/ |
| Users mostly leave in 10-20 seconds | Measured, 2010 | https://www.nngroup.com/articles/how-long-do-users-stay-on-web-pages/ |
| Shorter copy and grade 5-7 convert better | Vendor, correlational, 41K pages | https://unbounce.com/conversion-benchmark-report/ |
| A much longer page can win (30%, with rewritten objections) | Case study, confounded | https://www.conversion-rate-experts.com/crazy-egg-case-study/ |
| Choice overload averages about zero | Measured, N=5,036 | https://abcwest.cogs.indiana.edu/pmwiki/pdf/scheibehenne.options.2010.pdf |
| Dev-tool page patterns | Opinion, 100+ pages, no conversion data | https://evilmartians.com/chronicles/we-studied-100-devtool-landing-pages-here-is-what-actually-works-in-2025 |
| Security and pricing are top reasons to reject a tool | Measured, survey, 49K | https://survey.stackoverflow.co/2025/ |
| Show prices | Measured, qualitative, 2013 | https://www.nngroup.com/articles/show-price/ |
| Generated-design tells | Informal, 1,590 pages, no conversion link | https://adriankrebs.ch/blog/design-slop/ |
| Generated-copy tells | Opinion | https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing |
| Carousels hide content | Opinion, one case | https://www.nngroup.com/articles/auto-forwarding/ |

There's no measured data on the sections each page type needs. Those tables are judgment.

## Legal and stores

| Claim | Strength | Source |
| --- | --- | --- |
| Fake reviews and testimonials banned | Rule | https://www.law.cornell.edu/cfr/text/16/465.2 |
| FTC Q&A on disclosure and incentives | Rule | https://www.ftc.gov/business-guidance/resources/consumer-reviews-testimonials-rule-questions-answers |
| Penalty per violation | Rule | https://www.law.cornell.edu/cfr/text/16/1.98 |
| Apple support and privacy URLs | Rule | https://developer.apple.com/app-store/review/guidelines/ |
| Apple account deletion | Rule | https://developer.apple.com/help/app-review/guideline-reference/5-1-1-account-deletion |
| Google Play deletion page | Rule | https://support.google.com/googleplay/android-developer/answer/13327111?hl=en-GB |
| Apple badge rules | Rule | https://developer.apple.com/app-store/marketing/guidelines/ |
| Intrusive interstitials | Doc | https://developers.google.com/search/docs/appearance/avoid-intrusive-interstitials |
| Accessibility overlay claims | Rule, FTC order | https://techcrunch.com/2025/01/03/ftc-orders-ai-accessibility-startup-accessibe-to-pay-1m-for-misleading-advertising |

## Search

| Claim | Strength | Source |
| --- | --- | --- |
| Rendering, canonical and link rules | Doc | https://developers.google.com/search/docs/crawling-indexing/javascript/javascript-seo-basics |
| Flutter web and search | Doc | https://docs.flutter.dev/platform-integration/web/faq |
| AI crawlers don't run JS | Vendor, December 2024, no 2026 replication found | https://vercel.com/blog/the-rise-of-the-ai-crawler |
| Titles | Doc | https://developers.google.com/search/docs/appearance/title-link |
| Descriptions | Doc | https://developers.google.com/search/docs/appearance/snippet |
| Site name, favicon, Organization | Doc | https://developers.google.com/search/docs/appearance/site-names |
| Organization has no required properties; logo at least 112x112 | Doc | https://developers.google.com/search/docs/appearance/structured-data/organization |
| Software structured data | Doc | https://developers.google.com/search/docs/appearance/structured-data/software-app |
| FAQ, HowTo and other removals | Doc | https://developers.google.com/search/updates |
| Core Web Vitals | Doc | https://web.dev/articles/vitals |
| LCP guidance | Doc | https://web.dev/articles/optimize-lcp |
| AI features need no extra files; llms.txt not used by Google Search | Doc | https://developers.google.com/search/docs/fundamentals/ai-optimization-guide |
| llms.txt rarely fetched | Vendor, May 2026 | https://ahrefs.com/blog/llmstxt-study/ |
| GEO citation effects | Measured, simulated engine | https://arxiv.org/abs/2311.09735 |
| OpenAI crawlers | Doc | https://developers.openai.com/api/docs/bots |
| Anthropic crawlers | Doc | https://support.claude.com/en/articles/8896518-does-anthropic-crawl-data-from-the-web-and-how-can-site-owners-block-the-crawler |
| Perplexity crawlers | Doc | https://docs.perplexity.ai/guides/bots |
| Bing AI performance | Doc | https://blogs.bing.com/webmaster/February-2026/Introducing-AI-Performance-in-Bing-Webmaster-Tools-Public-Preview |
| Spam policies | Doc | https://developers.google.com/search/docs/essentials/spam-policies |

## Not verified

- X card specs: the docs failed to load, and `summary_large_image` is from memory.
- `apple-app-site-association` details and Apple's Smart App Banner page.
- Whether ChatGPT search relies on Bing's index.
- Waitlist, comparison, local and event pages have no primary data.
