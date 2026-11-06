# Installing beedle

beedle is a pair of Claude Code skills. `beedle` builds or changes landing pages and marketing sites, and `beedle-review` reviews them.

## Steps for the agent

1. Put the repo in the user's Claude skills folder, so the path is exactly `~/.claude/skills/beedle/SKILL.md`, and link the review skill next to it:
   ```
   mkdir -p ~/.claude/skills
   git clone https://github.com/mrgnhnt96/beedle ~/.claude/skills/beedle
   ln -s ~/.claude/skills/beedle/review ~/.claude/skills/beedle-review
   ```
   `beedle review` reads its rule files from `~/.claude/skills/beedle/...`, so use that path. If you keep the clone somewhere else, symlink it to that path. beedle writes only into the project it's working on, never into its own files, so a symlink is safe.
2. Put the `beedle` binary on the PATH. This needs `cargo`, and the skills can't audit or review without it:
   ```
   cargo install --path ~/.claude/skills/beedle/cli
   ```
   From the repo, that's `cargo install --path cli`. It puts `beedle` in `~/.cargo/bin`, so add that folder to the PATH if `command -v beedle` finds nothing. It has `audit` and `review` subcommands and reads the rule files from `~/.claude/skills/beedle`, or from the folder given with `--skill-dir`.
3. Start a new Claude Code session. Check that `beedle` and `beedle-review` are in the skills list.

## What it needs

| Needs | Why |
|---|---|
| Claude Code | Both skills run in it |
| Rust (cargo) | Building the `beedle` CLI in `cli/` and putting it on the PATH |
| The `claude` CLI on PATH | `beedle review` runs each review as a headless session (`claude -p`) |
| Node.js with `npx` (optional) | The performance, links and accessibility pass. Without it, that pass is listed under *Not checked* |

## Updating

```
git -C ~/.claude/skills/beedle pull
cargo install --path ~/.claude/skills/beedle/cli
```
