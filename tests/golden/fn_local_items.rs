pub fn configure() -> u32 {
    const LIMIT: u32 = 10;

    struct Local {
        value: u32,
    }

    fn helper(x: u32) -> u32 {
        x + 1
    }

    thread_local! {
        static COUNTER: u32 = 0;
    }

    let l = Local { value: helper(LIMIT) };
    l.value
}
