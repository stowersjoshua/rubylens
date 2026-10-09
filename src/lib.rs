//! rbview — read Rust code as Ruby.
//!
//! [`generate`] turns one Rust file into a read-only Ruby projection plus a
//! position map (see `docs/spec/source-map.md`). [`cache`] keeps views in the
//! `.rbview/` folder and builds whole repos (see `docs/spec/cache.md`); the CLI
//! is `main.rs`.

pub mod bench;
pub mod cache;
pub mod emit;
pub mod out;
pub mod range;

pub use cache::{BuildReport, CACHE_DIR, Kinds, View, cmd_build, find_root, gen_cached, load_kinds};
use emit::Gen;
pub use emit::{Kind, KindMap};
pub use out::SourceMap;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Map format version; bump when the map's format changes (docs/spec/source-map.md).
/// Generator changes don't need a bump: the build fingerprint invalidates cached views.
pub const GEN_VERSION: u32 = 2;
pub const PRELUDE: &str = include_str!("prelude.rb");

/// Cache key of a source text: GEN_VERSION, the generator's build fingerprint and the source.
pub fn hash_src(src: &str) -> String {
    let mut h = Sha256::new();
    h.update(GEN_VERSION.to_le_bytes());
    h.update(env!("RBVIEW_BUILD_ID").as_bytes());
    h.update(src.as_bytes());
    h.finalize().iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// `src/foo/bar.rs` -> ["foo", "bar"], `src/foo/mod.rs` -> ["foo"], lib.rs/main.rs/bin/* -> [].
/// Paths are taken relative to the nearest Cargo.toml, so examples/, tests/ and
/// benches/ files are crate roots (no module wrapper).
pub fn module_path(file: &Path) -> Vec<String> {
    let mut crate_dir = None;
    let mut d = file.parent();
    while let Some(p) = d {
        if p.join("Cargo.toml").exists() {
            crate_dir = Some(p);
            break;
        }
        d = p.parent();
    }
    let rel = match crate_dir.and_then(|c| file.strip_prefix(c).ok()) {
        Some(r) => r,
        None => return vec![],
    };
    let mut comps: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    if comps.first().map(|s| s.as_str()) != Some("src") {
        return vec![];
    }
    comps.remove(0);
    if comps.first().is_some_and(|c| c == "bin") {
        return vec![];
    }
    if let Some(last) = comps.pop() {
        let stem = last.trim_end_matches(".rs").to_string();
        if !(comps.is_empty() && (stem == "lib" || stem == "main")) && stem != "mod" {
            comps.push(stem);
        }
    }
    comps
}

pub struct Generated {
    pub ruby: String,
    pub map: SourceMap,
    pub parse_error: Option<String>,
    /// The types the file defines (what `rbview build` collects into `symbols.json`).
    pub defs: KindMap,
}

pub fn generate(src: &str, file: &Path, kinds: &KindMap) -> Generated {
    generate_hashed(src, file, kinds, hash_src(src))
}

/// [`generate`] with the source's [`hash_src`] already computed.
pub(crate) fn generate_hashed(src: &str, file: &Path, kinds: &KindMap, hash: String) -> Generated {
    match syn::parse_file(src) {
        Ok(ast) => {
            let mut g = Gen::new(src, kinds);
            g.file(&ast, &module_path(file));
            Generated {
                ruby: g.out.buf,
                map: SourceMap { version: GEN_VERSION, hash, deps: g.deps, segs: g.out.segs, blocks: g.out.blocks },
                parse_error: None,
                defs: g.local_kinds,
            }
        }
        Err(e) => {
            // Fall back to the raw source in a block comment, mapped line-for-line.
            let header = format!(
                "# rbview: could not parse this file ({} at line {}). Showing the Rust source.\n=begin\n",
                e,
                e.span().start().line
            );
            let offset = header.lines().count() as u32;
            let n = src.lines().count() as u32;
            let blocks = (0..n).map(|i| [i + offset, i + offset, i, i]).collect();
            Generated {
                ruby: format!("{header}{src}\n=end\n"),
                map: SourceMap { version: GEN_VERSION, hash, deps: Default::default(), segs: vec![], blocks },
                parse_error: Some(e.to_string()),
                defs: KindMap::default(),
            }
        }
    }
}

/// Type names an item list defines (structs, enums, traits, including inside inline modules).
pub fn collect_kinds(items: &[syn::Item], m: &mut KindMap) {
    for it in items {
        match it {
            syn::Item::Struct(s) => {
                m.insert(s.ident.to_string(), Kind::Struct);
            }
            syn::Item::Enum(e) => {
                m.insert(e.ident.to_string(), Kind::Enum);
            }
            syn::Item::Trait(t) => {
                m.insert(t.ident.to_string(), Kind::Trait);
            }
            syn::Item::Mod(syn::ItemMod { content: Some((_, items)), .. }) => collect_kinds(items, m),
            _ => {}
        }
    }
}
