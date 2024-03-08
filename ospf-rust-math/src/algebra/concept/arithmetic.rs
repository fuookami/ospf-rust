use std::fmt::{Display, Debug};

pub trait Arithmetic: 'static + Sized + Clone + PartialEq + PartialOrd {
    const ZERO: &'static Self;
    const ONE: &'static Self;
}

impl Arithmetic for bool {
    const ZERO: &'static Self = &false;
    const ONE: &'static Self = &true;
}

macro_rules! int_arithmetic_template {
    ($($type:ident)*) => ($(
        impl Arithmetic for $type {
            const ZERO: &'static Self = &0;
            const ONE: &'static Self = &1;
        }
    )*)
}
int_arithmetic_template! { i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize }

macro_rules! floating_arithmetic_template {
    ($($type:ident)*) => ($(
        impl Arithmetic for $type {
            const ZERO: &'static Self = &0.;
            const ONE: &'static Self = &1.;
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
