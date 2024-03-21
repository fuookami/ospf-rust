use std::ops::Sub;

use crate::algebra::concept::*;
use crate::algebra::operator::Abs;

use super::zero::*;

pub trait EqualOpr<T: Sized>: for<'a, 'b> Fn<(&'a T, &'b T), Output=bool> {
    fn precision(&self) -> &T;
}

#[derive(Clone, Copy, Debug)]
pub struct EqualInt {}

impl EqualInt {
    fn new() -> Self {
        Self {}
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for EqualInt {
    type Output = bool;

    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> bool {
        x == y
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for EqualInt {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> bool {
        x == y
    }
}

impl<T: Arithmetic> Fn<(&T, &T)> for EqualInt {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> bool {
        x == y
    }
}

impl<T: Arithmetic> EqualOpr<T> for EqualInt {
    fn precision(&self) -> &T {
        T::ZERO
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EqualFlt<T: Sized> {
    zero: ZeroFlt<T>
}

impl<T: Arithmetic> From<T> for EqualFlt<T> where ZeroFlt<T>: From<T> {
    fn from(precision: T) -> Self {
        Self {
            zero: ZeroFlt::from(precision)
        }
    }
}

impl<T: Arithmetic> EqualFlt<T> {
    pub fn new() -> Self where T: Precision {
        Self {
            zero: ZeroFlt::new()
        }
    }

    pub fn new_with(precision: T) -> Self where EqualFlt<T>: From<T> {
        Self::from(precision)
    }
}

impl<T: Arithmetic> FnOnce<(&T, &T)> for EqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    type Output = bool;
    
    default extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> bool {
        if x > y {
            self.zero.call_once((&(x - y), ))
        } else {
            self.zero.call_once((&(y - x), ))
        }
    }
}

impl<T: Arithmetic + Signed> FnOnce<(&T, &T)> for EqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> + Abs<Output=T> {
    extern "rust-call" fn call_once(self, (x, y): (&T, &T)) -> bool {
        self.zero.call_once((&(x - y), ))
    }
}

impl<T: Arithmetic> FnMut<(&T, &T)> for EqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    default extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> bool {
        if x > y {
            self.zero.call_mut((&(x - y), ))
        } else {
            self.zero.call_mut((&(y - x), ))
        }
    }
}
    
impl<T: Arithmetic + Signed> FnMut<(&T, &T)> for EqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> + Abs<Output=T> {
    extern "rust-call" fn call_mut(&mut self, (x, y): (&T, &T)) -> Self::Output {
        self.zero.call_mut((&(x - y), ))
    }
}
    
impl<T: Arithmetic> Fn<(&T, &T)> for EqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    default extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> bool {
        if x > y {
            self.zero.call((&(x - y), ))
        } else {
            self.zero.call((&(y - x), ))
        }
    }
}

impl<T: Arithmetic + Signed> Fn<(&T, &T)> for EqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> + Abs<Output=T> {
    extern "rust-call" fn call(&self, (x, y): (&T, &T)) -> Self::Output {
        self.zero.call((&(x - y), ))
    }
}

impl<T: Arithmetic> EqualOpr<T> for EqualFlt<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn precision(&self) -> &T {
        self.zero.precision()
    }
}

pub trait EqualOprBuilder<T> {
    fn new() -> Box<dyn EqualOpr<T>>;
    fn new_with(precision: T) -> Box<dyn EqualOpr<T>>;
}

pub struct Equal<T> {
    _marker: std::marker::PhantomData<T>
}

impl<T: Arithmetic> EqualOprBuilder<T> for Equal<T> {
    default fn new() -> Box<dyn EqualOpr<T>> {
        Box::new(EqualInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn EqualOpr<T>> {
        Box::new(EqualInt::new())
    }
}

impl<T: Arithmetic> EqualOprBuilder<T> for Equal<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    default fn new() -> Box<dyn EqualOpr<T>> {
        Box::new(EqualInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn EqualOpr<T>> where EqualFlt<T>: From<T> {
        Box::new(EqualFlt::new_with(precision))
    }
}

impl <T: Arithmetic + FloatingNumber> EqualOprBuilder<T> for Equal<T> where for<'a> &'a T: Sub<&'a T, Output=T> {
    fn new() -> Box<dyn EqualOpr<T>> where T: Precision {
        Box::new(EqualFlt::new())
    }

    fn new_with(precision: T) -> Box<dyn EqualOpr<T>> where EqualFlt<T>: From<T> {
        Box::new(EqualFlt::new_with(precision))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eq_int() {
        let eq = Equal::<i64>::new();
        assert_eq!(eq(&0, &0), true);
        assert_eq!(eq(&1, &0), false);
    }

    #[test]
    fn test_eq_flt() {
        let eq = Equal::<f64>::new();
        assert_eq!(eq(&0.0, &0.0), true);
        assert_eq!(eq(&1e-6, &0.0), false);

        let eq = Equal::<f64>::new_with(1e-5);
        assert_eq!(eq(&0.0, &0.0), true);
        assert_eq!(eq(&1e-6, &0.0), true);
    }
}
