# VS Code extension spec

Source: `editors/vscode/` (`extension.js`, `server.js`, `srcmap.js`, `package.json`). The committed `rubylens-0.1.0.vsix` must be rebuilt whenever those change, and `scripts/check.sh` verifies it is.

## Documents

- The Rust file `/path/x.rs` is shown as the virtual document `rubylens:/path/x.rs.rb`, with language `ruby` and read-only.
- Content comes from a `gen` request to one long-running `rubylens serve` per session, with the workspace folder as `root` ([serve.md](serve.md)). If the server can't answer (an older RubyLens, a crash), the view comes from `rubylens gen <file> --root <workspace folder> --json` instead. The map is kept in memory per document.
- Saving the Rust file refreshes its open view.

## Language features

Every feature maps the Ruby position to Rust (`rbToRs`), runs VS Code's own provider command against the Rust document (so rust-analyzer answers), and maps the result forward (`rsRangeToRb` on the target file's view, generating it if needed):

| Feature | VS Code command used |
|---|---|
| Go to Definition / Type Definition / Implementation / Declaration | `vscode.execute{Definition,TypeDefinition,Implementation,Declaration}Provider` |
| Hover | `vscode.executeHoverProvider`, with contents passed through unchanged |
| Find References | `vscode.executeReferenceProvider` |
| Document highlights | `vscode.executeDocumentHighlights` |
| Outline | `vscode.executeDocumentSymbolProvider` |

Targets in `.rs` files open as their Ruby view. Other targets open as-is.

Results that land in several files are mapped in parallel. Each view is requested at most once at a time (parallel lookups into the same file share one request). The server runs one request per CPU core; one-shot `rubylens gen` fallbacks are limited to one per core too. The CLI's cache makes this safe ([cache.md](cache.md)).

Position lookups (`srcmap.js`) cost O(segments on one line) or O(1) after a one-time index per view, never O(size of the view): an outline or a long reference list makes thousands of lookups in one view, and generated files have 100,000+ segments.

## Commands and settings

| ID | Title | Default key |
|---|---|---|
| `rubylens.open` | RubyLens: Open as Ruby (cursor follows) | `Ctrl+Alt+R` / `Cmd+Alt+R` in Rust editors |
| `rubylens.showRust` | RubyLens: Show Rust Source | the same key in Ruby views |
| `rubylens.buildCache` | RubyLens: Pre-generate Ruby Views for Workspace | |

| Setting | Default | |
|---|---|---|
| `rubylens.executable` | `rubylens` | path to the CLI |
| `rubylens.openBeside` | `true` | open the view beside the Rust file |

## Dependencies

`extensionDependencies: ["rust-lang.rust-analyzer"]`. The extension never talks to rust-analyzer directly. It only uses VS Code's provider commands, so any extension that registers Rust providers works. Demo captures use a minimal stand-in client (`tools/capture/rust-analyzer-client`).

## Tests

- `node editors/vscode/test/run.js`: the providers against a mock `vscode` module and a toy rust-analyzer, including references across files, how lookups scale, a server killed mid-batch, and a RubyLens without `serve`.
- `scripts/lsp-check.sh`: the same mapping flow against a real rust-analyzer.
- `tools/capture/capture.sh`: the real extension in real VS Code. Every step waits for the expected UI.
