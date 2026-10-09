use std::fmt;

pub trait Describe {
    type Output;
    const KIND: &'static str = "thing";

    fn name(&self) -> String;

    fn describe(&self) -> String {
        format!("{} ({})", self.name(), Self::KIND)
    }
}

pub struct Dog {
    pub name: String,
}

impl Describe for Dog {
    type Output = String;

    fn name(&self) -> String {
        self.name.clone()
    }
}

impl fmt::Display for Dog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Dog {}", self.name)
    }
}

pub fn loudest<T: Describe + Clone>(items: &[T]) -> Option<T>
where
    T: fmt::Debug,
{
    items.first().cloned()
}
