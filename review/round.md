# Review every round

Every round ends with an independent review, and the user hears the page is ready only after it.
A **round** is the first build and every later prompt that changes the page.

## 1. Choose the scope

- **First round:** the whole site.
- **Later rounds:** the diff and everything it touches.
- **Widen to the whole site** when the diff changes something shared: the layout, head tags, nav,
  footer, analytics, the brief, or most of the home page.

Keep rounds outside the repository, in `${TMPDIR}beedle/<project>/round-<n>/`. Before every review
pass, keep a copy of what the reviewer is about to see:

```sh
cp -R <built site> <round>/reviewed-<k>
```

For a later round or a re-check, compare the newest copy with the build as it is now. `diff` exits
1 when it finds changes, which isn't a failure.

```sh
diff -ruN <newest reviewed copy> <built site> > <round>/changes.diff
```

Do this in a repository too. `git diff` misses untracked files, which is every new file on a page
that hasn't been committed, and it shows what's uncommitted rather than what changed since the last
review. If there's no earlier copy, review the whole site.

## 2. Write the brief

The reviewer treats the brief as true, so it holds facts and never your view. Fill in this
template and add nothing else. Never say why you made a choice, what you changed in your own
words, what to look at "in particular", or how good you think it is. The diff shows what changed.

```text
Review a marketing page by the beedle review method. Don't edit any files.

Project: <absolute path>
Built site: <absolute path to the build output>
Exclude from the audit: <sections, or "none">
Page brief: <absolute path to docs/marketing/page-brief.md>
Lessons: <absolute path to design/LESSONS.md, or "none">
This round's request, in the user's words: "<prompt>"
Scope: <"the whole site" | "the changes in <absolute path to the diff> and everything they touch">
Still open from the last round: <one line each, "ID · severity · title", with the user's words if kept on purpose, or "none">
Handed to a person: <"none", or each item on its own line below this one, "ID · severity · title · who: what they must do">

Return the method's report. Name any problem outside the scope that you happen to see, but don't go looking for one.
```

## 3. Run it

```sh
beedle review --brief <round>/brief-1.md --add-dir <project>
```

Add another `--add-dir` if the built site is outside the project. Run it in the background and
wait for it. It starts a lean headless Claude that has never seen this conversation, and writes
`review-1.md` beside the brief, plus `review-1.json` with the cost, the verdict and `repeated`: any
handed-over item the report listed again. It should be empty. If it isn't, ignore those findings
and tell the user the run paid for them twice.

If `beedle` isn't on the PATH or the review can't start, use a general-purpose agent (not a fork,
which shares your context) with the same brief, starting with: "Read method.md and checklist.md in
~/.claude/skills/beedle/review/, then follow them. Don't load the beedle-review skill, which only
starts another review." The agent reads those files, never you. Its reply is the report: save it as
`review-1.md`. There's no `review-1.json`, so say the cost isn't known.

If that can't start either, stop and tell the user the review didn't run and the page isn't
reviewed. Never review it in this session.

## 4. Act on the findings

The reviewer only reports. Every decision here is yours, and a suggested fix is one option rather
than an order. Check each finding, then sort them by *Who can act*.

**Caller:**

- Fix every Blocker and Major.
- Fix Minors, unless a fix would undo what the user asked for this round. Report those.
- Never fix a claim problem by rewording something you can't support. Find a primary source and
  cite it, or remove the claim, or ask the user for the evidence.
- A plain description of what the product is and does can stay on the README alone, marked
  `README only` in the claims table. Everything else the README says needs that primary source. If
  removing what's unsupported leaves nothing a stranger could act on, that's a Person item: the
  page needs facts only the owner has.
- Never reverse what the user asked for to satisfy a finding. Report the disagreement instead.
- If a finding says Caller but only a person can fix it, treat it as Person, and name who and what.

**Person:** you can't fix these, however serious they are. Never work around one with a stand-in, a
fake value or softer wording. For each one:

- Leave the page honest in the meantime. If it works without the missing thing, take that out: a
  missing quote is a missing section. If it can't, leave a sentinel that fails loudly, such as
  `SCREENSHOT_TODO`, and let the page stay blocked.
- Tell the user who must do what.
- Copy it unchanged into the next brief's `Handed to a person` block, so no later round pays to
  find it again, and into *Waiting on a person* in the page brief, because round folders are
  temporary. Put each item on its own line, and keep the blank line after the block: `beedle review`
  reads up to it.
- Once the person has acted, take it off both. That's a new round, and the review checks it. If
  they've done only part of it, keep the item in both places, cut down to what's left.

Then go by the verdict:

- **Ready to ship:** go on to the report.
- **Fix first:** fix, then re-review as below.
- **Can't ship until a person acts:** stop. More rounds can't change the answer, so don't pay for
  one. Don't deploy, and don't call the page ready. Report what the person must do. Wait for them
  unless you've fixed a Caller finding that still needs checking. If the user chooses to ship
  anyway, that's their call: first say exactly what's still wrong.

If you fixed a Blocker or Major, send just those fixes to a fresh reviewer with this scope:

```text
Scope: check whether findings <IDs> in <absolute path to review-1.md> are resolved, using the changes in <absolute path to the fix diff>, and everything those changes touch.
```

Stop after two review passes and report what's left.

## 5. Report it

Tell the user the verdict, the scope, findings by severity, what you fixed, what's open and why,
who must do what, and the cost from each `review-<k>.json`.
