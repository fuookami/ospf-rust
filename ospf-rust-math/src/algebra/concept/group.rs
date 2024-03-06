use std::ops::*;

pub trait PlusSemiGroup: Sized + Add<Output=Self> + for<'a> Add<&'a Self, Output=Self> + AddAssign {}

impl<T: Add<Output=T> + for<'a> Add<&'a T, Output=T> + AddAssign> PlusSemiGroup for T {}

pub trait PlusGroup: PlusSemiGroup + Neg<Output=Self> + Sub<Output=Self> + for<'a> Sub<&'a Self, Output=Self> + SubAssign {}

impl<T: PlusSemiGroup + Neg<Output=T> + Sub<Output=T> + for<'a> Sub<&'a T, Output=T> + SubAssign> PlusGroup for T {}

pub trait TimesSemiGroup: Sized + Mul<Output=Self> + for<'a> Mul<&'a Self, Output=Self> + MulAssign {}

impl<T: Mul<Output=T> + for<'a> Mul<&'a T, Output=T> + MulAssign> TimesSemiGroup for T {}

pub trait TimesGroup: TimesSemiGroup + Div<Output=Self> + for<'a> Div<&'a Self, Output=Self> + DivAssign + Rem<Output=Self> + for<'a> Rem<&'a Self, Output=Self> + RemAssign {}

impl<T: TimesSemiGroup + Div<Output=T> + for<'a> Div<&'a T, Output=T> + DivAssign + Rem<Output=T> + for<'a> Rem<&'a T, Output=T> + RemAssign> TimesGroup for T {}

pub trait NumberRing: PlusGroup + TimesSemiGroup {}

impl<T: PlusGroup + TimesSemiGroup> NumberRing for T {}

pub trait NumberField: NumberRing + TimesGroup {}

impl<T: NumberRing + TimesGroup> NumberField for T {}
