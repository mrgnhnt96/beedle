import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { deflateSync } from 'node:zlib';
import { audit, checkLinks, visibleText } from '../review/audit.mjs';

const WORDS = 'word '.repeat(120);
const HEAD = `<title>Tool for teams</title><meta name="viewport" content="width=device-width">
<meta name="description" content="A tool."><link rel="canonical" href="https://a.dev/">
<meta property="og:title" content="t"><meta property="og:description" content="d"><meta property="og:type" content="website">
<meta property="og:url" content="https://a.dev/"><meta property="og:image" content="https://a.dev/og.jpg">
<meta name="twitter:card" content="summary_large_image"><link rel="icon" href="/i.png">
<script type="application/ld+json">[{"@type":"WebSite"},{"@type":"Organization"}]</script>`;
const page = (body, head = HEAD) => `<!doctype html><html lang="en"><head>${head}</head><body>${body}</body></html>`;
const GOOD = {
  'index.html': page(`<h1>Tool</h1><p>${WORDS}</p>`),
  '404.html': page('<h1>Lost</h1>'),
  'robots.txt': 'User-agent: *\nAllow: /\nSitemap: https://a.dev/sitemap.xml\n',
  'sitemap.xml': '<urlset><url><loc>https://a.dev/</loc></url></urlset>',
};

function site(files) {
  const dir = mkdtempSync(join(tmpdir(), 'beedle-'));
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, path)), { recursive: true });
    writeFileSync(join(dir, path), text);
  }
  return dir;
}
const messages = (files, level) => audit(site(files)).findings.filter((f) => !level || f.level === level).map((f) => f.message);
const has = (list, pattern) => assert.ok(list.some((m) => pattern.test(m)), `no match for ${pattern} in ${JSON.stringify(list)}`);

test('a complete site has no findings', () => {
  assert.deepEqual(messages(GOOD), []);
});

test('privacy text that denies analytics fails when analytics load', () => {
  const script = '<script src="https://cdn.posthog.com/array.js"></script>';
  const list = messages({ ...GOOD, 'index.html': page(`<h1>Tool</h1><p>${WORDS}</p>${script}`), 'privacy/index.html': page('<h1>Privacy</h1><p>There is no analytics on this site.</p>') }, 'error');
  has(list, /no analytics.*PostHog/i);
});

test('"no tracking pixel" alone is not a denial of analytics', () => {
  const script = '<script>posthog.init()</script>';
  const list = messages({ ...GOOD, 'index.html': page(`<h1>Tool</h1>${script}<p>${WORDS}</p>`), 'privacy/index.html': page('<h1>Privacy</h1><p>We use PostHog analytics but no tracking pixel.</p>') }, 'error');
  assert.ok(!list.some((m) => /load$/.test(m)));
});

test('a statement about background location is not a denial of analytics', () => {
  const list = messages({ ...GOOD, 'index.html': page(`<h1>Tool</h1><script>posthog.init()</script><p>${WORDS}</p>`), 'privacy/index.html': page('<h1>Privacy</h1><p>We do not track you in the background. PostHog measures visits.</p>') }, 'error');
  assert.ok(!list.some((m) => /Says/.test(m)));
});

test('analytics without a privacy page is an error', () => {
  has(messages({ ...GOOD, 'index.html': page(`<h1>T</h1><p>${WORDS}</p><script src="https://x.amplitude.com/a.js"></script>`) }, 'error'), /no privacy page/);
});

test('flags a Flutter shell and thin HTML', () => {
  const list = messages({ ...GOOD, 'index.html': page('<script src="flutter_bootstrap.js"></script>') });
  has(list, /Flutter web/);
  has(list, /Expected one h1/);
  has(list, /words in the initial HTML/);
});

test('noindex pages skip page checks', () => {
  assert.deepEqual(messages({ ...GOOD, 'auth/index.html': '<html><head><meta name="robots" content="noindex"></head><body></body></html>' }), []);
});

test('excluded sections are skipped', () => {
  const dir = site({ ...GOOD, 'docs/index.html': '<html><body>docs</body></html>' });
  assert.equal(audit(dir, ['docs']).pages, 2);
});

