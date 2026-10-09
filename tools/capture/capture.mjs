// Drives VS Code (code-server) through an rbview demo, saving stills and an animated GIF.
//
//   node capture.mjs --url http://127.0.0.1:8737 --workspace <dir> --out <dir> [--publish <dir>]
//
// Browser: set CHROME_PATH to any Chromium/Chrome binary, or install the optional
// @sparticuz/chromium package. GIF assembly needs ffmpeg on PATH.
import puppeteer from 'puppeteer-core';
import { execFileSync } from 'child_process';
import fs from 'fs';
import path from 'path';

const args = process.argv.slice(2);
const opt = (k, d) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : d; };
const URL_BASE = opt('--url', 'http://127.0.0.1:8737');
const WS = path.resolve(opt('--workspace', 'workspace'));
const OUT = path.resolve(opt('--out', 'out'));
const PUBLISH = opt('--publish', null);
const W = 1440, H = 860;

fs.rmSync(OUT, { recursive: true, force: true });
fs.mkdirSync(`${OUT}/frames`, { recursive: true });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(new Date().toISOString().slice(11, 19), ...a);

async function browserBinary() {
  if (process.env.CHROME_PATH) return { executablePath: process.env.CHROME_PATH, args: [] };
  try {
    const { default: chromium } = await import('@sparticuz/chromium');
    return { executablePath: await chromium.executablePath(), args: chromium.args };
  } catch {
    throw new Error('No browser: set CHROME_PATH or `npm i @sparticuz/chromium`');
  }
}
const bin = await browserBinary();
const browser = await puppeteer.launch({
  executablePath: bin.executablePath,
  args: [...bin.args, '--no-sandbox', `--window-size=${W},${H}`],
  headless: 'shell',
  defaultViewport: { width: W, height: H },
});
const page = await browser.newPage();
page.on('pageerror', (e) => log('pageerror', e.message.slice(0, 100)));

// ---------------------------------------------------------------- frames
const frames = []; // { file, ms }
let n = 0;
async function frame(ms, still) {
  const file = `${OUT}/frames/${String(n++).padStart(3, '0')}.png`;
  await page.screenshot({ path: file });
  frames.push({ file, ms });
  if (still) fs.copyFileSync(file, `${OUT}/${still}.png`);
}

// ---------------------------------------------------------------- mouse with a visible pointer
// (headless screenshots don't draw the OS cursor)
let mx = W / 2, my = H / 2;
async function installPointer() {
  await page.evaluate(() => {
    const d = document.createElement('div');
    d.innerHTML = '<svg width="22" height="28" viewBox="0 0 22 28"><path d="M2 2 L2 22 L7.5 17 L11.5 26 L15 24.5 L11 15.5 L18.5 15.5 Z" fill="white" stroke="black" stroke-width="1.6" stroke-linejoin="round"/></svg>';
    Object.assign(d.style, { position: 'fixed', left: '0', top: '0', zIndex: 2147483647, pointerEvents: 'none', transform: 'translate(-2px,-2px)', filter: 'drop-shadow(0 1px 2px rgba(0,0,0,.6))' });
    document.body.appendChild(d);
    document.addEventListener('mousemove', (e) => { d.style.left = e.clientX + 'px'; d.style.top = e.clientY + 'px'; }, true);
  });
}
async function moveTo(x, y, { frames: f = 8, ms = 60 } = {}) {
  for (let i = 1; i <= f; i++) {
    const t = i / f, e = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
    await page.mouse.move(mx + (x - mx) * e, my + (y - my) * e);
    await frame(ms);
  }
  mx = x; my = y;
}

