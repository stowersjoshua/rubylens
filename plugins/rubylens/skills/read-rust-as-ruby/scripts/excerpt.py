#!/usr/bin/env python3
"""Print the part of a RubyLens view that shows some lines of a Rust file.

    python3 excerpt.py <file.rs> <line>[-<line>] [--root DIR]

Lines are 1-based and inclusive, as editors show them; run it from any folder. The first
output line names the Rust lines the excerpt really covers (a whole statement or item, so
it can be wider than asked) and where they are in the view. A second `== in:` line, when
there is one, names the enclosing modules, classes and methods. The Ruby follows, exactly
as RubyLens wrote it, with the common indentation removed.

Uses `rubylens gen --json` ($RUBYLENS, else `rubylens` on PATH) and the view's block map:
[rb_start, rb_end, rs_start, rs_end], 0-based, inclusive (docs/spec/source-map.md).
Exit status: 0 printed, 1 failed (message on stderr), 2 usage.
"""
import json
import os
import subprocess
import sys

KNOWN_MAP_VERSIONS = {2}


def fail(msg, code=1):
    print(f"excerpt: {msg}", file=sys.stderr)
    sys.exit(code)


def parse_lines(spec):
    try:
        a, _, b = spec.partition("-")
        a, b = int(a), int(b or a)
    except ValueError:
        fail(f"lines must look like 40 or 40-52, not {spec!r}", 2)
    if a < 1 or b < a:
        fail(f"bad line range {spec!r}", 2)
    return a - 1, b - 1


def inside(x, y):
    """Block x's Rust range lies within y's and isn't the same range."""
    return y[2] <= x[2] and x[3] <= y[3] and (x[2], x[3]) != (y[2], y[3])


def outermost(blocks):
    return [k for k in blocks if not any(inside(k, o) for o in blocks)]


def select(blocks, a, b):
    """The blocks whose Ruby, together, is the smallest excerpt covering Rust lines a..b."""
    meets = [k for k in blocks if k[2] <= b and k[3] >= a]
    covering = [k for k in meets if k[2] <= a and b <= k[3]]
    if not covering:  # the lines span several top-level items
        return outermost(meets)
    parent = min(covering, key=lambda k: (k[3] - k[2], k[1] - k[0]))
    kids = outermost([k for k in meets if inside(k, parent)])
    # several children (statements, methods) cover the lines between them: show just those
    if kids and min(k[2] for k in kids) <= a and max(k[3] for k in kids) >= b:
        return kids
    return [parent]


def enclosing(ruby, start):
    """The class, module and def lines around view line `start`, outermost first."""
    first = next((l for l in ruby[start:] if l.strip()), "")
    level, crumbs = len(first) - len(first.lstrip()), []
    for line in reversed(ruby[:start]):
        text = line.strip()
        if not text or text.startswith("#"):
            continue
        indent = len(line) - len(line.lstrip())
        if indent < level:
            level = indent
            if text.startswith(("class ", "module ", "def ")):
                crumbs.append(text)
            if indent == 0:
                break
    return list(reversed(crumbs))


def shown(path):
    rel = os.path.relpath(path)
    return path if rel.startswith("..") else rel


def main(argv):
    args = list(argv)
    root = None
    if "--root" in args:
        i = args.index("--root")
        if i + 1 >= len(args):
            fail("--root needs a directory", 2)
        root = args[i + 1]
        del args[i : i + 2]
    if len(args) != 2:
        fail("usage: excerpt.py <file.rs> <line>[-<line>] [--root DIR]", 2)
    file, (a, b) = args[0], parse_lines(args[1])

    exe = os.environ.get("RUBYLENS", "rubylens")
    cmd = [exe, "gen", file, "--json"] + (["--root", root] if root else [])
    try:
        run = subprocess.run(cmd, capture_output=True, text=True)
    except FileNotFoundError:
        fail(f"{exe} isn't installed or isn't on PATH (see the setup-rubylens skill)")
    if run.returncode != 0:
        fail(run.stderr.strip() or f"{exe} gen failed")
    view = json.loads(run.stdout)
    ruby, smap = view["ruby"].split("\n"), view["map"]
    if smap.get("version") not in KNOWN_MAP_VERSIONS:
        fail(f"this helper reads map version {sorted(KNOWN_MAP_VERSIONS)}, but rubylens wrote "
             f"{smap.get('version')}; read the whole view with `rubylens gen {file}` instead")
    if any(line.startswith("# RubyLens: could not parse this file") for line in ruby[:3]):
        fail(f"rubylens couldn't parse {file}, so there's no Ruby view of it: explain the Rust itself")

    blocks = select(smap["blocks"], a, b)
    if not blocks:
        fail(f"nothing in the view comes from lines {a + 1}-{b + 1} (blank or comments only?); "
             "try the first line of the item or statement")
    rb0, rb1 = min(k[0] for k in blocks), max(k[1] for k in blocks)
    rs0, rs1 = min(k[2] for k in blocks), max(k[3] for k in blocks)

    lines = ruby[rb0 : rb1 + 1]
    indent = min((len(l) - len(l.lstrip()) for l in lines if l.strip()), default=0)
    print(f"== {shown(file)}:{rs0 + 1}-{rs1 + 1}  ->  {shown(view['rbPath'])}:{rb0 + 1}-{rb1 + 1}")
    crumbs = enclosing(ruby, rb0)
    if crumbs:
        print("== in: " + " > ".join(crumbs))
    print("\n".join(l[indent:] for l in lines))


if __name__ == "__main__":
    main(sys.argv[1:])
