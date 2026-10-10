# 0009. Editors talk to a long-running `rubylens serve`

Status: Accepted
Date: 2026-10-09

## Context

After [0007](0007-allocator-and-release-profile.md) and [0008](0008-views-record-their-symbol-kinds.md), most of a cached view's cost was starting a process: about 2 ms of a 2.3 ms cache hit on Linux, and more on Windows, where starting a process is slower. A reference list or a go-to-definition with many targets opens one view per file, so those costs add up. Each process also reloaded the repo's `symbols.json`.

## Decision

- `rubylens serve` answers `gen` requests as JSON lines over stdin/stdout ([spec](../spec/serve.md)). A `gen` result is exactly what `rubylens gen --json` prints; both use `View::to_json`.
- It runs requests in parallel on every core and keeps only each root's `symbols.json` in memory, reloaded when the file changes. Views still go through the `.rubylens/` cache, so it shares the cache with every other RubyLens command.
- It exits when stdin closes, after 10 idle minutes, or when its binary is replaced, so an upgrade takes effect without restarting the editor.
- The VS Code extension uses it for every view. Any request the server doesn't answer, because it isn't there (an older RubyLens), crashed, or exited, is retried as a one-shot `rubylens gen`. A server problem costs speed, never a view.

## Consequences

- A cached view takes 0.2–1.5 ms instead of 2.3–4.7 ms, and 50 views at once take 3 ms instead of 68 ms (2-core Linux machine; see [performance.md](../performance.md)). Starting a process on Windows costs more, so the gain there is larger.
- The extension has a small process manager (`editors/vscode/server.js`) with its own tests, including a server killed mid-batch and a RubyLens without `serve`.
- The server must stay stateless apart from `symbols.json`. Anything else cached in memory has to be keyed so it can't go stale, the same rule as decision 0008.
- On Windows, a running server keeps the binary locked, so `cargo install` can fail until the server exits (10 idle minutes, or closing VS Code).

## Alternatives considered

- **A real language server** (RubyLens speaking LSP). The views are VS Code virtual documents answered through rust-analyzer; an LSP server would duplicate rust-analyzer's role without making views faster than this.
- **A daemon shared across editors over a socket.** It saves a process per editor window, but brings discovery, permissions and version skew between clients, for little gain.
- **Caching views in the extension.** It already keeps each open view's map. The cost was producing views it doesn't have yet.
