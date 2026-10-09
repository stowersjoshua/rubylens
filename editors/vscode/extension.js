// rbview — read Rust as Ruby, with navigation delegated to rust-analyzer.
//
// A Rust file `/repo/src/foo.rs` is shown as the virtual document
// `rbview:/repo/src/foo.rs.rb`. Every language feature on that document is
// answered by mapping the position back to Rust, asking rust-analyzer
// (through VS Code's execute*Provider commands), and mapping the answer forward.
'use strict';

const vscode = require('vscode');
const cp = require('child_process');
const os = require('os');
const path = require('path');
const { SrcMap } = require('./srcmap');
const { GenServer, ServerGone } = require('./server');

const SCHEME = 'rbview';
/** rbview uri string -> { map: SrcMap, version } */
const maps = new Map();
const changed = new vscode.EventEmitter();
let output;

function rbUriFor(rsUri) {
  return vscode.Uri.from({ scheme: SCHEME, path: rsUri.path + '.rb' });
}

function rsUriFor(rbUri) {
  return vscode.Uri.file(rbUri.path.replace(/\.rb$/, ''));
}

function exe() {
  return vscode.workspace.getConfiguration('rbview').get('executable') || 'rbview';
}

/** The repo whose .rbview/ folder caches this file (deps cache under the active repo). */
function rootFor(rsUri) {
  const wf = vscode.workspace.getWorkspaceFolder(rsUri) || (vscode.workspace.workspaceFolders || [])[0];
  return wf ? wf.uri.fsPath : path.dirname(rsUri.fsPath);
}

/** The `rbview serve` process for the configured executable (a new one if the setting changes). */
let server = null;

function genServer() {
  const e = exe();
  if (!server || server.exe !== e) {
    if (server) server.stop();
    server = new GenServer(e, (line) => output.appendLine(line));
  }
  return server;
}

/** At most this many one-shot `rbview gen` processes at once (a reference list can span hundreds of files). */
const MAX_GEN = Math.max(2, os.cpus().length);
let running = 0;
const waiting = [];

async function limited(job) {
  while (running >= MAX_GEN) await new Promise((wake) => waiting.push(wake));
  running++;
  try {
    return await job();
  } finally {
    running--;
    const next = waiting.shift();
    if (next) next();
  }
}

/** The view and map for a Rust file, from the server, or from `rbview gen` if the server can't answer. */
async function runGen(rsUri) {
  try {
    return await genServer().request({ method: 'gen', file: rsUri.fsPath, root: rootFor(rsUri) });
  } catch (e) {
    if (!(e instanceof ServerGone)) throw e;
  }
  return limited(() => spawnGen(rsUri));
}

function spawnGen(rsUri) {
  return new Promise((resolve, reject) => {
    cp.execFile(
      exe(),
      ['gen', rsUri.fsPath, '--root', rootFor(rsUri), '--json'],
      { maxBuffer: 256 * 1024 * 1024 },
      (err, stdout, stderr) => {
        if (err) return reject(new Error(stderr || err.message));
        try {
          resolve(JSON.parse(stdout));
        } catch (e) {
          reject(e);
        }
      },
    );
  });
}

class RubyViewProvider {
  constructor() {
    this.onDidChange = changed.event;
  }
  async provideTextDocumentContent(uri) {
    const rsUri = rsUriFor(uri);
    try {
      const res = await runGen(rsUri);
      maps.set(uri.toString(), new SrcMap(res.map));
      return res.ruby;
    } catch (e) {
      output.appendLine(`rbview: ${e.message}`);
      return `# rbview failed for ${rsUri.fsPath}\n# ${String(e.message).split('\n').join('\n# ')}\n`;
    }
  }
}

/** rbview uri string -> Promise of the view being opened (so parallel lookups share one `rbview gen`) */
const opening = new Map();

/** Ensure the Ruby view (and its map) for a Rust file is loaded. */
async function mapFor(rbUri) {
  const key = rbUri.toString();
  if (!maps.has(key)) {
    if (!opening.has(key)) {
      opening.set(key, Promise.resolve(vscode.workspace.openTextDocument(rbUri)).finally(() => opening.delete(key)));
    }
    await opening.get(key);
  }
  return maps.get(key);
}

