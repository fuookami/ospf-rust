use std::ops::Neg;

use crate::algebra::concept::{Arithmetic, Signed};

pub trait Abs {
    type Output;

    fn abs(self) -> Self::Output;
}

impl<T: Abs + Clone> Abs for &T {
    type Output = <T as Abs>::Output;

    fn abs(self) -> Self::Output {
        self.clone().abs()
    }
}

pub fn abs<T: Abs>(value: T) -> T::Output {
    value.abs()
}

macro_rules! int_abs_template {
    ($($type:ident)*) => ($(
        impl Abs for $type {
            type Output = $type;

            fn abs(self) -> Self::Output {
                if self < 0 { -self } else { self }
            }
        }
    )*)
}
int_abs_template! { i8 i16 i32 i64 i128 }

macro_rules! uint_abs_template {
    ($($type:ident)*) => ($(
        impl Abs for $type {
            type Output = $type;

            fn abs(self) -> Self::Output {
                self
            }
        }
    )*)
}
uint_abs_template! { u8 u16 u32 u64 u128 }

macro_rules! flt_abs_template {
    ($($type:ident)*) => ($(
        impl Abs for $type {
            type Output = $type;

            fn abs(self) -> Self::Output {
                if self < 0. { -self } else { self }
            }
        }
    )*)
}
flt_abs_template! { f32 f64 }

#[cfg(test)]
mod tests {
    use std::ops::Add;
    use std::fmt::Debug;
    use bigdecimal::num_traits::Unsigned;

    use crate::algebra::concept::{Bounded, IntegerNumber, UIntegerNumber, FloatingNumber};
    use super::*;

    fn test_bounded<T: Arithmetic + Bounded + Abs<Output=T> + Debug>() {
        assert_eq!(T::ZERO.abs(), T::ZERO);
        assert_eq!((&T::ZERO).abs(), T::ZERO);
        assert_eq!(abs(T::ZERO), T::ZERO);
        assert_eq!(abs(&T::ZERO), T::ZERO);

        assert_eq!(T::POSITIVE_MINIMUM.abs(), T::POSITIVE_MINIMUM);
        assert_eq!((&T::POSITIVE_MINIMUM).abs(), T::POSITIVE_MINIMUM);
        assert_eq!(abs(T::POSITIVE_MINIMUM), T::POSITIVE_MINIMUM);
        assert_eq!(abs(&T::POSITIVE_MINIMUM), T::POSITIVE_MINIMUM);

        assert_eq!(T::MAXIMUM.map(|x| x.abs()), T::MAXIMUM);
        assert_eq!(T::MAXIMUM.map(|x| (&x).abs()), T::MAXIMUM);
        assert_eq!(T::MAXIMUM.map(|x| abs(x)), T::MAXIMUM);
        assert_eq!(T::MAXIMUM.map(|x| abs(&x)), T::MAXIMUM);
    }

    fn test_signed<T: Arithmetic + Bounded + Signed + Abs<Output=T> + Debug>() {
        test_bounded::<T>();

        assert_eq!((-T::POSITIVE_MINIMUM).abs(), T::POSITIVE_MINIMUM);
        assert_eq!((&-T::POSITIVE_MINIMUM).abs(), T::POSITIVE_MINIMUM);
        assert_eq!(abs(-T::POSITIVE_MINIMUM), T::POSITIVE_MINIMUM);
        assert_eq!(abs(&-T::POSITIVE_MINIMUM), T::POSITIVE_MINIMUM);
    }

    fn test_int<T: IntegerNumber + Add<Output=T> + Abs<Output=T> + Debug>() {
        test_signed::<T>();

        assert_eq!(T::MINIMUM.map(|x| (x + T::ONE.clone()).abs()), T::MAXIMUM);
        assert_eq!(T::MINIMUM.map(|x| (&(x + T::ONE.clone())).abs()), T::MAXIMUM);
        assert_eq!(T::MINIMUM.map(|x| abs(x + T::ONE.clone())), T::MAXIMUM);
        assert_eq!(T::MINIMUM.map(|x| abs(&(x + T::ONE.clone()))), T::MAXIMUM);
    }

    fn test_uint<T: UIntegerNumber + Add<Output=T> + Abs<Output=T> + Debug>() {
        test_bounded::<T>();
    }

    fn test_flt<T: FloatingNumber + Abs<Output=T> + Debug>() {
        test_signed::<T>();

        assert_eq!(T::MINIMUM.map(|x| x.abs()), T::MAXIMUM);
        assert_eq!(T::MINIMUM.map(|x| (&x).abs()), T::MAXIMUM);
        assert_eq!(T::MINIMUM.map(|x| abs(x)), T::MAXIMUM);
        assert_eq!(T::MINIMUM.map(|x| abs(&x)), T::MAXIMUM);
    }

    #[test]
    fn test() {
        test_int::<i8>();
        test_int::<i16>();
        test_int::<i32>();
        test_int::<i64>();
        test_int::<i128>();
        test_uint::<u8>();
        test_uint::<u16>();
        test_uint::<u32>();
        test_uint::<u64>();
        test_uint::<u128>();
        test_flt::<f32>();
        test_flt::<f64>();
    }
}
