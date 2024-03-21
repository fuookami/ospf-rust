use std::ops::Sub;

use crate::algebra::concept::*;
use crate::algebra::operator::Abs;

pub trait LessEqualOpr<T: Sized>: for<'a, 'b> Fn<(&'a T, &'b T), Output=bool> {
    fn precision(&self) -> &T;
}

#[derive(Clone, Copy, Debug)]
pub struct LessEqualInt {}

impl LessEqualInt {
    pub fn new() -> Self {
        Self {}
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for LessEqualInt {
    type Output = bool;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> bool {
        x <= y
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for LessEqualInt {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> bool {
        x <= y
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for LessEqualInt {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> bool {
        x <= y
    }
}

impl<T: Arithmetic> LessEqualOpr<T> for LessEqualInt {
    fn precision(&self) -> &T {
        T::ZERO
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LessEqualFlt<T: Arithmetic> {
    pub(self) precision: T
}

impl<T: Arithmetic> From<T> for LessEqualFlt<T> {
    default fn from(precision: T) -> Self {
        Self {
            precision
        }
    }
}

impl<T: Arithmetic + Signed> From<&T> for LessEqualFlt<T> where for<'a> &'a T: Abs<Output=T> {
    fn from(precision: &T) -> Self {
        Self {
            precision: precision.abs()
        }
    }
}

impl<T: Arithmetic + Signed + Copy> From<T> for LessEqualFlt<T> where T: Abs<Output=T> {
    fn from(precision: T) -> Self {
        Self {
            precision: precision.abs()
        }
    }
}

impl<T: Arithmetic> LessEqualFlt<T> {
    pub fn new() -> Self where T: Precision {
        Self {
            precision: <T as Precision>::DECIMAL_PRECISION.clone()
        }
    }

    pub fn new_with(precision: T) -> Self where Self: From<T> {
        Self::from(precision)
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for LessEqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    type Output = bool;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> bool {
        if x < y {
            true
        } else {
            &(x - y) <= self.precision()
        }
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for LessEqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> bool {
        if x < y {
            true
        } else {
            &(x - y) <= self.precision()
        }
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for LessEqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> bool {
        if x < y {
            true
        } else {
            &(x - y) <= self.precision()
        }
    }
}

impl<T: Arithmetic> LessEqualOpr<T> for LessEqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn precision(&self) -> &T {
        &self.precision
    }
}

pub trait LessEqualOprBuilder<T> {
    fn new() -> Box<dyn LessEqualOpr<T>>;
    fn new_with(precision: T) -> Box<dyn LessEqualOpr<T>>;
}

pub struct LessEqual<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: Arithmetic> LessEqualOprBuilder<T> for LessEqual<T> {
    default fn new() -> Box<dyn LessEqualOpr<T>> {
        Box::new(LessEqualInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn LessEqualOpr<T>> {
        Box::new(LessEqualInt::new())
    }
}

impl<T: Arithmetic> LessEqualOprBuilder<T> for LessEqual<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    default fn new() -> Box<dyn LessEqualOpr<T>> {
        Box::new(LessEqualInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn LessEqualOpr<T>> where LessEqualFlt<T>: From<T> {
        Box::new(LessEqualFlt::new_with(precision))
    }
}

impl<T: Arithmetic + FloatingNumber> LessEqualOprBuilder<T> for LessEqual<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn new() -> Box<dyn LessEqualOpr<T>> where T: Precision {
        Box::new(LessEqualFlt::new())
    }

    fn new_with(precision: T) -> Box<dyn LessEqualOpr<T>> where LessEqualFlt<T>: From<T> {
        Box::new(LessEqualFlt::new_with(precision))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leq_int() {
        let leq = LessEqual::<i64>::new();
        assert_eq!(leq(&1, &2), true);
        assert_eq!(leq(&2, &1), false);
        assert_eq!(leq(&1, &1), true);
    }

    #[test]
    fn test_leq_flt() {
        let leq = LessEqual::<f64>::new();
        assert_eq!(leq(&0.0, &0.0), true);
        assert_eq!(leq(&0.0, &1e-6), true);
        assert_eq!(leq(&1e-6, &0.0), false);
        assert_eq!(leq(&0.0, &1e-4), true);
        assert_eq!(leq(&1e-4, &0.0), false);

        let leq = LessEqual::<f64>::new_with(1e-5);
        assert_eq!(leq(&0.0, &0.0), true);
        assert_eq!(leq(&0.0, &1e-6), true);
        assert_eq!(leq(&1e-6, &0.0), true);
        assert_eq!(leq(&0.0, &1e-4), true);
        assert_eq!(leq(&1e-4, &0.0), false);
    }
}
