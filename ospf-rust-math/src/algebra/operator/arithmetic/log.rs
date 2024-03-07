use bigdecimal::num_bigint::ToBigInt;

use crate::algebra::*;

pub trait Log<Base: FloatingNumber = Self>: Sized {
    type Output;

    fn log(self, base: Base) -> Option<Self::Output>;

    fn lg2(self) -> Option<Self::Output> {
        self.log(Base::TWO)
    }

    fn lg(self) -> Option<Self::Output> {
        self.log(Base::TEN)
    }

    fn ln(self) -> Option<Self::Output> {
        self.log(Base::E)
    }
}

impl <T: Log<U> + Clone, U: FloatingNumber> Log<U> for &T {
    type Output = <T as Log<U>>::Output;
    
    fn log(self, base: U) -> Option<Self::Output> {
        self.clone().log(base)
    }

    fn lg2(self) -> Option<Self::Output> {
        self.clone().lg2()
    }

    fn lg(self) -> Option<Self::Output> {
        self.clone().lg()
    }

    fn ln(self) -> Option<Self::Output> {
        self.clone().ln()
    }
}

pub fn log<Lhs: Log<Rhs>, Rhs: FloatingNumber>(lhs: Lhs, rhs: Rhs) -> Option<Lhs::Output> {
    lhs.log(rhs)
}

pub fn lg2<Lhs: Log<Rhs>, Rhs: FloatingNumber>(lhs: Lhs) -> Option<Lhs::Output> {
    lhs.lg2()
}

pub fn lg<Lhs: Log<Rhs>, Rhs: FloatingNumber>(lhs: Lhs) -> Option<Lhs::Output> {
    lhs.lg()
}

pub fn ln<Lhs: Log<Rhs>, Rhs: FloatingNumber>(lhs: Lhs) -> Option<Lhs::Output> {
    lhs.ln()
}

macro_rules! int_log_template {
    ($($type:ident)*) => ($(
        impl Log<f64> for $type {
            type Output = f64;

            fn log(self, base: f64) -> Option<Self::Output> {
                Some((self as f64).log(base))
            }

            fn lg2(self) -> Option<Self::Output> {
                Some((self as f64).log2())
            }

            fn lg(self) -> Option<Self::Output> {
                Some((self as f64).log10())
            }

            fn ln(self) -> Option<Self::Output> {
                Some((self as f64).ln())
            }
        }
    )*)
}
int_log_template! { i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 }

macro_rules! floating_log_template {
    ($($type:ident)*) => ($(
        impl Log for $type {
            type Output = Self;

            fn log(self, base: Self) -> Option<Self::Output> {
                Some(self.log(base))
            }
        }
    )*);
}
floating_log_template! { f32 f64 }
