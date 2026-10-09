//! `rbview serve`: answers view requests over stdin/stdout, so an editor keeps one rbview process
//! instead of starting one per view. The protocol is in `docs/spec/serve.md`.
//!
//! One JSON request per line in, one JSON response per line out, matched by `id`. Requests run
//! in parallel on every core, so responses can arrive in any order. The process holds no state
//! that can go stale: views live in `.rbview/` exactly as with `rbview gen`, and the only thing
//! kept in memory, each root's `symbols.json`, is reloaded whenever that file changes.
//!
//! It exits when stdin closes, after `--idle-exit` seconds with nothing to do (default 600; 0
//! never), or when its own binary is replaced, so an upgrade takes effect without restarting the
//! editor. Requests it didn't answer are the client's to retry (the VS Code extension falls back
//! to `rbview gen`).

use crate::GEN_VERSION;
use crate::cache::{CACHE_DIR, Kinds, gen_cached, resolve};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime};

/// Identifies one version of a file without reading it. A replaced file (new inode, length or
/// time) has a different stamp.
#[derive(PartialEq, Clone, Copy, Debug)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    ino: u64,
}

fn stamp(path: &Path) -> Option<Stamp> {
    let m = fs::metadata(path).ok()?;
    #[cfg(unix)]
    let ino = std::os::unix::fs::MetadataExt::ino(&m);
    #[cfg(not(unix))]
    let ino = 0;
    Some(Stamp { len: m.len(), modified: m.modified().ok(), ino })
}

/// Each root's symbol kinds, kept while its `symbols.json` is unchanged. `Kinds` loads lazily,
/// so a root whose views never need kinds never reads the file.
#[derive(Default)]
struct KindsByRoot(Mutex<HashMap<PathBuf, Loaded>>);

/// A root's kinds and the stamp of the `symbols.json` they came from (None: there wasn't one).
type Loaded = (Option<Stamp>, Arc<Kinds>);

impl KindsByRoot {
    fn get(&self, root: &Path) -> Arc<Kinds> {
        let cache = root.join(CACHE_DIR);
        // stat before any read: if the file is replaced in between, the next request reloads
        let now = stamp(&cache.join("symbols.json"));
        let mut roots = self.0.lock().unwrap();
        match roots.get(root) {
            Some((was, kinds)) if *was == now => kinds.clone(),
            _ => {
                let kinds = Arc::new(Kinds::lazy(&cache));
                roots.insert(root.to_path_buf(), (now, kinds.clone()));
                kinds
            }
        }
    }
}

/// The `result` of one request, as JSON, or an error message.
fn answer(req: &Value, kinds: &KindsByRoot) -> Result<String, String> {
    let field = |k: &str| req.get(k).and_then(Value::as_str);
    match field("method") {
        Some("ping") => Ok(format!("{{\"version\":{GEN_VERSION},\"build\":\"{}\"}}", env!("RBVIEW_BUILD_ID"))),
        Some("gen") => {
            let file = field("file").ok_or("a gen request needs \"file\"")?;
            let (file, root) =
                resolve(Path::new(file), field("root").map(Path::new)).map_err(|e| format!("{file}: {e}"))?;
            let no_cache = req.get("noCache").and_then(Value::as_bool).unwrap_or(false);
            let view = gen_cached(&file, &root, &kinds.get(&root), !no_cache).map_err(|e| e.to_string())?;
            Ok(view.to_json())
        }
        Some(other) => Err(format!("unknown method {other:?}")),
        None => Err("a request needs \"method\"".into()),
    }
}

/// One response line for one request line. Never panics: a panic while generating is reported
/// as that request's error, and the worker carries on.
fn respond(line: &str, kinds: &KindsByRoot) -> String {
    let (id, result) = match serde_json::from_str::<Value>(line) {
        Err(e) => (Value::Null, Err(format!("not a JSON request: {e}"))),
        Ok(req) => {
            let id = req.get("id").cloned().unwrap_or(Value::Null);
            let result = catch_unwind(AssertUnwindSafe(|| answer(&req, kinds))).unwrap_or_else(|panic| {
                let msg = panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic");
                Err(format!("rbview failed on this request: {msg}"))
            });
            (id, result)
        }
    };
    // proc-macro2 keeps every parsed file's line table in a thread-local map; let it go
    proc_macro2::extra::invalidate_current_thread_spans();
    match result {
        Ok(json) => format!("{{\"id\":{id},\"result\":{json}}}"),
        Err(msg) => format!("{{\"id\":{id},\"error\":{}}}", Value::String(msg)),
    }
}

pub fn run(args: &[String]) {
    let mut idle_exit = Duration::from_secs(600);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--idle-exit" => {
                let secs = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                    eprintln!("rbview serve: --idle-exit needs a number of seconds");
                    std::process::exit(2)
                });
                idle_exit = Duration::from_secs(secs);
                i += 1;
            }
            other => {
                eprintln!("rbview serve: unknown option {other}");
                std::process::exit(2)
            }
        }
        i += 1;
    }

    let exe = std::env::current_exe().ok();
    let exe_stamp = exe.as_deref().and_then(stamp);
    let start = Instant::now();
    // requests read but not yet answered, and when one last arrived or finished (ms since start)
    let busy = Arc::new(AtomicUsize::new(0));
    let last = Arc::new(AtomicU64::new(0));
    let now_ms = move || start.elapsed().as_millis() as u64;

    if !idle_exit.is_zero() {
        let (busy, last) = (busy.clone(), last.clone());
        std::thread::spawn(move || {
            let limit = idle_exit.as_millis() as u64;
            loop {
                std::thread::sleep(Duration::from_millis(limit.clamp(50, 1000)));
                if busy.load(Ordering::SeqCst) == 0 && now_ms().saturating_sub(last.load(Ordering::SeqCst)) >= limit {
                    std::process::exit(0);
                }
            }
        });
    }

    let kinds = KindsByRoot::default();
    let stdout = Mutex::new(io::stdout());
    let (tx, rx) = mpsc::channel::<String>();
    let rx = Mutex::new(rx);
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                loop {
                    let next = rx.lock().unwrap().recv();
                    let Ok(line) = next else { break };
                    let reply = respond(&line, &kinds);
                    let written = {
                        let mut out = stdout.lock().unwrap();
                        out.write_all(reply.as_bytes()).and_then(|_| out.write_all(b"\n")).and_then(|_| out.flush())
                    };
                    if written.is_err() {
                        std::process::exit(0); // the client is gone
                    }
                    last.store(now_ms(), Ordering::SeqCst);
                    busy.fetch_sub(1, Ordering::SeqCst);
                }
            });
        }
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            if exe_stamp.is_some() && exe.as_deref().and_then(stamp) != exe_stamp {
                eprintln!("rbview serve: the rbview binary was replaced; exiting so the new one is used");
                break;
            }
            busy.fetch_add(1, Ordering::SeqCst);
            last.store(now_ms(), Ordering::SeqCst);
            if tx.send(line).is_err() {
                break;
            }
        }
        // stdin closed (or the binary changed): answer what's queued, then exit
        drop(tx);
    });
}
