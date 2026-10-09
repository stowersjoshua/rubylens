# frozen_string_literal: true
# rbview: read-only Ruby projection of Rust source. Not executable.
module Shapes
  # use std::collections::HashMap
  # use std::fmt

  # A shape we can measure.
  # #[derive(Debug, Clone, PartialEq)]
  module Shape  # enum
    Circle = Data.define(:radius) { include Shape }  # { radius: f64 }
    Rect = Data.define(:w, :h) { include Shape }  # { w: f64, h: f64 }
    Polygon = Data.define(:_0) { include Shape }  # (Vec<(f64, f64)>)
    Empty = Data.define { include Shape }
  end

  module Area  # trait
    # fn area(&self) -> f64
    def area = raise NotImplementedError  # required

    # fn describe(&self) -> String
    def describe
      "area = #{format('%.2f', self.area)}"
    end
  end

  module Shape  # impl Area for Shape
    include Area

    # fn area(&self) -> f64
    def area
      case self
      in Shape::Circle(radius:)
        Std::F64::Consts::PI * radius * radius
      in Shape::Rect(w:, h:) if w > 0.0
        w * h
      in Shape::Rect(**)
        0.0
      in Shape::Polygon[points]
        shoelace(points)
      in Shape::Empty
        0.0
      end
    end
  end

  class Canvas
    attr_accessor :name  # String
    attr_accessor :shapes  # Vec<Shape>
    attr_accessor :tags  # HashMap<String, usize>
  end

  class Canvas  # impl Canvas
    # fn new(name: &str) -> Self
    def self.new(name)
      Canvas[name: name.to_s, shapes: [], tags: {}]
    end

    # fn add(&mut self, shape: Shape, tag: Option<&str>) -> &mut Self
    def add(shape, tag)
      if (t = tag)
        @tags[t.to_s] = (@tags[t.to_s] || 0) + 1
      end
      @shapes.push(shape)
      self
    end

    # fn total_area(&self) -> f64
    def total_area
      @shapes.map { |s| s.area }.sum
    end

    # fn largest(&self) -> Option<&Shape>
    def largest
      @shapes.select { |s| !(s in Shape::Empty) }.max { |a, b| (a.area <=> b.area).unwrap }
    end

    # fn load(path: &str) -> Result<Canvas, String>
    def self.load(path)
      text = Std::Fs.read_to_string(path).map_err { |e| e.to_s }.try!
      canvas = Canvas.new(path)
      text.lines.enumerate.each do |i, line|
        parts = line.split(",")  # Vec<&str>
        shape = case parts
        in ["circle", r]
          Shape::Circle[radius: r.parse.map_err { |_| "bad radius on line #{i + 1}" }.try!]
        in ["rect", w, h]
          Shape::Rect[w: (w.parse || 0.0), h: (h.parse || 0.0)]
        in [] | [""]
          next
        in other
          return Err["unknown shape #{other.inspect}"]
        end
        canvas.add(shape, nil)
      end
      Ok[canvas]
    end
  end

  class Canvas  # impl fmt::Display for Canvas
    include Fmt::Display

    # fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    def fmt(f)
      (f << "Canvas #{@name} (#{@shapes.size} shapes)\n").try!
      @shapes.each do |shape|
        (f << "  #{shape.inspect}: #{shape.describe}").try!
      end
      Ok[nil]
    end
  end

  # fn shoelace(points: &[(f64, f64)]) -> f64
  def self.shoelace(points)
    n = points.size
    if n < 3
      return 0.0
    end
    sum = 0.0
    (0...n).each do |i|
      x1, y1 = points[i]
      x2, y2 = points[(i + 1) % n]
      sum += x1 * y2 - x2 * y1
    end
    (sum / 2.0).abs
  end

  # #[cfg(test)]
  module Tests
    # use super::*

    # #[test]
    # fn rect_area()
    def self.rect_area
      r = Shape::Rect[w: 2.0, h: 3.0]
      assert r.area == 6.0
      empty = Array.new(3, Shape::Empty)
      assert empty.all? { |s| s.area == 0.0 }, "all empty"
    end
  end
end
