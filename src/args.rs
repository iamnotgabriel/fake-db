use std::cmp::Ordering;

pub type CompareClosure<T> = dyn FnMut(&T, &T) -> Ordering;
pub type Matcher<T> = dyn FnMut(&&T) -> bool;

pub struct FindArguments<T> {
    pub matcher: Box<Matcher<T>>,
    pub order: Option<Box<CompareClosure<T>>>,
}

#[macro_export]
macro_rules! args {
    ($Args: ident <$generic: ident> { $($property: ident : $value: expr,)* $(,)?} ) => {
        $Args::<$generic> {
            $(
                $property: args!($property : $value),
            )*
            ..Default::default()
        }
    };
    (matcher : $value: expr) => {
        Box::new($value)
    };
    (order : $value: expr) => {
        Some(Box::new($value))
    };
    (updater : $value: expr) => {
        Box::new($value)
    }
}

impl<T> Default for FindArguments<T> {
    fn default() -> Self {
        Self {
            matcher: Box::new(|_: &&T| true),
            order: None,
        }
    }
}
pub type Updater<T> = dyn FnMut(&mut T);
pub struct UpdateArguments<T> {
    pub matcher: Box<Matcher<T>>,
    pub updater: Box<Updater<T>>,
}

impl<T> Default for UpdateArguments<T> {
    fn default() -> Self {
        Self {
            matcher: Box::new(|_: &&T| true),
            updater: Box::new(|_| {}),
        }
    }
}
