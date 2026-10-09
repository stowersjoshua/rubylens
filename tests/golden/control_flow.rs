pub fn find_pair(grid: &[Vec<i32>], target: i32) -> Option<(usize, usize)> {
    'outer: for (row, cells) in grid.iter().enumerate() {
        for col in 0..cells.len() {
            if cells[col] == target {
                return Some((row, col));
            }
            if cells[col] < 0 {
                continue 'outer;
            }
        }
    }
    None
}

pub fn drain(stack: &mut Vec<i32>) -> i32 {
    let mut sum = 0;
    while let Some(top) = stack.pop() {
        sum += top;
    }
    let mut n = 0;
    loop {
        n += 1;
        if n == 10 {
            break;
        }
    }
    while sum > 100 {
        sum -= 1;
    }
    sum
}

pub fn ranges() -> Vec<i32> {
    let mut v = Vec::new();
    for i in 1..=3 {
        v.push(i);
    }
    let tail = &v[1..];
    tail.to_vec()
}
