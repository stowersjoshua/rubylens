//! rbview — read Rust code as Ruby.
//!
//!   rbview gen <file.rs> [--root DIR] [--json] [--no-cache]
//!   rbview build [DIR] [--force]
//!
//! Generated views, position maps and a crate-wide symbol index live in a
//! hidden `.rbview/` folder at the repo root (like `.git` or `.idea`).

use rbview::{CACHE_DIR, cmd_build, find_root, gen_cached, load_kinds};
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "rbview — read Rust code as Ruby\n\n\
         USAGE:\n  \
         rbview gen <file.rs> [--root DIR] [--json] [--no-cache]   print the Ruby view of one file\n  \
         rbview build [DIR] [--force]                              pre-generate views for every .rs file\n\n\
         Views are cached in <repo>/.rbview/ (self-gitignored)."
    );
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else { usage() };
    let flag = |f: &str| args.iter().any(|a| a == f);
    let opt = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();
    let positional: Vec<&String> = {
        let mut v = vec![];
        let mut skip = false;
        for a in &args[1..] {
            if skip {
                skip = false;
                continue;
            }
            if a == "--root" {
                skip = true;
                continue;
            }
            if !a.starts_with("--") {
                v.push(a);
            }
        }
        v
    };
    match cmd.as_str() {
        "gen" => {
            let Some(file) = positional.first() else { usage() };
            let file = PathBuf::from(file).canonicalize().unwrap_or_else(|e| {
                eprintln!("rbview: {file}: {e}");
                std::process::exit(1)
            });
            let root = opt("--root").map(PathBuf::from).unwrap_or_else(|| find_root(&file));
            let root = root.canonicalize().unwrap_or(root);
            let kinds = load_kinds(&root.join(CACHE_DIR));
            match gen_cached(&file, &root, &kinds, !flag("--no-cache")) {
                Ok((ruby, map, rb_path, from_cache, _)) => {
                    if flag("--json") {
                        println!(
                            "{{\"rbPath\":{},\"cached\":{},\"ruby\":{},\"map\":{}}}",
                            serde_json::to_string(&rb_path.to_string_lossy()).unwrap(),
                            from_cache,
                            serde_json::to_string(&ruby).unwrap(),
                            map
                        );
                    } else {
                        print!("{ruby}");
                    }
                }
                Err(e) => {
                    eprintln!("rbview: {e}");
                    std::process::exit(1)
                }
            }
        }
        "build" => {
            let dir = positional.first().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
            cmd_build(&dir, flag("--force"));
        }
        _ => usage(),
    }
}
