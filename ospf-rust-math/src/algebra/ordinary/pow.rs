use std::fmt::{Debug, Display, Formatter};
use std::ops::{Div, Mul};

use crate::algebra::concept::Arithmetic;

#[derive(Debug)]
pub struct NegativeIndexError<T: Debug> {
    index: i64,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Debug> NegativeIndexError<T> {
    fn new(index: i64) -> Self {
        Self {
            index,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: Debug> Display for NegativeIndexError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid argument for negative index \'{}\' exponential function: {}",
            self.index,
            core::any::type_name::<T>()
        )
    }
}

pub(self) fn pow_pos_impl<T: Arithmetic>(value: &T, base: &T, index: i64) -> T
    where for<'a> &'a T: Mul<&'a T, Output=T> {
    if index == 0 {
        T::ONE.clone()
    } else {
        pow_pos_impl(&(value * base), base, index - 1)
    }
}

pub(self) fn pow_neg_impl<T: Arithmetic>(value: &T, base: &T, index: i64) -> T
    where for<'a> &'a T: Div<&'a T, Output=T>{
    if index == 0 {
        T::ONE.clone()
    } else {
        pow_neg_impl(&(value / base), base, index + 1)
    }
}

pub(crate) fn pow_times_semi_group<T: Arithmetic + Debug>(
    base: &T,
    index: i64,
) -> Result<T, NegativeIndexError<T>> where for<'a> &'a T: Mul<&'a T, Output=T> {
    if index >= 1 {
        Ok(pow_pos_impl(T::ONE, base, index))
    } else if index <= -1 {
        Err(NegativeIndexError::new(index))
    } else {
        Ok(T::ONE.clone())
    }
}

pub(crate) fn pow_times_group<T: Arithmetic>(
    base: &T,
    index: i64,
) -> T where for<'a> &'a T: Mul<&'a T, Output=T> + Div<&'a T, Output=T> {
    if index >= 1 {
        pow_pos_impl(T::ONE, base, index)
    } else if index <= -1 {
        pow_neg_impl(T::ONE, base, index)
    } else {
        T::ONE.clone()
    }
}
