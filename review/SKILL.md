---
name: beedle-review
description: Review a landing page or marketing site for SEO, discoverability, clarity, unnecessary sections and honest claims. Returns findings only and never fixes anything. Use whenever the user asks to review, audit or check a marketing page, landing page or product site, asks why a site isn't being found, or asks for an SEO audit. The beedle skill also starts this automatically after every round. Do not use for app UI or UX flows (use ultrahand:review) or for code review.
---

# Beedle review

Start a review and pass back what it finds. You don't do the review here: a fresh reviewer in its
own locked process reads the rules and checks the page, so none of that loads into this session.

Don't read `method.md`, `checklist.md` or anything in `reference/` for the review. Only reviewers
load those.

If you built this page in the current session, follow [round.md](round.md) instead. It starts the
same review, and says what to do with the findings.

## 1. Write the brief

Fill in the template in section 2 of [round.md](round.md) and save it as `brief-1.md` in
`${TMPDIR}beedle/<project>/direct-<n>/`, outside the repository. It holds facts and never your view.

- **Built site:** the reviewer can't build. Build the site first, without changing tracked files,
  or name an existing build. If there's nothing you can build, such as a live site you only have
  the address of, write `none` and add a `Live URL:` line under it.
- **Page brief, Lessons:** the path, or `none`.
- **Scope:** `the whole site`, unless the user named a part.
- **Still open, Handed to a person:** `none`, unless the user gave you items from an earlier review.

## 2. Run it

```sh
beedle review --brief <folder>/brief-1.md --add-dir <project>
```

Add another `--add-dir` if the built site is outside the project. Run it in the background and wait
for it, which can take 20 minutes. It writes `review-1.md` beside the brief and prints one line with
the verdict and the cost.

If `beedle` isn't on the PATH or the review can't start, use the fallback in section 3 of
[round.md](round.md).

## 3. Return the findings

Read `review-1.md` and nothing else. Give it back as written, with the cost, and stop.

This skill never fixes anything. Deciding what to do about the findings, and doing it, belong to
whoever called it, even if you were asked to "review and fix": that's a separate step, after this
skill is done. A suggested fix is one option, never an instruction.
