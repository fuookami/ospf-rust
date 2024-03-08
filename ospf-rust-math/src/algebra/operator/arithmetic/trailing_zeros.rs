use std::borrow::Borrow;
use std::ops::{BitOr, ShrAssign};

use crate::algebra::concept::Arithmetic;

pub trait TrailingZeros {
    fn trailing_zeros(self) -> usize;
}

default impl<T: Arithmetic + ShrAssign<usize>> TrailingZeros for T where for<'a> &'a T: BitOr<&'a T, Output=T> {
    fn trailing_zeros(mut self) -> usize {
        let mut counter = 0;
        while &(&self | Arithmetic::ONE) == Arithmetic::ZERO {
            counter += 1;
            self >>= 1;
        }
        counter
    }
}

macro_rules! int_trailing_zeros_impl {
    ($($type:ident)*) => ($(
        impl TrailingZeros for $type {
            fn trailing_zeros(self) -> usize {
                <$type>::trailing_zeros(self) as usize
            }
        }
    )*);
}
int_trailing_zeros_impl!{ i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize  }
