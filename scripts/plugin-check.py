#!/usr/bin/env python3
"""The agent plugin (plugins/rubylens, listed by .claude-plugin/marketplace.json) must stay true to
RubyLens: other people's agents follow these skills, so a stale command, flag or example is a bug.

  scripts/plugin-check.py          (run by scripts/check.sh, after the release build)

Checks, each named in its failure message:
  manifests   marketplace.json and plugin.json are complete and agree; the source path exists
  version     plugin.json, Cargo.toml and the extension's package.json have one version
  format      skills follow the Agent Skills format, so any agent can load them
  commands    every `rubylens <command> --flag` the skills show exists (from the CLI's usage text)
  examples    every Ruby snippet the skills show is real RubyLens output (tests/golden, the demo)
  messages    every `# RubyLens…` line the skills quote is still produced by src/ or the extension
  extension   command titles, settings and keys the skills name exist in the extension
  links       links into this repository name files that exist
  excerpt     the excerpt helper finds every demo function's Ruby, and reports parse failures
  claude      `claude plugin validate` passes (only if the claude CLI is installed)
"""
import glob
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
os.chdir(ROOT)
PLUGIN = "plugins/rubylens"
EXE = os.path.abspath("target/release/rubylens")
SPEC_FIELDS = {"name", "description", "license", "compatibility", "metadata", "allowed-tools"}
problems = []


def bad(check, msg):
    problems.append(f"{check}: {msg}")


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def code_of(text):
    """Code spans and the bodies of fenced blocks, where commands are written."""
    fence = r"^[ \t]*(```|~~~)[^\n]*\n(.*?)^[ \t]*\1"  # fenced blocks, also indented in lists
    blocks = re.findall(fence, text, flags=re.S | re.M)
    rest = re.sub(fence, "", text, flags=re.S | re.M)
    return [b for _, b in blocks] + re.findall(r"`([^`\n]+)`", rest)


skills = sorted(glob.glob(f"{PLUGIN}/skills/*/SKILL.md"))
docs = sorted(set(skills + glob.glob(f"{PLUGIN}/skills/*/*.md")))
texts = {p: read(p) for p in docs}

# ---------------------------------------------------------------- manifests, version
try:
    market = json.loads(read(".claude-plugin/marketplace.json"))
    plugin = json.loads(read(f"{PLUGIN}/.claude-plugin/plugin.json"))
except (OSError, ValueError) as e:
    sys.exit(f"manifests: {e}")
for key in ("name", "owner", "plugins"):
    if key not in market:
        bad("manifests", f"marketplace.json has no {key!r}")
entries = {p.get("name"): p for p in market.get("plugins", [])}
entry = entries.get(plugin.get("name"))
if not entry:
    bad("manifests", f"marketplace.json lists no plugin named {plugin.get('name')!r} (plugin.json's name)")
else:
    src = entry.get("source", "")
    if not isinstance(src, str) or not src.startswith("./") or ".." in src:
        bad("manifests", f"the plugin's source must be a ./relative path without '..', not {src!r}")
    elif os.path.normpath(src) != os.path.normpath(PLUGIN):
        bad("manifests", f"the plugin's source {src!r} isn't {PLUGIN}")
if not re.fullmatch(r"[a-z0-9]+(-[a-z0-9]+)*", plugin.get("name", "")):
    bad("manifests", "plugin.json's name must be kebab-case")

cargo = re.search(r'^\[package\][^\[]*?^version\s*=\s*"([^"]+)"', read("Cargo.toml"), flags=re.M | re.S)
ext = json.loads(read("editors/vscode/package.json"))
versions = {"plugin.json": plugin.get("version"), "Cargo.toml": cargo and cargo.group(1),
            "editors/vscode/package.json": ext.get("version")}
if len(set(versions.values())) != 1:
    bad("version", "one version for the project; bump them together: "
        + ", ".join(f"{k} {v}" for k, v in versions.items()))

# ---------------------------------------------------------------- format
contributor = {os.path.basename(d) for d in glob.glob(".claude/skills/*")}
if not skills:
    bad("format", f"no skills under {PLUGIN}/skills/")
for path in skills:
    text, folder = texts[path], os.path.basename(os.path.dirname(path))
    m = re.match(r"---\n(.*?)\n---\n", text, flags=re.S)
    if not m:
        bad("format", f"{path} has no frontmatter")
        continue
    front = dict(re.findall(r"^([A-Za-z_-]+):\s*(.*)$", m.group(1), flags=re.M))
    for key in sorted(set(front) - SPEC_FIELDS):
        bad("format", f"{path}: {key!r} isn't an Agent Skills field, so other agents may reject the skill")
    name, desc = front.get("name", ""), front.get("description", "").strip("\"'")
    if name != folder:
        bad("format", f"{path}: name {name!r} must equal its folder {folder!r}")
    if not re.fullmatch(r"[a-z0-9]+(-[a-z0-9]+)*", name) or len(name) > 64:
        bad("format", f"{path}: name must be kebab-case and at most 64 characters")
    if not 1 <= len(desc) <= 1024:
        bad("format", f"{path}: description must be 1 to 1024 characters, not {len(desc)}")
    if len(front.get("compatibility", "")) > 500:
        bad("format", f"{path}: compatibility must be at most 500 characters")
    if text.count("\n") >= 500:
        bad("format", f"{path}: keep SKILL.md under 500 lines; move detail to a file beside it")
    if name in contributor:
        bad("format", f"{path}: {name!r} is also a contributor skill in .claude/skills; pick another name")

