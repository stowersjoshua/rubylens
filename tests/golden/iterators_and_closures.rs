use std::collections::{BTreeMap, HashSet};

pub fn summarize(words: &[String]) -> BTreeMap<String, usize> {
    words
        .iter()
        .filter(|w| !w.is_empty())
        .map(|w| (w.to_lowercase(), w.len()))
        .collect::<BTreeMap<_, _>>()
}

pub fn total(numbers: &[i64]) -> i64 {
    numbers.iter().fold(0, |acc, n| acc + n)
}

pub fn indexed(names: &[&str]) -> Vec<String> {
    names.iter().enumerate().map(|(i, n)| format!("{i}: {n}")).collect()
}

pub fn unique(names: Vec<String>) -> HashSet<String> {
    names.into_iter().collect::<HashSet<_>>()
}

pub fn sorted_by_len(mut names: Vec<String>) -> Vec<String> {
    names.sort_by(|a, b| a.len().cmp(&b.len()));
    names.retain(|n| n.starts_with('a'));
    names
}

pub fn joined(parts: &[&str]) -> String {
    let make = |s: &str| s.trim().to_uppercase();
    parts.iter().map(|p| make(p)).collect::<String>()
}

pub fn any_long(names: &[String]) -> bool {
    names.iter().any(|n| {
        let len = n.chars().count();
        len > 10
    })
}
