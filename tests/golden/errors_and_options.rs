use std::collections::HashMap;
use std::fs;

#[derive(Debug)]
pub enum ConfigError {
    Missing(String),
    Invalid { line: usize },
}

pub fn read_port(path: &str) -> Result<u16, ConfigError> {
    let text = fs::read_to_string(path).map_err(|e| ConfigError::Missing(e.to_string()))?;
    let line = text.lines().next().ok_or(ConfigError::Invalid { line: 1 })?;
    let port = line.trim().parse::<u16>().map_err(|_| ConfigError::Invalid { line: 1 })?;
    Ok(port)
}

pub fn count_words(text: &str) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word.to_string()).or_insert(0) += 1;
    }
    counts
}

pub fn lookup(map: &HashMap<String, usize>, key: &str) -> usize {
    let found = map.get(key).copied();
    if found.is_some() {
        return found.unwrap();
    }
    found.unwrap_or(0)
}

pub fn first_even(numbers: &[i32]) -> Option<i32> {
    numbers.iter().find(|n| *n % 2 == 0).copied()
}
