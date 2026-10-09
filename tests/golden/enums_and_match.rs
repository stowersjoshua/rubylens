#[derive(Debug)]
pub enum Shape {
    Circle { radius: f64 },
    Rect { w: f64, h: f64 },
    Polygon(Vec<(f64, f64)>),
    Empty,
}

pub fn area(shape: &Shape) -> f64 {
    match shape {
        Shape::Circle { radius } => 3.14 * radius * radius,
        Shape::Rect { w, h } if *w > 0.0 => w * h,
        Shape::Rect { .. } => 0.0,
        Shape::Polygon(points) => points.len() as f64,
        Shape::Empty => 0.0,
    }
}

pub fn describe(value: Option<i32>) -> &'static str {
    match value {
        Some(0) => "zero",
        Some(n) if n < 0 => "negative",
        Some(_) => "positive",
        None => "nothing",
    }
}

pub fn first_word(text: &str) -> &str {
    if let Some(i) = text.find(' ') {
        return &text[..i];
    }
    let Some(c) = text.chars().next() else {
        return "";
    };
    if matches!(c, 'a'..='z' | 'A'..='Z') { text } else { "" }
}

pub fn classify(n: u8) -> &'static str {
    match n {
        0 | 1 => "tiny",
        2..=9 => "small",
        _ => "big",
    }
}
