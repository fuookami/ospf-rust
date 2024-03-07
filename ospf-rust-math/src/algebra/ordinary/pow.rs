use std::fmt::{Debug, Display, Formatter};
use std::ops::{Div, Mul, Sub};

use crate::algebra::concept::{Arithmetic, FloatingNumber, TimesSemiGroup, TimesGroup};

use super::ln;

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

pub(self) fn pow_pos_impl<T: Arithmetic + TimesSemiGroup>(value: T, base: T, index: i64) -> T {
    if index == 0 {
        T::ONE.clone()
    } else {
        pow_pos_impl(value * base.clone(), base, index - 1)
    }
}

pub(self) fn pow_neg_impl<T: Arithmetic + TimesGroup>(value: T, base: T, index: i64) -> T {
    if index == 0 {
        T::ONE.clone()
    } else {
        pow_neg_impl(value / base.clone(), base, index + 1)
    }
}

pub(crate) fn pow_times_semi_group<T: Arithmetic + TimesSemiGroup + Debug>(
    base: T,
    index: i64,
) -> Result<T, NegativeIndexError<T>> {
    if index >= 1 {
        Ok(pow_pos_impl(T::ONE.clone(), base, index))
    } else if index <= -1 {
        Err(NegativeIndexError::new(index))
    } else {
        Ok(T::ONE.clone())
    }
}

pub(crate) fn pow_times_group<T: Arithmetic + TimesGroup>(
    base: T,
    index: i64,
) -> T {
    if index >= 1 {
        pow_pos_impl(T::ONE, base, index)
    } else if index <= -1 {
        pow_neg_impl(T::ONE, base, index)
    } else {
        T::ONE.clone()
    }
}

pub fn exp<T: FloatingNumber>(index: T) -> T {
    let mut value = T::ONE.clone();
    let mut base = index.clone();
    let mut i = T::ONE.clone();
    loop {
        let this_item = base.clone() / i.clone();
        value += this_item.clone();
        base *= index.clone();
        i += T::ONE;

        if this_item <= T::EPSILON {
            break;
        }
    }
    value
}

pub fn powf<T: FloatingNumber>(base: T, index: T) -> Option<T>
    where for<'a> &'a T: Sub<&'a T, Output=T> +
        Mul<&'a T, Output=T> +
        Div<&'a T, Output=T> {
    if let Some(ln_base) = ln(base) {
        Some(exp(index * ln_base))
    } else {
        T::NAN
    }
}
