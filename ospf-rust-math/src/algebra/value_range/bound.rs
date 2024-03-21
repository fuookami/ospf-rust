use crate::algebra::concept::Arithmetic;

use super::interval::{Interval, IntervalType, Closed};
use super::value_wrapper::ValueWrapper;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BoundSide {
    Lower,
    Upper,
}

pub struct BoundStc<T: Arithmetic, I: IntervalType = Closed> {
    value: ValueWrapper<T>,
    side: BoundSide,
    _marker: std::marker::PhantomData<I>,
}

impl<T: Arithmetic, I: IntervalType> Clone for BoundStc<T, I>
    where
        ValueWrapper<T>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            side: self.side,
            _marker: std::marker::PhantomData::<I> {},
        }
    }
}

impl<T: Arithmetic, I: IntervalType> Copy for BoundStc<T, I> where ValueWrapper<T>: Copy {}

impl<T: Arithmetic, I: IntervalType> std::fmt::Display for BoundStc<T, I> where ValueWrapper<T>: std::fmt::Display {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.side {
            BoundSide::Lower => write!(f, "{}{}", I::LB_SIGN, self.value),
            BoundSide::Upper => write!(f, "{}{}", self.value, I::UB_SIGN)
        }
    }
}

impl<T: Arithmetic, I: IntervalType> std::fmt::Debug for BoundStc<T, I> where ValueWrapper<T>: std::fmt::Display {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.side {
            BoundSide::Lower => write!(f, "{}{}", I::LB_SIGN, self.value),
            BoundSide::Upper => write!(f, "{}{}", self.value, I::UB_SIGN)
        }
    }
}

pub struct Bound<T: Arithmetic> {
    pub value: ValueWrapper<T>,
    pub interval: Interval,
    pub side: BoundSide,
}

impl<T: Arithmetic> Clone for Bound<T> where ValueWrapper<T>: Clone {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            interval: self.interval,
            side: self.side,
        }
    }
}

impl<T: Arithmetic> Copy for Bound<T> where ValueWrapper<T>: Copy {}

impl<T: Arithmetic> std::fmt::Display for Bound<T> where ValueWrapper<T>: std::fmt::Display {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.side {
            BoundSide::Lower => write!(f, "{}{}", self.interval.lb_sign(), self.value),
            BoundSide::Upper => write!(f, "{}{}", self.value, self.interval.ub_sign())
        }
    }
}

impl<T: Arithmetic> std::fmt::Debug for Bound<T> where ValueWrapper<T>: std::fmt::Display {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.side {
            BoundSide::Lower => write!(f, "{}{}", self.interval.lb_sign(), self.value),
            BoundSide::Upper => write!(f, "{}{}", self.value, self.interval.ub_sign())
        }
    }
}
