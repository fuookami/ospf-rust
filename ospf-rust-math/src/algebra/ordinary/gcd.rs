use std::ops::Rem;

use crate::algebra::{Abs, Integer};

pub(self) fn gcd_impl<I: Integer + Abs<Output=I> + Rem<I, Output=I>>(x: I, y: I) -> I {
    let remainder = x % y;

    if remainder == I::ZERO {
        y
    } else {
        gcd_impl(y, remainder)
    }
}

pub fn gcd<I: Integer + Abs<Output=I> + Rem<I, Output=I>>(x: I, y: I) -> I {
    gcd_impl(x.abs(), y.abs())
}
