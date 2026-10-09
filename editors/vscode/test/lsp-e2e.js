// Headless end-to-end test against a REAL rust-analyzer.
//
// Replays exactly what the VS Code extension does for each request:
//   Ruby position --srcmap--> Rust position --LSP--> rust-analyzer
//   --> target Rust location --rbview gen + srcmap--> Ruby view position
// and checks that the Ruby text at the landing spot is the identifier rust-analyzer pointed at.
//
// Usage:
//   node test/lsp-e2e.js --ra <rust-analyzer> --rbview <rbview> --project <dir> --file <rel.rs> [--file ...] [--limit N]
'use strict';

const cp = require('child_process');
const fs = require('fs');
const path = require('path');
const { pathToFileURL, fileURLToPath } = require('url');
const { SrcMap } = require('../srcmap');

// ---------------------------------------------------------------- args
const args = process.argv.slice(2);
const opt = (k, d) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : d; };
const all = (k) => args.flatMap((a, i) => (a === k ? [args[i + 1]] : []));
const RA = opt('--ra', 'rust-analyzer');
const RBVIEW = opt('--rbview', path.resolve(__dirname, '../../../target/release/rbview'));
const PROJECT = path.resolve(opt('--project', '.'));
const FILES = all('--file');
const LIMIT = Number(opt('--limit', '400'));
const VERBOSE = args.includes('--verbose');
// rust-analyzer settings, sent at startup and whenever it asks (workspace/configuration)
const RA_CONFIG = {
  checkOnSave: false,
  cachePriming: { enable: true },
  procMacro: { enable: true },
  cargo: { features: args.includes('--all-features') ? 'all' : [] },
};

// ---------------------------------------------------------------- minimal LSP client
class Lsp {
  constructor(cmd, cwd) {
    this.p = cp.spawn(cmd, [], { cwd, stdio: ['pipe', 'pipe', 'pipe'] });
    this.buf = Buffer.alloc(0);
    this.id = 0;
    this.pending = new Map();
    this.onNotify = () => {};
    this.p.stdout.on('data', (d) => this.read(d));
    this.p.stderr.on('data', (d) => VERBOSE && process.stderr.write(d));
    this.p.on('exit', (c) => { for (const { reject } of this.pending.values()) reject(new Error(`rust-analyzer exited ${c}`)); });
  }
  read(d) {
    this.buf = Buffer.concat([this.buf, d]);
    for (;;) {
      const sep = this.buf.indexOf('\r\n\r\n');
      if (sep < 0) return;
      const len = Number(/Content-Length: (\d+)/i.exec(this.buf.slice(0, sep).toString())[1]);
      if (this.buf.length < sep + 4 + len) return;
      const msg = JSON.parse(this.buf.slice(sep + 4, sep + 4 + len).toString());
      this.buf = this.buf.slice(sep + 4 + len);
      this.dispatch(msg);
    }
  }
  dispatch(m) {
    if (m.id !== undefined && !m.method) {
      const p = this.pending.get(m.id);
      this.pending.delete(m.id);
      if (p) m.error ? p.reject(new Error(m.error.message)) : p.resolve(m.result);
    } else if (m.id !== undefined && m.method) {
      // server -> client requests
      const result = m.method === 'workspace/configuration' ? m.params.items.map(() => RA_CONFIG) : null;
      this.send({ jsonrpc: '2.0', id: m.id, result });
    } else {
      this.onNotify(m);
    }
  }
  send(obj) {
    const body = Buffer.from(JSON.stringify(obj));
    this.p.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
    this.p.stdin.write(body);
  }
  request(method, params) {
    const id = ++this.id;
    this.send({ jsonrpc: '2.0', id, method, params });
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }
  notify(method, params) { this.send({ jsonrpc: '2.0', method, params }); }
}

