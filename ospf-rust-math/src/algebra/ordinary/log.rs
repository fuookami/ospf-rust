use std::ops::*;

use crate::algebra::concept::{FloatingNumber, NumberField};

pub fn ln<T: FloatingNumber + NumberField + Clone>(x: T) -> Option<T> {
    if x <= T::ZERO {
        T::NAN
    } else {
        let frac_e = T::ONE.clone() / T::E.clone();

        let mut val = T::ZERO.clone();
        let mut xp = x;
        if xp < T::ONE {
            while xp <= frac_e {
                xp *= T::E.clone();
                val -= T::ONE.clone();
            }
        } else if xp > T::ONE.clone() {
            while xp >= T::E.clone() {
                xp /= T::E.clone();
                val += T::ONE.clone();
            }
        }
        let mut base = xp.clone() - T::ONE.clone();
        let mut signed = T::ONE.clone();
        let mut i = T::ONE.clone();
        loop {
            let this_item = signed.clone() * base.clone() / i.clone();
            val += this_item.clone();
            base *= xp.clone() - T::ONE.clone();
            signed = -signed;
            i += T::ONE.clone();

            if this_item <= T::EPSILON {
                break;
            }
        }
        Some(val)
    }
}

pub fn log<T: FloatingNumber + NumberField + Clone>(nature: T, x: T) -> Option<T> {
    if let (Some(ln_nature), Some(ln_x)) = (ln(nature), ln(x)) {
        Some(ln_x / ln_nature)
    } else {
        None
    }
}

pub fn lg10<T: FloatingNumber + NumberField + Clone>(x: T) -> Option<T> {
    return log(T::TEN, x);
}

pub fn lg2<T: FloatingNumber + NumberField + Clone>(x: T) -> Option<T> {
    return log(T::TWO, x);
}
