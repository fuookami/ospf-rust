use std::ops::Sub;

use crate::algebra::concept::*;
use crate::algebra::operator::Abs;

pub trait GreaterOpr<T: Sized>: for<'a, 'b> Fn<(&'a T, &'b T), Output=bool> {
    fn precision(&self) -> &T;
}

#[derive(Clone, Copy, Debug)]
pub struct GreaterInt {}

impl GreaterInt {
    pub fn new() -> Self {
        Self {}
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for GreaterInt {
    type Output = bool;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> Self::Output {
        x > y
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for GreaterInt {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> Self::Output {
        x > y
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for GreaterInt {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> Self::Output {
        x > y
    }
}

impl<T: Arithmetic> GreaterOpr<T> for GreaterInt {
    fn precision(&self) -> &T {
        T::ZERO
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GreaterFlt<T: Arithmetic> {
    pub(self) precision: T
}

impl<T: Arithmetic> From<T> for GreaterFlt<T> {
    default fn from(precision: T) -> Self {
        Self {
            precision
        }
    }
}

impl<T: Arithmetic + Signed> From<&T> for GreaterFlt<T> where for<'a> &'a T: Abs<Output=T> {
    fn from(precision: &T) -> Self {
        Self {
            precision: precision.abs()
        }
    }
}

impl<T: Arithmetic + Signed + Copy> From<T> for GreaterFlt<T> where T: Abs<Output=T> {
    fn from(precision: T) -> Self {
        Self {
            precision: precision.abs()
        }
    }
}

impl<T: Arithmetic> GreaterFlt<T> {
    pub fn new() -> Self where T: Precision {
        Self {
            precision: <T as Precision>::DECIMAL_PRECISION.clone()
        }
    }

    pub fn new_with(precision: T) -> Self where Self: From<T> {
        Self::from(precision)
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for GreaterFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    type Output = bool;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> Self::Output {
        if x < y {
            false
        } else {
            &(x - y) > self.precision()
        }
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for GreaterFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> Self::Output {
        if x < y {
            false
        } else {
            &(x - y) > self.precision()
        }
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for GreaterFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> Self::Output {
        if x < y {
            false
        } else {
            &(x - y) > self.precision()
        }
    }
}

impl<T: Arithmetic> GreaterOpr<T> for GreaterFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn precision(&self) -> &T {
        &self.precision
    }
}

pub trait GreaterOprBuilder<T> {
    fn new() -> Box<dyn GreaterOpr<T>>;
    fn new_with(precision: T) -> Box<dyn GreaterOpr<T>>;
}

pub struct Greater<T> {
    _marker: std::marker::PhantomData<T>
}

impl<T: Arithmetic> GreaterOprBuilder<T> for Greater<T> {
    default fn new() -> Box<dyn GreaterOpr<T>> {
        Box::new(GreaterInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn GreaterOpr<T>> {
        Box::new(GreaterInt::new())
    }
}

impl<T: Arithmetic> GreaterOprBuilder<T> for Greater<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    default fn new() -> Box<dyn GreaterOpr<T>> {
        Box::new(GreaterInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn GreaterOpr<T>> where GreaterFlt<T>: From<T> {
        Box::new(GreaterFlt::new_with(precision))
    }
}

impl<T: Arithmetic + FloatingNumber> GreaterOprBuilder<T> for Greater<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn new() -> Box<dyn GreaterOpr<T>> {
        Box::new(GreaterFlt::new())
    }

    fn new_with(precision: T) -> Box<dyn GreaterOpr<T>> where GreaterFlt<T>: From<T> {
        Box::new(GreaterFlt::new_with(precision))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gr_int() {
        let gr = Greater::<i64>::new();
        assert_eq!(gr(&1, &2), false);
        assert_eq!(gr(&2, &1), true);
        assert_eq!(gr(&1, &1), false);
    }

    #[test]
    fn test_gr_flt() {
        let gr = Greater::<f64>::new();
        assert_eq!(gr(&0.0, &0.0), false);
        assert_eq!(gr(&0.0, &1e-6), false);
        assert_eq!(gr(&1e-6, &0.0), true);
        assert_eq!(gr(&0.0, &1e-4), false);
        assert_eq!(gr(&1e-4, &0.0), true);

        let gr = Greater::<f64>::new_with(1e-5);
        assert_eq!(gr(&0.0, &0.0), false);
        assert_eq!(gr(&0.0, &1e-6), false);
        assert_eq!(gr(&1e-6, &0.0), false);
        assert_eq!(gr(&0.0, &1e-4), false);
        assert_eq!(gr(&1e-4, &0.0), true);
    }
}
