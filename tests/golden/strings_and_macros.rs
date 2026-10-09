use std::fmt::Write;

pub fn report(name: &str, score: f64, tags: &[&str]) -> String {
    let mut out = String::new();
    writeln!(out, "{name}: {:.1}", score).unwrap();
    write!(out, "tags={:?}", tags).unwrap();
    println!("built report for {}", name);
    eprintln!("debug: {}", out.len());
    out
}

pub fn literals() -> Vec<String> {
    let c = 'x';
    let bytes = b"raw";
    let escaped = "tab\there \"quoted\" #{not interpolated}";
    let raw = r"C:\path";
    vec![c.to_string(), String::from_utf8_lossy(bytes).to_string(), escaped.to_string(), raw.to_string()]
}

pub fn zeros(n: usize) -> Vec<u8> {
    vec![0; n]
}

pub fn is_digit(b: u8) -> bool {
    match b {
        b'0'..=b'9' => true,
        b'_' => false,
        _ => false,
    }
}

pub fn checks(a: i32, b: i32) {
    assert!(a > 0, "a must be positive, got {}", a);
    assert_eq!(a, b);
    let _ = dbg!(a + b);
    if a > 100 {
        panic!("too big: {}", a);
    }
    todo!()
}
