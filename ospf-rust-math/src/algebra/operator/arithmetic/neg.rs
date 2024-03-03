use crate::algebra::*;

pub trait Neg {
    type Output;

    fn neg(&self) -> Self::Output;
}

fn neg<T: Neg>(x: &T) -> T::Output {
    x.neg()
}

macro_rules! signed_neg_template {
    ($($type:ty)*) => ($(
        impl Neg for $type {
            type Output = Self;

            fn neg(&self) -> Self::Output {
                -self
            }
        }
    )*)
}
signed_neg_template! { i8 i16 i32 i64 i128 ix f32 f64 dec }

macro_rules! unsigned_neg_template {
    ($($type:ty)*) => ($(
        impl Neg for $type {
            type Output = Self;

            fn neg(&self) -> Self::Output {
                <$type>::MAX - self + 1
            }
        }
    )*)
}
unsigned_neg_template! { u8 u16 u32 u64 u128 }

impl Neg for uix {
    type Output = ix;

    fn neg(&self) -> Self::Output {
        -ix::from(*self)
    }
}
