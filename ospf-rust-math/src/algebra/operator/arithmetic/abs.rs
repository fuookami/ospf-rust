use std::ops::Neg;

pub trait Abs {
    type Output;

    fn abs(self) -> Self::Output;
}

impl <T: Abs + Clone> Abs for &T {
    type Output = <T as Abs>::Output;

    default fn abs(self) -> Self::Output {
        self.clone().abs()
    }
}

fn abs<T: Abs>(value: T) -> T::Output {
    value.abs()
}

macro_rules! int_abs_template {
    ($($type:ty)*) => ($(
        impl Abs for $type {
            type Output = $type;

            fn abs(self) -> Self::Output {
                if self < 0 { -self } else { self }
            }
        }
    )*)
}
int_abs_template! { i8 i16 i32 i64 i128 }

macro_rules! uint_abs_template {
    ($($type:ty)*) => ($(
        impl Abs for $type {
            type Output = $type;

            fn abs(self) -> Self::Output {
                self
            }
        }
    )*)
}
uint_abs_template! { u8 u16 u32 u64 u128 }

macro_rules! floating_abs_template {
    ($($type:ty)*) => ($(
        impl Abs for $type {
            type Output = $type;

            fn abs(self) -> Self::Output {
                if self < 0. { -self } else { self }
            }
        }
    )*)
}
floating_abs_template! { f32 f64 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i8() {
        assert_eq!((1i8).abs(), 1);
        assert_eq!((&1i8).abs(), 1);
        assert_eq!((-1i8).abs(), 1);
        assert_eq!((&-1i8).abs(), 1);
    }

    #[test]
    fn test_i16() {
        assert_eq!((1i16).abs(), 1);
        assert_eq!((&1i16).abs(), 1);
        assert_eq!((-1i16).abs(), 1);
        assert_eq!((&-1i16).abs(), 1);
    }
}
