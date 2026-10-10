//! `rubylens serve` (docs/spec/serve.md): answers exactly what `rubylens gen --json` prints, in
//! parallel, keeps up with `symbols.json`, and exits when it should.

use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

const RUBYLENS: &str = env!("CARGO_BIN_EXE_rubylens");

struct Repo(PathBuf);

impl Repo {
    fn new(name: &str) -> Repo {
        let dir = std::env::temp_dir().join(format!("rubylens-serve-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"t\"\n").unwrap();
        Repo(dir.canonicalize().unwrap())
    }

    fn write(&self, rel: &str, text: &str) -> String {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        // replace, as an editor's save or `rubylens build` does
        let tmp = p.with_extension("tmp");
        fs::write(&tmp, text).unwrap();
        fs::rename(&tmp, &p).unwrap();
        p.to_string_lossy().into_owned()
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Server {
    fn start(exe: &Path, args: &[&str]) -> Server {
        let mut child = Command::new(exe)
            .arg("serve")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start rubylens serve");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Server { child, stdin, stdout }
    }

    fn send(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().unwrap();
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
    }

    fn recv(&mut self) -> Option<Value> {
        let mut line = String::new();
        (self.stdout.read_line(&mut line).unwrap() > 0).then(|| serde_json::from_str(&line).expect("a JSON response"))
    }

    /// Send every request, then collect the responses by id.
    fn ask(&mut self, reqs: &[Value]) -> HashMap<String, Value> {
        for r in reqs {
            self.send(&r.to_string());
        }
        (0..reqs.len()).map(|_| self.recv().expect("a response")).map(|r| (r["id"].to_string(), r)).collect()
    }

    fn wait_exit(&mut self, within: Duration) -> Option<std::process::ExitStatus> {
        let t = Instant::now();
        while t.elapsed() < within {
            if let Some(status) = self.child.try_wait().unwrap() {
                return Some(status);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn gen_json(file: &str, root: &Path) -> Value {
    let out = Command::new(RUBYLENS).args(["gen", file, "--root", root.to_str().unwrap(), "--json"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn answers_what_gen_json_prints() {
    let r = Repo::new("parity");
    let lib = r
        .write("src/lib.rs", "pub mod a;\npub struct Point { x: i32 }\nimpl Point { fn x(&self) -> i32 { self.x } }\n");
    let a = r.write("src/a.rs", "pub fn twice(v: Vec<u32>) -> Vec<u32> { v.iter().map(|x| x * 2).collect() }\n");
    let root = r.0.to_str().unwrap();
    let mut s = Server::start(Path::new(RUBYLENS), &[]);
    let got = s.ask(&[
        json!({"id": 1, "method": "gen", "file": lib, "root": root}),
        json!({"id": "two", "method": "gen", "file": a, "root": root}),
        json!({"id": 3, "method": "ping"}),
    ]);
    for (id, file) in [("1", &lib), ("\"two\"", &a)] {
        let mut served = got[id]["result"].clone();
        let mut printed = gen_json(file, &r.0);
        assert_eq!(served["cached"], false, "the server generated it");
        assert_eq!(printed["cached"], true, "and gen reused what the server wrote");
        served["cached"] = Value::Null;
        printed["cached"] = Value::Null;
        assert_eq!(served, printed, "{file}");
    }
    assert_eq!(got["3"]["result"]["version"], rubylens::GEN_VERSION);
}

#[test]
fn parallel_requests_are_matched_by_id() {
    let r = Repo::new("parallel");
    let files: Vec<String> =
        (0..12).map(|i| r.write(&format!("src/m{i}.rs"), &format!("pub fn f{i}() -> u32 {{ {i} }}\n"))).collect();
    let mut s = Server::start(Path::new(RUBYLENS), &[]);
    let reqs: Vec<Value> = (0..36)
        .map(|n| json!({"id": n, "method": "gen", "file": files[n % 12], "root": r.0, "noCache": n % 3 == 0}))
        .collect();
    let got = s.ask(&reqs);
    for n in 0..36 {
        let ruby = got[&n.to_string()]["result"]["ruby"].as_str().unwrap();
        assert!(ruby.contains(&format!("def self.f{}", n % 12)), "request {n} got another file's view");
    }
}

#[test]
fn errors_are_answered_and_the_server_carries_on() {
    let r = Repo::new("errors");
    let ok = r.write("src/lib.rs", "pub fn f() {}\n");
    let mut s = Server::start(Path::new(RUBYLENS), &[]);
    s.send("this is not json");
    let bad = s.recv().unwrap();
    assert!(bad["id"].is_null() && bad["error"].as_str().unwrap().contains("not a JSON request"));
    let got = s.ask(&[
        json!({"id": 1, "method": "gen", "file": r.0.join("src/missing.rs")}),
        json!({"id": 2, "method": "explode"}),
        json!({"id": 3, "method": "gen"}),
        json!({"id": 4, "method": "gen", "file": ok, "root": r.0}),
    ]);
    assert!(got["1"]["error"].as_str().unwrap().contains("missing.rs"));
    assert!(got["2"]["error"].as_str().unwrap().contains("unknown method"));
    assert!(got["3"]["error"].as_str().unwrap().contains("needs \"file\""));
    assert!(got["4"]["result"]["ruby"].as_str().unwrap().contains("def f"));
}

#[test]
fn follows_symbols_json_without_a_restart() {
    let r = Repo::new("kinds");
    let area = r.write("src/area.rs", "impl Shape {}\n");
    let mut s = Server::start(Path::new(RUBYLENS), &[]);
    let mut view = |n: u32| {
        let got = s.ask(&[json!({"id": n, "method": "gen", "file": area, "root": r.0})]);
        got[&n.to_string()]["result"]["ruby"].as_str().unwrap().to_string()
    };
    assert!(view(1).contains("class Shape"));
    r.write(".rubylens/symbols.json", r#"{"kinds": {"Shape": "enum"}}"#);
    assert!(view(2).contains("module Shape"), "a new symbols.json is read");
    r.write(".rubylens/symbols.json", r#"{"kinds": {"Shape": "struct"}}"#);
    assert!(view(3).contains("class Shape"), "and again");
}

#[test]
fn exits_when_stdin_closes_after_answering() {
    let r = Repo::new("eof");
    let f = r.write("src/lib.rs", &"pub fn f() -> u32 { 1 }\n".repeat(200));
    let mut s = Server::start(Path::new(RUBYLENS), &[]);
    for n in 0..4 {
        s.send(&json!({"id": n, "method": "gen", "file": f, "root": r.0, "noCache": true}).to_string());
    }
    drop(s.stdin.take());
    let answered = std::iter::from_fn(|| s.recv()).count();
    assert_eq!(answered, 4, "queued requests are answered before exiting");
    assert!(s.wait_exit(Duration::from_secs(10)).is_some_and(|st| st.success()));
}

#[test]
fn exits_when_idle() {
    let mut s = Server::start(Path::new(RUBYLENS), &["--idle-exit", "1"]);
    s.send(&json!({"id": 1, "method": "ping"}).to_string());
    assert!(s.recv().is_some());
    assert!(s.wait_exit(Duration::from_secs(10)).is_some_and(|st| st.success()), "should exit after 1 s idle");
}

#[cfg(unix)]
#[test]
fn exits_when_its_binary_is_replaced() {
    let r = Repo::new("upgrade");
    let exe = r.0.join("rubylens-copy");
    fs::copy(RUBYLENS, &exe).unwrap();
    let mut s = Server::start(&exe, &[]);
    s.send(&json!({"id": 1, "method": "ping"}).to_string());
    assert!(s.recv().is_some());
    // what `cargo install` does: a new file renamed over the old path
    let new = r.0.join("rubylens-new");
    fs::copy(RUBYLENS, &new).unwrap();
    fs::rename(&new, &exe).unwrap();
    s.send(&json!({"id": 2, "method": "ping"}).to_string());
    assert!(s.recv().is_none(), "the old binary must not answer");
    assert!(s.wait_exit(Duration::from_secs(10)).is_some_and(|st| st.success()));
}
