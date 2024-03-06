use std::ops::*;

use crate::algebra::operator::Reciprocal;

pub trait PlusSemiGroup: Sized + Add<Output=Self> + AddAssign {}

impl<T: Add<Output=Self> + AddAssign> PlusSemiGroup for T {}

pub trait PlusGroup: PlusSemiGroup + Neg<Output=Self> + Sub<Output=Self> + SubAssign {}

impl<T: PlusSemiGroup + Neg<Output=Self> + Sub<Output=Self> + SubAssign> PlusGroup for T {}

pub trait TimesSemiGroup: Sized + Mul<Output=Self> + MulAssign {}

impl<T: Mul<Output=Self> + MulAssign> TimesSemiGroup for T {}

pub trait TimesGroup: TimesSemiGroup + Reciprocal<Output=Option<Self>> + Div<Output=Self> + DivAssign + Rem<Output=Self> {}

impl <T: TimesSemiGroup + Reciprocal<Output=Option<Self>> + Div<Output=Self> + DivAssign + Rem<Output=Self>> TimesGroup for T {}

pub trait NumberRing: PlusGroup + TimesSemiGroup {}

impl<T: PlusGroup + TimesSemiGroup> NumberRing for T {}

pub trait NumberField: NumberRing + TimesGroup {}

impl<T: NumberRing + TimesGroup> NumberField for T {}
