# Server spec (`rbview serve`)

Rationale: [decision 0009](../decisions/0009-rbview-serve.md). Source: `src/serve.rs` (server), `editors/vscode/server.js` (the VS Code client).

`rbview serve` answers the same requests as `rbview gen --json`, but from one long-running process, so an editor doesn't start a process for every view. It reads and writes the same `.rbview/` cache as every other command ([cache.md](cache.md)).

## Protocol

JSON lines over stdin/stdout: one request per line in, one response per line out. stderr is for humans (log it).

**Requests**

| Method | Fields | Result |
|---|---|---|
| `gen` | `file` (path to a `.rs` file), `root` (cache root; optional, else found as `rbview gen` finds it), `noCache` (optional, default `false`) | exactly the object `rbview gen <file> --root <root> --json [--no-cache]` prints: `{ "rbPath", "cached", "ruby", "map" }` |
| `ping` | | `{ "version": GEN_VERSION, "build": RBVIEW_BUILD_ID }` |

Every request has an `id` (any JSON value), echoed in its response:

```json
{"id": 7, "method": "gen", "file": "/repo/src/shapes.rs", "root": "/repo"}
{"id": 7, "result": {"rbPath": "/repo/.rbview/src/shapes.rb", "cached": true, "ruby": "…", "map": {…}}}
{"id": 8, "error": "/repo/src/gone.rs: No such file or directory (os error 2)"}
```

- Requests run in parallel, one per CPU core, so **responses come in any order**. Match them by `id`.
- A request that fails gets `error` (a message) instead of `result`, and the server carries on. That includes a panic while generating. A line that isn't JSON gets an error with `"id": null`.
- Unknown methods are errors, so clients can probe for newer methods.

## State

- Views and maps are read and written in `.rbview/` exactly as `rbview gen` does, so the server, `rbview gen` and `rbview build` can run at the same time on the same cache.
- The one thing kept in memory is each root's `symbols.json`. It's checked (one `stat`) on every request and reloaded when it changes, so a `rbview build` takes effect immediately.

## Lifetime

The server exits with status 0:

- when stdin closes, after answering everything already received;
- after `--idle-exit SECS` (default 600; `0` never) with no request in progress;
- when its own binary is replaced (an upgrade or a rebuild): the next request line isn't answered, and the process exits once the requests already running are answered.

## Clients

A client must expect the server to go away at any time, and the server to be missing from an older rbview, which exits with status 2 on `serve` (a usage error). The VS Code client:

- starts the server on the first view it needs and keeps it for the session;
- retries every request a vanished server didn't answer as a one-shot `rbview gen`, and starts a new server for the next request;
- uses one-shot `rbview gen` for the rest of the session if `serve` exits with status 2, can't be started, or exits three times in a row without answering anything;
- starts a new server when the `rbview.executable` setting changes, and closes its stdin on deactivation.
