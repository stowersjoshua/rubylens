# frozen_string_literal: true
# rbview: read-only Ruby projection of Rust source. Not executable.
# use std::fmt::Write

# fn report(name: &str, score: f64, tags: &[&str]) -> String
def report(name, score, tags)
  out = ""
  (out << "#{name}: #{format('%.1f', score)}\n").unwrap
  (out << "tags=#{tags.inspect}").unwrap
  puts "built report for #{name}"
  warn "debug: #{out.size}"
  out
end

# fn literals() -> Vec<String>
def literals
  c = "x"
  bytes = "raw".b
  escaped = "tab\there \"quoted\" \#{not interpolated}"
  raw = "C:\\path"
  [c.to_s, String.from_utf8_lossy(bytes).to_s, escaped.to_s, raw.to_s]
end

# fn zeros(n: usize) -> Vec<u8>
def zeros(n)
  Array.new(n, 0)
end

# fn is_digit(b: u8) -> bool
def is_digit(b)
  case b
  in (48..57)  # b'0'..=b'9'
    true
  in 95  # b'_'
    false
  in _
    false
  end
end

# fn checks(a: i32, b: i32)
def checks(a, b)
  assert a > 0, "a must be positive, got #{a}"
  assert a == b
  _ = p(a + b)
  if a > 100
    raise "too big: #{a}"
  end
  raise NotImplementedError
end
