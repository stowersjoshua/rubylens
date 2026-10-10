# frozen_string_literal: true
# RubyLens: read-only Ruby projection of Rust source. Not executable.
# #[derive(Debug)]
module Shape  # enum
  Circle = Data.define(:radius) { include Shape }  # { radius: f64 }
  Rect = Data.define(:w, :h) { include Shape }  # { w: f64, h: f64 }
  Polygon = Data.define(:_0) { include Shape }  # (Vec<(f64, f64)>)
  Empty = Data.define { include Shape }
end

# fn area(shape: &Shape) -> f64
def area(shape)
  case shape
  in Shape::Circle(radius:)
    3.14 * radius * radius
  in Shape::Rect(w:, h:) if w > 0.0
    w * h
  in Shape::Rect(**)
    0.0
  in Shape::Polygon[points]
    points.size.to_f
  in Shape::Empty
    0.0
  end
end

# fn describe(value: Option<i32>) -> &'static str
def describe(value)
  case value
  in 0
    "zero"
  in n if !n.nil? && n < 0
    "negative"
  in _
    "positive"
  in nil
    "nothing"
  end
end

# fn first_word(text: &str) -> &str
def first_word(text)
  if (i = text.find(" "))
    return text[(...i)]
  end
  c = text.chars.next
  if c.nil?  # let-else
    return ""
  end
  if (c in ("a".."z") | ("A".."Z"))
    text
  else
    ""
  end
end

# fn classify(n: u8) -> &'static str
def classify(n)
  case n
  in 0 | 1
    "tiny"
  in (2..9)
    "small"
  in _
    "big"
  end
end
