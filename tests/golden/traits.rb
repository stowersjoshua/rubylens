# frozen_string_literal: true
# rbview: read-only Ruby projection of Rust source. Not executable.
# use std::fmt

module Describe  # trait
  # associated type Output

  KIND = "thing"  # &'static str

  # fn name(&self) -> String
  def name = raise NotImplementedError  # required

  # fn describe(&self) -> String
  def describe
    "#{self.name} (#{Describe::KIND})"
  end
end

class Dog
  attr_accessor :name  # String
end

class Dog  # impl Describe for Dog
  include Describe

  # type Output = String

  # fn name(&self) -> String
  def name
    @name
  end
end

class Dog  # impl fmt::Display for Dog
  include Fmt::Display

  # fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
  def fmt(f)
    f << "Dog #{@name}"
  end
end

# fn loudest<T: Describe + Clone>(items: &[T]) -> Option<T> where T: fmt::Debug,
def loudest(items)
  items.first
end
