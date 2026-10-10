//! Golden tests: the executable form of docs/spec/translation.md and docs/spec/source-map.md.
//!
//! For every `tests/golden/<case>.rs` this checks:
//!
//! 1. **snapshot**: the generated Ruby equals `tests/golden/<case>.rb`
//! 2. **bounds**: every map segment/block lies inside both texts
//! 3. **accuracy**: a mapped identifier lands on the same identifier in Rust (or an allowed rename)
//! 4. **coverage**: nearly every Rust identifier is reachable from the Ruby view
//! 5. **ruby -c**: the view is valid Ruby (if `ruby` is installed), unless the case is marked
//!    `// rubylens-golden: ruby-invalid (...)`, in which case it must stay invalid
//! 6. **spec link**: every case is referenced from docs/spec/translation.md and vice versa
//!
//! Update snapshots after an intended change:  UPDATE_GOLDEN=1 cargo test --test golden
//! then review them like code:                 git diff tests/golden

use proc_macro2::{TokenStream, TokenTree};
use rubylens::emit::{DROPPED_METHODS, KEYWORD_RENAMES, MACRO_RENAMES, METHOD_RENAMES};
use rubylens::{SourceMap, generate};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

/// Minimum share of Rust identifiers that must be reachable from the view. Keywords and
/// things erased by design (DROPPED_METHODS, `Some(x)` -> `x`, `_`) don't count.
const MIN_COVERAGE: f64 = 0.95;
const ERASED_BY_DESIGN: &[&str] = &["Some", "_", "collect"];

const RUST_KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
    "union",
    "macro_rules",
    "doc",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn cases() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(root().join("tests/golden"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .collect();
    v.sort();
    assert!(!v.is_empty(), "no golden cases found");
    v
}

fn name(p: &Path) -> String {
    p.file_stem().unwrap().to_string_lossy().to_string()
}

/// Text between UTF-16 columns of one line (the map's unit, like LSP).
fn u16_slice(line: &str, col: u32, len: u32) -> String {
    let units: Vec<u16> = line.encode_utf16().collect();
    let a = (col as usize).min(units.len());
    let b = ((col + len) as usize).min(units.len());
    String::from_utf16_lossy(&units[a..b])
}

fn u16_len(line: &str) -> u32 {
    line.encode_utf16().count() as u32
}

fn is_ident(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(ch) if ch.is_alphabetic() || ch == '_') && c.all(|ch| ch.is_alphanumeric() || ch == '_')
}

/// Normalize for comparison: camelized module names, keyword-escaped `end_`, raw `r#type`.
fn norm(s: &str) -> String {
    s.trim_start_matches("r#").trim_start_matches('@').trim_end_matches('_').replace('_', "").to_lowercase()
}

fn allowed_rename(rb: &str, rs: &str) -> bool {
    METHOD_RENAMES.iter().chain(MACRO_RENAMES).chain(KEYWORD_RENAMES).any(|(r, b)| *r == rs && *b == rb)
        || rs == "Self"
        || (rs == "self" && rb == "self")
}

struct Case {
    name: String,
    rust: String,
    ruby: String,
    map: SourceMap,
}

fn run(path: &Path) -> Case {
    let rust = fs::read_to_string(path).unwrap();
    let g = generate(&rust, path, &Default::default());
    assert!(g.parse_error.is_none(), "{}: syn could not parse the case: {:?}", path.display(), g.parse_error);
    Case { name: name(path), rust, ruby: g.ruby, map: g.map }
}

#[test]
fn snapshots_match() {
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut failures = vec![];
    for path in cases() {
        let c = run(&path);
        let expected_path = path.with_extension("rb");
        if update {
            fs::write(&expected_path, &c.ruby).unwrap();
            continue;
        }
        let expected = fs::read_to_string(&expected_path).unwrap_or_default();
        if expected != c.ruby {
            let diff: Vec<String> = expected
                .lines()
                .chain(std::iter::repeat(""))
                .zip(c.ruby.lines().chain(std::iter::repeat("")))
                .enumerate()
                .take(expected.lines().count().max(c.ruby.lines().count()))
                .filter(|(_, (a, b))| a != b)
                .take(6)
                .map(|(i, (a, b))| format!("    line {}:\n      expected: {a}\n      actual:   {b}", i + 1))
                .collect();
            failures.push(format!("{}:\n{}", c.name, diff.join("\n")));
        }
    }
    assert!(
        failures.is_empty(),
        "golden snapshots differ:\n{}\n\nIf the change is intended: UPDATE_GOLDEN=1 cargo test --test golden && git diff tests/golden",
        failures.join("\n")
    );
}

#[test]
fn map_segments_are_in_bounds() {
    for path in cases() {
        let c = run(&path);
        let rb: Vec<&str> = c.ruby.split('\n').collect();
        let rs: Vec<&str> = c.rust.split('\n').collect();
        for s in &c.map.segs {
            let [rbl, rbc, rbn, rsl, rsc, rsn] = *s;
            assert!(rbn > 0 && rsn > 0, "{}: empty segment {s:?}", c.name);
            assert!(
                (rbl as usize) < rb.len() && rbc + rbn <= u16_len(rb[rbl as usize]),
                "{}: Ruby side out of bounds {s:?}",
                c.name
            );
            assert!(
                (rsl as usize) < rs.len() && rsc + rsn <= u16_len(rs[rsl as usize]).max(rsc + 1),
                "{}: Rust side out of bounds {s:?}",
                c.name
            );
        }
        for b in &c.map.blocks {
            let [rb0, rb1, rs0, rs1] = *b;
            assert!(rb0 <= rb1 && (rb1 as usize) < rb.len(), "{}: bad Ruby block {b:?}", c.name);
            assert!(rs0 <= rs1 && (rs1 as usize) < rs.len(), "{}: bad Rust block {b:?}", c.name);
        }
    }
}

