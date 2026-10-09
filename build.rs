// Fingerprint the generator's own source so cached views are invalidated
// automatically whenever the translation logic changes.
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn main() {
    let mut h = DefaultHasher::new();
    let mut files: Vec<_> = std::fs::read_dir("src").unwrap().flatten().map(|e| e.path()).collect();
    files.sort();
    for f in files {
        println!("cargo:rerun-if-changed={}", f.display());
        std::fs::read(&f).unwrap().hash(&mut h);
    }
    println!("cargo:rustc-env=RBVIEW_BUILD_ID={:016x}", h.finish());
}
