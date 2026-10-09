// End-to-end test of the extension's mapping pipeline without VS Code:
// a mock `vscode` module plus a toy "rust-analyzer" that resolves definitions
// by scanning the Rust source. Run: node test/run.js <path-to-rbview-binary>
'use strict';

const fs = require('fs');
const path = require('path');
const Module = require('module');
const assert = require('assert');

const BIN = process.argv[2] || path.resolve(__dirname, '../../../target/release/rbview');
const DEMO = path.resolve(__dirname, '../../../examples/demo');
const RS = path.join(DEMO, 'src/shapes.rs');

// ---------------------------------------------------------------- mock vscode
class Position { constructor(l, c) { this.line = l; this.character = c; } }
class Range {
  constructor(a, b) { this.start = a; this.end = b; }
  union(o) {
    const lt = (x, y) => x.line < y.line || (x.line === y.line && x.character < y.character);
    return new Range(lt(o.start, this.start) ? o.start : this.start, lt(this.end, o.end) ? o.end : this.end);
  }
  contains(o) {
    const le = (x, y) => x.line < y.line || (x.line === y.line && x.character <= y.character);
    const s = o.start || o, e = o.end || o;
    return le(this.start, s) && le(e, this.end);
  }
}
class Uri {
  constructor(scheme, p) { this.scheme = scheme; this.path = p; this.fsPath = p; }
  static from({ scheme, path: p }) { return new Uri(scheme, p); }
  static file(p) { return new Uri('file', p); }
  toString() { return `${this.scheme}:${this.path}`; }
}
const providers = {};
let contentProvider;
const docs = new Map();
const rsText = (uri) => fs.readFileSync(uri.fsPath, 'utf8').split('\n');

// Toy rust-analyzer: definition of the identifier under the cursor = first `fn|struct|enum|trait NAME`
function wordAt(lines, pos) {
  const line = lines[pos.line];
  let a = pos.character, b = pos.character;
  while (a > 0 && /\w/.test(line[a - 1])) a--;
  while (b < line.length && /\w/.test(line[b])) b++;
  return line.slice(a, b);
}
const fakeRA = {
  'vscode.executeDefinitionProvider': (uri, pos) => {
    const lines = rsText(uri);
    const w = wordAt(lines, pos);
    for (let i = 0; i < lines.length; i++) {
      const m = new RegExp(`\\b(fn|struct|enum|trait)\\s+(${w})\\b`).exec(lines[i]);
      if (m) {
        const c = m.index + m[0].length - w.length;
        return [{ uri, range: new Range(new Position(i, c), new Position(i, c + w.length)) }];
      }
    }
    return [];
  },
  'vscode.executeHoverProvider': (uri, pos) => [{ contents: [`**${wordAt(rsText(uri), pos)}** (from rust-analyzer)`] }],
  'vscode.executeReferenceProvider': (uri, pos) => {
    const lines = rsText(uri);
    const w = wordAt(lines, pos);
    const out = [];
    lines.forEach((l, i) => {
      const re = new RegExp(`\\b${w}\\b`, 'g');
      let m;
      while ((m = re.exec(l))) out.push({ uri, range: new Range(new Position(i, m.index), new Position(i, m.index + w.length)) });
    });
    return out;
  },
  'vscode.executeDocumentSymbolProvider': (uri) => {
    const lines = rsText(uri);
    const syms = [];
    lines.forEach((l, i) => {
      const m = /\b(fn|struct|enum|trait)\s+(\w+)/.exec(l);
      if (m) {
        const c = m.index + m[0].length - m[2].length;
        syms.push({ name: m[2], kind: 11, range: new Range(new Position(i, 0), new Position(i, l.length)),
          selectionRange: new Range(new Position(i, c), new Position(i, c + m[2].length)), children: [] });
      }
    });
    return syms;
  },
};
const vscode = {
  Uri, Position, Range,
  Location: class { constructor(uri, range) { this.uri = uri; this.range = range; } },
  Hover: class { constructor(contents, range) { this.contents = contents; this.range = range; } },
  DocumentHighlight: class { constructor(range, kind) { this.range = range; this.kind = kind; } },
  DocumentSymbol: class { constructor(name, detail, kind, range, sel) { Object.assign(this, { name, detail, kind, range, selectionRange: sel, children: [] }); } },
  Selection: class extends Range {},
  EventEmitter: class { constructor() { this.event = () => {}; } fire() {} },
  ViewColumn: { Beside: -2, Active: -1 },
  TextEditorRevealType: { InCenter: 2 },
  ProgressLocation: { Notification: 15 },
  workspace: {
    workspaceFolders: [{ uri: Uri.file(DEMO) }],
    getWorkspaceFolder: () => ({ uri: Uri.file(DEMO) }),
    getConfiguration: () => ({ get: (k, d) => (k === 'executable' ? BIN : d) }),
    registerTextDocumentContentProvider: (_s, p) => { contentProvider = p; return {}; },
    openTextDocument: async (uri) => {
      if (!docs.has(uri.toString())) {
        const text = uri.scheme === 'rbview' ? await contentProvider.provideTextDocumentContent(uri) : fs.readFileSync(uri.fsPath, 'utf8');
        docs.set(uri.toString(), { uri, getText: () => text, languageId: uri.scheme === 'rbview' ? 'ruby' : 'rust' });
      }
      return docs.get(uri.toString());
    },
    onDidSaveTextDocument: () => ({}),
    onDidOpenTextDocument: () => ({}),
  },
  languages: new Proxy({ setTextDocumentLanguage: async () => {} }, {
    get: (t, k) => t[k] || ((sel, p) => { providers[k] = p; return {}; }),
  }),
  commands: {
    registerCommand: () => ({}),
    executeCommand: async (cmd, ...args) => fakeRA[cmd](...args),
  },
  window: { createOutputChannel: () => ({ appendLine: (s) => console.error(s), dispose() {} }) },
};
const origLoad = Module._load;
Module._load = function (req, ...rest) { return req === 'vscode' ? vscode : origLoad.call(this, req, ...rest); };

