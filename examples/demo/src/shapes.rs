use std::collections::HashMap;
use std::fmt;

/// A shape we can measure.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Circle { radius: f64 },
    Rect { w: f64, h: f64 },
    Polygon(Vec<(f64, f64)>),
    Empty,
}

pub trait Area {
    fn area(&self) -> f64;

    fn describe(&self) -> String {
        format!("area = {:.2}", self.area())
    }
}

impl Area for Shape {
    fn area(&self) -> f64 {
        match self {
            Shape::Circle { radius } => std::f64::consts::PI * radius * radius,
            Shape::Rect { w, h } if *w > 0.0 => w * h,
            Shape::Rect { .. } => 0.0,
            Shape::Polygon(points) => shoelace(points),
            Shape::Empty => 0.0,
        }
    }
}

pub struct Canvas {
    pub name: String,
    shapes: Vec<Shape>,
    tags: HashMap<String, usize>,
}

impl Canvas {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), shapes: Vec::new(), tags: HashMap::new() }
    }

    pub fn add(&mut self, shape: Shape, tag: Option<&str>) -> &mut Self {
        if let Some(t) = tag {
            *self.tags.entry(t.to_string()).or_insert(0) += 1;
        }
        self.shapes.push(shape);
        self
    }

    pub fn total_area(&self) -> f64 {
        self.shapes.iter().map(|s| s.area()).sum()
    }

    pub fn largest(&self) -> Option<&Shape> {
        self.shapes
            .iter()
            .filter(|s| !matches!(s, Shape::Empty))
            .max_by(|a, b| a.area().partial_cmp(&b.area()).unwrap())
    }

    pub fn load(path: &str) -> Result<Canvas, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut canvas = Canvas::new(path);
        for (i, line) in text.lines().enumerate() {
            let parts: Vec<&str> = line.split(',').collect();
            let shape = match parts.as_slice() {
                ["circle", r] => Shape::Circle { radius: r.parse().map_err(|_| format!("bad radius on line {}", i + 1))? },
                ["rect", w, h] => Shape::Rect { w: w.parse().unwrap_or(0.0), h: h.parse().unwrap_or(0.0) },
                [] | [""] => continue,
                other => return Err(format!("unknown shape {other:?}")),
            };
            canvas.add(shape, None);
        }
        Ok(canvas)
    }
}

impl fmt::Display for Canvas {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Canvas {} ({} shapes)", self.name, self.shapes.len())?;
        for shape in &self.shapes {
            write!(f, "  {:?}: {}", shape, shape.describe())?;
        }
        Ok(())
    }
}

fn shoelace(points: &[(f64, f64)]) -> f64 {
    let n = points.len();
    if n < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..n {
        let (x1, y1) = points[i];
        let (x2, y2) = points[(i + 1) % n];
        sum += x1 * y2 - x2 * y1;
    }
    (sum / 2.0).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_area() {
        let r = Shape::Rect { w: 2.0, h: 3.0 };
        assert_eq!(r.area(), 6.0);
        let empty = vec![Shape::Empty; 3];
        assert!(empty.iter().all(|s| s.area() == 0.0), "all empty");
    }
}