#[test]
fn mapped_identifiers_land_on_the_same_identifier() {
    let mut bad = vec![];
    for path in cases() {
        let c = run(&path);
        let rb: Vec<&str> = c.ruby.split('\n').collect();
        let rs: Vec<&str> = c.rust.split('\n').collect();
        for s in &c.map.segs {
            let [rbl, rbc, rbn, rsl, rsc, rsn] = *s;
            let rb_text = u16_slice(rb[rbl as usize], rbc, rbn);
            let rs_text = u16_slice(rs[rsl as usize], rsc, rsn);
            let rb_word = rb_text.trim_start_matches('@');
            if !is_ident(rb_word) || !is_ident(rs_text.trim_start_matches("r#")) {
                continue; // literals, operators, whole signatures in comments...
            }
            if norm(rb_word) != norm(&rs_text) && !allowed_rename(rb_word, &rs_text) {
                bad.push(format!("{}:{}  Ruby `{rb_text}` -> Rust `{rs_text}` (line {})", c.name, rbl + 1, rsl + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "position map points at the wrong identifier:\n  {}\n\nAn intended rename belongs in emit::METHOD_RENAMES, MACRO_RENAMES or KEYWORD_RENAMES.",
        bad.join("\n  ")
    );
}

/// Identifier tokens of the Rust source with their 0-based (line, utf16 col), keywords/lifetimes excluded.
fn rust_identifiers(src: &str) -> Vec<(String, u32, u32)> {
    fn walk(ts: TokenStream, lines: &[&str], out: &mut Vec<(String, u32, u32)>) {
        let mut prev_quote = false;
        for tt in ts {
            match &tt {
                TokenTree::Group(g) => walk(g.stream(), lines, out),
                TokenTree::Ident(id) => {
                    let s = id.to_string();
                    let lc = id.span().start();
                    let erased = ERASED_BY_DESIGN.contains(&s.as_str()) || DROPPED_METHODS.contains(&s.as_str());
                    if !prev_quote && !erased && !RUST_KEYWORDS.contains(&s.as_str()) && lc.line > 0 {
                        let line = lines.get(lc.line - 1).copied().unwrap_or("");
                        let col16: u32 = line.chars().take(lc.column).map(|c| c.len_utf16() as u32).sum();
                        out.push((s, (lc.line - 1) as u32, col16));
                    }
                }
                _ => {}
            }
            prev_quote = matches!(&tt, TokenTree::Punct(p) if p.as_char() == '\'');
        }
    }
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out = vec![];
    walk(TokenStream::from_str(src).unwrap(), &lines, &mut out);
    out
}

#[test]
fn rust_identifiers_are_reachable_from_the_view() {
    let mut report = vec![];
    for path in cases() {
        let c = run(&path);
        let idents = rust_identifiers(&c.rust);
        let covered = |l: u32, col: u32| c.map.segs.iter().any(|s| s[3] == l && s[4] <= col && col < s[4] + s[5]);
        let missing: Vec<String> =
            idents.iter().filter(|(_, l, col)| !covered(*l, *col)).map(|(s, l, _)| format!("{s}@{}", l + 1)).collect();
        let ratio = 1.0 - missing.len() as f64 / idents.len().max(1) as f64;
        if ratio < MIN_COVERAGE {
            report.push(format!(
                "{}: {:.1}% of {} identifiers mapped; unmapped: {}",
                c.name,
                ratio * 100.0,
                idents.len(),
                missing.join(", ")
            ));
        }
    }
    assert!(
        report.is_empty(),
        "identifiers unreachable from the Ruby view (minimum {:.0}%):\n  {}",
        MIN_COVERAGE * 100.0,
        report.join("\n  ")
    );
}

#[test]
fn views_are_valid_ruby_unless_marked() {
    if Command::new("ruby").arg("--version").output().is_err() {
        eprintln!("ruby not installed; skipping `ruby -c` checks");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rubylens-golden-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut bad = vec![];
    for path in cases() {
        let c = run(&path);
        let marked_invalid = c.rust.contains("rubylens-golden: ruby-invalid");
        let file = dir.join(format!("{}.rb", c.name));
        fs::write(&file, &c.ruby).unwrap();
        let out = Command::new("ruby").arg("-c").arg(&file).output().unwrap();
        match (out.status.success(), marked_invalid) {
            (false, false) => bad.push(format!(
                "{}: not valid Ruby: {}",
                c.name,
                String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")
            )),
            (true, true) => {
                bad.push(format!("{}: now valid Ruby — remove its `rubylens-golden: ruby-invalid` marker", c.name))
            }
            _ => {}
        }
    }
    let _ = fs::remove_dir_all(&dir);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn every_case_is_specified_and_every_spec_reference_exists() {
    let spec = fs::read_to_string(root().join("docs/spec/translation.md")).expect("docs/spec/translation.md");
    let referenced: BTreeSet<String> = spec
        .match_indices("golden/")
        .map(|(i, m)| spec[i + m.len()..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect())
        .filter(|s: &String| !s.is_empty())
        .collect();
    let existing: BTreeSet<String> = cases().iter().map(|p| name(p)).collect();
    let unspecified: Vec<_> = existing.difference(&referenced).collect();
    let dangling: Vec<_> = referenced.difference(&existing).collect();
    assert!(unspecified.is_empty(), "golden cases not referenced in docs/spec/translation.md: {unspecified:?}");
    assert!(dangling.is_empty(), "docs/spec/translation.md references missing golden cases: {dangling:?}");
}
