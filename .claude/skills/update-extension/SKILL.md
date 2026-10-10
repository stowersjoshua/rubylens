---
name: update-extension
description: Use when changing the VS Code extension in editors/vscode (extension.js, server.js, srcmap.js, package.json — commands, settings, providers, keybindings, the rubylens serve client) or when the README's screenshots/GIF need regenerating. Covers tests, rebuilding the committed .vsix, and recapturing media with tools/capture.
---

# Change the VS Code extension

The contract is `docs/spec/extension.md`. The committed `editors/vscode/rubylens-0.1.0.vsix` is what users install, so `scripts/check.sh` fails if it doesn't match the source.

## 1. Make the change

- **`extension.js`** holds providers and commands. Every language feature must go through VS Code's `vscode.execute*Provider` commands against the Rust document. Never call rust-analyzer directly (decision 0001).
- **`srcmap.js`** is pure position lookup with no `vscode` import. Its behaviour is specified in `docs/spec/source-map.md` § Lookup.
- **`server.js`** is the `rubylens serve` client, also free of `vscode`. Every request it can't get answered must fail with `ServerGone` so `extension.js` retries it with `rubylens gen` (`docs/spec/serve.md` § Clients). Test a change with the crash and no-`serve` cases in `test/run.js`.
- **`package.json`**: new commands, settings and keybindings also go in the tables in `docs/spec/extension.md` and the README's Use section.

## 2. Test

```sh
node editors/vscode/test/run.js      # providers vs. a mock vscode + toy rust-analyzer
scripts/lsp-check.sh                  # the same mapping flow vs. a real rust-analyzer (100% required)
npx eslint .
```

If you add a provider, add a case to `test/run.js`. It shows how to fake `vscode.execute*Provider` results.

## 3. Rebuild the .vsix

```sh
cd editors/vscode && npx @vscode/vsce package --allow-missing-repository --skip-license -o rubylens-0.1.0.vsix
```

Bump `version` in `editors/vscode/package.json` for user-visible changes. If you do, rename the `.vsix` and update every reference (`grep -rn rubylens-0.1.0.vsix`).

## 4. Recapture media if the UI changed

```sh
tools/capture/capture.sh    # see tools/capture/README.md for CODE_SERVER, RUST_ANALYZER, CHROME_PATH
```

- The script fails with a `fail.png` (kept with `KEEP=1`) if any demo step doesn't produce the expected UI. Treat that as a bug in the change, not the script.
- **Look at every image** in `docs/media/` before committing. The hover has content, the jump landed on the named token, and the code is legible.
- If a scene's meaning changed, update the captions in `README.md` (Screenshots section).

## 5. Check and commit

`scripts/check.sh --full`, then commit the source, the `.vsix` and the media together.
