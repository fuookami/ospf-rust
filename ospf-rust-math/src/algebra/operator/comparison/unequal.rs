use std::marker::Tuple;
use std::ops::Sub;

use crate::algebra::concept::{Arithmetic, Precision};
use crate::algebra::operator::Abs;
use crate::Equal;

use super::Zero;

pub struct Unequal<T: Sized> {
    pub(self) zero: Zero<T>,
}

impl <T> From<T> for Unequal<T> where Zero<T>: From<T> {
    fn from(precision: T) -> Self {
        Self {
            zero: Zero::from(precision)
        }
    }
}

impl<T: Arithmetic> Unequal<T> {
    pub fn new() -> Self where T: Precision {
        Self {
            zero: Zero::new(),
        }
    }

    pub fn new_with(precision: T) -> Self where Self: From<T> {
        Self::from(precision)
    }

    pub fn precision(&self) -> &T {
        self.zero.precision()
    }
}

impl<T: Arithmetic, U> FnOnce<(T, U)> for Unequal<T>
    where
        for<'a> &'a T: Sub<&'a U, Output=T>,
        Zero<T>: FnOnce<(T, ), Output=bool> {
    type Output = bool;

    extern "rust-call" fn call_once(self, args: (T, U)) -> Self::Output {
        !self.zero.call_once((&args.0 - &args.1, ))
    }
}

impl<T: Arithmetic, U> FnOnce<(&T, &U)> for Unequal<T>
    where
        for<'a> &'a T: Sub<&'a U, Output=T>,
        Zero<T>: FnOnce<(T, ), Output=bool> {
    type Output = bool;

    extern "rust-call" fn call_once(self, args: (&T, &U)) -> Self::Output {
        !self.zero.call_once((args.0 - args.1, ))
    }
}

impl<T: Tuple> FnMut<T> for Unequal<T> where Unequal<T>: FnOnce<T> {
    extern "rust-call" fn call_mut(&mut self, args: T) -> Self::Output {
        self.call_once(args)
    }
}

impl<T: Tuple> Fn<T> for Unequal<T> where Unequal<T>: FnMut<T> {
    extern "rust-call" fn call(&self, args: T) -> Self::Output {
        self.call_once(args)
    }
}
