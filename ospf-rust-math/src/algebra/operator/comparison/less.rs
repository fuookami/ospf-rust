use std::marker::Tuple;
use std::ops::{Neg, Sub};

use crate::algebra::concept::{Arithmetic, Precision, Signed, Unsigned};
use crate::algebra::operator::Abs;

struct LessUnsigned<T: Sized> {}

struct LessSigned<T: Sized + Signed> {}

pub struct Less<T: Sized> {}

impl<T: Signed> From<T> for Less<T> where for<'a> &'a T: Abs<Output=T> + Neg<Output=T>

impl<T: Arithmetic + Abs<Output=T> + Neg<Output=T>> Less<T> {
    pub fn new() -> Self
        where
            T: Precision,
    {
        Self::new_with(<T as Precision>::DECIMAL_PRECISION)
    }

    pub fn new_with(precision: T) -> Self {
        let actual_precision = precision.abs();
        let neg_precision = actual_precision.neg();
        Self {
            precision: actual_precision,
            neg_precision: neg_precision,
        }
    }

    pub fn precision(&self) -> &T {
        &self.precision
    }
}

impl<T: Tuple> FnMut<T> for Less<T> where Less<T>: FnOnce<T> {
    extern "rust-call" fn call_mut(&mut self, args: T) -> Self::Output {
        self.call_once(args)
    }
}

impl<T: Tuple> Fn<T> for Less<T> where Less<T>: FnMut<T> {
    extern "rust-call" fn call(&self, args: T) -> Self::Output {
        self.call_once(args)
    }
}
