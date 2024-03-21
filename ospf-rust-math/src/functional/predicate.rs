pub type Predicate<'a, T> = dyn Fn(&'a T) -> bool;
pub type Comparator<T> = dyn Fn(&T, &T) -> bool;
