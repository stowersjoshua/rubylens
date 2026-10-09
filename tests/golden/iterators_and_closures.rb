# frozen_string_literal: true
# rbview: read-only Ruby projection of Rust source. Not executable.
# use std::collections::{BTreeMap, HashSet}

# fn summarize(words: &[String]) -> BTreeMap<String, usize>
def summarize(words)
  words.select { |w| !w.empty? }.map { |w| [w.downcase, w.size] }.to_h
end

# fn total(numbers: &[i64]) -> i64
def total(numbers)
  numbers.inject(0) { |acc, n| acc + n }
end

# fn indexed(names: &[&str]) -> Vec<String>
def indexed(names)
  names.enumerate.map { |(i, n)| "#{i}: #{n}" }
end

# fn unique(names: Vec<String>) -> HashSet<String>
def unique(names)
  names.to_set
end

# fn sorted_by_len(mut names: Vec<String>) -> Vec<String>
def sorted_by_len(names)
  names.sort! { |a, b| (a.size <=> b.size) }
  names.select! { |n| n.start_with?("a") }
  names
end

# fn joined(parts: &[&str]) -> String
def joined(parts)
  make = ->(s) { s.strip.upcase }
  parts.map { |p| make(p) }.join
end

# fn any_long(names: &[String]) -> bool
def any_long(names)
  names.any? do |n| 
    len = n.chars.count
    len > 10
  end
end
