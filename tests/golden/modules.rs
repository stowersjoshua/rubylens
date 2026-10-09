use std::collections::{HashMap, HashSet as Set};
use std::io::{self, Read};

pub type Index = HashMap<String, Vec<usize>>;

pub const MAX_ITEMS: usize = 100;
pub static GREETING: &str = "hello";

pub mod storage {
    pub fn save(key: &str) -> bool {
        !key.is_empty()
    }

    pub mod disk {
        pub fn path() -> &'static str {
            "/tmp"
        }
    }
}

pub fn run() -> bool {
    let _ = storage::disk::path();
    storage::save("k") && crate::MAX_ITEMS > 0
}