// ---------------------------------------------------------------- VS Code helpers
async function key(combo) {
  const parts = combo.split('+');
  for (const p of parts.slice(0, -1)) await page.keyboard.down(p);
  await page.keyboard.press(parts.at(-1));
  for (const p of parts.slice(0, -1).reverse()) await page.keyboard.up(p);
}
async function typeSlow(s, everyN = 4) {
  for (let i = 0; i < s.length; i++) {
    await page.keyboard.type(s[i]);
    if (i % everyN === everyN - 1) await frame(70);
  }
}
async function waitFor(fn, arg, timeout = 30000) {
  try {
    return await page.waitForFunction(fn, { timeout, polling: 200 }, arg);
  } catch (e) {
    await page.screenshot({ path: `${OUT}/fail.png` });
    throw new Error(`${e.message} (see ${OUT}/fail.png)`);
  }
}
/** Any editor line (in the active group, optionally) matching a regex. */
function lineMatches(source, activeOnly = false) {
  return waitFor(({ source, activeOnly }) => {
    const re = new RegExp(source);
    const sel = activeOnly ? '.editor-group-container.active .view-line' : '.editor-group-container .view-line';
    return [...document.querySelectorAll(sel)].some((l) => re.test(l.textContent.replace(/\u00a0/g, ' ')));
  }, { source, activeOnly });
}
async function waitRustAnalyzer(timeout = 300000) {
  await waitFor(() => [...document.querySelectorAll('.statusbar-item')].some((e) => /rust-analyzer/.test(e.textContent) && e.querySelector('.codicon-check')), null, timeout);
}
async function quickOpen(name) {
  await key('Control+KeyP');
  await page.waitForSelector('.quick-input-widget input', { visible: true });
  await typeSlow(name);
  await sleep(700);
  await frame(500);
  await key('Enter');
  await sleep(1200);
}
async function command(s) {
  await key('F1');
  await page.waitForSelector('.quick-input-widget input', { visible: true });
  await typeSlow(s);
  await sleep(600);
  await frame(700);
  await key('Enter');
}
async function gotoLine(lineNo) {
  await key('Control+KeyG');
  await page.waitForSelector('.quick-input-widget input', { visible: true });
  await page.keyboard.type(String(lineNo));
  await sleep(300);
  await key('Enter');
  await sleep(600);
}
/** Screen rect of `word` in the first visible line of editor group `g` that contains `hint`. */
async function tokenRect(g, word, hint = '') {
  const r = await page.evaluate(({ g, word, hint }) => {
    const groups = [...document.querySelectorAll('.editor-group-container')];
    const ed = groups[g] && groups[g].querySelector('.monaco-editor .view-lines');
    if (!ed) return null;
    const lines = [...ed.querySelectorAll('.view-line')].sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top);
    for (const line of lines) {
      const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
      let full = '';
      const nodes = [];
      while (walker.nextNode()) { nodes.push([walker.currentNode, full.length]); full += walker.currentNode.textContent; }
      if (hint && !full.replace(/\u00a0/g, ' ').includes(hint)) continue;
      const re = new RegExp(`(?<![A-Za-z0-9_])${word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?![A-Za-z0-9_])`);
      const m = re.exec(full);
      if (!m) continue;
      const at = (off) => { for (let i = nodes.length - 1; i >= 0; i--) if (off >= nodes[i][1]) return [nodes[i][0], off - nodes[i][1]]; };
      const range = document.createRange();
      range.setStart(...at(m.index));
      range.setEnd(...at(m.index + word.length));
      const b = range.getBoundingClientRect();
      return { x: b.left, y: b.top, w: b.width, h: b.height };
    }
    return null;
  }, { g, word, hint });
  if (!r) throw new Error(`token not found: ${word} [${hint}] in editor group ${g}`);
  return r;
}
async function pointAt(g, word, hint) {
  const r = await tokenRect(g, word, hint);
  await moveTo(r.x + r.w / 2, r.y + r.h / 2);
}
async function hoverToken(g, word, hint) {
  await pointAt(g, word, hint);
  await waitFor(() => { const h = document.querySelector('.monaco-hover:not(.hidden) .hover-contents, .monaco-hover:not(.hidden) .markdown-hover'); return h && h.textContent.trim().length > 10; }, null, 20000);
  await sleep(400);
}
async function clickToken(g, word, hint) {
  await pointAt(g, word, hint);
  await page.mouse.click(mx, my);
  await sleep(300);
}
const lineOf = (file, needle) => fs.readFileSync(file, 'utf8').split('\n').findIndex((l) => l.includes(needle)) + 1;