# ---------------------------------------------------------------- commands
run = subprocess.run([EXE], capture_output=True, text=True)
usage = {}
for line in run.stderr.splitlines():
    m = re.match(r"\s*rubylens ([a-z]+)\b(.*)", line)
    if m:
        usage[m.group(1)] = set(re.findall(r"--[a-z][a-z-]*", m.group(2)))
if not usage:
    sys.exit(f"commands: couldn't read the usage text of {EXE} (run cargo build --release)")
for path, text in texts.items():
    for code in code_of(text):
        for m in re.finditer(r"(?:^|[\s(|;&])rubylens ([a-z][a-z-]*)([^\n|;&)]*)", code):
            cmd, rest = m.group(1), m.group(2)
            if cmd not in usage:
                bad("commands", f"{path}: `rubylens {cmd}` isn't a rubylens command ({', '.join(sorted(usage))})")
                continue
            for flag in re.findall(r"(?<![\w-])--[a-z][a-z-]*", rest):
                if flag not in usage[cmd]:
                    bad("commands", f"{path}: `rubylens {cmd}` has no {flag} (it takes {sorted(usage[cmd])})")

# ---------------------------------------------------------------- examples, messages
outputs = {p: [l.strip() for l in read(p).splitlines()] for p in
           sorted(glob.glob("tests/golden/*.rb")) + ["examples/demo/shapes.rb"]}
every_line = [l for lines in outputs.values() for l in lines]


def contiguous(seq, lines):
    lines = [l for l in lines if l]
    return any(lines[i:i + len(seq)] == seq for i in range(len(lines) - len(seq) + 1))


for path, text in texts.items():
    for table in re.findall(r"^\| In the view \|.*\n\|[-| ]+\|\n((?:\|.*\n)*)", text, flags=re.M):
        for row in table.splitlines():
            cell = re.split(r"(?<!\\)\|", row)[1]
            for span in re.findall(r"`([^`]+)`", cell):
                span = span.replace("\\|", "|")
                if not any(span in line for line in every_line):
                    bad("examples", f"{path}: `{span}` isn't in any RubyLens output (tests/golden/*.rb, the demo view)")
    for block in re.findall(r"^[ \t]*```ruby\n(.*?)^[ \t]*```", text, flags=re.S | re.M):
        cut = [l.strip() for l in block.splitlines()]
        parts, seq = [], []
        for l in cut + ["# …"]:
            if l == "# …":
                parts, seq = parts + ([seq] if seq else []), []
            elif l:
                seq.append(l)
        for seq in parts:
            if not any(contiguous(seq, lines) for lines in outputs.values()):
                bad("examples", f"{path}: this Ruby isn't RubyLens output: {' / '.join(seq)[:120]}")

sources = "".join(read(p) for p in glob.glob("src/*.rs") + ["editors/vscode/extension.js"])
for path, text in texts.items():
    for span in re.findall(r"`(# RubyLens[^`]*)`", text):
        stem = span.rstrip(" .…")
        if stem.endswith("…"):
            stem = stem[:-1].rstrip()
        if stem not in sources:
            bad("messages", f"{path}: RubyLens no longer writes `{span}` (looked in src/ and extension.js)")

# ---------------------------------------------------------------- extension
titles = {f"{c.get('category', '')}: {c['title']}" for c in ext["contributes"]["commands"]}
names = set(ext["contributes"]["configuration"]["properties"]) | {c["command"] for c in ext["contributes"]["commands"]}
keys = {k.get(f) for k in ext["contributes"]["keybindings"] for f in ("key", "mac")}
for path, text in texts.items():
    for title in re.findall(r"\*\*(RubyLens: [^*]+)\*\*", text):
        if title not in titles:
            bad("extension", f"{path}: there's no command {title!r} in editors/vscode/package.json")
    for name in re.findall(r"`(rubylens\.[A-Za-z.]+)`", text):
        if name not in names:
            bad("extension", f"{path}: {name} is neither a setting nor a command of the extension")
    for key in re.findall(r"`((?:Ctrl|Cmd|Alt|Shift)\+[A-Za-z0-9+]+)`", text):
        if key.lower() not in keys:
            bad("extension", f"{path}: no extension keybinding is {key}")

