use std::ops::Sub;

use crate::algebra::concept::*;
use crate::algebra::operator::Abs;
use crate::{Equal, EqualFlt, EqualInt};

use super::zero::*;

pub trait UnequalOpr<T: Sized>: for <'a> Fn<(&'a T, &'a T), Output=bool> {
    fn precision(&self) -> &T;
}

#[derive(Clone, Copy, Debug)]
pub struct UnequalInt {}

impl UnequalInt {
    fn new() -> Self {
        Self {}
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for UnequalInt {
    type Output = bool;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> Self::Output {
        x != y
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for UnequalInt {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> Self::Output {
        x != y
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for UnequalInt {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> Self::Output {
        x != y
    }
}

impl<T: Arithmetic> UnequalOpr<T> for UnequalInt {
    fn precision(&self) -> &T {
        T::ZERO
    }
}

#[derive(Clone, Copy, Debug)]
pub struct UnequalFlt<T> {
    zero: ZeroFlt<T>
}

impl<T: Arithmetic> From<T> for UnequalFlt<T> where ZeroFlt<T>: From<T> {
    fn from(precision: T) -> Self {
        Self {
            zero: ZeroFlt::from(precision)
        }
    }
}

impl<T: Arithmetic> UnequalFlt<T> {
    pub fn new() -> Self where T: Precision {
        Self {
            zero: ZeroFlt::new()
        }
    }

    pub fn new_with(precision: T) -> Self where UnequalFlt<T>: From<T> {
        Self::from(precision)
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for UnequalFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    type Output = bool;

    default extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> bool {
        if x > y {
            !self.zero.call_once((&(x - y), ))
        } else {
            !self.zero.call_once((&(y - x), ))
        }
    }
}

impl<T: Arithmetic + Signed> FnOnce<(&T, &T)> for UnequalFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> + Abs<Output=T> {
    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> bool {
        !self.zero.call_once((&(x - y), ))
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for UnequalFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    default extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> bool {
        if x > y {
            !self.zero.call_mut((&(x - y), ))
        } else {
            !self.zero.call_mut((&(y - x), ))
        }
    }
}

impl<T: Arithmetic + Signed> FnMut<(&T, &T)> for UnequalFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> + Abs<Output=T> {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> bool {
        !self.zero.call_mut((&(x - y), ))
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for UnequalFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    default extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> bool {
        if x > y {
            !self.zero.call((&(x - y), ))
        } else {
            !self.zero.call((&(y - x), ))
        }
    }
}

impl<T: Arithmetic + Signed> Fn<(&T, &T)> for UnequalFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> + Abs<Output=T> {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> bool {
        !self.zero.call((&(x - y), ))
    }
}

impl<T: Arithmetic> UnequalOpr<T> for UnequalFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn precision(&self) -> &T {
        self.zero.precision()
    }
}

pub trait UnequalOprBuilder<T> {
    fn new() -> Box<dyn UnequalOpr<T>>;
    fn new_with(precision: T) -> Box<dyn UnequalOpr<T>>;
}

pub struct Unequal<T> {
    _marker: std::marker::PhantomData<T>
}

impl<T: Arithmetic> UnequalOprBuilder<T> for Unequal<T> {
    default fn new() -> Box<dyn UnequalOpr<T>> {
        Box::new(UnequalInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn UnequalOpr<T>> {
        Box::new(UnequalInt::new())
    }
}

impl<T: Arithmetic> UnequalOprBuilder<T> for Unequal<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    default fn new() -> Box<dyn UnequalOpr<T>> {
        Box::new(UnequalInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn UnequalOpr<T>> where UnequalFlt<T>: From<T> {
        Box::new(UnequalFlt::new_with(precision))
    }
}

impl<T: Arithmetic + FloatingNumber> UnequalOprBuilder<T> for Unequal<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn new() -> Box<dyn UnequalOpr<T>> where T: Precision {
        Box::new(UnequalFlt::new())
    }

    fn new_with(precision: T) -> Box<dyn UnequalOpr<T>> where UnequalFlt<T>: From<T> {
        Box::new(UnequalFlt::new_with(precision))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neq_int() {
        let neq = Unequal::<i64>::new();
        assert_eq!(neq(&0, &0), false);
        assert_eq!(neq(&1, &0), true);
    }

    #[test]
    fn test_neq_flt() {
        let neq = Unequal::<f64>::new();
        assert_eq!(neq(&0.0, &0.0), false);
        assert_eq!(neq(&1e-6, &0.0), true);

        let neq = Unequal::<f64>::new_with(1e-5);
        assert_eq!(neq(&0.0, &0.0), false);
        assert_eq!(neq(&1e-6, &0.0), false);
    }
}
