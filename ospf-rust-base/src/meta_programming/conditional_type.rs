pub trait Conditional {
    type Type;
}

pub struct ConditionalImpl<T, F, const C: bool> {
    _marker: std::marker::PhantomData<(T, F)>,
}

impl<T, F> Conditional for ConditionalImpl<T, F, true> {
    type Type = T;
}

impl<T, F> Conditional for ConditionalImpl<T, F, false> {
    type Type = F;
}

pub type ConditionalType<T, F, const C: bool> = <ConditionalImpl<T, F, { C }> as Conditional>::Type;

#[cfg(test)]
mod tests {
    use crate::meta_programming::{MetaJudgement, IsSameType};
    use super::*;

    #[test]
    fn test_conditional_type() {
        assert!(IsSameType::<ConditionalType<i32, i64, true>, i32>::VALUE);
        assert!(IsSameType::<ConditionalType<i32, i64, false>, i64>::VALUE);
    }
}
