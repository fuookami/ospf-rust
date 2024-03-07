use super::{Arithmetic, Precision};

pub trait Bounded: Sized {
    const MINIMUM: Option<Self>;
    const MAXIMUM: Option<Self>;
    const POSITIVE_MINIMUM: Self;
}

impl Bounded for bool {
    const MINIMUM: Option<bool> = Some(false);
    const MAXIMUM: Option<bool> = Some(true);
    const POSITIVE_MINIMUM: Self = true;
}

macro_rules! int_bound_template {
    ($($type:ident)*) => ($(
        impl Bounded for $type {
            const MINIMUM: Option<Self> = Some(<$type>::MIN);
            const MAXIMUM: Option<Self> = Some(<$type>::MAX);
            const POSITIVE_MINIMUM: Self = Self::ONE;
        }
    )*)
}
int_bound_template! { i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize }

macro_rules! floating_bound_template {
    ($($type:ident)*) => ($(
        impl Bounded for $type {
            const MINIMUM: Option<Self> = Some(<$type>::MIN);
            const MAXIMUM: Option<Self> = Some(<$type>::MAX);
            const POSITIVE_MINIMUM: Self = Self::EPSILON;
        }
    )*)
}
floating_bound_template! { f32 f64 }
