use std::ops::Div;

use crate::algebra::concept::Arithmetic;

pub trait Reciprocal {
    type Output;

    fn reciprocal(&self) -> Self::Output;
}

impl <T: Arithmetic + Div<Self> + Clone> Reciprocal for T {
    type Output = <Self as Div<Self>>::Output;

    fn reciprocal(&self) -> Self::Output {
        T::ONE.clone() / self.clone()
    }
}

fn reciprocal<T: Reciprocal>(value: T) -> T::Output {
    value.reciprocal()
}