// ---------------------------------------------------------------- helpers
const views = new Map(); // rs path -> { lines, map, rsLines }
function view(rsPath) {
  if (!views.has(rsPath)) {
    const res = JSON.parse(cp.execFileSync(RBVIEW, ['gen', rsPath, '--root', PROJECT, '--json'], { maxBuffer: 1 << 28 }));
    views.set(rsPath, {
      lines: res.ruby.split('\n'),
      map: new SrcMap(res.map),
      rsLines: fs.readFileSync(rsPath, 'utf8').split('\n'),
    });
  }
  return views.get(rsPath);
}
// LSP columns are UTF-16 code units
const u16 = (line, a, b) => Buffer.from(line || '', 'utf16le').slice(a * 2, b * 2).toString('utf16le');
const textAt = (lines, r) => (r.start.line === r.end.line ? u16(lines[r.start.line], r.start.character, r.end.character) : null);
const norm = (s) => (s || '').replace(/^r#/, '').replace(/^@/, '').replace(/_+$/, '').replace(/_/g, '').toLowerCase();
const isIdent = (s) => /^[A-Za-z_][A-Za-z0-9_]*$/.test(s);
function kindOf(p) {
  if (p.startsWith(PROJECT + path.sep)) return 'project';
  if (p.includes(`${path.sep}registry${path.sep}src${path.sep}`)) return 'dependency';
  if (p.includes(`${path.sep}rustlib${path.sep}`) || p.includes(`${path.sep}library${path.sep}`)) return 'std';
  return 'other';
}

// ---------------------------------------------------------------- main
(async () => {
  const t0 = Date.now();
  const lsp = new Lsp(RA, PROJECT);
  let quiescentResolve;
  const quiescent = new Promise((r) => (quiescentResolve = r));
  // rust-analyzer may report "quiescent" before it starts loading the workspace;
  // wait for busy -> quiescent (or quiescent that persists for 20s).
  let sawBusy = false;
  let idleTimer;
  lsp.onNotify = (m) => {
    if (m.method !== 'experimental/serverStatus') return;
    clearTimeout(idleTimer);
    if (!m.params.quiescent) sawBusy = true;
    else if (sawBusy) quiescentResolve(m.params);
    else idleTimer = setTimeout(() => quiescentResolve(m.params), 20e3);
  };
  const rootUri = pathToFileURL(PROJECT).href;
  await lsp.request('initialize', {
    processId: process.pid,
    rootUri,
    workspaceFolders: [{ uri: rootUri, name: path.basename(PROJECT) }],
    capabilities: {
      textDocument: {
        hover: { contentFormat: ['markdown', 'plaintext'] },
        definition: { linkSupport: true },
        references: {},
        documentSymbol: { hierarchicalDocumentSymbolSupport: true },
      },
      window: { workDoneProgress: true },
      experimental: { serverStatusNotification: true },
    },
    initializationOptions: RA_CONFIG,
  });
  lsp.notify('initialized', {});
  const status = await Promise.race([quiescent, new Promise((_, rej) => setTimeout(() => rej(new Error('rust-analyzer did not finish indexing in 15 min')), 15 * 60e3))]);
  console.log(`rust-analyzer ready in ${((Date.now() - t0) / 1000).toFixed(1)}s (health: ${status.health}${status.message ? ', ' + status.message.split('\n')[0] : ''})`);

  const totals = { tokens: 0, noDef: 0, def: 0, defOk: 0, byKind: {}, hover: 0, hoverOk: 0, syms: 0, symOk: 0, refs: 0, refOk: 0 };
  const failures = [];

  for (const rel of FILES) {
    const file = path.resolve(PROJECT, rel);
    const uri = pathToFileURL(file).href;
    lsp.notify('textDocument/didOpen', { textDocument: { uri, languageId: 'rust', version: 1, text: fs.readFileSync(file, 'utf8') } });
    const v = view(file);

    // Identifier tokens as a reader would click them in the Ruby view
    let tokens = v.map.segs.filter((s) => {
      const rb = u16(v.lines[s[0]], s[1], s[1] + s[2]);
      return isIdent(rb) && norm(rb) === norm(u16(v.rsLines[s[3]], s[4], s[4] + s[5]));
    });
    if (tokens.length > LIMIT) {
      const step = tokens.length / LIMIT;
      tokens = Array.from({ length: LIMIT }, (_, i) => tokens[Math.floor(i * step)]);
    }

    const f = { tokens: tokens.length, def: 0, defOk: 0 };
    for (const s of tokens) {
      const rbPos = { line: s[0], character: s[1] + Math.floor(s[2] / 2) }; // click mid-word
      const rsPos = v.map.rbToRs(rbPos);
      const res = await lsp.request('textDocument/definition', { textDocument: { uri }, position: rsPos });
      const list = res ? (Array.isArray(res) ? res : [res]) : [];
      totals.tokens++;
      if (!list.length) { totals.noDef++; continue; }
      f.def++; totals.def++;
      const loc = list[0];
      const tUri = loc.targetUri || loc.uri;
      const tSel = loc.targetSelectionRange || loc.range;
      const tPath = fileURLToPath(tUri);
      const kind = kindOf(tPath);
      const k = (totals.byKind[kind] ||= { def: 0, ok: 0 });
      k.def++;
      if (!tPath.endsWith('.rs')) { k.ok++; f.defOk++; totals.defOk++; continue; }
      const tv = view(tPath);
      const expected = textAt(tv.rsLines, tSel);
      const rbR = tv.map.rsRangeToRb(tSel);
      const got = rbR ? textAt(tv.lines, rbR) : null;
      let ok;
      if (tSel.start.line !== tSel.end.line) {
        // a module: rust-analyzer targets the whole file; landing anywhere in its Ruby view is right
        ok = !!rbR;
      } else {
        // the Ruby line we land on must show the name (exact token, or within the line for renamed/keyword targets)
        ok = got !== null && (norm(got) === norm(expected) || (rbR && norm(tv.lines[rbR.start.line]).includes(norm(expected))));
        // keywords (`break` -> its `for` loop) have no Ruby twin; the landing line must map back to the target line
        const KEYWORDS = ['for', 'while', 'loop', 'fn', 'match', 'if', 'async', 'unsafe', 'impl', 'closure', 'self'];
        if (!ok && rbR && KEYWORDS.includes(expected)) {
          const back = tv.map.rbToRs({ line: rbR.start.line, character: 0 });
          // (multi-line signatures put `self` a line or two below the `fn` line)
          ok = !!back && Math.abs(back.line - tSel.start.line) <= 3;
        }
      }
      if (ok) { k.ok++; f.defOk++; totals.defOk++; }
      else failures.push({ from: `${rel}:${rbPos.line + 1}  ${v.lines[rbPos.line].trim()}`, to: `${path.relative(PROJECT, tPath).replace(/^.*registry\/src\/[^/]+\//, '')}:${tSel.start.line + 1}`, expected, got, rbLine: rbR ? tv.lines[rbR.start.line].trim() : '(unmapped)' });
    }

    // Hover on a sample of tokens
    for (const s of tokens.slice(0, 60)) {
      const rsPos = v.map.rbToRs({ line: s[0], character: s[1] });
      const h = await lsp.request('textDocument/hover', { textDocument: { uri }, position: rsPos });
      totals.hover++;
      if (h && h.contents && JSON.stringify(h.contents).length > 20) totals.hoverOk++;
    }

    // Outline: every symbol's name lands on its Ruby name
    const syms = (await lsp.request('textDocument/documentSymbol', { textDocument: { uri } })) || [];
    const walk = (list) => list.forEach((sym) => {
      totals.syms++;
      const r = v.map.rsRangeToRb(sym.selectionRange);
      const got = r && textAt(v.lines, r);
      // rust-analyzer names impls like "impl Trait for Foo<'a>": compare the last type name
      const name = sym.name.startsWith('impl ') ? (sym.name.replace(/<.*$/, '').split(/\s+/).pop()) : sym.name;
      if (got && (norm(got) === norm(name) || norm(v.lines[r.start.line]).includes(norm(name)))) totals.symOk++;
      else if (VERBOSE) console.log('  symbol miss:', sym.name, '->', got, '|', r && v.lines[r.start.line].trim());
      walk(sym.children || []);
    });
    walk(syms);

    // References for functions defined in this file
    const fns = [];
    const collect = (list) => list.forEach((sym) => { if (sym.kind === 12 || sym.kind === 6) fns.push(sym); collect(sym.children || []); });
    collect(syms);
    for (const fn of fns.slice(0, 15)) {
      const refs = (await lsp.request('textDocument/references', { textDocument: { uri }, position: fn.selectionRange.start, context: { includeDeclaration: true } })) || [];
      for (const r of refs) {
        const p = fileURLToPath(r.uri);
        if (!p.endsWith('.rs')) continue;
        const tv = view(p);
        totals.refs++;
        const rb = tv.map.rsRangeToRb(r.range);
        const got = rb && textAt(tv.lines, rb);
        if (got && norm(got) === norm(textAt(tv.rsLines, r.range))) totals.refOk++;
        else if (VERBOSE) console.log('  ref miss:', textAt(tv.rsLines, r.range), '->', got, '|', rb && tv.lines[rb.start.line].trim());
      }
    }
    console.log(`  ${rel}: ${f.tokens} identifiers clicked, ${f.def} resolved by rust-analyzer, ${f.defOk} landed on the right Ruby token`);
  }

  const pct = (a, b) => (b ? `${((100 * a) / b).toFixed(1)}%` : 'n/a');
  console.log('\n== Go to definition (Ruby -> rust-analyzer -> Ruby)');
  console.log(`   clicked ${totals.tokens}; rust-analyzer resolved ${totals.def} (${totals.noDef} had no definition, e.g. std without rust-src, locals in macros)`);
  console.log(`   landed correctly: ${totals.defOk}/${totals.def} = ${pct(totals.defOk, totals.def)}`);
  for (const [k, v] of Object.entries(totals.byKind)) console.log(`     ${k.padEnd(10)} ${v.ok}/${v.def} = ${pct(v.ok, v.def)}`);
  console.log(`== Hover returned content: ${totals.hoverOk}/${totals.hover} = ${pct(totals.hoverOk, totals.hover)}`);
  console.log(`== Outline symbols mapped onto their names: ${totals.symOk}/${totals.syms} = ${pct(totals.symOk, totals.syms)}`);
  console.log(`== References mapped onto the right token: ${totals.refOk}/${totals.refs} = ${pct(totals.refOk, totals.refs)}`);
  if (failures.length) {
    console.log(`\n${failures.length} definition misses (first 25):`);
    for (const x of failures.slice(0, 25)) console.log(`  ${x.from}\n     -> ${x.to}  expected "${x.expected}" got "${x.got}"  [${x.rbLine.slice(0, 90)}]`);
  }
  console.log(`\ntotal ${((Date.now() - t0) / 1000).toFixed(1)}s`);
  await lsp.request('shutdown', null).catch(() => {});
  lsp.notify('exit', null);
  setTimeout(() => process.exit(0), 200);
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