test('robots that block a search crawler fail, and blocking a training crawler does not', () => {
  has(messages({ ...GOOD, 'robots.txt': 'User-agent: OAI-SearchBot\nDisallow: /\n\nUser-agent: *\nAllow: /\nSitemap: https://a.dev/s.xml\n' }, 'error'), /Blocks oai-searchbot/);
  assert.deepEqual(messages({ ...GOOD, 'robots.txt': 'User-agent: GPTBot\nDisallow: /\n\nUser-agent: *\nAllow: /\nSitemap: https://a.dev/sitemap.xml\n' }, 'error'), []);
});

test('flags missing alt, dead links, placeholders and bad JSON-LD', () => {
  const list = messages({ ...GOOD, 'index.html': page(`<h1>T</h1><img src="a.png"><a href="#">x</a><p>lorem ipsum ${WORDS}</p>`, `${HEAD}<script type="application/ld+json">{bad</script>`) }, 'error');
  has(list, /no alt/);
  has(list, /goes nowhere/);
  has(list, /Placeholder/);
  has(list, /does not parse/);
});

test('rating claims and bare VideoGame are caught', () => {
  const ld = '<script type="application/ld+json">{"@type":"VideoGame","aggregateRating":{"ratingValue":5}}</script>';
  const list = messages({ ...GOOD, 'index.html': page(`<h1>T</h1><p>${WORDS}</p>`, HEAD + ld) });
  has(list, /co-typed/);
  has(list, /ratings or reviews/);
});

test('visibleText ignores scripts and styles', () => {
  assert.equal(visibleText('<body><style>a{}</style><script>x</script><p>Hi &amp; bye</p></body>'), 'Hi & bye');
});

const SPOTTER = fileURLToPath(new URL('fixtures/spotter/site', import.meta.url));
const AUDIT = fileURLToPath(new URL('../review/audit.mjs', import.meta.url));

function chunk(name, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  return Buffer.concat([length, Buffer.from(name, 'latin1'), data, Buffer.alloc(4)]);
}

function png({ size = 120, type = 2, depth = 8, line, palette, filter = 0 }) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(size, 0);
  header.writeUInt32BE(size, 4);
  header.set([depth, type], 8);
  const rows = Array.from({ length: size }, (_, y) => Buffer.concat([Buffer.from([filter]), line(y)]));
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    ...(palette ? [chunk('PLTE', Buffer.from(palette))] : []),
    chunk('IDAT', deflateSync(Buffer.concat(rows))),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function jpeg(width, height, length) {
  const bytes = Buffer.alloc(length);
  bytes.set([0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08]);
  bytes.writeUInt16BE(height, 7);
  bytes.writeUInt16BE(width, 9);
  return bytes;
}

const fill = (size, ...pixel) => () => Buffer.concat(Array.from({ length: size }, () => Buffer.from(pixel)));
const noise = (size, step) => (y) => Buffer.from(Array.from({ length: size * step }, (_, x) => (x * 37 + y * 101) % 256));
const withImage = (name, bytes, html = `<img src="/${name}" alt="a" width="120" height="120">`) => ({
  ...GOOD,
  'index.html': page(`<h1>Tool</h1><p>${WORDS}</p>${html}`),
  [name]: bytes,
});
const blanks = (files, level) => messages(files, level).filter((m) => /looks blank/.test(m));

