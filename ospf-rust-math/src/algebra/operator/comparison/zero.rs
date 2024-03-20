use crate::algebra::concept::*;
use crate::algebra::operator::Abs;

pub trait ZeroOpr<T: Sized>: for<'a> Fn<(&'a T, ), Output=bool> {
    fn precision(&self) -> &T;
}

#[derive(Clone, Copy, Debug)]
pub struct ZeroInt {}

impl ZeroInt {
    pub fn new() -> Self {
        Self {}
    }
}

impl<T: Arithmetic> FnOnce<(&T, )> for ZeroInt {
    type Output = bool;

    extern "rust-call" fn call_once(self, (x, ): (&T, )) -> bool {
        x == T::ZERO
    }
}

impl<T: Arithmetic> FnMut<(&T, )> for ZeroInt {
    extern "rust-call" fn call_mut(&mut self, (x, ): (&T, )) -> bool {
        x == T::ZERO
    }
}

impl<T: Arithmetic> Fn<(&T, )> for ZeroInt {
    extern "rust-call" fn call(&self, (x, ): (&T, )) -> bool {
        x == T::ZERO
    }
}

impl<T: Arithmetic> ZeroOpr<T> for ZeroInt {
    fn precision(&self) -> &T {
        T::ZERO
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ZeroFlt<T: Sized> {
    pub(self) precision: T,
}

impl<T: Arithmetic> From<T> for ZeroFlt<T> {
    default fn from(precision: T) -> Self {
        Self {
            precision
        }
    }
}

impl<T: Arithmetic + Signed> From<&T> for ZeroFlt<T> where for<'a> &'a T: Abs<Output=T> {
    fn from(precision: &T) -> Self {
        Self {
            precision: precision.abs()
        }
    }
}

impl<T: Arithmetic + Signed + Copy> From<T> for ZeroFlt<T> where T: Abs<Output=T> {
    fn from(precision: T) -> Self {
        Self {
            precision: precision.abs()
        }
    }
}

impl<T: Arithmetic> ZeroFlt<T> {
    pub fn new() -> Self where T: Precision {
        Self {
            precision: <T as Precision>::DECIMAL_PRECISION.clone()
        }
    }

    pub fn new_with(precision: T) -> Self where Self: From<T> {
        Self::from(precision)
    }
}

impl<T: Arithmetic> FnOnce<(&T, )> for ZeroFlt<T> {
    type Output = bool;

    default extern "rust-call" fn call_once(self, (x, ): (&T, )) -> bool {
        x <= self.precision()
    }
}

impl<T: Arithmetic + Signed> FnOnce<(&T, )> for ZeroFlt<T> where for<'a> &'a T: Abs<Output=T> {
    extern "rust-call" fn call_once(self, (x, ): (&T, )) -> bool {
        &x.abs() <= self.precision()
    }
}

impl<T: Arithmetic> FnMut<(&T, )> for ZeroFlt<T> {
    default extern "rust-call" fn call_mut(&mut self, (x, ): (&T, )) -> bool {
        x <= self.precision()
    }
}

impl<T: Arithmetic + Signed> FnMut<(&T, )> for ZeroFlt<T> where for<'a> &'a T: Abs<Output=T> {
    extern "rust-call" fn call_mut(&mut self, (x, ): (&T, )) -> bool {
        &x.abs() <= self.precision()
    }
}

impl<T: Arithmetic> Fn<(&T, )> for ZeroFlt<T> {
    default extern "rust-call" fn call(&self, (x, ): (&T, )) -> bool {
        x <= self.precision()
    }
}

impl<T: Arithmetic + Signed> Fn<(&T, )> for ZeroFlt<T> where for<'a> &'a T: Abs<Output=T> {
    extern "rust-call" fn call(&self, (x, ): (&T, )) -> bool {
        &x.abs() <= self.precision()
    }
}

impl<T: Arithmetic> ZeroOpr<T> for ZeroFlt<T> {
    fn precision(&self) -> &T {
        &self.precision
    }
}

pub trait ZeroOprBuilder<T> {
    fn new() -> Box<dyn ZeroOpr<T>>;
    fn new_with(precision: T) -> Box<dyn ZeroOpr<T>>;
}

pub struct Zero<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: Arithmetic> ZeroOprBuilder<T> for Zero<T> {
    default fn new() -> Box<dyn ZeroOpr<T>> {
        Box::new(ZeroInt::new())
    }

    default fn new_with(precision: T) -> Box<dyn ZeroOpr<T>> where ZeroFlt<T>: From<T> {
        Box::new(ZeroFlt::new_with(precision))
    }
}

impl<T: Arithmetic + FloatingNumber> ZeroOprBuilder<T> for Zero<T> {
    fn new() -> Box<dyn ZeroOpr<T>> where T: Precision {
        Box::new(ZeroFlt::new())
    }

    fn new_with(precision: T) -> Box<dyn ZeroOpr<T>> where ZeroFlt<T>: From<T> {
        Box::new(ZeroFlt::new_with(precision))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_int() {
        let zero = Zero::<i64>::new();
        assert_eq!(zero(&0), true);
        assert_eq!(zero(&1), false);
    }

    #[test]
    fn test_zero_flt() {
        let zero = Zero::<f64>::new();
        assert_eq!(zero(&0.0), true);
        assert_eq!(zero(&1e-6), false);

        let zero = Zero::<f64>::new_with(1e-5);
        assert_eq!(zero(&0.0), true);
        assert_eq!(zero(&1e-6), true);
    }
}