// ---------------------------------------------------------------- the demo
log('loading workbench');
await page.goto(`${URL_BASE}/?folder=${encodeURIComponent(WS)}`, { waitUntil: 'networkidle2', timeout: 90000 });
await page.waitForSelector('.monaco-workbench', { timeout: 90000 });
await sleep(3000);
await installPointer();
await page.mouse.move(mx, my);

log('opening shapes.rs; waiting for rust-analyzer');
await quickOpen('shapes.rs');
await waitRustAnalyzer();
await key('Control+KeyB'); // hide the sidebar for room
await sleep(500);

// 1. The Rust file, cursor on total_area
await gotoLine(lineOf(`${WS}/src/shapes.rs`, 'pub fn total_area'));
await clickToken(0, 'total_area', 'fn total_area');
await frame(1600, 'rust-source');

// 2. Open the Ruby view beside it (cursor follows)
log('open as Ruby');
await command('rbview: Open as Ruby');
await lineMatches('def total_area');
await sleep(1200);
await frame(2600, 'open-as-ruby');

// 3. Hover in the Ruby view -> rust-analyzer's answer
log('hover');
await hoverToken(1, 'area', '@shapes.map');
await frame(2600, 'hover');
await key('Escape');

// 4. Go to definition inside the Ruby view (trait method call -> its impl)
log('go to definition');
await clickToken(1, 'area', '@shapes.map');
await frame(400);
await key('F12');
await lineMatches('case self', true);
await sleep(900);
await frame(2600, 'go-to-definition');

// 5. main.rs -> hover and jump into a dependency (semver)
log('main.rs');
await key('Control+Digit1');
await quickOpen('main.rs');
await sleep(600);
await gotoLine(lineOf(`${WS}/src/main.rs`, 'VersionReq::parse'));
await clickToken(0, 'parse', 'VersionReq::parse');
await command('rbview: Open as Ruby');
await lineMatches('VersionReq\\.parse');
await sleep(1000);
await frame(1800, 'main-as-ruby');
log('hover VersionReq.parse');
await hoverToken(1, 'parse', 'VersionReq.parse');
await frame(2200, 'hover-into-dependency');
await key('Escape');
await clickToken(1, 'parse', 'VersionReq.parse');
await key('F12');
await lineMatches('def self\\.parse\\(text\\)', true);
await sleep(1000);
await frame(3000, 'dependency-as-ruby');

// 6. Show the dependency's real Rust source beside its Ruby view
log('show rust');
await key('Control+Digit1');
await command('View: Close All Editors in Group'); // make room: Ruby + Rust side by side
await sleep(1000);
await key('Control+Digit1');
await sleep(400);
await command('rbview: Show Rust Source');
await lineMatches('pub fn parse\\(');
await sleep(1000);
await frame(3000, 'dependency-source');
await browser.close();
log(`captured ${frames.length} frames`);

// ---------------------------------------------------------------- GIF
const list = frames.map((f) => `file '${f.file}'\nduration ${(f.ms / 1000).toFixed(3)}\n`).join('') + `file '${frames.at(-1).file}'\n`;
fs.writeFileSync(`${OUT}/frames.txt`, list);
execFileSync('ffmpeg', [
  '-v', 'error', '-y', '-f', 'concat', '-safe', '0', '-i', `${OUT}/frames.txt`,
  '-vf', 'fps=12,scale=1200:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=160:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle',
  '-loop', '0', `${OUT}/rbview-demo.gif`,
]);
log(`wrote ${OUT}/rbview-demo.gif`);

if (PUBLISH) {
  fs.mkdirSync(PUBLISH, { recursive: true });
  for (const f of ['rbview-demo.gif', 'open-as-ruby.png', 'hover.png', 'hover-into-dependency.png', 'dependency-source.png']) {
    fs.copyFileSync(`${OUT}/${f}`, path.join(PUBLISH, f));
  }
  log(`published to ${PUBLISH}`);
}
