# frozen_string_literal: true
# rbview prelude — stub definitions for the Rust-isms that survive translation.
# Nothing here is meant to run; it exists so the Ruby view reads (and indexes) cleanly.

# Result<T, E>
Ok  = Data.define(:value)   # Ok(value)
Err = Data.define(:error)   # Err(error)

class Object
  # Rust's `?` operator: unwrap an Ok/Some, or *return early* from the
  # enclosing method with the Err/None. (Ruby can't express this; read it as
  # "or bail out".)
  def try! = self

  # Option/Result#unwrap — "this cannot be nil/Err here, or we panic".
  def unwrap = self
  def expect(_message) = self
  def unwrap_or_else = self
  def unwrap_or_default = self
  def ok_or(_err) = self
  def map_err = self
  def and_then = yield(self)

  # Iterator#enumerate — yields [index, item] (note: index first, unlike each_with_index)
  def enumerate = each_with_index.map { |x, i| [i, x] }

  # Struct literal: `Point { x: 1, y: 2 }` is shown as `Point[x: 1, y: 2]`.
  # It sets the fields directly; it is *not* a call to `Point.new`.
  def self.[](*positional, **fields) = allocate

  # `default` / `Default::default()` — the type's zero value
  def default = nil
end

# Raw Rust that rbview didn't translate is wrapped as rust!(%q{ ... }).
def rust!(source) = source

# assert!/assert_eq!/debug_assert!
def assert(_condition, _message = nil) = nil
