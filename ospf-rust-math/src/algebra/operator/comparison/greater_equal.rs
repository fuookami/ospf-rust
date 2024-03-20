use std::ops::Sub;

use crate::algebra::concept::*;
use crate::algebra::operator::Abs;

pub trait GreaterEqualOpr<T: Sized>: for<'a> Fn<(&'a T, &'a T), Output=bool> {
    fn precision(&self) -> &T;
}

#[derive(Clone, Copy, Debug)]
pub struct GreaterEqualInt {}

impl GreaterEqualInt {
    pub fn new() -> Self {
        Self {}
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for GreaterEqualInt {
    type Output = bool;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> Self::Output {
        x >= y
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for GreaterEqualInt {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> Self::Output {
        x >= y
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for GreaterEqualInt {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> Self::Output {
        x >= y
    }
}

impl<T: Arithmetic> GreaterEqualOpr<T> for GreaterEqualInt {
    fn precision(&self) -> &T {
        T::ZERO
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GreaterEqualFlt<T: Arithmetic> {
    pub(self) precision: T
}

impl<T: Arithmetic> From<T> for GreaterEqualFlt<T> {
    default fn from(precision: T) -> Self {
        Self {
            precision
        }
    }
}

impl<T: Arithmetic + Signed> From<&T> for GreaterEqualFlt<T> where for<'a> &'a T: Abs<Output=T> {
    fn from(precision: &T) -> Self {
        Self {
            precision: precision.abs()
        }
    }
}

impl<T: Arithmetic + Signed + Copy> From<T> for GreaterEqualFlt<T> where T: Abs<Output=T> {
    fn from(precision: T) -> Self {
        Self {
            precision: precision.abs()
        }
    }
}

impl<T: Arithmetic> GreaterEqualFlt<T> {
    pub fn new() -> Self where T: Precision {
        Self {
            precision: <T as Precision>::DECIMAL_PRECISION.clone()
        }
    }

    pub fn new_with(precision: T) -> Self where Self: From<T> {
        Self::from(precision)
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for GreaterEqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    type Output = bool;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> Self::Output {
        if x > y {
            true
        } else {
            &(y - x) <= self.precision()
        }
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for GreaterEqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> Self::Output {
        if x > y {
            true
        } else {
            &(y - x) <= self.precision()
        }
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for GreaterEqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> Self::Output {
        if x > y {
            true
        } else {
            &(y - x) <= self.precision()
        }
    }
}

impl<T: Arithmetic> GreaterEqualOpr<T> for GreaterEqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn precision(&self) -> &T {
        &self.precision
    }
}

pub trait GreaterEqualOprBuilder<T> {
    fn new() -> Box<dyn GreaterEqualOpr<T>>;
    fn new_with(precision: T) -> Box<dyn GreaterEqualOpr<T>>;
}

pub struct GreaterEqual<T> {
    _marker: std::marker::PhantomData<T>
}

impl<T: Arithmetic> GreaterEqualOprBuilder<T> for GreaterEqual<T> {
    default fn new() -> Box<dyn GreaterEqualOpr<T>> {
        Box::new(GreaterEqualInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn GreaterEqualOpr<T>> {
        Box::new(GreaterEqualInt::new())
    }
}

impl<T: Arithmetic> GreaterEqualOprBuilder<T> for GreaterEqual<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    default fn new() -> Box<dyn GreaterEqualOpr<T>> {
        Box::new(GreaterEqualInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn GreaterEqualOpr<T>> where GreaterEqualFlt<T>: From<T> {
        Box::new(GreaterEqualFlt::new_with(precision))
    }
}

impl<T: Arithmetic + FloatingNumber> GreaterEqualOprBuilder<T> for GreaterEqual<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn new() -> Box<dyn GreaterEqualOpr<T>> {
        Box::new(GreaterEqualFlt::new())
    }

    fn new_with(precision: T) -> Box<dyn GreaterEqualOpr<T>> where GreaterEqualFlt<T>: From<T> {
        Box::new(GreaterEqualFlt::new_with(precision))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geq_int() {
        let geq = GreaterEqual::<i64>::new();
        assert_eq!(geq(&1, &2), false);
        assert_eq!(geq(&2, &1), true);
        assert_eq!(geq(&1, &1), true);
    }

    #[test]
    fn test_geq_flt() {
        let geq = GreaterEqual::<f64>::new();
        assert_eq!(geq(&0.0, &0.0), true);
        assert_eq!(geq(&0.0, &1e-6), false);
        assert_eq!(geq(&1e-6, &0.0), true);
        assert_eq!(geq(&0.0, &1e-4), false);
        assert_eq!(geq(&1e-4, &0.0), true);

        let geq = GreaterEqual::<f64>::new_with(1e-5);
        assert_eq!(geq(&0.0, &0.0), true);
        assert_eq!(geq(&0.0, &1e-6), true);
        assert_eq!(geq(&1e-6, &0.0), true);
        assert_eq!(geq(&0.0, &1e-4), false);
        assert_eq!(geq(&1e-4, &0.0), true);
    }
}
