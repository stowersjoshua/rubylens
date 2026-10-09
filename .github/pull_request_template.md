## What and why

<!-- What changes for someone reading Rust through rbview, and why. Link the spec section or decision record. -->

## Golden diff

<!-- For changes to src/emit.rs: paste the relevant part of `git diff tests/golden` (what readers will see differently). Write "n/a" otherwise. -->

## Checklist

- [ ] `scripts/check.sh` passes (`--full` if `src/emit.rs`, `src/out.rs` or `editors/vscode/srcmap.js` changed)
- [ ] Golden case added or updated and its diff reviewed; `docs/spec/` row updated
- [ ] No threshold lowered, or the trade-off is argued above
- [ ] Extension changed → `.vsix` rebuilt; UI changed → `tools/capture/capture.sh` re-run and images reviewed
- [ ] Docs that state behaviour or numbers (README, specs) still match
- [ ] No AI attribution in commits or in this description
