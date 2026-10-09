#[allow(non_camel_case_types)]
pub struct timeval {
    pub tv_sec: i64,
}

pub struct Range {
    pub begin: usize,
    pub end: usize,
}

pub fn next(r#type: &str, end: usize) -> usize {
    let begin = end - 1;
    let r = Range { begin, end };
    let t = timeval { tv_sec: 0 };
    r.end + r.begin + t.tv_sec as usize + r#type.len()
}

pub fn span(r: &Range) -> usize {
    let Range { begin, end } = r;
    end - begin
}