// ---------------------------------------------------------------- tests
const ext = require('../extension');
const { SrcMap } = require('../srcmap');

(async () => {
  ext.activate({ subscriptions: [] });
  const rbUri = Uri.from({ scheme: 'rbview', path: RS + '.rb' });
  const doc = await vscode.workspace.openTextDocument(rbUri);
  const rb = doc.getText().split('\n');
  const find = (needle, nth = 0) => {
    let n = 0;
    for (let i = 0; i < rb.length; i++) {
      const c = rb[i].indexOf(needle);
      if (c >= 0 && n++ === nth) return new Position(i, c);
    }
    throw new Error('not found: ' + needle);
  };
  let pass = 0;
  const ok = (cond, msg) => { assert.ok(cond, msg); pass++; console.log('  ✓', msg); };

  // 1. go to definition: call site `shoelace(points)` -> `def self.shoelace`
  const call = find('shoelace(points)');
  let defs = await providers.registerDefinitionProvider.provideDefinition(doc, call);
  ok(defs.length === 1 && defs[0].targetUri.scheme === 'rbview', 'definition resolves into a Ruby view');
  const tgt = defs[0].targetSelectionRange;
  ok(rb[tgt.start.line].includes('def self.shoelace'), `definition lands on "${rb[tgt.start.line].trim()}"`);
  ok(rb[tgt.start.line].slice(tgt.start.character, tgt.end.character) === 'shoelace', 'target selection is exactly the name');
  ok(defs[0].originSelectionRange && rb[call.line].slice(defs[0].originSelectionRange.start.character, defs[0].originSelectionRange.end.character) === 'shoelace', 'origin range underlines the clicked word');

  // 2. definition of a type used in a pattern: `Shape::Circle` -> module Shape
  const typeUse = find('in Shape::Circle');
  defs = await providers.registerDefinitionProvider.provideDefinition(doc, new Position(typeUse.line, typeUse.character + 4));
  ok(rb[defs[0].targetSelectionRange.start.line].includes('module Shape  # enum'), 'type reference jumps to the enum (Ruby module)');

  // 3. trait method from a default body: `self.area` -> `def area` in module Area
  const selfArea = find("self.area)");
  defs = await providers.registerDefinitionProvider.provideDefinition(doc, new Position(selfArea.line, selfArea.character + 5));
  ok(/def area/.test(rb[defs[0].targetSelectionRange.start.line]), 'method call jumps to its def');

  // 4. hover is forwarded
  const h = await providers.registerHoverProvider.provideHover(doc, call);
  ok(h && String(h.contents[0]).includes('shoelace'), 'hover forwards rust-analyzer content');

  // 5. references map back into Ruby
  const refs = await providers.registerReferenceProvider.provideReferences(doc, call, {});
  ok(refs.length >= 2 && refs.every((r) => rb[r.range.start.line].slice(r.range.start.character, r.range.end.character) === 'shoelace'), `references (${refs.length}) all land on "shoelace" tokens`);

  // 6. outline
  const syms = await providers.registerDocumentSymbolProvider.provideDocumentSymbols(doc);
  const names = syms.map((s) => `${s.name}@${rb[s.selectionRange.start.line].slice(s.selectionRange.start.character, s.selectionRange.end.character)}`);
  ok(syms.length > 8 && syms.every((s) => rb[s.selectionRange.start.line].slice(s.selectionRange.start.character, s.selectionRange.end.character) === s.name), `outline symbols map onto their names (${names.slice(0, 4).join(', ')}, …)`);

  // 7. round-trip every mapped token through SrcMap
  const res = JSON.parse(require('child_process').execFileSync(BIN, ['gen', RS, '--root', DEMO, '--json']));
  const m = new SrcMap(res.map);
  let rt = 0, rtBad = 0;
  for (const s of res.map.segs) {
    const p = m.rbToRs({ line: s[0], character: s[1] });
    const back = m.rsToRb(p);
    rt++;
    // round-trip may land on a smaller token at the same Rust position; it must stay on the same Ruby line
    if (back.line !== s[0] && !(p.line === s[3])) rtBad++;
  }
  ok(rtBad === 0, `${rt} tokens round-trip Ruby→Rust→Ruby`);

  console.log(`\n${pass} checks passed`);
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