# ---------------------------------------------------------------- links
repo_link = re.compile(r"https://(?:github\.com/stowersjoshua/rubylens/(?:blob|raw|tree)/master|"
                       r"raw\.githubusercontent\.com/stowersjoshua/rubylens/master)/([^\s)`\"'>#]+)")
for path in docs + ["README.md"]:
    for target in repo_link.findall(read(path)):
        if not os.path.exists(target):
            bad("links", f"{path}: links to {target}, which doesn't exist in this repository")

# ---------------------------------------------------------------- excerpt
helper = f"{PLUGIN}/skills/read-rust-as-ruby/scripts/excerpt.py"
env = dict(os.environ, RUBYLENS=EXE)
demo = "examples/demo/src/shapes.rs"


def excerpt(*args):
    return subprocess.run([sys.executable, helper, *args], capture_output=True, text=True, env=env)


# --root as for the committed demo view, so the cache lands in examples/demo/.rubylens
view = json.loads(subprocess.run([EXE, "gen", demo, "--root", "examples/demo", "--json"],
                                 capture_output=True, text=True).stdout)["ruby"].split("\n")
fns = [(i + 1, m.group(1)) for i, l in enumerate(read(demo).splitlines())
       if (m := re.match(r"\s*(?:pub(?:\([a-z]+\))? )?fn (\w+)", l))]


def parent_mark(lines, i):
    """What the view's `== in:` line must show for a Rust fn on line i that's inside an impl, trait or mod."""
    for l in reversed(lines[:i - 1]):
        if l.strip() and not l[:1].isspace():
            m = re.match(r"(?:pub(?:\([a-z]+\))? )?(impl|trait|mod)\b\s*([A-Za-z_]*)", l)
            if not m:
                return None
            kind, name = m.groups()
            return {"impl": "# impl", "trait": "# trait"}.get(kind) or f"module {name.title().replace('_', '')}"
    return None


demo_lines = read(demo).splitlines()
marks = {line: parent_mark(demo_lines, line) for line, _ in fns if demo_lines[line - 1][:1].isspace()}
for line, name in fns:
    r = excerpt(demo, str(line), "--root", "examples/demo")
    head, _, body = r.stdout.partition("\n")
    m = re.fullmatch(r"== (.+):(\d+)-(\d+)  ->  (.+):(\d+)-(\d+)", head)
    if r.returncode != 0 or not m:
        bad("excerpt", f"{helper} {demo} {line} failed: {r.stderr.strip() or head}")
        continue
    if not int(m.group(2)) <= line <= int(m.group(3)):
        bad("excerpt", f"fn {name}: reported Rust lines {m.group(2)}-{m.group(3)} don't include line {line}")
    crumbs = []
    if body.startswith("== in: "):
        context, _, body = body.partition("\n")
        crumbs = context[len("== in: "):].split(" > ")
    above = {l.strip() for l in view[:int(m.group(5)) - 1]}
    for crumb in crumbs:
        if crumb not in above:
            bad("excerpt", f"fn {name}: the enclosing line {crumb!r} isn't a view line above the excerpt")
    mark = marks.get(line)
    if mark and not any(mark in c for c in crumbs):
        bad("excerpt", f"fn {name} (line {line}): the excerpt's `== in:` line should show its {mark!r}, got {crumbs}")
    if not re.search(rf"^\s*def (self\.)?{name}\b", body, flags=re.M):
        bad("excerpt", f"fn {name} (line {line}): the excerpt has no def {name}")
    shown = [l.strip() for l in body.rstrip("\n").split("\n")]
    if shown != [l.strip() for l in view[int(m.group(5)) - 1:int(m.group(6))]]:
        bad("excerpt", f"fn {name}: the excerpt isn't view lines {m.group(5)}-{m.group(6)} as RubyLens wrote them")
if len(fns) < 5:
    bad("excerpt", f"expected the demo to have several functions, found {len(fns)}")
with tempfile.TemporaryDirectory() as tmp:
    broken = os.path.join(tmp, "broken.rs")
    with open(broken, "w") as f:
        f.write("fn half(\n")
    r = excerpt(broken, "1")
    if r.returncode != 1 or "couldn't parse" not in r.stderr:
        bad("excerpt", f"a file that doesn't parse must fail with \"couldn't parse\", got: {r.stderr.strip()!r}")

# ---------------------------------------------------------------- claude
if shutil.which("claude"):
    for target in (".", PLUGIN):
        try:
            r = subprocess.run(["claude", "plugin", "validate", target], capture_output=True, text=True, timeout=120)
        except subprocess.TimeoutExpired:
            print(f"claude plugin validate {target}: timed out; skipped")
            continue
        if r.returncode != 0:
            bad("claude", f"claude plugin validate {target}:\n{r.stdout}{r.stderr}")

if problems:
    sys.exit("\n".join(problems))
print(f"{len(skills)} skills, {len(fns)} demo functions excerpted, all consistent")
