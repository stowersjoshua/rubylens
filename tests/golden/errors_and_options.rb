# frozen_string_literal: true
# RubyLens: read-only Ruby projection of Rust source. Not executable.
# use std::collections::HashMap
# use std::fs

# #[derive(Debug)]
module ConfigError  # enum
  Missing = Data.define(:_0) { include ConfigError }  # (String)
  Invalid = Data.define(:line) { include ConfigError }  # { line: usize }
end

# fn read_port(path: &str) -> Result<u16, ConfigError>
def read_port(path)
  text = Fs.read_to_string(path).map_err { |e| ConfigError::Missing[e.to_s] }.try!
  line = text.lines.next.ok_or(ConfigError::Invalid[line: 1]).try!
  port = line.strip.parse.map_err { |_| ConfigError::Invalid[line: 1] }.try!
  Ok[port]
end

# fn count_words(text: &str) -> HashMap<String, usize>
def count_words(text)
  counts = {}
  text.split.each do |word|
    counts[word.to_s] = (counts[word.to_s] || 0) + 1
  end
  counts
end

# fn lookup(map: &HashMap<String, usize>, key: &str) -> usize
def lookup(map, key)
  found = map[key]
  if !found.nil?
    return found.unwrap
  end
  (found || 0)
end

# fn first_even(numbers: &[i32]) -> Option<i32>
def first_even(numbers)
  numbers.find { |n| n % 2 == 0 }
end
