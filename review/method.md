# Beedle review method

Review a marketing page as a stranger, a crawler and a sceptic. This is a separate job from `ultrahand:review`, which inspects app flows and controls and has no
checks for search, copy or claims.

Rules live in `~/.claude/skills/beedle/reference/`: `page-types.md`, `message.md`, `seo.md` and
`sources.md`. If the first three aren't already in front of you, read them before you start. Every
check is in [checklist.md](checklist.md).

## This review never fixes anything

It returns findings and stops. Deciding what to do about them, and doing it, belong to whoever
called it: the user, or the session that built the page.

- Don't edit, commit, deploy or rebuild anything for the project, even a one-word fix.
- If you're asked to "review and fix", the review still ends with the report. Fixing is the
  caller's own step, after this review is done.
- A suggested fix is one way to solve the problem, never an instruction. The caller can take it,
  change it or turn it down.
- The verdict reads the findings. It doesn't decide whether to ship.

## 1. Get the evidence

You can't build anything. This review runs locked: it reads files, and its shell runs only
`beedle audit` and read-only checks of a live URL, one command at a time, with no pipes or
redirects. So the brief has to name what you check. Say what you have, because it limits what you
can confirm:

- **Built output:** the best evidence. Use the folder the brief names under *Built site*, and note
  its date.
- **No built output:** if the brief names none, or the folder is missing or empty, say so in the
  paragraph under the verdict, and don't try to build one. Read the source for head tags and copy,
  and mark anything about the output as Likely. Everything that needs the output goes under *Not
  checked*: the audit, the performance, links and accessibility pass, and anything else you could
  only confirm from built files.
- **Live URL:** check it with `beedle fetch <url>`, and say so.

Read `docs/marketing/page-brief.md` and `design/LESSONS.md` if they exist. With no brief, infer
the type, audience and one job from the page, and list that as a finding (no claims record).

## 2. Run the audit

```sh
beedle audit --dir <built site> --exclude <docs section>
```

Its errors become findings, after you check they're real. A script can't tell intent: `noindex`
on an invite page, or a deliberately minimal page, may be right.

The audit opens every image file and requests every external link, so it reports blank images and
dead links without you looking for them. It doesn't see CSS background images. On a machine with no
network, add `--offline` and list external links under *Not checked*. A warning that the site's own
domain isn't live is fine before launch and a Blocker after it, so the brief decides.

If there's no built output, or `beedle` isn't on the PATH, you can't run it: list the audit under
*Not checked*, and check by hand what you can.

## 3. Work through the checklist

Go through every pass in [checklist.md](checklist.md) in order. A pass you can't run goes under
*Not checked*, with the reason. Never guess.

## 4. Rate each finding

| Severity | Meaning |
| --- | --- |
| Blocker | Untrue or illegal claim, a page crawlers can't read, a broken primary CTA, a blank image passed off as a product screenshot, or a missing required store page. Stops a release. |
| Major | Hurts discovery or understanding for most visitors: the hero fails the ten-second test, no canonical or share image, or a section that buries the CTA. |
| Minor | A real improvement with limited effect. |
| Note | Optional, or depends on the owner's judgment. |

Then say who can act on it:

- **Caller:** anyone with the project and the brief can do it.
- **Person: what they must do:** it needs something only a person has: an account, a published
  release, a domain, a real asset, evidence or the owner's decision. This includes the case where
  the caller's only option is to take away something the page can't work without, such as its
  main CTA or the product's core claim.

## 5. Items the brief carries

A brief can list items that are already dealt with, so nobody pays to hear about them twice.

**Handed to a person** means the caller has passed the item on and can do nothing more. **Still
open** means the caller has seen the item and left it, for now or on purpose. For both:

- Don't check, re-rate, reword or report them, and don't report audit output that matches them.
- Keep each item's ID and severity. They still count towards the verdict.
- A fault that only restates an item isn't a finding. If a change made one worse, report just
  what's new, under a new ID.

The caller takes an item off the brief once it's been dealt with, and then it's checked as normal.

## 6. Report

The first line is exactly one of these, because the caller reads it:

- `Verdict: Ready to ship`: no open Blocker or Major.
- `Verdict: Fix first`: at least one open Blocker or Major that the caller can act on.
- `Verdict: Can't ship until a person acts`: there are open Blockers or Majors and every one of
  them needs a person. Nothing the caller can do changes the answer.

Open means this report's findings plus the items the brief carries. Follow the verdict with a short
paragraph: type and state, evidence, counts by severity, and the top problem.

Then each new finding, most severe first, under a heading like `### B3 · Major · Title`. IDs carry
on from the highest ID in the brief, and never get reused.

- **Where:** page and element
- **Evidence:** Confirmed (seen in the output or measured) or Likely
- **Problem:** what goes wrong, for whom
- **Rule:** the reference file and rule, with its strength when it's only opinion
- **Suggested fix:** one concrete way to solve it, with the replacement words or tag
- **Who can act:** `Caller`, or `Person: what they must do`

If the scope asks whether earlier findings are resolved, add `## Re-check` with one line each,
Resolved or Not resolved. One that isn't resolved goes in the findings under its old ID.

If the brief carries items, list them one line each, copied unchanged:

```text
## Waiting on a person
- B1 · Blocker · Every GitHub link is dead · Owner: publish the repository

## Still open
- B7 · Minor · Search words missing from the title
```

Then the sections you'd cut, what works (keep this short), and *Not checked*. Separate what's
documented from what's judgment: never present opinion as a platform rule.
