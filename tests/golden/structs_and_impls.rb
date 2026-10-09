# frozen_string_literal: true
# rbview: read-only Ruby projection of Rust source. Not executable.
# A 2D point.
# #[derive(Debug, Clone, Copy, PartialEq)]
class Point
  attr_accessor :x  # f64
  attr_accessor :y  # f64
end

class Meters
  # tuple struct: self[0]: f64
end

class Marker; end

class Point  # impl Point
  ORIGIN = Point[x: 0.0, y: 0.0]  # Point

  # fn new(x: f64, y: f64) -> Self
  def self.new(x, y)
    Point[x: x, y: y]
  end

  # fn distance_to(&self, other: &Point) -> f64
  def distance_to(other)
    ((@x - other.x).pow(2) + (@y - other.y).pow(2)).sqrt
  end

  # fn translate(&mut self, dx: f64, dy: f64)
  def translate(dx, dy)
    @x += dx
    @y += dy
  end

  # fn with_x(self, x: f64) -> Point
  def with_x(x)
    Point[x: x, **self.to_h]
  end
end

class Meters  # impl Meters
  # fn value(&self) -> f64
  def value
    self[0]
  end
end
