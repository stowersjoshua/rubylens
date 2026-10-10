//! `rubylens bench` — in-process generator benchmark (no process-spawn noise, single-threaded).
//!
//!   rubylens bench <file-or-dir>... [--repeat N] [--top K] [--digest FILE]
//!
//! Reports per-phase totals (read, parse, emit, map JSON), throughput, per-file latency
//! percentiles and the slowest files. `--digest FILE` writes one `sha256  path` line per file
//! over the generated Ruby + map, so optimizations can be proven output-identical:
//!
//!   rubylens bench <corpus> --digest before.txt   # on the old code
//!   rubylens bench <corpus> --digest after.txt    # on the new code
//!   diff before.txt after.txt                   # must be empty
//!
//! See docs/performance.md.

use crate::emit::Gen;
use crate::{GEN_VERSION, SourceMap, module_path};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Default, Clone, Copy)]
struct Phases {
    read: Duration,
    parse: Duration,
    emit: Duration,
    json: Duration,
}

impl Phases {
    fn total(&self) -> Duration {
        self.read + self.parse + self.emit + self.json
    }
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|e| e == "rs") {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(rd) = fs::read_dir(path) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        collect(&p, out);
    }
}

/// One generation with phase timings; returns (phases, digest of ruby + map if asked, bytes).
fn measure(file: &Path, kinds: &crate::KindMap, digest: bool) -> (Phases, String, usize) {
    let mut ph = Phases::default();
    let t = Instant::now();
    let src = fs::read_to_string(file).unwrap_or_default();
    ph.read = t.elapsed();

    let t = Instant::now();
    let parsed = syn::parse_file(&src);
    ph.parse = t.elapsed();

    let (ruby, map) = match parsed {
        Ok(ast) => {
            let t = Instant::now();
            let mut g = Gen::new(&src, kinds);
            g.file(&ast, &module_path(file));
            let out = g.out;
            let map = SourceMap {
                version: GEN_VERSION,
                hash: String::new(),
                deps: g.deps,
                segs: out.segs,
                blocks: out.blocks,
            };
            ph.emit = t.elapsed();
            (out.buf, map)
        }
        Err(_) => (String::new(), SourceMap::default()),
    };

    let t = Instant::now();
    let json = serde_json::to_string(&map).unwrap();
    ph.json = t.elapsed();

    // proc-macro2 keeps every parsed file's line table in a thread-local map that only grows;
    // clear it so each file is measured the way `rubylens gen` sees it (one file per process).
    proc_macro2::extra::invalidate_current_thread_spans();

    if !digest {
        return (ph, String::new(), src.len());
    }
    let mut h = Sha256::new();
    h.update(ruby.as_bytes());
    h.update(json.as_bytes());
    let digest = h.finalize().iter().take(12).map(|b| format!("{b:02x}")).collect();
    (ph, digest, src.len())
}

fn pct(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn ms(d: Duration) -> String {
    format!("{:.2} ms", d.as_secs_f64() * 1e3)
}

pub fn run(args: &[String]) {
    let mut paths = vec![];
    let mut repeat = 1usize;
    let mut top = 10usize;
    let mut digest_out: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--repeat" => {
                repeat = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(1).max(1);
                i += 1;
            }
            "--top" => {
                top = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(10);
                i += 1;
            }
            "--digest" => {
                digest_out = args.get(i + 1).map(PathBuf::from);
                i += 1;
            }
            p => paths.push(PathBuf::from(p)),
        }
        i += 1;
    }
    let mut files = vec![];
    for p in &paths {
        collect(p, &mut files);
    }
    if files.is_empty() {
        eprintln!("rubylens bench: no .rs files under {paths:?}");
        std::process::exit(2);
    }
    let kinds = crate::KindMap::default();

    // Each file: best of `repeat` runs (least noisy estimate of the cost).
    let mut rows: Vec<(PathBuf, usize, Phases, String)> = Vec::with_capacity(files.len());
    let wall = Instant::now();
    for f in &files {
        let mut best: Option<(Phases, String, usize)> = None;
        for _ in 0..repeat {
            let m = measure(f, &kinds, digest_out.is_some());
            if best.as_ref().is_none_or(|b| m.0.total() < b.0.total()) {
                best = Some(m);
            }
        }
        let (ph, dg, bytes) = best.unwrap();
        rows.push((f.clone(), bytes, ph, dg));
    }
    let wall = wall.elapsed();

    let mut sum = Phases::default();
    let mut bytes = 0usize;
    for (_, b, ph, _) in &rows {
        bytes += b;
        sum.read += ph.read;
        sum.parse += ph.parse;
        sum.emit += ph.emit;
        sum.json += ph.json;
    }
    let mut lat: Vec<Duration> = rows.iter().map(|r| r.2.total()).collect();
    lat.sort();
    let total = sum.total();
    let share = |d: Duration| 100.0 * d.as_secs_f64() / total.as_secs_f64().max(1e-9);

    let mut o = String::new();
    let _ = writeln!(o, "{} files, {:.1} MB, best of {repeat} (wall {:.2?})", rows.len(), bytes as f64 / 1e6, wall);
    let _ = writeln!(o, "  total  {:>10}   {:.1} MB/s", ms(total), bytes as f64 / 1e6 / total.as_secs_f64().max(1e-9));
    for (name, d) in [("read", sum.read), ("parse", sum.parse), ("emit", sum.emit), ("json", sum.json)] {
        let _ = writeln!(o, "  {name:<6} {:>10}   {:>5.1}%", ms(d), share(d));
    }
    let _ = writeln!(
        o,
        "  per file: p50 {}  p95 {}  p99 {}  max {}",
        ms(pct(&lat, 0.5)),
        ms(pct(&lat, 0.95)),
        ms(pct(&lat, 0.99)),
        ms(*lat.last().unwrap())
    );
    if top > 0 {
        rows.sort_by_key(|r| std::cmp::Reverse(r.2.total()));
        let _ = writeln!(o, "  slowest:");
        for (p, b, ph, _) in rows.iter().take(top) {
            let us_per_kb = ph.total().as_secs_f64() * 1e6 / (*b as f64 / 1024.0).max(0.001);
            let _ = writeln!(
                o,
                "    {:>10}  {:>7.0} µs/KB  parse {:>4.0}%  emit {:>4.0}%  {:>8} B  {}",
                ms(ph.total()),
                us_per_kb,
                100.0 * ph.parse.as_secs_f64() / ph.total().as_secs_f64(),
                100.0 * ph.emit.as_secs_f64() / ph.total().as_secs_f64(),
                b,
                p.display()
            );
        }
    }
    print!("{o}");

    if let Some(out) = digest_out {
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        let text: String = rows.iter().map(|(p, _, _, d)| format!("{d}  {}\n", p.display())).collect();
        fs::write(&out, text).expect("write digest");
        eprintln!("digest: {} files -> {}", rows.len(), out.display());
    }
}
