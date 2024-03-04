use std::ops::*;

use bigdecimal::*;

use crate::algebra::concept::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct dec {
    value: BigDecimal,
}

impl<T> From<T> for dec where BigDecimal: From<T> {
    fn from(value: T) -> Self {
        dec { value: BigDecimal::from(value) }
    }
}

impl From<f32> for dec {
    fn from(value: f32) -> Self {
        dec { value: BigDecimal::from_f32(value).unwrap() }
    }
}

impl From<f64> for dec {
    fn from(value: f64) -> Self {
        dec { value: BigDecimal::from_f64(value).unwrap() }
    }
}

impl Add for dec {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        dec { value: self.value + other.value }
    }
}

impl Sub for dec {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        dec { value: self.value - other.value }
    }
}

impl Mul for dec {
    type Output = Self;

    fn mul(self, other: Self) -> Self::Output {
        dec { value: self.value * other.value }
    }
}

impl Div for dec {
    type Output = Self;

    fn div(self, other: Self) -> Self::Output {
        dec { value: self.value / other.value }
    }
}

impl Bounded for dec {
    const MINIMUM: Option<Self> = None;
    const MAXIMUM: Option<Self> = None;
    const POSITIVE_MINIMUM: Self = dec::from(1e-28);
}

impl Arithmetic for dec {
    const ZERO: Self = dec::ZERO;
    const ONE: Self = dec::ONE;
}

impl Precision for dec {
    const EPSILON: Self = dec::from(1e-28);
    const DECIMAL_DIGITS: Option<usize> = Some(28);
    const DECIMAL_PRECISION: Self = dec::from(1e-28);
}

impl Scalar for dec {}

impl RealNumber for dec {
    const TWO: Self = dec::from(2);
    const THREE: Self = dec::from(3);
    const TEN: Self = dec::from(10);

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
