#!/usr/bin/env bash
# Regenerates docs/media/: screenshots and a GIF of rbview running in VS Code.
#
# Runs code-server (VS Code in the browser) with a throwaway profile, installs the
# rbview extension plus a minimal rust-analyzer client, and drives it with headless
# Chromium. Your own VS Code settings and extensions are never touched.
#
# Environment (all optional):
#   CODE_SERVER    command that runs code-server   (default: code-server)
#   RUST_ANALYZER  rust-analyzer binary            (default: from PATH)
#   RBVIEW         rbview binary                   (default: target/release/rbview, built if missing)
#   CHROME_PATH    Chromium/Chrome binary          (default: @sparticuz/chromium if installed)
#   PORT           code-server port                (default: 8737)
#   PUBLISH        where finished media goes       (default: docs/media)
#   KEEP=1         keep the temporary work folder (logs, frames, fail.png) for debugging
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
CODE_SERVER="${CODE_SERVER:-code-server}"
RUST_ANALYZER="${RUST_ANALYZER:-$(command -v rust-analyzer || true)}"
RBVIEW="${RBVIEW:-$ROOT/target/release/rbview}"
PORT="${PORT:-8737}"
PUBLISH="${PUBLISH:-$ROOT/docs/media}"
WORK="$(mktemp -d)"
say() { printf '\033[1m==> %s\033[0m\n' "$*"; }

for tool in ffmpeg node npm npx curl; do
  command -v "$tool" >/dev/null || { echo "missing: $tool" >&2; exit 1; }
done
[ -x "$RUST_ANALYZER" ] || { echo "rust-analyzer not found; set RUST_ANALYZER" >&2; exit 1; }
if [ ! -x "$RBVIEW" ]; then
  say "building rbview"
  cargo build --release --manifest-path "$ROOT/Cargo.toml"
fi

say "installing capture dependencies"
(cd "$HERE" && npm install --silent --no-audit --no-fund)

say "packaging the rust-analyzer client and rbview extensions"
(cd "$HERE/rust-analyzer-client" && npm install --silent --no-audit --no-fund \
  && npx -y @vscode/vsce package --allow-missing-repository --skip-license -o "$WORK/rust-analyzer-client.vsix" >/dev/null)
(cd "$ROOT/editors/vscode" && npx -y @vscode/vsce package --allow-missing-repository --skip-license -o "$WORK/rbview.vsix" >/dev/null)

say "preparing a fresh workspace and VS Code profile in $WORK"
cp -r "$HERE/workspace" "$WORK/workspace"
(cd "$WORK/workspace" && cargo fetch --quiet)
mkdir -p "$WORK/user-data/User"
cat > "$WORK/user-data/User/settings.json" <<EOF
{
  "rbview.executable": "$RBVIEW",
  "rust-analyzer.server.path": "$RUST_ANALYZER",
  "rust-analyzer.checkOnSave": false,
  "rust-analyzer.diagnostics.enable": false,
  "workbench.colorTheme": "Default Dark Modern",
  "workbench.startupEditor": "none",
  "workbench.tips.enabled": false,
  "workbench.secondarySideBar.defaultVisibility": "hidden",
  "workbench.layoutControl.enabled": false,
  "editor.fontFamily": "'DejaVu Sans Mono', monospace",
  "editor.fontSize": 14,
  "editor.lineHeight": 21,
  "editor.minimap.enabled": false,
  "editor.hover.delay": 250,
  "editor.stickyScroll.enabled": false,
  "editor.inlayHints.enabled": "off",
  "security.workspace.trust.enabled": false,
  "telemetry.telemetryLevel": "off",
  "extensions.ignoreRecommendations": true,
  "chat.disableAIFeatures": true,
  "chat.commandCenter.enabled": false,
  "git.enabled": false,
  "window.commandCenter": false,
  "update.mode": "none",
  "workbench.enableExperiments": false,
  "files.exclude": { "**/target": true }
}
EOF
CS_ARGS=(--user-data-dir "$WORK/user-data" --extensions-dir "$WORK/extensions")
for vsix in "$WORK/rust-analyzer-client.vsix" "$WORK/rbview.vsix"; do
  $CODE_SERVER "${CS_ARGS[@]}" --install-extension "$vsix" >/dev/null
done

say "starting code-server on port $PORT"
# its own process group, so the extension host and rust-analyzer are stopped with it
setsid $CODE_SERVER "${CS_ARGS[@]}" --auth none --bind-addr "127.0.0.1:$PORT" \
  --disable-telemetry --disable-update-check --disable-workspace-trust \
  "$WORK/workspace" > "$WORK/code-server.log" 2>&1 &
CS_PID=$!
trap 'kill -- -"$CS_PID" 2>/dev/null || true; [ -n "${KEEP:-}" ] && echo "kept $WORK" || rm -rf "$WORK"' EXIT
for _ in $(seq 60); do
  curl -s -o /dev/null "http://127.0.0.1:$PORT/" && break
  sleep 1
done

say "capturing"
node "$HERE/capture.mjs" --url "http://127.0.0.1:$PORT" --workspace "$WORK/workspace" \
  --out "$WORK/out" --publish "$PUBLISH"
say "done: $PUBLISH"
