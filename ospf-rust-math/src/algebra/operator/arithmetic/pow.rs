use std::ops::Div;
use crate::algebra::concept::FloatingNumber;
use crate::algebra::ordinary;

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

impl <T: Pow + Clone> Pow for &T {
    type Output = <T as Pow>::Output;

    fn pow(self, index: i64) -> Self::Output {
        self.clone().pow(index)
    }

    fn square(self) -> Self::Output {
        self.clone().square()
    }

    fn cubic(self) -> Self::Output {
        self.clone().cubic()
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

pub trait PowF<Index: FloatingNumber + Div<Output=Index> = Self>: Sized {
    type Output: FloatingNumber;

    fn powf(self, index: Index) -> Option<Self::Output>;

    fn sqrt(self) -> Option<Self::Output> {
        self.powf(Index::ONE.clone() / Index::TWO.clone())
    }

    fn cbrt(self) -> Option<Self::Output> {
        self.powf(Index::ONE.clone() / Index::THREE.clone())
    }
}

impl <T: PowF<U> + Clone, U: FloatingNumber + Div<Output=U>> PowF<U> for &T {
    type Output = <T as PowF<U>>::Output;

    fn powf(self, index: U) -> Option<Self::Output> {
        self.clone().powf(index)
    }

    fn sqrt(self) -> Option<Self::Output> {
        self.clone().sqrt()
    }

    fn cbrt(self) -> Option<Self::Output> {
        self.clone().cbrt()
    }
}

pub trait Exp {
    type Output;

    fn exp(self) -> Self::Output;
}

impl <T: Exp + Clone> Exp for &T {
    type Output = <T as Exp>::Output;

    fn exp(self) -> Self::Output {
        self.clone().exp()
    }
}

macro_rules! int_pow_template {
    ($($type:ident)*) => ($(
        impl Pow for $type {
            type Output = Self;

            fn pow(self, index: i64) -> Self::Output {
                ordinary::pow_times_semi_group(self, index).unwrap()
            }
        }

        impl PowF<f64> for $type {
            type Output = f64;

            fn powf(self, index: f64) -> Option<Self::Output> {
                Some((self as f64).powf(index))
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
int_pow_template! { u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 }

macro_rules! floating_pow_template {
    ($($type:ident)*) => ($(
        impl Pow for $type {
            type Output = Self;

            fn pow(self, index: i64) -> Self::Output {
                ordinary::pow_times_group(self, index)
            }
        }

        impl PowF for $type {
            type Output = Self;

            fn powf(self, index: Self) -> Option<Self::Output> {
                Some(<$type>::powf(self, index))
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