test('a flat PNG is blank, and the report names the colour', () => {
  has(blanks(withImage('a.png', png({ line: fill(120, 0x30, 0x40, 0x60) })), 'error'), /a\.png is one flat colour \(#304060\)/);
});

test('a flat 16-bit PNG with alpha is blank', () => {
  has(blanks(withImage('a.png', png({ type: 6, depth: 16, line: fill(120, 0x30, 0, 0x40, 0, 0x60, 0, 0xff, 0xff) })), 'error'), /#304060/);
});

test('a flat PNG stored with a row filter is blank', () => {
  const line = (y) => (y ? Buffer.alloc(360) : Buffer.concat([Buffer.from([0x30, 0x40, 0x60]), Buffer.alloc(357)]));
  has(blanks(withImage('a.png', png({ filter: 1, line: (y) => (y ? Buffer.concat([Buffer.from([0x30, 0x40, 0x60]), Buffer.alloc(357)]) : line(y)) })), 'error'), /#304060/);
});

test('a one-colour palette PNG is blank and a two-colour one is not', () => {
  const palette = [0x30, 0x40, 0x60, 0xff, 0xff, 0xff];
  has(blanks(withImage('a.png', png({ type: 3, depth: 1, palette, line: () => Buffer.alloc(15) })), 'error'), /#304060/);
  assert.deepEqual(blanks(withImage('a.png', png({ type: 3, depth: 1, palette, line: () => Buffer.alloc(15, 0b10101010) }))), []);
});

test('a PNG with real variation is not blank, whichever filter it uses', () => {
  for (const filter of [0, 1, 2, 3, 4]) assert.deepEqual(blanks(withImage('a.png', png({ filter, line: noise(120, 3) }))), []);
});

test('an image too small to be a screenshot is ignored', () => {
  assert.deepEqual(blanks(withImage('a.png', png({ size: 90, line: fill(90, 1, 2, 3) }))), []);
});

test('a JPEG with almost no bytes only warns, because a gradient looks the same', () => {
  has(blanks(withImage('a.jpg', jpeg(1600, 1000, 9_700)), 'warn'), /1600x1000 in 9700 bytes/);
  assert.deepEqual(blanks(withImage('a.jpg', jpeg(1600, 1000, 30_000))), []);
});

test('srcset candidates, picture sources and the local share image are all checked', () => {
  const flat = png({ line: fill(120, 9, 9, 9) });
  has(blanks(withImage('a.png', flat, '<img src="/real.svg" srcset="/a.png 1x" alt="a" width="1" height="1">')), /a\.png/);
  has(blanks(withImage('a.png', flat, '<picture><source srcset="/a.png 800w"><img src="/real.svg" alt="a" width="1" height="1"></picture>')), /a\.png/);
  const share = HEAD.replace('https://a.dev/og.jpg', 'https://a.dev/a.png');
  has(blanks({ ...GOOD, 'index.html': page(`<h1>Tool</h1><p>${WORDS}</p>`, share), 'a.png': flat }), /a\.png/);
});

test('the Spotter trial: both blank screenshots are errors and its real images pass', () => {
  const list = audit(SPOTTER).findings.filter((f) => f.level === 'error').map((f) => f.message);
  has(list, /menu-1\.2\.0-1600\.webp is 1600x1000 in 110 bytes/);
  has(list, /menu-1\.2\.0-800\.webp is 800x500 in 62 bytes/);
  assert.equal(list.length, 2);
});

function fakeFetch(answers) {
  const asked = [];
  const get = async (url) => {
    asked.push(url);
    const answer = answers[url] ?? 200;
    if (typeof answer === 'string') throw Object.assign(new TypeError('fetch failed'), { cause: { code: answer } });
    return { status: answer };
  };
  return { asked, get };
}

const linkSet = (urls, ownHosts = []) => ({ links: urls.map((url) => ({ url, pages: ['index.html'] })), ownHosts });
const check = (urls, answers, ownHosts) => checkLinks(linkSet(urls, ownHosts), { fetch: fakeFetch(answers).get });
const OK_LINK = 'https://ok.dev/';

test('the Spotter trial: dead GitHub links are errors and the unlaunched domain warns', async () => {
  const result = audit(SPOTTER);
  const repo = 'https://github.com/spotter-demo/spotter';
  const { get, asked } = fakeFetch({ [repo]: 404, [`${repo}/releases/latest`]: 404, [`${repo}/releases`]: 404, 'https://spotter-demo.dev/': 'ENOTFOUND' });
  const found = await checkLinks(result, { fetch: get });
  assert.deepEqual(found.filter((f) => f.level === 'error').map((f) => f.message).sort(), [
    `Dead link (404): ${repo}`, `Dead link (404): ${repo}/releases`, `Dead link (404): ${repo}/releases/latest`,
  ]);
  has(found.filter((f) => f.level === 'warn').map((f) => f.message), /own domain is not live \(domain does not resolve\)/);
  assert.equal(found.length, 4);
  assert.equal(asked.filter((url) => url.includes('spotter-demo.dev')).length, 1);
});

test('a gone page and a domain that does not exist are dead links', async () => {
  const found = await check(['https://a.dev/x', 'https://b.dev/', OK_LINK], { 'https://a.dev/x': 410, 'https://b.dev/': 'ENOTFOUND' });
  assert.deepEqual(found.map((f) => f.level), ['error', 'error']);
});

test('a network failure warns and a refused check is only a note', async () => {
  const found = await check(['https://a.dev/', 'https://b.dev/', 'https://c.dev/', OK_LINK], { 'https://a.dev/': 'ECONNRESET', 'https://b.dev/': 403, 'https://c.dev/': 503 });
  assert.deepEqual(found.map((f) => f.level), ['warn', 'info', 'info']);
});

test('a machine with no network gets one warning, not a dead link for every URL', async () => {
  const urls = ['https://a.dev/', 'https://b.dev/'];
  const found = await check(urls, Object.fromEntries(urls.map((url) => [url, 'ENOTFOUND'])));
  assert.equal(found.length, 1);
  assert.equal(found[0].level, 'warn');
  has([found[0].message], /offline/);
});

test('a site with only its own unlaunched domain is told that, not that it is offline', async () => {
  const found = await check([], { 'https://a.dev/': 'ENOTFOUND' }, ['a.dev']);
  has(found.map((f) => f.message), /own domain is not live/);
});

test('pages on the own domain are not requested, since they may not be deployed yet', async () => {
  const { get, asked } = fakeFetch({});
  await checkLinks(linkSet(['https://a.dev/pricing', OK_LINK], ['a.dev']), { fetch: get });
  assert.deepEqual(asked.sort(), ['https://a.dev/', OK_LINK]);
});

test('a page repeating a dead link is counted once, and localhost is left out', () => {
  const files = { ...GOOD, 'index.html': page(`<h1>Tool</h1><p>${WORDS}</p><a href="https://x.dev/a#top">a</a><a href="http://localhost:3000/">l</a><a href="mailto:a@a.dev">m</a>`), 'b/index.html': page('<h1>B</h1><a href="https://x.dev/a">a</a>') };
  const { links } = audit(site(files));
  const dead = links.filter(({ url }) => url.includes('x.dev'));
  assert.deepEqual(dead.map(({ url, pages }) => [url, pages.length]), [['https://x.dev/a', 2]]);
  assert.ok(!links.some(({ url }) => url.includes('localhost') || url.startsWith('mailto')));
});

test('structured data links are collected, and schema.org itself is not', () => {
  const ld = '<script type="application/ld+json">{"@context":"https://schema.org","@type":"WebSite","sameAs":["https://x.dev/me"]}</script>';
  const { links } = audit(site({ ...GOOD, 'index.html': page('<h1>T</h1>', HEAD + ld) }));
  assert.ok(links.some(({ url }) => url === 'https://x.dev/me'));
  assert.ok(!links.some(({ url }) => url.includes('schema.org')));
});

test('the real request reads the status from a live server', async (t) => {
  const server = createServer((request, response) => {
    response.statusCode = request.url === '/gone' ? 404 : 200;
    response.end('body');
  });
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  t.after(() => server.close());
  const base = `http://127.0.0.1:${server.address().port}`;
  const found = await checkLinks(linkSet([`${base}/gone`, `${base}/here`]));
  assert.deepEqual(found.map((f) => f.message), [`Dead link (404): ${base}/gone`]);
});

test('--offline makes no requests and still fails on the blank screenshots', () => {
  const run = spawnSync(process.execPath, [AUDIT, '--dir', SPOTTER, '--offline', '--json'], { encoding: 'utf8' });
  assert.equal(run.status, 1);
  assert.ok(JSON.parse(run.stdout).findings.every((f) => !/link/i.test(f.message)));
});
