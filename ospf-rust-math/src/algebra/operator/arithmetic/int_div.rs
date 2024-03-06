use std::ops::Div;

use crate::algebra::concept::FloatingNumber;

pub trait IntDiv<Rhs = Self> {
    type Output;

    fn int_div(self, rhs: Rhs) -> Self::Output;
}

impl<T: IntDiv<U> + Clone, U> IntDiv<U> for &T {
    type Output = <T as IntDiv<U>>::Output;

    default fn int_div(self, rhs: U) -> Self::Output {
        self.clone().int_div(rhs)
    }
}

pub fn int_div<T: IntDiv>(lhs: T, rhs: T) -> T::Output {
    lhs.int_div(rhs)
}

macro_rules! int_int_div_template {
    ($($type:ident)*) => ($(
        impl <U> IntDiv<U> for $type where $type: Div<U> {
            type Output = <$type as Div<U>>::Output;

            fn int_div(self, rhs: U) -> Self::Output {
                self / rhs
            }
        }
    )*)
}
int_int_div_template! { u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 }

macro_rules! floating_int_div_template {
    ($($type:ident)*) => ($(
        impl <U, V> IntDiv<U> for $type where $type: Div<U, Output=V>, V: FloatingNumber {
            type Output = <$type as Div<U>>::Output;

            fn int_div(self, rhs: U) -> Self::Output {
                (self / rhs).floor()
            }
        }
    )*)
}
floating_int_div_template! { f32 f64 }

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use crate::algebra::concept::{ Integer, FloatingNumber };
    use super::*;

    fn test_int<T: Integer + IntDiv<Output=T> + for<'a> IntDiv<&'a T, Output=T> + Debug>() {
        assert_eq!(T::ONE.clone().int_div(T::TWO.clone()), T::ZERO);
        assert_eq!((&T::ONE).int_div(T::TWO.clone()), T::ZERO);
        assert_eq!(T::ONE.clone().int_div(&T::TWO), T::ZERO);
        assert_eq!((&T::ONE).int_div(&T::TWO), T::ZERO);

        assert_eq!(T::TWO.clone().int_div(T::ONE.clone()), T::TWO);
        assert_eq!((&T::TWO).int_div(T::ONE.clone()), T::TWO);
        assert_eq!(T::TWO.clone().int_div(&T::ONE), T::TWO);
        assert_eq!((&T::TWO).int_div(&T::ONE), T::TWO);
    }

    fn test_flt<T: FloatingNumber + IntDiv<Output=T> + for<'a> IntDiv<&'a T, Output=T> + Debug>() {
        assert_eq!(T::ONE.clone().int_div(T::TWO.clone()), T::ZERO);
        assert_eq!((&T::ONE).int_div(T::TWO.clone()), T::ZERO);
        assert_eq!(T::ONE.clone().int_div(&T::TWO), T::ZERO);
        assert_eq!((&T::ONE).int_div(&T::TWO), T::ZERO);

        assert_eq!(T::TWO.clone().int_div(T::ONE.clone()), T::TWO);
        assert_eq!((&T::TWO).int_div(T::ONE.clone()), T::TWO);
        assert_eq!(T::TWO.clone().int_div(&T::ONE), T::TWO);
        assert_eq!((&T::TWO).int_div(&T::ONE), T::TWO);
    }

    #[test]
    fn test() {
        test_int::<u8>();
        test_int::<u16>();
        test_int::<u32>();
        test_int::<u64>();
        test_int::<u128>();
        test_int::<i8>();
        test_int::<i16>();
        test_int::<i32>();
        test_int::<i64>();
        test_int::<i128>();
        test_flt::<f32>();
        test_flt::<f64>();
    }
}
