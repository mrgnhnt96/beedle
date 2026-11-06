import assert from 'node:assert/strict';
import { test } from 'node:test';
import { claudeArgs, defaultOut, handedOver, readReport, summarise, systemPrompt } from '../review/run.mjs';

test('the reviewer gets the skill, checklist and rules', () => {
  const prompt = systemPrompt();
  for (const text of ['review/SKILL.md', 'review/checklist.md', 'reference/seo.md', 'reference/message.md']) assert.ok(prompt.includes(text));
});

test('the reviewer starts with no settings, MCP servers or session', () => {
  const args = claudeArgs({ system: 's', model: 'opus', addDirs: ['/p'] });
  for (const flag of ['--setting-sources', '--strict-mcp-config', '--no-session-persistence']) assert.ok(args.includes(flag));
  assert.equal(args[args.indexOf('--tools') + 1], 'Read,Glob,Grep,Bash');
  assert.ok(args.includes('/p'));
});

test('the report comes from the result event', () => {
  const summary = summarise([JSON.stringify({ type: 'result', subtype: 'success', result: 'report', num_turns: 2, total_cost_usd: 1.5 })]);
  assert.deepEqual([summary.ok, summary.report, summary.turns, summary.costUsd], [true, 'report', 2, 1.5]);
});

test('the output sits beside the brief', () => {
  assert.equal(defaultOut('/r/brief-2.md'), '/r/review-2');
});

test('the reviewer is told it never fixes anything', () => {
  const prompt = systemPrompt();
  for (const text of ['This skill never fixes anything', '**Suggested fix:**', "Verdict: Can't ship until a person acts", 'Handed to a person']) assert.ok(prompt.includes(text), text);
  assert.ok(!prompt.includes('**Fix:**'));
});

const BRIEF = `Scope: the whole site
Handed to a person: B1 · Blocker · Dead links · Owner: publish it
B4 · Major · Domain · Owner: buy it

Return the skill's report. B9 · is not here.`;

test('every handed-over item is read from the brief', () => {
  assert.deepEqual(handedOver(BRIEF), ['B1', 'B4']);
  assert.deepEqual(handedOver('Handed to a person: none\n\nReturn'), []);
  assert.deepEqual(handedOver('Scope: all\n'), []);
});

test('the verdict is read from the first line, whatever the emphasis or apostrophe', () => {
  const read = (report) => readReport(report, '').verdict;
  assert.equal(read('Verdict: Ready to ship\n\nText'), 'Ready to ship');
  assert.equal(read('**Verdict: Fix first**'), 'Fix first');
  assert.equal(read('**Verdict:** Can’t ship until a person acts'), "Can't ship until a person acts");
  assert.equal(read('No verdict here'), null);
});

test('a handed-over item reported as a finding counts as repeated', () => {
  const report = "Verdict: Fix first\n\n### B1 · Blocker · Dead links\n\n### B15 · Minor · New\n\n## Waiting on a person\n- B4 · Major · Domain · Owner: buy it\n";
  assert.deepEqual(readReport(report, BRIEF).repeated, ['B1']);
});

test('the waiting list on its own is not a repeat', () => {
  const report = "Verdict: Can't ship until a person acts\n\n## Waiting on a person\n- B1 · Blocker · Dead links · Owner: publish it\n";
  assert.deepEqual(readReport(report, BRIEF).repeated, []);
});
