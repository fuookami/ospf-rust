use std::ops::Div;
use crate::algebra::concept::FloatingNumber;
use crate::algebra::ordinary;
use crate::{RealNumber, Reciprocal};

pub trait Pow: Sized {
    type Output;

    fn pow(self, index: i64) -> Self::Output;

    fn square(self) -> Self::Output {
        self.pow(2)
    }

    fn cubic(self) -> Self::Output {
        self.pow(3)
    }
}

pub fn pow<Lhs: Pow>(lhs: Lhs, index: i64) -> Lhs::Output {
    lhs.pow(index)
}

pub fn square<Lhs: Pow>(lhs: Lhs) -> Lhs::Output {
    lhs.square()
}

pub fn cubic<Lhs: Pow>(lhs: Lhs) -> Lhs::Output {
    lhs.cubic()
}

pub trait PowF<Index: FloatingNumber = Self>: Sized where for<'a> &'a Index: Reciprocal<Output=Index> {
    type Output: FloatingNumber;

    fn powf(self, index: &Index) -> Option<Self::Output>;

    fn sqrt(self) -> Option<Self::Output> {
        self.powf(&Index::TWO.reciprocal().unwrap())
    }

    fn cbrt(self) -> Option<Self::Output> {
        self.powf(&Index::THREE.reciprocal().unwrap())
    }
}

pub fn powf<Lhs: PowF<Rhs>, Rhs: FloatingNumber>(lhs: Lhs, rhs: &Rhs) -> Option<Lhs::Output>
    where for<'a> &'a Rhs: Reciprocal<Output=Rhs> {
    lhs.powf(rhs)
}

pub fn sqrt<Lhs: PowF<Rhs>, Rhs: FloatingNumber>(lhs: Lhs) -> Option<Lhs::Output>
    where for<'a> &'a Rhs: Reciprocal<Output=Rhs> {
    lhs.sqrt()
}

pub fn cbrt<Lhs: PowF<Rhs>, Rhs: FloatingNumber>(lhs: Lhs) -> Option<Lhs::Output>
    where for<'a> &'a Rhs: Reciprocal<Output=Rhs> {
    lhs.cbrt()
}

pub trait Exp {
    type Output;

    fn exp(self) -> Self::Output;
}

pub fn exp<Lhs: Exp>(lhs: Lhs) -> Lhs::Output {
    lhs.exp()
}

macro_rules! int_pow_template {
    ($($type:ident)*) => ($(
        impl Pow for $type {
            type Output = Self;

            fn pow(self, index: i64) -> Self::Output {
                ordinary::pow_times_semi_group(&self, index).unwrap()
            }
        }

        impl PowF<f64> for $type {
            type Output = f64;

            fn powf(self, index: &f64) -> Option<Self::Output> {
                Some((self as f64).powf(*index))
            }

            fn sqrt(self) -> Option<Self::Output> {
                Some((self as f64).sqrt())
            }

            fn cbrt(self) -> Option<Self::Output> {
                Some((self as f64).cbrt())
            }
        }

        impl Exp for $type {
            type Output = f64;

            fn exp(self) -> Self::Output {
                (self as f64).exp()
            }
        }
    )*)
}
int_pow_template! { i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize }

macro_rules! floating_pow_template {
    ($($type:ident)*) => ($(
        impl Pow for $type {
            type Output = Self;

            fn pow(self, index: i64) -> Self::Output {
                ordinary::pow_times_group(&self, index)
            }
        }

        impl PowF for $type {
            type Output = Self;

            fn powf(self, index: &Self) -> Option<Self::Output> {
                Some(<$type>::powf(self, *index))
            }

            fn sqrt(self) -> Option<Self::Output> {
                Some(<$type>::sqrt(self))
            }

            fn cbrt(self) -> Option<Self::Output> {
                Some(<$type>::cbrt(self))
            }
        }

        impl Exp for $type {
            type Output = Self;

            fn exp(self) -> Self::Output {
                <$type>::exp(self)
            }
        }
    )*)
}
floating_pow_template! { f32 f64 }
