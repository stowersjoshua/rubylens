# frozen_string_literal: true
# rbview: read-only Ruby projection of Rust source. Not executable.
# #[allow(non_camel_case_types)]
class Timeval
  attr_accessor :tv_sec  # i64
end

class Range
  attr_accessor :begin  # usize
  attr_accessor :end  # usize
end

# fn next(r#type: &str, end: usize) -> usize
def next(type, end_)
  begin_ = end_ - 1
  r = Range[begin: begin_, end: end_]
  t = Timeval[tv_sec: 0]
  r.end + r.begin + t.tv_sec.to_i + type.size
end

# fn span(r: &Range) -> usize
def span(r)
  r => Range(begin: begin_, end: end_)
  end_ - begin_
end
