use std::ops::Rem;

use crate::algebra::concept::*;
use crate::algebra::operator::*;

pub(self) fn gcd_impl<I: Arithmetic + Rem<I, Output=I>>(x: I, y: I) -> I {
    let remainder = x % y.clone();

    if remainder == I::ZERO {
        y
    } else {
        gcd_impl(y, remainder)
    }
}

pub fn gcd<I: Arithmetic + Abs<Output=I> + Rem<I, Output=I>>(x: I, y: I) -> I {
    gcd_impl(x.abs(), y.abs())
}
