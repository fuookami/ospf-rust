use std::ops::*;
use bigdecimal::num_bigint::BigInt;

use num::BigUint;

use crate::algebra::concept::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct uix {
    value: BigUint,
}

impl<T> From<T> for uix where BigUint: From<T> {
    fn from(value: T) -> Self {
        uix { value: BigUint::from(value) }
    }
}

impl Add for uix {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        uix { value: self.value + other.value }
    }
}

impl Sub for uix {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        uix { value: self.value - other.value }
    }
}

impl Mul for uix {
    type Output = Self;

    fn mul(self, other: Self) -> Self::Output {
        uix { value: self.value * other.value }
    }
}

impl Div for uix {
    type Output = Self;

    fn div(self, other: Self) -> Self::Output {
        uix { value: self.value / other.value }
    }
}

impl Bounded for uix {
    const MINIMUM: Option<Self> = Some(Self::ZERO);
    const MAXIMUM: Option<Self> = None;
    const POSITIVE_MINIMUM: Self = Self::ONE;
}

impl Arithmetic for uix {
    const ZERO: Self = uix::from(0);
    const ONE: Self = uix::from(1);
}

impl Precision for uix {
    const EPSILON: Self = Self::ZERO;
    const DECIMAL_DIGITS: Option<usize> = None;
    const DECIMAL_PRECISION: Self = Self::EPSILON;
}

impl Scalar for uix {}

impl RealNumber for uix {
    const TWO: Self = uix::from(2);
    const THREE: Self = uix::from(3);
    const TEN: Self = uix::from(10);
}

impl Integer for uix {}

impl IntegerNumber for uix {}