const P = (p) => new vscode.Position(p.line, p.character);
const R = (r) => new vscode.Range(P(r.start), P(r.end));

/** Map a Rust Location/LocationLink to a Ruby LocationLink (only .rs targets are translated). */
async function forwardLocation(loc, originRange) {
  const targetUri = loc.targetUri || loc.uri;
  const targetRange = loc.targetRange || loc.range;
  const targetSel = loc.targetSelectionRange || targetRange;
  if (!targetUri.path.endsWith('.rs')) {
    return { originSelectionRange: originRange, targetUri, targetRange, targetSelectionRange: targetSel };
  }
  const rb = rbUriFor(targetUri);
  const m = await mapFor(rb);
  if (!m) return { originSelectionRange: originRange, targetUri, targetRange, targetSelectionRange: targetSel };
  const sel = m.rsRangeToRb(targetSel) || m.rsRangeToRb(targetRange);
  const full = m.rsRangeToRb(targetRange) || sel;
  if (!sel) return null;
  // targetRange must contain targetSelectionRange
  const fullR = R(full).union(R(sel));
  return { originSelectionRange: originRange, targetUri: rb, targetRange: fullR, targetSelectionRange: R(sel) };
}

/** Run a Rust-side execute*Provider command for a Ruby position. */
async function viaRust(document, position, command, ...extra) {
  const m = await mapFor(document.uri);
  if (!m) return { m: null, result: null };
  const rsPos = m.rbToRs(position);
  if (!rsPos) return { m, result: null };
  const result = await vscode.commands.executeCommand(command, rsUriFor(document.uri), P(rsPos), ...extra);
  const tok = m.rbTokenRange(position);
  return { m, result, origin: tok ? R(tok) : undefined };
}

function locationProvider(command) {
  return async (document, position) => {
    const { result, origin } = await viaRust(document, position, command);
    if (!result) return null;
    const list = Array.isArray(result) ? result : [result];
    // targets in different files are mapped in parallel (each may need its own `rbview gen`)
    const out = await Promise.all(list.map((loc) => forwardLocation(loc, origin)));
    return out.filter(Boolean);
  };
}

async function provideReferences(document, position) {
  const { result } = await viaRust(document, position, 'vscode.executeReferenceProvider');
  if (!result) return [];
  const out = await Promise.all(result.map((loc) => forwardLocation(loc)));
  return out.filter(Boolean).map((f) => new vscode.Location(f.targetUri, f.targetSelectionRange));
}

async function provideHover(document, position) {
  const { result, origin } = await viaRust(document, position, 'vscode.executeHoverProvider');
  if (!result || !result.length) return null;
  const contents = result.flatMap((h) => h.contents);
  return new vscode.Hover(contents, origin);
}

async function provideHighlights(document, position) {
  const { m, result } = await viaRust(document, position, 'vscode.executeDocumentHighlights');
  if (!result || !m) return [];
  return result
    .map((h) => {
      const r = m.rsRangeToRb(h.range);
      return r ? new vscode.DocumentHighlight(R(r), h.kind) : null;
    })
    .filter(Boolean);
}

async function provideSymbols(document) {
  const m = await mapFor(document.uri);
  if (!m) return [];
  const syms = await vscode.commands.executeCommand('vscode.executeDocumentSymbolProvider', rsUriFor(document.uri));
  if (!syms) return [];
  const conv = (s) => {
    const range = s.range || (s.location && s.location.range);
    const sel = s.selectionRange || range;
    const r = m.rsRangeToRb(range);
    const sr = m.rsRangeToRb(sel) || r;
    if (!r || !sr) return null;
    const full = R(r).union(R(sr));
    const d = new vscode.DocumentSymbol(s.name, s.detail || '', s.kind, full, R(sr));
    d.children = (s.children || []).map(conv).filter((c) => c && full.contains(c.range));
    return d;
  };
  return syms.map(conv).filter(Boolean);
}

// ---------------------------------------------------------------- commands

