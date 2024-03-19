use std::ops::Sub;

use crate::algebra::concept::*;
use crate::algebra::operator::Abs;

pub trait LessOpr<T: Sized>: for<'a> Fn<(&'a T, &'a T), Output=bool> {
    fn precision(&self) -> &T;
}

#[derive(Clone, Copy, Debug)]
pub struct LessInt {}

impl LessInt {
    pub fn new() -> Self {
        Self {}
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for LessInt {
    type Output = bool;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> bool {
        x < y
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for LessInt {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> bool {
        x < y
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for LessInt {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> bool {
        x < y
    }
}

impl<T: Arithmetic> LessOpr<T> for LessInt {
    fn precision(&self) -> &T {
        T::ZERO
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LessFlt<T: Arithmetic> {
    pub(self) precision: T
}

impl<T: Arithmetic> From<T> for LessFlt<T> {
    fn from(precision: T) -> Self {
        Self {
            precision
        }
    }
}

impl<T: Arithmetic + Signed> From<&T> for LessFlt<T> where for<'a> &'a T: Abs<Output=T> {
    fn from(precision: &T) -> Self {
        Self {
            precision: precision.abs()
        }
    }
}

impl<T: Arithmetic> LessFlt<T> {
    pub fn new() -> Self where T: Precision {
        Self {
            precision: <T as Precision>::DECIMAL_PRECISION.clone()
        }
    }

    pub fn new_with(precision: T) -> Self where Self: From<T> {
        Self::from(precision)
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for LessFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    type Output = bool;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> bool {
        if (x > y) {
            false
        } else {
            &(y - x) >= self.precision()
        }
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for LessFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> bool {
        if (x > y) {
            false
        } else {
            &(y - x) >= self.precision()
        }
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for LessFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> bool {
        if (x > y) {
            false
        } else {
            &(y - x) >= self.precision()
        }
    }
}

impl<T: Arithmetic> LessOpr<T> for LessFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn precision(&self) -> &T {
        &self.precision
    }
}

pub trait LessOprBuilder<T> {
    fn new() -> Box<dyn LessOpr<T, Output=bool>>;
    fn new_with(precision: T) -> Box<dyn LessOpr<T, Output=bool>>;
}

pub struct Less<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: Arithmetic> LessOprBuilder<T> for Less<T> {
    default fn new() -> Box<dyn LessOpr<T, Output=bool>> {
        Box::new(LessInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn LessOpr<T, Output=bool>> {
        Box::new(LessInt::new())
    }
}

impl<T: Arithmetic + FloatingNumber> LessOprBuilder<T> for Less<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn new() -> Box<dyn LessOpr<T, Output=bool>> where T: Precision {
        Box::new(LessFlt::new())
    }

    fn new_with(precision: T) -> Box<dyn LessOpr<T, Output=bool>> where LessFlt<T>: From<T> {
        Box::new(LessFlt::new_with(precision))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ls_int() {
        let ls = Less::<i64>::new();
        assert_eq!(ls(&1, &2), true);
        assert_eq!(ls(&2, &1), false);
        assert_eq!(ls(&1, &1), false);
    }

    #[test]
    fn test_ls_flt() {
        let ls = Less::<f64>::new();
        assert_eq!(ls(&0.0, &0.0), false);
        assert_eq!(ls(&0.0, &1e-6), true);
        assert_eq!(ls(&1e-6, &0.0), false);
        assert_eq!(ls(&0.0, &1e-4), true);
        assert_eq!(ls(&1e-4, &0.0), false);

        let ls = Less::<f64>::new_with(1e-5);
        assert_eq!(ls(&0.0, &0.0), false);
        assert_eq!(ls(&0.0, &1e-6), false);
        assert_eq!(ls(&1e-6, &0.0), false);
        assert_eq!(ls(&0.0, &1e-4), true);
        assert_eq!(ls(&1e-4, &0.0), false);
    }
}
