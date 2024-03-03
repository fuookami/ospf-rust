use std::cmp::Ordering;
use num::FromPrimitive;

use crate::algebra::*;
use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Rem};

pub trait Scalar: Arithmetic + PlusSemiGroup + TimesSemiGroup + Bounded + Cross + Abs {}

pub trait RealNumber: Scalar + Precision + Invariant {
    const TWO: Self;
    const THREE: Self;
    const TEN: Self;

    const NAN: Option<Self> = None;
    const INF: Option<Self> = None;
    const NEG_INF: Option<Self> = None;

    fn is_nan(&self) -> bool {
        Self::NAN.is_some_and(|nan_value| *self == nan_value)
    }

    fn is_inf(&self) -> bool {
        Self::INF.is_some_and(|inf_value| *self == inf_value)
    }

    fn is_neg_inf(&self) -> bool {
        Self::NEG_INF.is_some_and(|inf_value| *self == inf_value)
    }
}

pub trait Integer: RealNumber + RangeTo + Log<f64> + PowF<f64> + Exp + Ord + Eq {}
pub trait IntegerNumber: Integer + Signed + NumberField + Pow {}
pub trait UIntegerNumber: Integer + Unsigned + NumberField + Pow {}

pub trait RationalNumber<I: Integer + NumberField>:
    RealNumber + NumberField + Log<f64> + PowF<f64> + Exp + Pow + Ord + Eq
{
    fn num(&self) -> &I;
    fn den(&self) -> &I;
}

pub trait FloatingNumber: RealNumber + Signed + NumberField + Log + PowF + Exp + Pow {
    const PI: Self;
    const E: Self;

    fn floor(&self) -> Self;
    fn ceil(&self) -> Self;
    fn round(&self) -> Self;
    fn trunc(&self) -> Self;
    fn fract(&self) -> Self;
}

pub trait NumericIntegerNumber<I: IntegerNumber>:
    Integer
    + Signed
    + PlusGroup
    + TimesSemiGroup
    + Reciprocal
    + Div
    + IntDiv<Output = Self>
    + Rem<Output = Self>
    + Pow
    + Ord
    + Eq
{
}

pub trait NumericUIntegerNumber<I: UIntegerNumber>:
    Integer
    + Unsigned
    + PlusSemiGroup
    + TimesSemiGroup
    + Neg
    + Mul
    + Reciprocal
    + Div
    + IntDiv<Output = Self>
    + Rem<Output = Self>
    + Pow
    + Ord
    + Eq
{
}

macro_rules! int_real_number_template {
    ($($type:ty)*) => ($(
        impl Arithmetic for $type {
            const ZERO: Self = 0;
            const ONE: Self = 1;
        }

        impl Scalar for $type {}

        impl RealNumber for $type {
            const TWO: Self = 2;
            const THREE: Self = 3;
            const TEN: Self = 10;
        }

        impl Integer for $type {}
        impl IntegerNumber for $type {}
    )*)
}
int_real_number_template! { i8 i16 i32 i64 i128 }

impl Arithmetic for ix {
    const ZERO: Self = ix::from(0);
    const ONE: Self = ix::from(1);
}

impl Scalar for ix {}

impl PartialEq for ix {
    fn eq(&self, other: &Self) -> bool {
        todo!()
    }
}

impl PartialOrd for ix {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        todo!()
    }
}

impl Add<Output=Self> for ix {
    type Output = ();

    fn add(self, rhs: Self) -> Self::Output {
        todo!()
    }
}

impl AddAssign for ix {
    fn add_assign(&mut self, rhs: Self) {
        todo!()
    }
}

impl Mul<Output=Self> for ix {
    type Output = ();

    fn mul(self, rhs: Self) -> Self::Output {
        todo!()
    }
}

impl MulAssign for ix {
    fn mul_assign(&mut self, rhs: Self) {
        todo!()
    }
}

impl Cross for ix {
    type Output = ();

    fn cross(self, rhs: Self) -> Self::Output {
        todo!()
    }
}

impl RealNumber for ix {
    const TWO: Self = ix::from(2);
    const THREE: Self = ix::from(3);
    const TEN: Self = ix::from(10);
}

impl Integer for ix {}
impl IntegerNumber for ix {}

macro_rules! uint_real_number_template {
    ($($type:ty)*) => ($(
        impl Arithmetic for $type {
            const ZERO: Self = 0;
            const ONE: Self = 1;
        }

        impl Scalar for $type {}

        impl RealNumber for $type {
            const TWO: Self = 2;
            const THREE: Self = 3;
            const TEN: Self = 10;
        }

        impl Integer for $type {}
        impl UIntegerNumber for $type {}
    )*)
}
uint_real_number_template! { u8 u16 u32 u64 u128 }

impl Arithmetic for uix {
    const ZERO: Self = uix::from(0);
    const ONE: Self = uix::from(1);
}

impl Scalar for uix {}

impl RealNumber for uix {
    const TWO: Self = ix::from(2);
    const THREE: Self = ix::from(3);
    const TEN: Self = ix::from(10);
}

impl Integer for uix {}
impl IntegerNumber for uix {}

macro_rules! floating_real_number_template {
    ($($type:ty)*) => ($(
        impl Arithmetic for $type {
            const ZERO: Self = 0.;
            const ONE: Self = 1.;
        }

        impl Scalar for $type {}

        impl RealNumber for $type {
            const TWO: Self = 2.;
            const THREE: Self = 3.;
            const TEN: Self = 10.;

            const NAN: Option<Self> = Some(<$type>::NAN);
            const INF: Option<Self> = Some(<$type>::INFINITY);
            const NEG_INF: Option<Self> = Some(<$type>::NEG_INFINITY);
        }
    )*)
}
floating_real_number_template! { f32 f64 }

impl FloatingNumber for f32 {
    const PI: Self = std::f32::consts::PI;
    const E: Self = std::f32::consts::E;

    fn floor(&self) -> Self {
        (*self).floor()
    }

    fn ceil(&self) -> Self {
        (*self).ceil()
    }

    fn round(&self) -> Self {
        (*self).round()
    }

    fn trunc(&self) -> Self {
        (*self).trunc()
    }

    fn fract(&self) -> Self {
        (*self).fract()
    }
}

impl FloatingNumber for f64 {
    const PI: Self = std::f64::consts::PI;
    const E: Self = std::f64::consts::E;

    fn floor(&self) -> Self {
        (*self).floor()
    }

    fn ceil(&self) -> Self {
        (*self).ceil()
    }

    fn round(&self) -> Self {
        (*self).round()
    }

    fn trunc(&self) -> Self {
        (*self).trunc()
    }

    fn fract(&self) -> Self {
        (*self).fract()
    }
}

impl Arithmetic for dec {
    const ZERO: Self = dec::ZERO;
    const ONE: Self = dec::ONE;
}

impl Scalar for dec {}

impl RealNumber for dec {
    const TWO: Self = dec::TWO;
    const THREE: Self = dec::from_u128(3).unwrap();
    const TEN: Self = dec::TEN;

    const NAN: Option<Self> = None;
    const INF: Option<Self> = None;
    const NEG_INF: Option<Self> = None;
}

impl FloatingNumber for dec {
    const PI: Self = dec::PI;
    const E: Self = dec::E;

    fn floor(&self) -> Self {
        self.floor()
    }

    fn ceil(&self) -> Self {
        self.ceil()
    }

    fn round(&self) -> Self {
        self.round()
    }

    fn trunc(&self) -> Self {
        self.trunc()
    }

    fn fract(&self) -> Self {
        self.fract()
    }
}
