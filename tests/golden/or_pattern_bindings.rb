# frozen_string_literal: true
# RubyLens: read-only Ruby projection of Rust source. Not executable.
module Either  # enum
  Left = Data.define(:_0) { include Either }  # (i32)
  Right = Data.define(:_0) { include Either }  # (i32)
end

# fn value(e: &Either) -> i32
def value(e)
  case e
  in Either::Left[n] | Either::Right[n]
    n
  end
end
