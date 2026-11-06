#!/usr/bin/env node
// Check a built marketing site for what a script can decide. No dependencies.
//
//   node audit.mjs --dir <built site> [--exclude <path prefix>]... [--offline] [--json]
//
// Exits 1 if there are errors. Judgment (message, proof, what to cut) stays with the reviewer.
// External links are requested over the network unless --offline is given.

import { existsSync, readdirSync, readFileSync, realpathSync, statSync } from 'node:fs';
import { join, relative, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { gzipSync, inflateSync } from 'node:zlib';

const ANALYTICS = {
  PostHog: /posthog/i, Amplitude: /amplitude/i, 'Google Analytics': /googletagmanager\.com|gtag\(|google-analytics\.com/i,
  Plausible: /plausible\.io/i, Umami: /umami/i, Fathom: /usefathom/i, 'Meta Pixel': /fbq\(|connect\.facebook\.net/i,
  Clarity: /clarity\.ms/i, Segment: /segment\.(com|io)\/analytics|analytics\.js/i, Mixpanel: /mixpanel/i,
};
const NO_TRACKING = /\b(no analytics|no tracking(?! pixel)|no trackers|not use (any )?(analytics|tracking|cookies)|(don['’]t|do not) use (analytics|cookies)|(don['’]t|do not) track (you|visitors|anyone)( on| across)? (this|our) (site|website))\b/i;
const PLACEHOLDERS = /lorem ipsum|\bTODO\b|\bTBD\b|example\.com|your-domain|yourcompany|_TODO\b/;
const COPY_TELLS = [/it['’]s not just\b/i, /\bdelve\b/i, /\bpivotal\b/i, /\bseamless(ly)?\b/i, /\bunlock\b/i, /\belevate\b/i, /\bgame[- ]chang/i, /\bserves as\b/i, /\brevolutioni[sz]e/i, /\bsupercharge/i];
const SEARCH_BOTS = ['googlebot', 'bingbot', 'oai-searchbot', 'claude-searchbot', 'claude-user', 'perplexitybot'];
const JS_BUDGET = 50 * 1024;
const BLANK_MIN_PIXELS = 10_000;
const PNG_FLAT_RANGE = 2;
const PNG_SAMPLES = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 };
// Measured at 1600x1000: a flat fill at high quality lands under these, one line of text on white over them.
const BLANK_BYTES_PER_PIXEL = { jpeg: 0.014, webp: 0.0025, 'webp-lossless': 0.0005, avif: 0.0005, gif: 0.0005, png: 0.0005 };
// Size alone can't tell a flat fill from a soft gradient in these, so they only warn.
const LOSSY_FORMATS = new Set(['jpeg', 'webp', 'avif']);
const LINK_TIMEOUT = 10_000;
const LINK_CONCURRENCY = 8;
const LINK_USER_AGENT = 'Mozilla/5.0 (compatible; beedle-audit)';

const attr = (tag, name) => tag.match(new RegExp(`\\b${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`, 'i'))?.slice(1).find((v) => v !== undefined);
const tags = (html, name) => html.match(new RegExp(`<${name}\\b[^>]*>`, 'gi')) ?? [];

function meta(html, key) {
  const tag = tags(html, 'meta').find((t) => [attr(t, 'name'), attr(t, 'property')].some((v) => v?.toLowerCase() === key));
  return tag ? attr(tag, 'content') : undefined;
}

function link(html, rel) {
  const tag = tags(html, 'link').find((t) => attr(t, 'rel')?.toLowerCase().split(/\s+/).includes(rel));
  return tag ? attr(tag, 'href') : undefined;
}

export function visibleText(html) {
  return (html.match(/<body\b[\s\S]*$/i)?.[0] ?? html)
    .replace(/<(script|style|noscript|svg|template)\b[\s\S]*?<\/\1>/gi, ' ')
    .replace(/<!--[\s\S]*?-->/g, ' ')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/g, ' ').replace(/&amp;/g, '&').replace(/&#39;|&apos;|&#x27;/g, "'").replace(/&quot;/g, '"')
    .replace(/\s+/g, ' ').trim();
}

function jsonLd(html) {
  const blocks = html.match(/<script\b[^>]*application\/ld\+json[^>]*>[\s\S]*?<\/script>/gi) ?? [];
  const items = [];
  let invalid = 0;
  for (const block of blocks) {
    try {
      const data = JSON.parse(block.replace(/^<script\b[^>]*>/i, '').replace(/<\/script>$/i, ''));
      const list = Array.isArray(data) ? data : data['@graph'] ?? [data];
      items.push(...list);
    } catch { invalid++; }
  }
  return { items, invalid };
}

const typesOf = (item) => [item['@type']].flat().filter(Boolean);

function pngSize(path) {
  const bytes = readFileSync(path);
  return bytes.toString('latin1', 1, 4) === 'PNG' ? [bytes.readUInt32BE(16), bytes.readUInt32BE(20)] : null;
}

function jpegSize(bytes) {
  for (let at = 2; at + 9 < bytes.length; at += 2 + bytes.readUInt16BE(at + 2)) {
    const marker = bytes[at + 1];
    if (bytes[at] !== 0xff) return null;
    if (marker >= 0xc0 && marker <= 0xcf && ![0xc4, 0xc8, 0xcc].includes(marker)) {
      return { format: 'jpeg', width: bytes.readUInt16BE(at + 7), height: bytes.readUInt16BE(at + 5) };
    }
  }
  return null;
}

function webpSize(bytes) {
  const chunk = bytes.toString('latin1', 12, 16);
  if (chunk === 'VP8 ') return { format: 'webp', width: bytes.readUInt16LE(26) & 0x3fff, height: bytes.readUInt16LE(28) & 0x3fff };
  if (chunk === 'VP8X') return { format: 'webp', width: bytes.readUIntLE(24, 3) + 1, height: bytes.readUIntLE(27, 3) + 1 };
  if (chunk !== 'VP8L') return null;
  const bits = bytes.readUInt32LE(21);
  return { format: 'webp-lossless', width: (bits & 0x3fff) + 1, height: ((bits >>> 14) & 0x3fff) + 1 };
}

function imageInfo(bytes) {
  const text = (from, to) => bytes.toString('latin1', from, to);
  if (text(1, 4) === 'PNG') return { format: 'png', width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) };
  if (text(0, 4) === 'GIF8') return { format: 'gif', width: bytes.readUInt16LE(6), height: bytes.readUInt16LE(8) };
  if (text(0, 4) === 'RIFF' && text(8, 12) === 'WEBP') return webpSize(bytes);
  if (bytes[0] === 0xff && bytes[1] === 0xd8) return jpegSize(bytes);
  const ispe = bytes.indexOf('ispe');
  if (text(4, 8) === 'ftyp' && ispe > 0) return { format: 'avif', width: bytes.readUInt32BE(ispe + 8), height: bytes.readUInt32BE(ispe + 12) };
  return null;
}

function pngChunks(bytes) {
  const chunks = { IDAT: [] };
  for (let at = 8; at + 8 <= bytes.length; at += 12 + bytes.readUInt32BE(at)) {
    const name = bytes.toString('latin1', at + 4, at + 8);
    const data = bytes.subarray(at + 8, at + 8 + bytes.readUInt32BE(at));
    if (name === 'IDAT') chunks.IDAT.push(data);
    else chunks[name] = data;
  }
  return chunks;
}

function unfilter(row, previous, filter, step) {
  const line = Buffer.alloc(row.length);
  for (let i = 0; i < row.length; i++) {
    const left = i >= step ? line[i - step] : 0;
    const up = previous[i];
    const corner = i >= step ? previous[i - step] : 0;
    const guess = left + up - corner;
    const [dl, du, dc] = [left, up, corner].map((v) => Math.abs(guess - v));
    const paeth = dl <= du && dl <= dc ? left : du <= dc ? up : corner;
    const predicted = [0, left, up, (left + up) >> 1, paeth][filter];
    line[i] = row[i] + predicted;
  }
  return line;
}

function pngPixel(line, x, { type, depth, samples, palette, alpha }) {
  const raw = (i) => {
    if (depth === 16) return line[i * 2];
    if (depth === 8) return line[i];
    return (line[(i * depth) >> 3] >> (8 - depth - ((i * depth) % 8))) & ((1 << depth) - 1);
  };
  const value = (i) => (depth < 8 ? Math.round((raw(i) * 255) / ((1 << depth) - 1)) : raw(i));
  const at = x * samples;
  if (type === 3) return [...palette.subarray(raw(at) * 3, raw(at) * 3 + 3), alpha?.[raw(at)] ?? 255];
  if (type === 0) return [value(at), value(at), value(at), 255];
  if (type === 4) return [value(at), value(at), value(at), value(at + 1)];
  return [value(at), value(at + 1), value(at + 2), type === 6 ? value(at + 3) : 255];
}

function flatPngColour(bytes) {
  const [depth, type] = [bytes[24], bytes[25]];
  const samples = PNG_SAMPLES[type];
  const { width, height } = imageInfo(bytes);
  const chunks = pngChunks(bytes);
  const format = { type, depth, samples, palette: chunks.PLTE, alpha: chunks.tRNS };
  const pixels = inflateSync(Buffer.concat(chunks.IDAT));
  const stride = Math.ceil((width * samples * depth) / 8);
  const step = Math.max(1, (samples * depth) >> 3);
  const low = [255, 255, 255, 255];
  const high = [0, 0, 0, 0];
  let previous = Buffer.alloc(stride);
  for (let y = 0; y < height; y++) {
    const start = y * (stride + 1);
    previous = unfilter(pixels.subarray(start + 1, start + 1 + stride), previous, pixels[start], step);
    for (let x = 0; x < width; x++) {
      const pixel = pngPixel(previous, x, format);
      for (let c = 0; c < 4; c++) {
        low[c] = Math.min(low[c], pixel[c]);
        high[c] = Math.max(high[c], pixel[c]);
        if (high[c] - low[c] > PNG_FLAT_RANGE) return null;
      }
    }
  }
  return `#${low.slice(0, 3).map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}

function blankImage(file) {
  const bytes = readFileSync(file);
  try {
    const info = imageInfo(bytes);
    if (!info || info.width * info.height < BLANK_MIN_PIXELS) return null;
    const interlaced = bytes[28] !== 0;
    if (info.format === 'png' && !interlaced) {
      const colour = flatPngColour(bytes);
      return colour && { level: 'error', detail: `is one flat colour (${colour})` };
    }
    if (bytes.length / (info.width * info.height) >= BLANK_BYTES_PER_PIXEL[info.format]) return null;
    const level = LOSSY_FORMATS.has(info.format) ? 'warn' : 'error';
    return { level, detail: `is ${info.width}x${info.height} in ${bytes.length} bytes, too little to hold a picture` };
  } catch {
    return null;
  }
}

function imageSources(html, shareImage) {
  const candidates = (tag) => (attr(tag, 'srcset') ?? '').split(',').map((part) => part.trim().split(/\s+/)[0]).filter(Boolean);
  const inPage = [...tags(html, 'img'), ...tags(html, 'source')].flatMap((tag) => [attr(tag, 'src'), ...candidates(tag)]);
  return [...new Set([...inPage, shareImage].filter((src) => src && !src.startsWith('data:')))];
}

function localPath(dir, url) {
  if (!url) return null;
  let path = url;
  try { path = new URL(url).pathname; } catch {}
  const file = join(dir, decodeURIComponent(path.split(/[?#]/)[0]));
  return existsSync(file) && statSync(file).isFile() ? file : null;
}

function walk(dir, root = dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return walk(path, root);
    return entry.name.endsWith('.html') ? [relative(root, path).split(sep).join('/')] : [];
  });
}

function auditStructuredData(page, html, isHome, add) {
  const { items, invalid } = jsonLd(html);
  if (invalid) add('error', page, 'JSON-LD does not parse');
  const types = items.flatMap(typesOf);
  for (const item of items) {
    const itemTypes = typesOf(item);
    if (itemTypes.includes('VideoGame') && itemTypes.length === 1) add('error', page, 'VideoGame must be co-typed, such as ["VideoGame","MobileApplication"]');
    if (itemTypes.some((t) => /Business|Store|Service$/.test(t)) && !item.address && !item.areaServed) add('error', page, `${itemTypes[0]} has no address or areaServed`);
    if (item.aggregateRating || item.review) add('warn', page, 'Structured data claims ratings or reviews: confirm they are real');
    if (itemTypes.some((t) => /Application|VideoGame/.test(t)) && !item.offers) add('warn', page, `${itemTypes[0]} has no offers`);
  }
  for (const type of types.filter((t) => t === 'FAQPage' || t === 'HowTo')) add('info', page, `${type} no longer earns a rich result`);
  if (isHome) {
    if (!types.includes('WebSite')) add('warn', page, 'Home page has no WebSite JSON-LD (sets the site name in search)');
    if (!types.some((t) => t === 'Organization' || t === 'Person' || /Business/.test(t))) add('warn', page, 'Home page has no Organization, Person or LocalBusiness JSON-LD');
  }
}

function auditPage(dir, page, html, add) {
  const head = html.match(/<head\b[\s\S]*?<\/head>/i)?.[0] ?? '';
  const text = visibleText(html);
  const isHome = page === 'index.html';

  if (!/<html\b[^>]*\blang=/i.test(html)) add('warn', page, 'No lang attribute on <html>');
  if (!meta(head, 'viewport')) add('error', page, 'No viewport meta tag');
  if (!/<title\b[^>]*>[^<]*\S/i.test(head)) add('error', page, 'No title');
  if (!meta(head, 'description')) add('error', page, 'No meta description');

  const canonical = link(head, 'canonical');
  if (!canonical) add('error', page, 'No canonical link in the initial HTML');
  else if (!/^https:\/\//.test(canonical)) add('error', page, `Canonical is not absolute https: ${canonical}`);

  const h1s = tags(html, 'h1').length;
  if (h1s !== 1) add('error', page, `Expected one h1, found ${h1s}`);

  if (/flutter_bootstrap\.js|main\.dart\.js/.test(html)) add('error', page, 'Flutter web page: marketing content must be static HTML');
  const words = text ? text.split(' ').length : 0;
  if (isHome && words < 100) add('warn', page, `Only ${words} words in the initial HTML: is the content client-rendered?`);

  for (const key of ['og:title', 'og:description', 'og:type', 'og:url', 'og:image']) {
    if (!meta(head, key)) add('error', page, `No ${key}`);
  }
  const card = meta(head, 'twitter:card');
  if (card !== 'summary_large_image') add('warn', page, `twitter:card is ${card ?? 'missing'}, expected summary_large_image`);

  const image = meta(head, 'og:image');
  if (image && !/^https?:\/\//.test(image)) add('error', page, 'og:image must be an absolute URL');
  const imageFile = localPath(dir, image);
  if (imageFile?.endsWith('.png')) {
    const size = pngSize(imageFile);
    if (size && (size[0] < 1200 || Math.abs(size[0] / size[1] - 1.91) > 0.1)) add('warn', page, `og:image is ${size.join('x')}, expected 1200x630`);
  }

  if (!link(head, 'icon') && !link(head, 'shortcut') && !existsSync(join(dir, 'favicon.ico'))) add('warn', page, 'No favicon');

  const images = tags(html, 'img');
  images.forEach((img, index) => {
    const name = attr(img, 'src') ?? 'image';
    if (attr(img, 'alt') === undefined) add('error', page, `Image has no alt: ${name}`);
    if (!attr(img, 'width') || !attr(img, 'height')) add('warn', page, `Image has no width and height: ${name}`);
    if (index === 0 && attr(img, 'loading') === 'lazy') add('warn', page, `First image is lazy-loaded and may be the LCP element: ${name}`);
  });

  for (const src of imageSources(html, image)) {
    const file = localPath(dir, new URL(src, `https://site.invalid/${page}`).pathname);
    const blank = file && blankImage(file);
    if (blank) add(blank.level, page, `Image looks blank: ${src} ${blank.detail}`);
  }

  for (const anchor of tags(html, 'a')) {
    if (attr(anchor, 'href') === '#') add('error', page, 'Link goes nowhere (href="#")');
  }

  let js = 0;
  for (const script of tags(html, 'script')) {
    const file = localPath(dir, attr(script, 'src'));
    if (file) js += gzipSync(readFileSync(file)).length;
  }
  if (js > JS_BUDGET) add('warn', page, `${Math.round(js / 1024)}KB of gzipped JS is over the 50KB starting budget`);

  if (PLACEHOLDERS.test(text) || PLACEHOLDERS.test(head)) add('error', page, `Placeholder text: ${(text.match(PLACEHOLDERS) ?? head.match(PLACEHOLDERS))[0]}`);
  const tells = COPY_TELLS.map((re) => text.match(re)?.[0]).filter(Boolean);
  if (tells.length) add('info', page, `Generated-sounding wording: ${tells.join(', ')}`);

  auditStructuredData(page, html, isHome, add);
}

function auditPrivacy(pages, add) {
  const found = new Map();
  for (const [page, html] of pages) {
    for (const [name, pattern] of Object.entries(ANALYTICS)) {
      const scripts = (html.match(/<script\b[\s\S]*?<\/script>/gi) ?? []).join('\n');
      if (pattern.test(scripts)) found.set(name, [...(found.get(name) ?? []), page]);
    }
  }
  const privacy = [...pages.keys()].filter((page) => /privacy/i.test(page));
  const names = [...found.keys()];
  if (names.length && !privacy.length) add('error', 'site', `${names.join(' and ')} load but there is no privacy page`);
  for (const page of privacy) {
    const text = visibleText(pages.get(page));
    const claim = text.match(NO_TRACKING);
    const quote = claim && text.slice(Math.max(0, claim.index - 50), claim.index + claim[0].length + 50);
    if (names.length && claim) add('error', page, `Says "...${quote}..." but ${names.join(' and ')} load`);
    else if (names.length && !names.some((n) => text.toLowerCase().includes(n.toLowerCase())) && !/analytics/i.test(text)) add('warn', page, `Does not mention ${names.join(' or ')}`);
  }
  return names;
}

function auditRobots(dir, add) {
  const file = join(dir, 'robots.txt');
  if (!existsSync(file)) return add('warn', 'robots.txt', 'Missing');
  const text = readFileSync(file, 'utf8');
  if (!/^sitemap:/im.test(text)) add('warn', 'robots.txt', 'No Sitemap line');
  const groups = [];
  let group = null;
  for (const raw of text.split('\n')) {
    const line = raw.replace(/#.*/, '').trim();
    const [key, ...rest] = line.split(':');
    const value = rest.join(':').trim();
    if (/^user-agent$/i.test(key)) {
      if (!group || group.rules.length) groups.push(group = { agents: [], rules: [] });
      group.agents.push(value.toLowerCase());
    } else if (group && /^(dis)?allow$/i.test(key)) group.rules.push(`${key.toLowerCase()}:${value}`);
  }
  const blocks = (rules) => rules.includes('disallow:/') || rules.includes('disallow:/*');
  const wildcard = groups.find((g) => g.agents.includes('*'));
  for (const bot of SEARCH_BOTS) {
    const rules = (groups.find((g) => g.agents.includes(bot)) ?? wildcard)?.rules ?? [];
    if (blocks(rules)) add('error', 'robots.txt', `Blocks ${bot}`);
  }
}

function auditSitemap(dir, indexable, add) {
  const file = join(dir, 'sitemap.xml');
  const index = join(dir, 'sitemap-index.xml');
  if (!existsSync(file) && !existsSync(index)) return add('warn', 'sitemap.xml', 'Missing');
  if (!existsSync(file)) return;
  const text = readFileSync(file, 'utf8');
  if (/<(priority|changefreq)>/.test(text)) add('info', 'sitemap.xml', 'Google ignores priority and changefreq');
  const urls = (text.match(/<loc>[^<]*<\/loc>/g) ?? []).map((l) => l.replace(/<\/?loc>/g, '').trim().replace(/\/$/, ''));
  for (const [page, canonical] of indexable) {
    if (canonical && !urls.includes(canonical.replace(/\/$/, ''))) add('warn', 'sitemap.xml', `Does not list ${page}`);
  }
}

function urlsIn(value, key = '') {
  if (typeof value === 'string') return key !== '@context' && /^https?:\/\//i.test(value) ? [value] : [];
  if (Array.isArray(value)) return value.flatMap((item) => urlsIn(item, key));
  if (value && typeof value === 'object') return Object.entries(value).flatMap(([name, item]) => urlsIn(item, name));
  return [];
}

function collectLinks(pages) {
  const found = new Map();
  const ownHosts = new Set();
  for (const [page, html] of pages) {
    const head = html.match(/<head\b[\s\S]*?<\/head>/i)?.[0] ?? '';
    const canonical = link(head, 'canonical');
    if (/^https?:\/\//i.test(canonical ?? '')) ownHosts.add(new URL(canonical).hostname);
    const urls = [...tags(html, 'a').map((a) => attr(a, 'href')), canonical, meta(head, 'og:url'), meta(head, 'og:image'), ...urlsIn(jsonLd(html).items)];
    for (const url of urls) {
      if (!/^https?:\/\//i.test(url ?? '') || /^https?:\/\/((www\.)?schema\.org|localhost|127\.0\.0\.1)\b/i.test(url)) continue;
      const clean = url.replace(/&amp;/g, '&').split('#')[0];
      found.set(clean, [...new Set([...(found.get(clean) ?? []), page])]);
    }
  }
  return { links: [...found].map(([url, onPages]) => ({ url, pages: onPages })), ownHosts: [...ownHosts] };
}

async function request(get, url, timeout) {
  try {
    const response = await get(url, { redirect: 'follow', headers: { 'user-agent': LINK_USER_AGENT }, signal: AbortSignal.timeout(timeout) });
    await response.body?.cancel();
    return { status: response.status };
  } catch (error) {
    return { failure: error.cause?.code ?? error.code ?? error.name };
  }
}

function linkFinding({ url, pages, own }, { status, failure }) {
  const where = pages.length > 1 ? `${pages[0]} (+${pages.length - 1} more)` : pages[0];
  if (status >= 200 && status < 400) return null;
  const reason = failure === 'ENOTFOUND' ? 'domain does not resolve' : failure ?? status;
  const dead = status === 404 || status === 410 || failure === 'ENOTFOUND';
  if (own && (dead || failure)) return { level: 'warn', where: 'site', message: `The site's own domain is not live (${reason}): ${url}` };
  if (dead) return { level: 'error', where, message: `Dead link (${reason}): ${url}` };
  if (failure) return { level: 'warn', where, message: `Link did not answer (${reason}): ${url}` };
  return { level: 'info', where, message: `Could not confirm link, the server refused the check (${reason}): ${url}` };
}

// Own-domain pages may not be deployed yet, so that host is asked only whether it is live.
export async function checkLinks({ links, ownHosts }, { fetch: get = globalThis.fetch, timeout = LINK_TIMEOUT, concurrency = LINK_CONCURRENCY } = {}) {
  const isOwn = (url) => ownHosts.includes(new URL(url).hostname);
  const targets = [
    ...links.filter(({ url }) => !isOwn(url)),
    ...ownHosts.map((host) => ({ url: `https://${host}/`, pages: ['site'], own: true })),
  ];
  const outcomes = new Array(targets.length);
  let next = 0;
  const worker = async () => {
    while (next < targets.length) {
      const index = next++;
      outcomes[index] = await request(get, targets[index].url, timeout);
    }
  };
  await Promise.all(Array.from({ length: Math.min(concurrency, targets.length) }, worker));
  const hasExternal = targets.some((target) => !target.own);
  if (hasExternal && outcomes.every((outcome) => outcome.failure)) {
    return [{ level: 'warn', where: 'site', message: `None of ${targets.length} external links answered, so none were checked: is this machine offline?` }];
  }
  return targets.map((target, index) => linkFinding(target, outcomes[index])).filter(Boolean);
}

const LEVEL_RANK = { error: 0, warn: 1, info: 2 };
const byLevel = (a, b) => LEVEL_RANK[a.level] - LEVEL_RANK[b.level];

export function audit(dir, exclude = []) {
  const findings = [];
  const add = (level, where, message) => {
    if (!findings.some((f) => f.level === level && f.where === where && f.message === message)) findings.push({ level, where, message });
  };
  const skipped = (page) => exclude.some((prefix) => page.startsWith(prefix.replace(/^\/|\/$/g, '') + '/'));
  const pages = new Map(walk(dir).filter((p) => !skipped(p)).map((page) => [page, readFileSync(join(dir, page), 'utf8')]));
  if (!pages.size) add('error', 'site', `No HTML files in ${dir}`);
  if (!pages.has('index.html')) add('error', 'site', 'No index.html');

  const indexable = new Map();
  const seen = { title: new Map(), description: new Map() };
  for (const [page, html] of pages) {
    const head = html.match(/<head\b[\s\S]*?<\/head>/i)?.[0] ?? '';
    const noindex = /noindex/i.test(meta(head, 'robots') ?? '');
    const redirect = tags(head, 'meta').some((t) => attr(t, 'http-equiv')?.toLowerCase() === 'refresh');
    if (noindex || redirect || /^(404|500)(\/index)?\.html$/.test(page)) continue;
    auditPage(dir, page, html, add);
    indexable.set(page, link(head, 'canonical'));
    const title = head.match(/<title\b[^>]*>([^<]*)/i)?.[1].trim();
    for (const [key, value] of [['title', title], ['description', meta(head, 'description')]]) {
      if (!value) continue;
      if (seen[key].has(value)) add('warn', page, `Same ${key} as ${seen[key].get(value)}`);
      else seen[key].set(value, page);
    }
  }
  if (!pages.has('404.html') && !pages.has('404/index.html')) add('warn', 'site', 'No 404 page');

  const analytics = auditPrivacy(pages, add);
  auditRobots(dir, add);
  auditSitemap(dir, indexable, add);
  findings.sort(byLevel);
  return { pages: pages.size, indexable: indexable.size, analytics, findings, ...collectLinks(pages) };
}

export async function main(argv = process.argv.slice(2)) {
  let dir;
  let json = false;
  let offline = false;
  const exclude = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--dir') dir = argv[++i];
    else if (argv[i] === '--exclude') exclude.push(argv[++i]);
    else if (argv[i] === '--json') json = true;
    else if (argv[i] === '--offline') offline = true;
    else { process.stderr.write(`unknown option ${argv[i]}\n`); return 2; }
  }
  if (!dir || !existsSync(dir)) {
    process.stderr.write('usage: audit.mjs --dir <built site> [--exclude <prefix>]... [--offline] [--json]\n');
    return 2;
  }
  const result = audit(resolve(dir), exclude);
  if (!offline) {
    result.findings.push(...await checkLinks(result));
    result.findings.sort(byLevel);
  }
  if (json) process.stdout.write(JSON.stringify(result, null, 2) + '\n');
  else {
    for (const f of result.findings) process.stdout.write(`${f.level.padEnd(5)}  ${f.where}: ${f.message}\n`);
    const count = (level) => result.findings.filter((f) => f.level === level).length;
    const links = offline ? 'external links not checked' : `${result.links.length} external links checked`;
    process.stdout.write(`\n${result.pages} pages, ${result.indexable} indexable, analytics: ${result.analytics.join(', ') || 'none'}, ${links}\n`);
    process.stdout.write(`${count('error')} errors, ${count('warn')} warnings, ${count('info')} notes\n`);
  }
  return result.findings.some((f) => f.level === 'error') ? 1 : 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href) {
  process.exitCode = await main();
}
