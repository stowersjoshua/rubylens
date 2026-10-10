//! The `.rubylens/` cache (docs/spec/cache.md): a view is reused exactly when it's still right.
//!
//! Each test builds a small throwaway repo, so these never touch the real cache.

use rubylens::{CACHE_DIR, Kinds, cmd_build, gen_cached};
use std::fs;
use std::path::{Path, PathBuf};

struct Repo(PathBuf);

impl Repo {
    fn new(name: &str) -> Repo {
        let dir = std::env::temp_dir().join(format!("rubylens-cache-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"t\"\n").unwrap();
        // a .git marker makes this folder the cache root
        fs::create_dir_all(dir.join(".git")).unwrap();
        Repo(dir.canonicalize().unwrap())
    }

    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, text).unwrap();
        p
    }

    fn view(&self, rel: &str) -> String {
        let rb = self.0.join(CACHE_DIR).join(rel).with_extension("rb");
        fs::read_to_string(rb).unwrap()
    }

    fn open(&self, file: &Path) -> rubylens::View {
        gen_cached(file, &self.0, &Kinds::lazy(&self.0.join(CACHE_DIR)), true).unwrap()
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn unchanged_files_are_reused() {
    let r = Repo::new("reuse");
    r.write("src/lib.rs", "pub mod a;\npub struct Point { x: i32 }\n");
    let a = r.write("src/a.rs", "pub fn f() -> i32 { 1 }\n");
    let first = cmd_build(&r.0, false);
    assert_eq!((first.generated, first.unchanged), (2, 0));
    let again = cmd_build(&r.0, false);
    assert_eq!((again.generated, again.unchanged), (0, 2), "a second build must reuse everything");
    assert!(r.open(&a).cached, "gen must reuse a view build made");

    r.write("src/a.rs", "pub fn f() -> i32 { 2 }\n");
    let edited = cmd_build(&r.0, false);
    assert_eq!((edited.generated, edited.unchanged), (1, 1), "only the edited file is regenerated");
    assert!(cmd_build(&r.0, true).generated == 2, "--force regenerates everything");
}

#[test]
fn views_follow_the_symbol_kinds_they_used() {
    let r = Repo::new("kinds");
    r.write("src/lib.rs", "pub mod shape;\npub mod area;\n");
    let area = r.write("src/area.rs", "impl Shape {\n    fn area(&self) -> f64 { 0.0 }\n}\n");
    cmd_build(&r.0, false);
    assert!(r.view("src/area.rs").contains("class Shape"), "unknown type: a class");

    // Shape becomes a known enum: the view that relied on it must change, the others not.
    let shape = r.write("src/shape.rs", "pub enum Shape { Circle }\n");
    let b = cmd_build(&r.0, false);
    assert!(r.view("src/area.rs").contains("module Shape"), "an enum's impl is a module");
    assert_eq!((b.generated, b.unchanged), (2, 1), "new shape.rs and dependent area.rs only");
    assert!(r.open(&area).cached);

    // ...and back when the definition goes away (symbols.json drops deleted files' types).
    fs::remove_file(&shape).unwrap();
    cmd_build(&r.0, false);
    assert!(r.view("src/area.rs").contains("class Shape"));
}

#[test]
fn a_cold_build_sees_types_from_other_files() {
    let r = Repo::new("cold");
    r.write("src/lib.rs", "pub mod area;\npub mod shape;\n");
    r.write("src/area.rs", "impl Shape {}\nimpl Area for Shape {}\n");
    r.write("src/shape.rs", "pub enum Shape { Circle }\npub trait Area {}\n");
    let b = cmd_build(&r.0, false);
    assert_eq!((b.generated, b.unchanged), (3, 0), "each file counts once");
    let view = r.view("src/area.rs");
    assert!(view.contains("module Shape") && !view.contains("class Shape"), "{view}");
    assert_eq!(cmd_build(&r.0, false).generated, 0);
}

#[test]
fn gen_regenerates_a_view_whose_kinds_changed() {
    let r = Repo::new("gen-kinds");
    r.write("src/lib.rs", "pub mod area;\n");
    let area = r.write("src/area.rs", "impl Shape {}\n");
    assert!(!r.open(&area).cached);
    assert!(r.open(&area).cached);
    // a build elsewhere learns that Shape is a trait; this view hasn't been rebuilt yet
    r.write("other/src/lib.rs", "pub trait Shape {}\n");
    cmd_build(&r.0.join("other"), false);
    let v = r.open(&area);
    assert!(!v.cached, "the cached view assumed Shape was unknown");
    assert!(v.ruby.contains("module Shape"));
}

#[test]
fn concurrent_readers_never_see_a_half_written_view() {
    let r = Repo::new("concurrent");
    let body: String = (0..400).map(|i| format!("pub fn f{i}(x: u32) -> u32 {{ x + {i} }}\n")).collect();
    let file = r.write("src/lib.rs", &body);
    let kinds = Kinds::lazy(&r.0.join(CACHE_DIR));
    std::thread::scope(|s| {
        for t in 0..8 {
            let (file, root, kinds) = (&file, &r.0, &kinds);
            s.spawn(move || {
                for i in 0..20 {
                    // half the callers rewrite the view while the others read it
                    let v = gen_cached(file, root, kinds, (t + i) % 2 == 0).unwrap();
                    let map: serde_json::Value = serde_json::from_str(&v.map_json).expect("a complete map");
                    assert!(map["segs"].as_array().is_some_and(|s| !s.is_empty()));
                    assert!(v.ruby.ends_with('\n') && v.ruby.contains("def f399"), "a complete view");
                }
            });
        }
    });
    let leftovers: Vec<_> = fs::read_dir(r.0.join(CACHE_DIR).join("src"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temporary files left behind");
}

#[test]
fn builds_are_deterministic() {
    let snapshot = |r: &Repo| {
        let mut out = vec![];
        let mut stack = vec![r.0.join(CACHE_DIR)];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(d).unwrap().flatten() {
                if e.path().is_dir() {
                    stack.push(e.path());
                } else {
                    out.push((e.path().strip_prefix(&r.0).unwrap().to_path_buf(), fs::read(e.path()).unwrap()));
                }
            }
        }
        out.sort();
        out
    };
    let files = [
        ("src/lib.rs", "pub mod a;\npub mod b;\npub struct Error;\n"),
        ("src/a.rs", "pub enum Error { A }\nimpl Error {}\n"),
        ("src/b.rs", "pub trait Kind {}\nimpl Error {}\nimpl Kind for u8 {}\n"),
    ];
    let one = Repo::new("det-1");
    let two = Repo::new("det-2");
    for (p, t) in files {
        one.write(p, t);
    }
    // write in a different order, so directory order differs too
    for (p, t) in files.iter().rev() {
        two.write(p, t);
    }
    cmd_build(&one.0, false);
    cmd_build(&two.0, false);
    assert_eq!(snapshot(&one), snapshot(&two));
}
