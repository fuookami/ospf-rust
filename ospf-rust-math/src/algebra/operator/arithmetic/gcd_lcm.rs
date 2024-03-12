use std::ops::{ Mul, Div };

use crate::algebra::ordinary::{ gcd_euclid, gcd_stein };

pub trait GcdLcm: Sized {
    type Output;

    fn gcd(self, rhs: Self) -> Self::Output;
    fn gcd_list(values: &[Self]) -> Self::Output;

    fn lcm(self, rhs: Self) -> Self::Output;
    fn lcm_list(values: &[Self]) -> Self::Output;

    fn gcd_lcm(self, rhs: Self) -> (Self::Output, Self::Output);
    fn gcd_lcm_list(values: &[Self]) -> (Self::Output, Self::Output);
}

pub fn gcd<T: GcdLcm>(lhs: T, rhs: T) -> T::Output {
    lhs.gcd(rhs)
}

pub fn gcd_list<T: GcdLcm>(values: &[T]) -> T::Output {
    T::gcd_list(values)
}

pub fn lcm<T: GcdLcm>(lhs: T, rhs: T) -> T::Output {
    lhs.lcm(rhs)
}

pub fn lcm_list<T: GcdLcm>(values: &[T]) -> T::Output {
    T::lcm_list(values)
}

pub fn gcd_lcm<T: GcdLcm>(lhs: T, rhs: T) -> (T::Output, T::Output) {
    lhs.gcd_lcm(rhs)
}

pub fn gcd_lcm_list<T: GcdLcm>(values: &[T]) -> (T::Output, T::Output) {
    T::gcd_lcm_list(values)
}

macro_rules! gcd_template {
    ($type:ident, $gcd:ident) => {
        impl GcdLcm for $type {
            type Output = $type;

            fn gcd(self, rhs: $type) -> Self::Output {
                $gcd(self.clone(), rhs.clone())
            }

            fn gcd_list(values: &[$type]) -> Self::Output {
                todo!()
            }

            fn lcm(self, rhs: $type) -> Self::Output {
                let this_gcd = (&self).gcd(&rhs);
                self * (rhs / this_gcd)
            }

            fn lcm_list(values: &[$type]) -> Self::Output {
                todo!()
            }

            fn gcd_lcm(self, rhs: $type) -> (Self::Output, Self::Output) {
                let this_gcd = (&self).gcd(&rhs);
                let this_lcm = self * (rhs / &this_gcd);
                (this_gcd, this_lcm)
            }

            fn gcd_lcm_list(values: &[$type]) -> (Self::Output, Self::Output) {
                todo!()
            }
        }

        impl GcdLcm for &$type {
            type Output = $type;

            fn gcd(self, rhs: &$type) -> Self::Output {
                $gcd(self.clone(), rhs.clone())
            }

            fn gcd_list(values: &[&$type]) -> Self::Output {
                todo!()
            }

            fn lcm(self, rhs: &$type) -> Self::Output {
                let this_gcd = self.gcd(rhs);
                self * (rhs / &this_gcd)
            }

            fn lcm_list(values: &[&$type]) -> Self::Output {
                todo!()
            }

            fn gcd_lcm(self, rhs: Self) -> (Self::Output, Self::Output) {
                let this_gcd = self.gcd(rhs);
                let this_lcm = self * (rhs / &this_gcd);
                (this_gcd, this_lcm)
            }

            fn gcd_lcm_list(values: &[Self]) -> (Self::Output, Self::Output) {
                todo!()
            }
        }
    };
}

macro_rules! small_gcd_template {
    ($($type:ident)*) => ($(
        gcd_template!($type, gcd_euclid);
    )*)
}
small_gcd_template! { i8 i16 i32 u8 u16 u32 f32 f64 }

macro_rules! big_gcd_template {
    ($($type:ident)*) => ($(
        gcd_template!($type, gcd_stein);
    )*)
}
big_gcd_template! { i64 i128 isize u64 u128 usize }
