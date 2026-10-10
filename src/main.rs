//! RubyLens — read Rust code as Ruby.
//!
//!   rubylens gen <file.rs> [--root DIR] [--json] [--no-cache]
//!   rubylens build [DIR] [--force]
//!   rubylens serve [--idle-exit SECS]
//!
//! Generated views, position maps and a crate-wide symbol index live in a
//! hidden `.rubylens/` folder at the repo root (like `.git` or `.idea`).

use rubylens::{CACHE_DIR, Kinds, cmd_build, gen_cached, resolve};
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "RubyLens — read Rust code as Ruby\n\n\
         USAGE:\n  \
         rubylens gen <file.rs> [--root DIR] [--json] [--no-cache]   print the Ruby view of one file\n  \
         rubylens build [DIR] [--force]                              pre-generate views for every .rs file\n  \
         rubylens serve [--idle-exit SECS]                           answer gen requests over stdin/stdout (for editors)\n  \
         rubylens bench <file-or-dir>... [--repeat N] [--top K] [--digest FILE]   in-process benchmark\n\n\
         Views are cached in <repo>/.rubylens/ (self-gitignored)."
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
            let root = opt("--root").map(PathBuf::from);
            let (file, root) = resolve(&PathBuf::from(file), root.as_deref()).unwrap_or_else(|e| {
                eprintln!("rubylens: {file}: {e}");
                std::process::exit(1)
            });
            let kinds = Kinds::lazy(&root.join(CACHE_DIR));
            match gen_cached(&file, &root, &kinds, !flag("--no-cache")) {
                Ok(v) if flag("--json") => println!("{}", v.to_json()),
                Ok(v) => print!("{}", v.ruby),
                Err(e) => {
                    eprintln!("rubylens: {e}");
                    std::process::exit(1)
                }
            }
        }
        "build" => {
            let dir = positional.first().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
            eprintln!("{}", cmd_build(&dir, flag("--force")));
        }
        "bench" => rubylens::bench::run(&args[1..]),
        "serve" => rubylens::serve::run(&args[1..]),
        _ => usage(),
    }
}

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