async function openRubyView(uri) {
  const editor = vscode.window.activeTextEditor;
  const rsUri = uri instanceof vscode.Uri ? uri : editor && editor.document.uri;
  if (!rsUri || !rsUri.path.endsWith('.rs')) {
    return vscode.window.showWarningMessage('rbview: open a Rust file first.');
  }
  const rbUri = rbUriFor(rsUri);
  const doc = await vscode.workspace.openTextDocument(rbUri);
  await vscode.languages.setTextDocumentLanguage(doc, 'ruby');
  const beside = vscode.workspace.getConfiguration('rbview').get('openBeside', true);
  const shown = await vscode.window.showTextDocument(doc, {
    viewColumn: beside ? vscode.ViewColumn.Beside : vscode.ViewColumn.Active,
    preview: false,
  });
  // put the cursor where it was in the Rust file
  const m = maps.get(rbUri.toString());
  if (m && editor && editor.document.uri.toString() === rsUri.toString()) {
    const p = m.rsToRb(editor.selection.active);
    if (p) {
      shown.selection = new vscode.Selection(P(p), P(p));
      shown.revealRange(new vscode.Range(P(p), P(p)), vscode.TextEditorRevealType.InCenter);
    }
  }
}

async function showRust() {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.uri.scheme !== SCHEME) return;
  const m = await mapFor(editor.document.uri);
  const p = m && m.rbToRs(editor.selection.active);
  const doc = await vscode.workspace.openTextDocument(rsUriFor(editor.document.uri));
  const shown = await vscode.window.showTextDocument(doc, { viewColumn: vscode.ViewColumn.Beside });
  if (p) {
    shown.selection = new vscode.Selection(P(p), P(p));
    shown.revealRange(new vscode.Range(P(p), P(p)), vscode.TextEditorRevealType.InCenter);
  }
}

function buildCache() {
  const folders = vscode.workspace.workspaceFolders || [];
  if (!folders.length) return;
  const root = folders[0].uri.fsPath;
  return vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: 'rbview: generating Ruby views…' },
    () =>
      new Promise((resolve) => {
        cp.execFile(exe(), ['build', root], { maxBuffer: 64 * 1024 * 1024 }, (err, _stdout, stderr) => {
          output.appendLine(stderr);
          if (err) vscode.window.showErrorMessage(`rbview build failed: ${err.message}`);
          else vscode.window.showInformationMessage(stderr.split('\n')[0] || 'rbview: done');
          resolve();
        });
      }),
  );
}

function activate(context) {
  output = vscode.window.createOutputChannel('rbview');
  const sel = { scheme: SCHEME };
  context.subscriptions.push(
    output,
    vscode.workspace.registerTextDocumentContentProvider(SCHEME, new RubyViewProvider()),
    vscode.languages.registerDefinitionProvider(sel, { provideDefinition: locationProvider('vscode.executeDefinitionProvider') }),
    vscode.languages.registerTypeDefinitionProvider(sel, { provideTypeDefinition: locationProvider('vscode.executeTypeDefinitionProvider') }),
    vscode.languages.registerImplementationProvider(sel, { provideImplementation: locationProvider('vscode.executeImplementationProvider') }),
    vscode.languages.registerDeclarationProvider(sel, { provideDeclaration: locationProvider('vscode.executeDeclarationProvider') }),
    vscode.languages.registerReferenceProvider(sel, { provideReferences }),
    vscode.languages.registerHoverProvider(sel, { provideHover }),
    vscode.languages.registerDocumentHighlightProvider(sel, { provideDocumentHighlights: provideHighlights }),
    vscode.languages.registerDocumentSymbolProvider(sel, { provideDocumentSymbols: provideSymbols }),
    vscode.commands.registerCommand('rbview.open', openRubyView),
    vscode.commands.registerCommand('rbview.showRust', showRust),
    vscode.commands.registerCommand('rbview.buildCache', buildCache),
    // keep open views fresh when the Rust source is saved
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.uri.path.endsWith('.rs')) {
        const rb = rbUriFor(doc.uri);
        maps.delete(rb.toString());
        changed.fire(rb);
      }
    }),
    // views opened by go-to-definition: make sure they're highlighted as Ruby
    vscode.workspace.onDidOpenTextDocument((doc) => {
      if (doc.uri.scheme === SCHEME && doc.languageId !== 'ruby') {
        vscode.languages.setTextDocumentLanguage(doc, 'ruby');
      }
    }),
  );
}

function deactivate() {
  if (server) server.stop();
}

module.exports = { activate, deactivate };
