# Capturing the demo media

`capture.sh` regenerates everything in `docs/media/`: the README's GIF and screenshots.

It runs the real rbview extension in a real VS Code (code-server, VS Code in the browser) against a real rust-analyzer. A headless Chromium then clicks through the demo, and ffmpeg assembles the GIF. Everything runs in a throwaway profile under a temp folder, so your own VS Code settings and extensions are untouched.

```sh
npm i -g code-server        # or any other code-server install; npm needs the Node version it asks for
rustup component add rust-analyzer rust-src
tools/capture/capture.sh
```

| Variable | Default | |
|---|---|---|
| `CODE_SERVER` | `code-server` | command that runs code-server; may include arguments |
| `RUST_ANALYZER` | from `PATH` | rust-analyzer binary |
| `RBVIEW` | `target/release/rbview` | built automatically if missing |
| `CHROME_PATH` | `@sparticuz/chromium` | any Chromium or Chrome binary |
| `PORT` | `8737` | |
| `PUBLISH` | `docs/media` | where the finished files are copied |
| `KEEP` | unset | `KEEP=1` keeps the temp folder (frames, logs, `fail.png`) |

Also needs `ffmpeg`, `node` and `npm`.

## What's here

- **`capture.mjs`** — the scripted demo (Puppeteer). Each step waits for VS Code to show the expected result, so a broken feature fails the run with a `fail.png` instead of producing a misleading GIF.
- **`workspace/`** — the demo crate: `shapes.rs` plus a small app using semver, regex, serde_json, itertools and anyhow, so go-to-definition can jump into dependencies.
- **`rust-analyzer-client/`** — a minimal stand-in for the official rust-analyzer VS Code extension, with the same extension ID. It launches the rust-analyzer binary over LSP and shows a status-bar check when indexing finishes. It exists because the official extension may not be installable where the capture runs, for example in a sandbox. The official extension wires up the same LSP requests.
