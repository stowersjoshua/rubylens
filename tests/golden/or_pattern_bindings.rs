// rbview-golden: ruby-invalid (Ruby forbids binding variables inside alternative patterns)

pub enum Either {
    Left(i32),
    Right(i32),
}

pub fn value(e: &Either) -> i32 {
    match e {
        Either::Left(n) | Either::Right(n) => *n,
    }
}
