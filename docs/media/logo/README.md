# The RubyLens logo

A loupe whose lens is a cut ruby: you read Rust through a Ruby lens. The SVGs here are the source; everything else is rendered from them.

| File | Use |
|---|---|
| `rubylens-mark.svg` | the mark on light backgrounds |
| `rubylens-mark-dark.svg` | the mark on dark backgrounds |
| `rubylens-logo.svg` | mark and wordmark on light backgrounds (the README header) |
| `rubylens-logo-dark.svg` | mark and wordmark on dark backgrounds |
| `rubylens-icon.svg` | the mark on a white rounded tile: the source of the extension icon |

Each SVG works as a favicon as it is.

## Colours and type

| | Light background | Dark background |
|---|---|---|
| Ring, handle, "Lens" | `#1E1A22` | `#F1EDF2` |
| "Ruby" | `#AE162F` | `#F98597` |

The ruby's facets run from `#5E0A18` through `#860F22`, `#AE162F`, `#D21F3C`, `#EE5068` and `#F98597` to the table, `#FFD3DA`. Light falls from the top left.

The wordmark is Space Grotesk SemiBold (SIL Open Font License) at −0.02 em tracking, converted to outlines so it needs no font.

## Changing the extension icon

`editors/vscode/icon.png` is `rubylens-icon.svg` rendered at 256 × 256 with transparent corners, for example `rsvg-convert -w 256 -h 256 docs/media/logo/rubylens-icon.svg -o editors/vscode/icon.png`. Then rebuild the `.vsix` (`.claude/skills/update-extension/SKILL.md`). `scripts/check.sh` fails until you do.

Don't base new artwork on the Ruby language logo, Rust's logo or Ferris. RubyLens's mark is its own.
