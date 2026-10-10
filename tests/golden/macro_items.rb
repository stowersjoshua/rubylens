# frozen_string_literal: true
# RubyLens: read-only Ruby projection of Rust source. Not executable.
# macro definition (Rust, verbatim):
#   macro_rules! cfg_fast {
#       ($($item:item)*) => {
#           $( #[cfg(feature = "fast")] $item )*
#       };
#   }

# cfg_fast! {
# fn fast_path(n: u64) -> u64
def fast_path(n)
  n * 2
end

class FastCache
  attr_accessor :hits  # u64
end
# }

class Runner; end

class Runner  # impl Runner
  # cfg_fast! {
  # fn run_fast(&self) -> u64
  def run_fast
    fast_path(21)
  end
  # }

  # fn run(&self) -> u64
  def run
    # select_like! {
    #     a = first() => a,
    #     b = second() => b,
    # }
  end
end
