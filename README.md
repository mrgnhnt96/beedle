# Beedle

Claude Code skills for landing pages and marketing sites: pages that search engines and AI crawlers can read, that a stranger understands in ten seconds, and that claim only what can be proved.

The name comes from Beedle in The Legend of Zelda, the travelling merchant who always has his shop set up where you need it. This is an unofficial fan reference, with no link to Nintendo.

## What it does

- **Builds from a brief.** Each site gets a page brief: the audience, the page's one job, the search phrases, and every claim with its evidence.
- **Writes the words first.** Copy comes before layout, and a section stays only if it answers a real objection.
- **Never invents proof.** A missing testimonial is a missing section, and anything only a person can supply is listed as waiting on them.
- **Audits the built site** with a script for what a script can decide.
- **Reviews every round.** After each change a fresh reviewer in its own session checks the page as a stranger, a crawler and a sceptic, and returns findings only.

## Install

Follow [INSTALL.md](INSTALL.md), or ask Claude Code to.

## Layout

- `SKILL.md`: the `beedle` skill, which builds or changes a site
- `review/SKILL.md`: the `beedle-review` skill, which reviews one and never fixes anything
- `review/checklist.md`, `review/round.md`: every check, and how each round is reviewed
- `review/audit.mjs`, `review/run.mjs`: the audit, and the runner that starts the fresh reviewer
- `reference/`: page types, message, SEO, a small theme, and the sources behind the rules
- `tests/`: tests for the audit and the runner, and Spotter, a made-up product used as a test site

## Tests

```sh
node --test tests/*.test.mjs
```

Name the files: a bare `tests/` directory argument fails.

## License

[MIT](LICENSE)
