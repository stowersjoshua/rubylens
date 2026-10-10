#!/usr/bin/env python3
"""Regression check over a pinned corpus of real crates (tests/corpus/Cargo.lock).

For every .rs file in every corpus crate:
  * rubylens must not crash, and should parse it (raw-source fallback counts as "unparsed")
  * the view should be valid Ruby (`ruby -c`)
  * every mapped identifier must land on the same identifier in Rust, or an intended rename
    from src/emit.rs (METHOD_RENAMES / MACRO_RENAMES / KEYWORD_RENAMES)
  * Rust identifiers must stay reachable from the view (coverage), so a change that silently
    stops mapping something is caught even though nothing maps *wrongly*

Fails (exit 1) if any threshold is missed. Thresholds reflect the current baseline; raise them
when you improve things, never lower them to make a change pass without saying why in the PR.

    scripts/corpus-check.py [--rubylens target/release/rubylens] [--jobs N] [--show 10]
"""
import argparse
import concurrent.futures as cf
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = os.path.join(ROOT, "tests/corpus/Cargo.toml")
DEFAULT_RUBYLENS = os.path.join(ROOT, "target/release/rubylens")

# Baseline (corpus as pinned): 2,789 files, 97.81% valid Ruby, 1 unparsed, 2 of 2.1M
# identifier tokens mismatched, 99.07% of 2.3M identifiers reachable. Thresholds sit just
# below it. See docs/testing.md.
MIN_VALID_RUBY = 97.7   # percent of parsed files
MAX_UNPARSED = 1        # files that fall back to raw source
MAX_MISMATCH = 0.01     # percent of checked identifier tokens
MAX_CRASHES = 0
MIN_COVERAGE = 98.8     # percent of Rust identifier occurrences reachable from the view

IDENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
RUST_KEYWORDS = set("""as async await break const continue crate dyn else enum extern false fn for if impl in let loop
match mod move mut pub ref return self Self static struct super trait true type unsafe use where while union
macro_rules""".split())
# Comments, strings and char literals are blanked (keeping columns) before finding identifiers.
NOISE = re.compile(r'//[^\n]*|/\*.*?\*/|b?r(#*)".*?"\1|b?"(?:\\.|[^"\\])*"|b?\'(?:\\.|[^\'\\])\'', re.S)
WORD = re.compile(r"(?<!['\w])(?:r#)?[A-Za-z_][A-Za-z0-9_]*")


def erased_by_design():
    """Identifiers the view drops on purpose: DROPPED_METHODS plus Some/_/collect (as tests/golden.rs)."""
    src = open(os.path.join(ROOT, "src/emit.rs"), encoding="utf-8").read()
    m = re.search(r"pub const DROPPED_METHODS\b[^=]*=\s*&\[(.*?)\];", src, re.S)
    return set(re.findall(r'"([^"]+)"', m.group(1))) | {"Some", "_", "collect"}


def rust_identifiers(text, erased):
    """(line, utf16_col) of identifier occurrences the view should make reachable."""
    blanked = NOISE.sub(lambda m: re.sub(r"[^\n]", " ", m.group(0)), text)
    out = []
    for ln, line in enumerate(blanked.split("\n")):
        for m in WORD.finditer(line):
            w = m.group(0)
            if w in RUST_KEYWORDS or w in erased:
                continue
            out.append((ln, len(line[:m.start()].encode("utf-16-le")) // 2))
    return out


def renames():
    """(rust, ruby) pairs from the tables in src/emit.rs — the single source of truth."""
    src = open(os.path.join(ROOT, "src/emit.rs"), encoding="utf-8").read()
    pairs = set()
    for table in ("METHOD_RENAMES", "MACRO_RENAMES", "KEYWORD_RENAMES"):
        m = re.search(r"pub const " + table + r"\b[^=]*=\s*&\[(.*?)\];", src, re.S)
        if not m:
            sys.exit(f"could not find {table} in src/emit.rs")
        pairs |= set(re.findall(r'\("([^"]+)",\s*"([^"]*)"\)', m.group(1)))
    return pairs


def norm(s):
    return s.removeprefix("r#").lstrip("@").rstrip("_").replace("_", "").lower()


def u16(line, col, n):
    b = line.encode("utf-16-le")
    return b[col * 2:(col + n) * 2].decode("utf-16-le", errors="replace")


def corpus_files():
    subprocess.run(["cargo", "fetch", "--quiet", "--manifest-path", MANIFEST], check=True)
    meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1", "--manifest-path", MANIFEST],
                                     check=True, capture_output=True, text=True).stdout)
    files = []
    for p in meta["packages"]:
        if not (p.get("source") or "").startswith("registry"):
            continue
        crate_dir = os.path.dirname(p["manifest_path"])
        for dirpath, dirnames, filenames in os.walk(crate_dir):
            dirnames[:] = [d for d in dirnames if d not in ("target", ".git")]
            files += [os.path.join(dirpath, f) for f in filenames if f.endswith(".rs")]
    return sorted(files)


