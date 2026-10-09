macro_rules! cfg_fast {
    ($($item:item)*) => {
        $( #[cfg(feature = "fast")] $item )*
    };
}

cfg_fast! {
    pub fn fast_path(n: u64) -> u64 {
        n * 2
    }

    pub struct FastCache {
        pub hits: u64,
    }
}

pub struct Runner;

impl Runner {
    cfg_fast! {
        pub fn run_fast(&self) -> u64 {
            fast_path(21)
        }
    }

    pub fn run(&self) -> u64 {
        select_like! {
            a = first() => a,
            b = second() => b,
        }
    }
}
