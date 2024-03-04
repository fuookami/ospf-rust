use std::ops::*;

use num::*;

use crate::algebra::concept::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ix {
    value: BigInt,
}

impl<T> From<T> for ix where BigInt: From<T> {
    fn from(value: T) -> Self {
        ix { value: BigInt::from(value) }
    }
}

impl Add for ix {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        ix { value: self.value + other.value }
    }
}

impl Sub for ix {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        ix { value: self.value - other.value }
    }
}

impl Mul for ix {
    type Output = Self;

    fn mul(self, other: Self) -> Self::Output {
        ix { value: self.value * other.value }
    }
}

impl Div for ix {
    type Output = Self;

    fn div(self, other: Self) -> Self::Output {
        ix { value: self.value / other.value }
    }
}

impl Bounded for ix {
    const MINIMUM: Option<Self> = None;
    const MAXIMUM: Option<Self> = None;
    const POSITIVE_MINIMUM: Self = Self::ONE;
}

impl Precision for ix {
    const EPSILON: Self = Self::ZERO;
    const DECIMAL_DIGITS: Option<usize> = None;
    const DECIMAL_PRECISION: Self = Self::EPSILON;
}

impl Arithmetic for ix {
    const ZERO: Self = ix::from(0);
    const ONE: Self = ix::from(1);
}

impl Scalar for ix {}

impl RealNumber for ix {
    const TWO: Self = ix::from(2);
    const THREE: Self = ix::from(3);
    const TEN: Self = ix::from(10);
}

impl Integer for ix {}

impl IntegerNumber for ix {}
