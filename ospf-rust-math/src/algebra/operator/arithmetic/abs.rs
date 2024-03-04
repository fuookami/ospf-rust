use crate::algebra::numeric_integer::*;

pub trait Abs {
    type Output;

    fn abs(&self) -> Self::Output;
}

fn abs<T: Abs>(value: &T) -> T::Output {
    value.abs()
}

macro_rules! int_abs_template {
    ($($type:ty)*) => ($(
        impl Abs for $type {
            type Output = $type;

            fn abs(&self) -> Self::Output {
                if *self < 0 { -self } else { self.clone() }
            }
        }
    )*)
}
int_abs_template! { i8 i16 i32 i64 i128 ix }

macro_rules! uint_abs_template {
    ($($type:ty)*) => ($(
        impl Abs for $type {
            type Output = $type;

            fn abs(&self) -> Self::Output {
                self.clone()
            }
        }
    )*)
}
uint_abs_template! { u8 u16 u32 u64 u128 uix }

macro_rules! floating_abs_template {
    ($($type:ty)*) => ($(
        impl Abs for $type {
            type Output = $type;

            fn abs(&self) -> Self::Output {
                if *self < 0. { -self } else { self.clone() }
            }
        }
    )*)
}
floating_abs_template! { f32 f64 dec }
