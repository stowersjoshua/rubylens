#!/usr/bin/env python3
"""Generation must scale linearly with input size.

Writes synthetic Rust files in the shapes that have been superlinear before (deep nesting,
long lines, long match expressions, ...) at size n and 8n, times them with `rubylens bench`
(release build, in-process, best of several runs), and fails if 8x the input takes more
than MAX_RATIO times as long. Linear is ~8x; O(n^2) is ~64x.

    scripts/scaling-check.py [--rubylens target/release/rubylens]

See docs/performance.md.
"""
import argparse
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MAX_RATIO = 20
SCALE = 8


def nested_ifs(n):
    return "fn f(a: bool, mut x: u32) {\n" + "".join("if a { x += 1;\n" for _ in range(n)) + "}" * n + "\n}\n"


def nested_mods(n):
    return "".join("mod m { pub struct S;\n" for _ in range(n)) + "}" * n + "\n"


def long_ascii_line(n):
    return "const A: [u32; %d] = [%s];\n" % (n, ", ".join(str(i) for i in range(n)))


def long_unicode_line(n):
    return "const A: [&str; %d] = [%s];\n" % (n, ", ".join('"é%d"' % i for i in range(n)))


def many_fns(n):
    return "".join("pub fn f%d(x: u32) -> u32 { x + %d }\n" % (i, i) for i in range(n))


def long_match(n):
    arms = "".join("        %d => %d,\n" % (i, i + 1) for i in range(n))
    return "fn f(x: u32) -> u32 {\n    match x {\n%s        _ => 0,\n    }\n}\n" % arms


def doc_comment(n):
    return "".join("/// line %d of the documentation\n" % i for i in range(n)) + "fn f() {}\n"


def method_chain(n):
    return "fn f(x: Vec<u32>) -> usize {\n    x" + "".join("\n        .clone()" for _ in range(n)) + "\n        .len()\n}\n"


CASES = [  # (name, generator, n): n is chosen so the small file takes a millisecond or more
    ("nested ifs", nested_ifs, 150),
    ("nested modules", nested_mods, 150),
    ("long ASCII line", long_ascii_line, 2000),
    ("long non-ASCII line", long_unicode_line, 2000),
    ("many functions", many_fns, 500),
    ("long match", long_match, 1000),
    ("long doc comment", doc_comment, 2000),
    ("long method chain", method_chain, 150),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rubylens", default=os.path.join(ROOT, "target/release/rubylens"))
    args = ap.parse_args()
    with tempfile.TemporaryDirectory() as tmp:
        files = {}
        for i, (name, gen, n) in enumerate(CASES):
            for size in (n, n * SCALE):
                path = os.path.join(tmp, "c%d_%d.rs" % (i, size))
                with open(path, "w") as f:
                    f.write(gen(size))
                files[(name, size)] = path
        out = subprocess.run(
            [args.rubylens, "bench", *files.values(), "--repeat", "5", "--top", str(len(files))],
            capture_output=True, text=True, check=True,
        ).stdout
    ms = {}
    for line in out.splitlines():
        m = re.match(r"\s+([\d.]+) ms .* (\S+\.rs)$", line)
        if m:
            ms[os.path.basename(m.group(2))] = float(m.group(1))
    failed = []
    print(f"{'case':<22} {'n':>8} {'time':>9} {f'{SCALE}n':>8} {'time':>9}  ratio (linear ~{SCALE}, limit {MAX_RATIO})")
    for name, _, n in CASES:
        a = ms[os.path.basename(files[(name, n)])]
        b = ms[os.path.basename(files[(name, n * SCALE)])]
        ratio = b / max(a, 1e-6)
        ok = ratio <= MAX_RATIO
        failed += [] if ok else [name]
        print(f"{name:<22} {n:>8} {a:>7.2f}ms {n * SCALE:>8} {b:>7.2f}ms  {ratio:5.1f}x {'ok' if ok else 'FAIL: superlinear'}")
    if failed:
        print(f"FAIL: superlinear generation for: {', '.join(failed)} (see docs/performance.md)")
        sys.exit(1)
    print("OK: generation scales linearly")


if __name__ == "__main__":
    main()
