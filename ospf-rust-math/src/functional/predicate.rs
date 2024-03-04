pub type Predicate<T> = impl Fn(T) -> bool;
pub type Comparator<T> = impl Fn(&T, &T) -> bool;