def check_file(path, rubylens, tmp_root, allowed, erased):
    r = subprocess.run([rubylens, "gen", path, "--root", tmp_root, "--json", "--no-cache"], capture_output=True, text=True)
    if r.returncode != 0:
        return {"path": path, "crash": r.stderr.strip()[-300:]}
    out = json.loads(r.stdout)
    if out["ruby"].startswith("# RubyLens: could not parse"):
        return {"path": path, "unparsed": True}
    chk = subprocess.run(["ruby", "-c", out["rbPath"]], capture_output=True, text=True)
    rb_lines = out["ruby"].split("\n")
    rs_text = open(path, encoding="utf-8", errors="replace").read()
    rs_lines = rs_text.split("\n")
    checked, bad = 0, []
    by_line = {}
    for s in out["map"]["segs"]:
        by_line.setdefault(s[3], []).append((s[4], s[4] + s[5]))
    idents = rust_identifiers(rs_text, erased)
    covered = sum(1 for ln, col in idents if any(a <= col < b for a, b in by_line.get(ln, ())))
    for rbl, rbc, rbn, rsl, rsc, rsn in out["map"]["segs"]:
        rb = u16(rb_lines[rbl], rbc, rbn).lstrip("@")
        rs = u16(rs_lines[rsl], rsc, rsn) if rsl < len(rs_lines) else ""
        if not IDENT.match(rb) or not IDENT.match(rs.removeprefix("r#")):
            continue
        checked += 1
        if norm(rb) != norm(rs) and (rs, rb) not in allowed and rs != "Self":
            bad.append(f"{rb}<-{rs}@{rsl + 1}")
    return {"path": path, "valid": chk.returncode == 0, "error": chk.stderr.strip().split("\n")[0] if chk.returncode else "",
            "checked": checked, "bad": bad, "idents": len(idents), "covered": covered}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--rubylens", default=DEFAULT_RUBYLENS, help="binary to test (default: rebuilt from this checkout)")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    ap.add_argument("--show", type=int, default=10, help="examples of each failure kind to print")
    a = ap.parse_args()
    for tool in ("ruby", "cargo"):
        if not shutil.which(tool):
            sys.exit(f"corpus-check needs `{tool}` on PATH")
    if a.rubylens == DEFAULT_RUBYLENS:
        # Always rebuild: checking a stale binary after editing src/ would report a false OK.
        subprocess.run(["cargo", "build", "--release", "--quiet", "--manifest-path", os.path.join(ROOT, "Cargo.toml")], check=True)
    if not os.access(a.rubylens, os.X_OK):
        sys.exit(f"rubylens binary not found: {a.rubylens}")

    allowed = renames()
    erased = erased_by_design()
    files = corpus_files()
    tmp_root = tempfile.mkdtemp(prefix="rubylens-corpus-")
    try:
        with cf.ThreadPoolExecutor(a.jobs) as ex:
            results = list(ex.map(lambda f: check_file(f, a.rubylens, tmp_root, allowed, erased), files))
    finally:
        shutil.rmtree(tmp_root, ignore_errors=True)

    crashes = [r for r in results if "crash" in r]
    unparsed = [r for r in results if r.get("unparsed")]
    parsed = [r for r in results if "valid" in r]
    invalid = [r for r in parsed if not r["valid"]]
    checked = sum(r["checked"] for r in parsed)
    mismatches = [(r["path"], b) for r in parsed for b in r["bad"]]
    valid_pct = 100 * (len(parsed) - len(invalid)) / max(len(parsed), 1)
    mismatch_pct = 100 * len(mismatches) / max(checked, 1)
    idents = sum(r["idents"] for r in parsed)
    coverage_pct = 100 * sum(r["covered"] for r in parsed) / max(idents, 1)

    rel = lambda p: re.sub(r"^.*/registry/src/[^/]+/", "", p)  # noqa: E731
    print(f"corpus: {len(files)} files")
    print(f"  crashes:     {len(crashes)}  (max {MAX_CRASHES})")
    print(f"  unparsed:    {len(unparsed)}  (max {MAX_UNPARSED})")
    print(f"  valid Ruby:  {valid_pct:.2f}%  (min {MIN_VALID_RUBY}%)  — {len(invalid)} invalid")
    print(f"  map:         {len(mismatches)} of {checked} identifier tokens mismatched = {mismatch_pct:.3f}%  (max {MAX_MISMATCH}%)")
    print(f"  coverage:    {coverage_pct:.2f}% of {idents} Rust identifiers reachable from the view  (min {MIN_COVERAGE}%)")
    for title, items in (("crashes", [(r["path"], r["crash"]) for r in crashes]),
                         ("invalid Ruby", [(r["path"], r["error"]) for r in invalid]),
                         ("map mismatches", mismatches)):
        if items and a.show:
            print(f"\n  {title} (first {min(a.show, len(items))}):")
            for p, msg in items[:a.show]:
                print(f"    {rel(p)}: {msg}")

    failed = (len(crashes) > MAX_CRASHES or len(unparsed) > MAX_UNPARSED
              or valid_pct < MIN_VALID_RUBY or mismatch_pct > MAX_MISMATCH or coverage_pct < MIN_COVERAGE)
    print("\nFAIL" if failed else "\nOK")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
