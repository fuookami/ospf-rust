use crate::algebra::concept::Arithmetic;
use crate::algebra::operator::comparison::*;
use crate::functional::predicate::Comparator;

pub trait IntervalType: Clone + Copy + PartialEq + Eq {
    const LB_SIGN: &'static str;
    const UB_SIGN: &'static str;

    fn lb_op<T: Arithmetic>() -> Box<Comparator<T>>;
    fn lb_op_with<T: Arithmetic>(precision: T) -> Box<Comparator<T>>;
    fn ub_op<T: Arithmetic>() -> Box<Comparator<T>>;
    fn ub_op_with<T: Arithmetic>(precision: T) -> Box<Comparator<T>>;
}

pub trait Union<Rhs: IntervalType>: IntervalType {
    type Result: IntervalType;

    const LB_SIGN: &'static str = Self::Result::LB_SIGN;
    const UB_SIGN: &'static str = Self::Result::UB_SIGN;

    fn lb_op<T: Arithmetic>() -> Box<Comparator<T>> {
        Self::Result::lb_op::<T>()
    }

    fn lb_op_with<T: Arithmetic>(precision: T) -> Box<Comparator<T>> {
        Self::Result::lb_op_with::<T>(precision)
    }

    fn ub_op<T: Arithmetic>() -> Box<Comparator<T>> {
        Self::Result::ub_op::<T>()
    }

    fn ub_op_with<T: Arithmetic>(precision: T) -> Box<Comparator<T>> {
        Self::Result::ub_op_with::<T>(precision)
    }
}

pub trait Intersect<Rhs: IntervalType>: IntervalType {
    type Result: IntervalType;

    const LB_SIGN: &'static str = Self::Result::LB_SIGN;
    const UB_SIGN: &'static str = Self::Result::UB_SIGN;

    fn lb_op<T: Arithmetic>() -> Box<Comparator<T>> {
        Self::Result::lb_op::<T>()
    }

    fn lb_op_with<T: Arithmetic>(precision: T) -> Box<Comparator<T>> {
        Self::Result::lb_op_with::<T>(precision)
    }

    fn ub_op<T: Arithmetic>() -> Box<Comparator<T>> {
        Self::Result::ub_op::<T>()
    }

    fn ub_op_with<T: Arithmetic>(precision: T) -> Box<Comparator<T>> {
        Self::Result::ub_op_with::<T>(precision)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Open {}

impl IntervalType for Open {
    const LB_SIGN: &'static str = "(";
    const UB_SIGN: &'static str = ")";

    fn lb_op<T: Arithmetic>() -> Box<Comparator<T>> {
        Less::<T>::new()
    }

    fn lb_op_with<T: Arithmetic>(precision: T) -> Box<Comparator<T>> {
        Less::<T>::new_with(precision)
    }

    fn ub_op<T: Arithmetic>() -> Box<Comparator<T>> {
        Greater::<T>::new()
    }

    fn ub_op_with<T: Arithmetic>(precision: T) -> Box<Comparator<T>> {
        Greater::<T>::new_with(precision)
    }
}

impl Union<Open> for Open {
    type Result = Open;
}

impl Union<Closed> for Open {
    type Result = Closed;
}

impl<T: IntervalType> Intersect<T> for Open {
    type Result = Open;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Closed {}

impl IntervalType for Closed {
    const LB_SIGN: &'static str = "[";
    const UB_SIGN: &'static str = "]";

    fn lb_op<T: Arithmetic>() -> Box<Comparator<T>> {
        LessEqual::<T>::new()
    }

    fn lb_op_with<T: Arithmetic>(precision: T) -> Box<Comparator<T>> {
        LessEqual::<T>::new_with(precision)
    }

    fn ub_op<T: Arithmetic>() -> Box<Comparator<T>> {
        GreaterEqual::<T>::new()
    }

    fn ub_op_with<T: Arithmetic>(precision: T) -> Box<Comparator<T>> {
        GreaterEqual::<T>::new_with(precision)
    }
}

impl<T: IntervalType> Union<T> for Closed {
    type Result = Closed;
}

impl Intersect<Open> for Closed {
    type Result = Open;
}

impl Intersect<Closed> for Closed {
    type Result = Closed;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Interval {
    Open,
    Closed,
}

impl Interval {
    pub fn lb_sign(&self) -> &'static str {
        match self {
            Self::Open => <Open as IntervalType>::LB_SIGN,
            Self::Closed => <Closed as IntervalType>::LB_SIGN,
        }
    }

    pub fn ub_sign(&self) -> &'static str {
        match self {
            Self::Open => <Open as IntervalType>::UB_SIGN,
            Self::Closed => <Closed as IntervalType>::UB_SIGN,
        }
    }

    pub fn union(self, rhs: Self) -> Self {
        if self == Self::Closed || rhs == Self::Closed {
            Self::Closed
        } else {
            Self::Open
        }
    }

    pub fn intersect(self, rhs: Self) -> Self {
        if self == Self::Open || rhs == Self::Open {
            Self::Open
        } else {
            Self::Closed
        }
    }

    pub fn lb_op<T: Arithmetic>(self) -> Box<Comparator<T>> {
        match self {
            Self::Open => <Open as IntervalType>::lb_op::<T>(),
            Self::Closed => <Closed as IntervalType>::lb_op::<T>(),
        }
    }

    pub fn lb_op_with<T: Arithmetic>(self, precision: T) -> Box<Comparator<T>> {
        match self {
            Self::Open => <Open as IntervalType>::lb_op_with::<T>(precision),
            Self::Closed => <Closed as IntervalType>::lb_op_with::<T>(precision),
        }
    }

    pub fn ub_op<T: Arithmetic>(self) -> Box<Comparator<T>> {
        match self {
            Self::Open => <Open as IntervalType>::ub_op::<T>(),
            Self::Closed => <Closed as IntervalType>::ub_op::<T>(),
        }
    }

    pub fn ub_op_with<T: Arithmetic>(self, precision: T) -> Box<Comparator<T>> {
        match self {
            Self::Open => <Open as IntervalType>::ub_op_with::<T>(precision),
            Self::Closed => <Closed as IntervalType>::ub_op_with::<T>(precision),
        }
    }
}
