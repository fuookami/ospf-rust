use std::ops::Range;
use crate::algebra::*;

pub trait RangeTo
where
    Self: Ord + Sized + Clone,
{
    fn until(self, rhs: Self) -> Range<Self> {
        Range {
            start: self.clone(),
            end: rhs.clone(),
        }
    }
}

macro_rules! int_range_to_template {
    ($($type:ty)*) => ($(
        impl RangeTo for $type {
            fn until(self, rhs: Self) -> Range<Self> {
                self..rhs
            }
        }
    )*)
}
int_range_to_template! { i8 i16 i32 i64 i128 ix u8 u16 u32 u64 u128 uix }