use std::ops::*;
use num::complex::ComplexFloat;

use crate::algebra::*;

pub trait Scalar: Arithmetic + Bounded {}

pub enum RealNumberCategory {
    Nan,
    Infinite,
    NegativeInfinite,
    Zero,
    Subnormal,
    Normal
}

pub trait RealNumber: Scalar + Precision + Invariant {
    const TWO: Self;
    const THREE: Self;
    const FIVE: Self;
    const TEN: Self;

    const NAN: Option<Self> = None;
    const INF: Option<Self> = None;
    const NEG_INF: Option<Self> = None;

    // todo: category

    fn is_nan(&self) -> bool {
        Self::NAN.is_some_and(|nan_value| *self == nan_value)
    }

    fn is_inf(&self) -> bool {
        Self::INF.is_some_and(|inf_value| *self == inf_value)
    }

    fn is_neg_inf(&self) -> bool {
        Self::NEG_INF.is_some_and(|inf_value| *self == inf_value)
    }

    fn is_finite(&self) -> bool {
        return !self.is_inf() && !self.is_inf() && !self.is_neg_inf()
    }
}

pub trait Integer: RealNumber + Ord + Eq {}

pub trait IntegerNumber: Integer + Signed {}

pub trait UIntegerNumber: Integer + Unsigned {}

pub trait RationalNumber<I: Integer>: RealNumber + Ord + Eq
{
    fn num(&self) -> &I;
    fn den(&self) -> &I;
}

pub trait FloatingNumber: RealNumber + Signed {
    const PI: Self;
    const E: Self;

    fn floor(&self) -> Self;
    fn ceil(&self) -> Self;
    fn round(&self) -> Self;
    fn trunc(&self) -> Self;
    fn fract(&self) -> Self;
}

pub trait NumericIntegerNumber<I: IntegerNumber>: Integer + Signed + Ord + Eq {}

pub trait NumericUIntegerNumber<I: UIntegerNumber>: Integer + Unsigned + Ord + Eq {}

macro_rules! int_real_number_template {
    ($($type:ident)*) => ($(
        impl Scalar for $type {}

        impl RealNumber for $type {
            const TWO: Self = 2;
            const THREE: Self = 3;
            const FIVE: Self = 3;
            const TEN: Self = 10;
        }

        impl Integer for $type {}
        impl IntegerNumber for $type {}
    )*)
}
int_real_number_template! { i8 i16 i32 i64 i128 }

macro_rules! uint_real_number_template {
    ($($type:ident)*) => ($(
        impl Scalar for $type {}

        impl RealNumber for $type {
            const TWO: Self = 2;
            const THREE: Self = 3;
            const FIVE: Self = 5;
            const TEN: Self = 10;
        }

        impl Integer for $type {}
        impl UIntegerNumber for $type {}
    )*)
}
uint_real_number_template! { u8 u16 u32 u64 u128 }

macro_rules! floating_real_number_template {
    ($($type:ident)*) => ($(
        impl Scalar for $type {}

        impl RealNumber for $type {
            const TWO: Self = 2.;
            const THREE: Self = 3.;
            const FIVE: Self = 5.;
            const TEN: Self = 10.;

            const NAN: Option<Self> = Some(<$type>::NAN);
            const INF: Option<Self> = Some(<$type>::INFINITY);
            const NEG_INF: Option<Self> = Some(<$type>::NEG_INFINITY);

            fn is_nan(&self) -> bool {
                <$type>::is_nan(*self)
            }

            fn is_inf(&self) -> bool {
                <$type>::is_infinite(*self) && <$type>::is_sign_positive(*self)
            }

            fn is_neg_inf(&self) -> bool {
                <$type>::is_infinite(*self) && <$type>::is_sign_negative(*self)
            }
        }

        impl FloatingNumber for $type {
                const PI: Self = std::$type::consts::PI;
                const E: Self = std::$type::consts::E;

                fn floor(&self) -> Self {
                    <$type>::floor(*self)
                }

                fn ceil(&self) -> Self {
                    <$type>::ceil(*self)
                }

                fn round(&self) -> Self {
                    <$type>::round(*self)
                }

                fn trunc(&self) -> Self {
                    <$type>::trunc(*self)
                }

                fn fract(&self) -> Self {
                    <$type>::fract(*self)
                }
            }
    )*)
}
floating_real_number_template! { f32 f64 }
