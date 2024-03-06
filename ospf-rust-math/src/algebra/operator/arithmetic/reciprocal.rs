use std::ops::Div;

use crate::algebra::concept::{ Arithmetic, RealNumber };

pub trait Reciprocal {
    type Output;

    fn reciprocal(self) -> Self::Output;
}

impl <T: Reciprocal + Clone> Reciprocal for &T {
    type Output = <T as Reciprocal>::Output;

    fn reciprocal(self) -> Self::Output {
        self.clone().reciprocal()
    }
}

fn reciprocal<T: Reciprocal>(value: T) -> T::Output {
    value.reciprocal()
}

macro_rules! int_reciprocal_template {
    ($($type:ident)*) => ($(
        impl Reciprocal for $type {
            type Output = Option<$type>;

            fn reciprocal(self) -> Self::Output {
                if (self == 0) {
                    <$type as RealNumber>::NAN
                } else if (self == 1) {
                    Some(1)
                } else if (self == -1) {
                    Some(-1)
                } else {
                    Some(0)
                }
            }
        }
    )*)
}
int_reciprocal_template! { i8 i16 i32 i64 i128 }

macro_rules! uint_reciprocal_template {
    ($($type:ident)*) => ($(
        impl Reciprocal for $type {
            type Output = Option<$type>;

            fn reciprocal(self) -> Self::Output {
                if (self == 0) {
                    <$type as RealNumber>::NAN
                } else if (self == 1) {
                    Some(1)
                } else {
                    Some(0)
                }
            }
        }
    )*)
}
uint_reciprocal_template! { u8 u16 u32 u64 u128 }

macro_rules! flt_reciprocal_template {
    ($($type:ident)*) => ($(
        impl Reciprocal for $type {
            type Output = Option<$type>;

            fn reciprocal(self) -> Self::Output {
                if (self == 0.) {
                    return <$type as RealNumber>::NAN;
                } else {
                    return Some(1. / self);
                }
            }
        }
    )*)
}
flt_reciprocal_template! { f32 f64 }

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use crate::algebra::concept::{ Integer, IntegerNumber, UIntegerNumber, FloatingNumber };
    use super::*;

    fn test_integer<T: Integer + Reciprocal<Output=Option<T>> + Debug>() {
        assert_eq!(T::ZERO.reciprocal(), T::NAN);
        assert_eq!((&T::ZERO).reciprocal(), T::NAN);
        assert_eq!(reciprocal(T::ZERO), T::NAN);
        assert_eq!(reciprocal(&T::ZERO), T::NAN);

        assert_eq!(T::ONE.reciprocal(), Some(T::ONE));
        assert_eq!((&T::ONE).reciprocal(), Some(T::ONE));
        assert_eq!(reciprocal(T::ONE), Some(T::ONE));
        assert_eq!(reciprocal(&T::ONE), Some(T::ONE));

        assert_eq!(T::TWO.reciprocal(), Some(T::ZERO));
        assert_eq!((&T::TWO).reciprocal(), Some(T::ZERO));
        assert_eq!(reciprocal(T::TWO), Some(T::ZERO));
        assert_eq!(reciprocal(&T::TWO), Some(T::ZERO));
    }

    fn test_int<T: IntegerNumber + Reciprocal<Output=Option<T>> + Debug>() {
        test_integer::<T>();

        assert_eq!((-T::ONE).reciprocal(), Some(-T::ONE));
        assert_eq!((&-T::ONE).reciprocal(), Some(-T::ONE));
        assert_eq!(reciprocal(-T::ONE), Some(-T::ONE));
        assert_eq!(reciprocal(&-T::ONE), Some(-T::ONE));
    }

    fn test_uint<T: UIntegerNumber + Reciprocal<Output=Option<T>> + Debug>() {
        test_integer::<T>();
    }

    fn test_flt<T: FloatingNumber + Div<Output=T> + Reciprocal<Output=Option<T>> + Debug>() {
        assert!(T::ZERO.reciprocal().unwrap().is_nan());
        assert!((&T::ZERO).reciprocal().unwrap().is_nan());
        assert!(reciprocal(T::ZERO).unwrap().is_nan());
        assert!(reciprocal(&T::ZERO).unwrap().is_nan());

        assert_eq!(T::ONE.reciprocal(), Some(T::ONE));
        assert_eq!((&T::ONE).reciprocal(), Some(T::ONE));
        assert_eq!(reciprocal(T::ONE), Some(T::ONE));
        assert_eq!(reciprocal(&T::ONE), Some(T::ONE));

        assert_eq!(T::TWO.reciprocal(), Some(T::ONE.clone() / T::TWO.clone()));
        assert_eq!((&T::TWO).reciprocal(), Some(T::ONE.clone() / T::TWO.clone()));
        assert_eq!(reciprocal(T::TWO), Some(T::ONE.clone() / T::TWO.clone()));
        assert_eq!(reciprocal(&T::TWO), Some(T::ONE.clone() / T::TWO.clone()));
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
