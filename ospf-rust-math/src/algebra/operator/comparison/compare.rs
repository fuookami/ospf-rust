use std::cmp::Ordering;
use std::ops::Sub;

use crate::algebra::concept::*;
use crate::algebra::operator::Abs;

use super::equal::*;

pub trait CompareOpr<T: Sized>: for<'a, 'b> Fn<(&'a T, &'b T), Output=Option<Ordering>> {
    fn precision(&self) -> &T;
}

#[derive(Clone, Copy, Debug)]
pub struct CompareInt {}

impl CompareInt {
    fn new() -> Self {
        Self {}
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for CompareInt {
    type Output = Option<Ordering>;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> Option<Ordering> {
        x.partial_cmp(y)
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for CompareInt {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> Option<Ordering> {
        x.partial_cmp(y)
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for CompareInt {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> Option<Ordering> {
        x.partial_cmp(y)
    }
}

impl<T: Arithmetic> CompareOpr<T> for CompareInt {
    fn precision(&self) -> &T {
        T::ZERO
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CompareFlt<T: Sized> {
    eq: EqualFlt<T>
}

impl<T: Arithmetic> From<T> for CompareFlt<T> where EqualFlt<T>: From<T> {
    fn from(precision: T) -> Self {
        Self {
            eq: EqualFlt::from(precision)
        }
    }
}

impl<T: Arithmetic> CompareFlt<T> {
    pub fn new() -> Self where T: Precision {
        Self {
            eq: EqualFlt::new()
        }
    }

    pub fn new_with(precision: T) -> Self where CompareFlt<T>: From<T> {
        Self::from(precision)
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for CompareFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    type Output = Option<Ordering>;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> Option<Ordering> {
        if (self.eq)(x, y) {
            Some(Ordering::Equal)
        } else if x < y {
            Some(Ordering::Less)
        } else if x > y {
            Some(Ordering::Greater)
        } else {
            None
        }
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for CompareFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> Option<Ordering> {
        if (self.eq)(x, y) {
            Some(Ordering::Equal)
        } else if x < y {
            Some(Ordering::Less)
        } else if x > y {
            Some(Ordering::Greater)
        } else {
            None
        }
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for CompareFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> Option<Ordering> {
        if (self.eq)(x, y) {
            Some(Ordering::Equal)
        } else if x < y {
            Some(Ordering::Less)
        } else if x > y {
            Some(Ordering::Greater)
        } else {
            None
        }
    }
}

impl<T: Arithmetic> CompareOpr<T> for CompareFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn precision(&self) -> &T {
        self.eq.precision()
    }
}

pub trait CompareOprBuilder<T> {
    fn new() -> Box<dyn CompareOpr<T>>;
    fn new_with(precision: T) -> Box<dyn CompareOpr<T>>;
}

pub struct Compare<T> {
    _marker: std::marker::PhantomData<T>
}

impl<T: Arithmetic> CompareOprBuilder<T> for Compare<T> {
    default fn new() -> Box<dyn CompareOpr<T>> {
        Box::new(CompareInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn CompareOpr<T>> {
        Box::new(CompareInt::new())
    }
}

impl<T: Arithmetic> CompareOprBuilder<T> for Compare<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    default fn new() -> Box<dyn CompareOpr<T>> {
        Box::new(CompareInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn CompareOpr<T>> where CompareFlt<T>: From<T> {
        Box::new(CompareFlt::new_with(precision))
    }
}

impl<T: Arithmetic + FloatingNumber> CompareOprBuilder<T> for Compare<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn new() -> Box<dyn CompareOpr<T>> where T: Precision {
        Box::new(CompareFlt::new())
    }

    fn new_with(precision: T) -> Box<dyn CompareOpr<T>> where CompareFlt<T>: From<T> {
        Box::new(CompareFlt::new_with(precision))
    }
}

#[cfg(test)]
mod tests {
    use crate::Greater;
    use super::*;

    #[test]
    fn test_cmp_int() {
        let cmp = Compare::<i64>::new();
        assert_eq!(cmp(&0, &0), Some(Ordering::Equal));
        assert_eq!(cmp(&1, &2), Some(Ordering::Less));
        assert_eq!(cmp(&2, &1), Some(Ordering::Greater));
    }

    #[test]
    fn test_cmp_flt() {
        let cmp = Compare::<f64>::new();
        assert_eq!(cmp(&0.0, &0.0), Some(Ordering::Equal));
        assert_eq!(cmp(&0.0, &1e-6), Some(Ordering::Less));
        assert_eq!(cmp(&1e-6, &0.0), Some(Ordering::Greater));
        assert_eq!(cmp(&0.0, &1e-4), Some(Ordering::Less));
        assert_eq!(cmp(&1e-4, &0.0), Some(Ordering::Greater));

        let gr = Compare::<f64>::new_with(1e-5);
        assert_eq!(gr(&0.0, &0.0), Some(Ordering::Equal));
        assert_eq!(gr(&0.0, &1e-6), Some(Ordering::Equal));
        assert_eq!(gr(&1e-6, &0.0), Some(Ordering::Equal));
        assert_eq!(gr(&0.0, &1e-4), Some(Ordering::Less));
        assert_eq!(gr(&1e-4, &0.0), Some(Ordering::Greater));
    }
}
