# frozen_string_literal: true
# rbview: read-only Ruby projection of Rust source. Not executable.
# fn configure() -> u32
def configure
  # local const LIMIT — shown after this method
  # local struct Local — shown after this method
  # local fn helper — shown after this method
  # thread_local! {
  # local static COUNTER — shown after this method
  # }
  l = Local[value: helper(LIMIT)]
  l.value
end

# (declared inside fn configure)
LIMIT = 10  # u32

# (declared inside fn configure)
class Local
  attr_accessor :value  # u32
end

# (declared inside fn configure)
# fn helper(x: u32) -> u32
def self.helper(x)
  x + 1
end

# (declared inside fn configure)
COUNTER = 0  # static u32
