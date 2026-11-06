# Installing beedle

beedle is a pair of Claude Code skills. `beedle` builds or changes landing pages and marketing sites, and `beedle-review` reviews them.

## Steps for the agent

1. Put the repo in the user's Claude skills folder, so the path is exactly `~/.claude/skills/beedle/SKILL.md`, and link the review skill next to it:
   ```
   mkdir -p ~/.claude/skills
   git clone https://github.com/mrgnhnt96/beedle ~/.claude/skills/beedle
   ln -s ~/.claude/skills/beedle/review ~/.claude/skills/beedle-review
   ```
   The skills call the scripts at `~/.claude/skills/beedle/...`, so use that path. If you keep the clone somewhere else, symlink it to that path. beedle writes only into the project it's working on, never into its own files, so a symlink is safe.
2. Build the CLI, which needs a Rust toolchain:
   ```
   cargo install --path ~/.claude/skills/beedle/cli
   ```
   This puts `beedle` in `~/.cargo/bin`. It has `audit` and `review` subcommands and reads the rule files from `~/.claude/skills/beedle`, or from the folder given with `--skill-dir`. The skills still call the two scripts for now, so this step is optional until they move over.
3. Start a new Claude Code session. Check that `beedle` and `beedle-review` are in the skills list.

## What it needs

| Needs | Why |
|---|---|
| Claude Code | Both skills run in it |
| Node.js (standard library only) | `review/audit.mjs` and `review/run.mjs` |
| The `claude` CLI on PATH | `review/run.mjs` and `beedle review` run each review as a headless session (`claude -p`) |
| Rust (cargo) | Building the `beedle` CLI in `cli/` |

## Updating

```
git -C ~/.claude/skills/beedle pull
cargo install --path ~/.claude/skills/beedle/cli
```
