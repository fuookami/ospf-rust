use std::fmt::{ Display, Debug };

pub trait Arithmetic: Sized + Clone + PartialEq + PartialOrd {
    const ZERO: Self;
    const ONE: Self;
}

macro_rules! int_arithmetic_template {
    ($($type:ty)*) => ($(
        impl Arithmetic for $type {
            const ZERO: Self = 0;
            const ONE: Self = 1;
        }
    )*)
}
int_arithmetic_template! { i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 }

macro_rules! floating_arithmetic_template {
    ($($type:ty)*) => ($(
        impl Arithmetic for $type {
            const ZERO: Self = 0.;
            const ONE: Self = 1.;
        }
    )*)
}
floating_arithmetic_template! { f32 f64 }

pub struct Infinity {}

pub const INF: Infinity = Infinity {};

impl Display for Infinity {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "inf")
    }
}

impl Debug for Infinity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "inf")
    }
}

pub struct NegativeInfinity {}

pub const NEG_INF: NegativeInfinity = NegativeInfinity {};

impl Display for NegativeInfinity {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "-inf")
    }
}

impl Debug for NegativeInfinity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "-inf")
    }
}

pub struct NaN {}

pub const NAN: NaN = NaN {};

impl Display for NaN {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "nan")
    }
}

impl Debug for NaN {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "nan")
    }
}
