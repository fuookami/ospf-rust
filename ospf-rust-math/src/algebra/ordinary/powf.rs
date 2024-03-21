use std::fmt::{Debug, Display, Formatter};
use std::ops::{Div, Mul, Sub};

use crate::algebra::concept::FloatingNumber;

use super::ln;

pub fn exp<T: FloatingNumber>(index: &T) -> T where for<'a> &'a T: Div<&'a T, Output=T> {
    let mut value = T::ONE.clone();
    let mut base = index.clone();
    let mut i = T::ONE.clone();
    loop {
        let this_item = &base / &i;
        value += &this_item;
        base *= index;
        i += T::ONE;

        if &this_item <= T::EPSILON {
            break;
        }
    }
    value
}

pub fn powf<T: FloatingNumber>(base: &T, index: &T) -> Option<T>
    where for<'a> &'a T: Sub<&'a T, Output=T> +
    Mul<&'a T, Output=T> +
    Div<&'a T, Output=T> {
    if let Some(ln_base) = ln(base) {
        Some(exp(&(index * &ln_base)))
    } else {
        T::NAN.clone()
    }
}
