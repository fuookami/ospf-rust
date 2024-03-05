use std::ops::Div;

use crate::algebra::concept::FloatingNumber;

pub trait IntDiv<Rhs = Self> {
    type Output;

    fn int_div(self, rhs: Rhs) -> Self::Output;
}

impl <T: Div<U>, U> IntDiv<U> for T {
    type Output = <T as Div<U>>::Output;

    default fn int_div(self, rhs: U) -> Self::Output {
        return self / rhs
    }
}

fn int_div<T: IntDiv>(lhs: T, rhs: T) -> T::Output {
    lhs.int_div(rhs)
}

macro_rules! floating_int_div_template {
    ($($type:ty)*) => ($(
        impl IntDiv for $type {
            fn int_div(self, rhs: Self) -> Self::Output {
                return (self / rhs).floor()
            }
        }

        impl IntDiv for &$type {
            fn int_div(self, rhs: Self) -> Self::Output {
                return (*self / *rhs).floor()
            }
        }
    )*)
}
floating_int_div_template! { f32 f64 }
