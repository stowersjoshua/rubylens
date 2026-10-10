# frozen_string_literal: true
# RubyLens: read-only Ruby projection of Rust source. Not executable.
# fn find_pair(grid: &[Vec<i32>], target: i32) -> Option<(usize, usize)>
def find_pair(grid, target)
  grid.enumerate.each do |row, cells|  # 'outer
    (0...cells.size).each do |col|
      if cells[col] == target
        return [row, col]
      end
      if cells[col] < 0
        next  # continue 'outer
      end
    end
  end
  nil
end

# fn drain(stack: &mut Vec<i32>) -> i32
def drain(stack)
  sum = 0
  while (top = stack.pop)
    sum += top
  end
  n = 0
  loop do
    n += 1
    if n == 10
      break
    end
  end
  while sum > 100
    sum -= 1
  end
  sum
end

# fn ranges() -> Vec<i32>
def ranges
  v = []
  (1..3).each do |i|
    v.push(i)
  end
  tail = v[(1...)]
  tail
end
