#!/usr/bin/env node
// Run one beedle review as its own headless `claude -p` session.
//
//   node run.mjs --brief <dir>/brief-1.md [--out <dir>/review-1] [--add-dir <dir>]...
//                [--model opus] [--effort <level>] [--timeout 20]
//
// The reviewer has none of the user's settings, plugins or MCP servers, and never sees the
// session that built the page. Writes <out>.md and <out>.json, which holds the cost, the verdict
// and any item handed to a person that the report repeated. Exits 1 if the review failed.
// $BEEDLE_CLAUDE names the claude program (the tests use a fake).

import { spawn } from 'node:child_process';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = dirname(realpathSync(fileURLToPath(import.meta.url)));
export const ROOT = resolve(HERE, '..');
export const TOOLS = ['Read', 'Glob', 'Grep', 'Bash'];
const FILES = ['review/SKILL.md', 'review/checklist.md', 'reference/page-types.md', 'reference/message.md', 'reference/seo.md'];

export function systemPrompt(root = ROOT) {
  const parts = [
    '# Beedle reviewer',
    '',
    'You review marketing pages, with no conversation before this one. The beedle-review skill,',
    'its checklist and the rules are below: follow the skill. Wherever they say',
    `~/.claude/skills/beedle/, use ${root}/ instead. Skills can't be loaded here; this is the`,
    'whole of it. The user message is the brief. Answer with the report only.',
  ];
  for (const rel of FILES) {
    parts.push('', `<file path="${rel}">`, readFileSync(join(root, rel), 'utf8').trimEnd(), '</file>');
  }
  return parts.join('\n') + '\n';
}

export function claudeArgs({ system, model, effort, addDirs = [], root = ROOT }) {
  const args = [
    '-p', '--model', model, '--system-prompt-file', system,
    '--setting-sources', '', '--strict-mcp-config', '--disable-slash-commands',
    '--no-session-persistence', '--output-format', 'stream-json', '--verbose',
    '--tools', TOOLS.join(','), '--allowedTools', TOOLS.join(','),
    '--permission-mode', 'dontAsk', '--add-dir', root,
  ];
  if (effort) args.push('--effort', effort);
  for (const dir of addDirs) args.push('--add-dir', dir);
  return args;
}

export function summarise(lines) {
  const out = { ok: false, report: '', turns: 0, seconds: 0, costUsd: 0, error: null };
  let lastText = '';
  for (const line of lines) {
    let event;
    try { event = JSON.parse(line); } catch { continue; }
    if (event.type === 'assistant' && event.message) {
      const text = (event.message.content ?? []).filter((c) => c.type === 'text').map((c) => c.text).join('');
      if (text.trim()) lastText = text;
    }
    if (event.type === 'result') {
      out.ok = event.subtype === 'success' && !event.is_error;
      out.report = typeof event.result === 'string' && event.result.trim() ? event.result : lastText;
      out.turns = event.num_turns ?? 0;
      out.seconds = Math.round((event.duration_ms ?? 0) / 1000);
      out.costUsd = event.total_cost_usd ?? 0;
      if (!out.ok) out.error = event.subtype ?? 'error';
    }
  }
  if (!out.report) out.report = lastText;
  return out;
}

const VERDICT = /^\W*Verdict:\W*(Ready to ship|Fix first|Can['’]t ship until a person acts)/m;
const FINDING_HEADING = /^#+\s*\**([A-Z]\d+)\s*·\s*(?:Blocker|Major|Minor|Note)\b/gm;

export function handedOver(brief) {
  const section = brief.match(/^Handed to a person:([\s\S]*?)(?:\n\s*\n|$(?![\s\S]))/m)?.[1] ?? '';
  return [...section.matchAll(/\b([A-Z]\d+)\s*·/g)].map((match) => match[1]);
}

export function readReport(report, brief) {
  const handed = new Set(handedOver(brief));
  const reported = [...report.matchAll(FINDING_HEADING)].map((match) => match[1]);
  return {
    verdict: report.match(VERDICT)?.[1].replace('’', "'") ?? null,
    repeated: [...new Set(reported.filter((id) => handed.has(id)))],
  };
}

export function defaultOut(brief) {
  const name = basename(brief, '.md');
  return join(dirname(brief), name.startsWith('brief') ? name.replace(/^brief/, 'review') : `${name}.review`);
}

function parse(argv) {
  const options = { model: 'opus', effort: '', timeout: 20, addDirs: [] };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const next = () => {
      if (i + 1 >= argv.length) throw new Error(`${arg} needs a value`);
      return argv[++i];
    };
    if (arg === '--brief') options.brief = next();
    else if (arg === '--out') options.out = next();
    else if (arg === '--add-dir') options.addDirs.push(resolve(next()));
    else if (arg === '--model') options.model = next();
    else if (arg === '--effort') options.effort = next();
    else if (arg === '--timeout') options.timeout = Number(next());
    else throw new Error(`unknown option ${arg}`);
  }
  return options;
}

export async function main(argv = process.argv.slice(2)) {
  const options = parse(argv);
  if (!options.brief) {
    process.stderr.write('usage: run.mjs --brief <file> [--out <path>] [--add-dir <dir>]...\n');
    return 2;
  }
  const brief = readFileSync(options.brief, 'utf8');
  const out = resolve(options.out ?? defaultOut(options.brief));
  const system = join(tmpdir(), `beedle-review-${process.pid}.md`);
  writeFileSync(system, systemPrompt());
  const args = claudeArgs({ system, model: options.model, effort: options.effort, addDirs: options.addDirs });

  const lines = [];
  let stderr = '';
  const code = await new Promise((done) => {
    const child = spawn(process.env.BEEDLE_CLAUDE || 'claude', args, {
      cwd: dirname(resolve(options.brief)),
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let buffer = '';
    const timer = setTimeout(() => child.kill('SIGTERM'), options.timeout * 60_000);
    child.stdout.on('data', (chunk) => {
      buffer += chunk;
      const parts = buffer.split('\n');
      buffer = parts.pop();
      lines.push(...parts);
    });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.on('error', (error) => { clearTimeout(timer); stderr = String(error); done(127); });
    child.on('close', (exit) => {
      clearTimeout(timer);
      if (buffer) lines.push(buffer);
      done(exit ?? 1);
    });
    child.stdin.end(brief);
  });

  const summary = summarise(lines);
  if (code !== 0) summary.ok = false;
  if (!summary.ok && !summary.error) summary.error = (stderr || `claude exited ${code}`).trim().slice(0, 500);
  mkdirSync(dirname(out), { recursive: true });
  if (summary.report) writeFileSync(`${out}.md`, summary.report.trimEnd() + '\n');
  const { report, ...record } = summary;
  const { verdict, repeated } = readReport(report, brief);
  writeFileSync(`${out}.json`, JSON.stringify({ brief: resolve(options.brief), model: options.model, ...record, verdict, repeated }, null, 2) + '\n');
  if (summary.ok && !verdict) process.stderr.write('review has no verdict line\n');
  if (repeated.length) process.stderr.write(`review repeated items handed to a person: ${repeated.join(', ')}\n`);
  const line = summary.ok
    ? `review: ${out}.md · ${verdict ?? 'no verdict'} · ${summary.turns} turns · ${summary.seconds}s · $${summary.costUsd.toFixed(2)}\n`
    : `review failed: ${summary.error} (${out}.json)\n`;
  (summary.ok ? process.stdout : process.stderr).write(line);
  return summary.ok ? 0 : 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href) {
  process.exitCode = await main();
}
