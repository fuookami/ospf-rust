use std::marker::Tuple;

use crate::algebra::concept::{Arithmetic, Precision, Unsigned, Signed};
use crate::algebra::operator::Abs;
use crate::FloatingNumber;

trait ZeroOpr<T: Sized>: Sized + FnOnce<(T, ), Output=bool> + for<'a> FnOnce<(&'a T, ), Output=bool> {
    fn precision(&self) -> &T;
}

struct ZeroInt {}

impl ZeroInt {
    fn new() -> Self {
        Self {}
    }
}

impl<T: Arithmetic> FnOnce<(T, )> for ZeroInt {
    type Output = bool;

    extern "rust-call" fn call_once(self, args: (T, )) -> Self::Output {
        &args.0 == T::ZERO
    }
}

impl<T: Arithmetic> FnOnce<(&T, )> for ZeroInt {
    type Output = bool;

    extern "rust-call" fn call_once(self, args: (&T, )) -> Self::Output {
        args.0 == T::ZERO
    }
}

impl<T: Arithmetic> ZeroOpr<T> for ZeroInt {
    fn precision(&self) -> &T {
        T::ZERO
    }
}

struct ZeroFlt<T: Sized> {
    pub(self) precision: T
}

impl <T: Arithmetic + Signed> From<&T> for ZeroFlt<T> where for<'a> &'a T: Abs<Output=T> {
    fn from(precision: &T) -> Self {
        Self {
            precision: precision.abs()
        }
    }
}

impl <T: Arithmetic + Unsigned> From<T> for ZeroFlt<T> {
    fn from(precision: T) -> Self {
        Self {
            precision
        }
    }
}

impl<T: Arithmetic> ZeroFlt<T> {
    fn new() -> Self where T: Precision {
        Self {
            precision: <T as Precision>::DECIMAL_PRECISION.clone()
        }
    }

    fn new_with(precision: T) -> Self where Self: From<T> {
        Self::from(precision)
    }
}

impl<T: Arithmetic> FnOnce<(T, )> for ZeroFlt<T> {
    type Output = bool;

    extern "rust-call" fn call_once(self, args: (T, )) -> Self::Output {
        &args.0 <= self.precision()
    }
}

impl<T: Arithmetic> FnOnce<(&T, )> for ZeroFlt<T> {
    type Output = bool;

    extern "rust-call" fn call_once(self, args: (&T, )) -> Self::Output {
        args.0 <= self.precision()
    }
}

impl<T: Arithmetic + Signed> FnOnce<(T, )> for ZeroFlt<T> where for<'a> &'a T: Abs<Output=T> {
    type Output = bool;

    extern "rust-call" fn call_once(self, args: (T, )) -> Self::Output {
        &(&args.0).abs() <= self.precision()
    }
}

impl<T: Arithmetic + Signed> FnOnce<(&T, )> for ZeroFlt<T> where for<'a> &'a T: Abs<Output=T> {
    type Output = bool;

    extern "rust-call" fn call_once(self, args: (&T, )) -> Self::Output {
        &args.0.abs() <= self.precision()
    }
}

impl<T: Arithmetic> ZeroOpr<T> for ZeroFlt<T> {
    fn precision(&self) -> &T {
        &self.precision
    }
}
