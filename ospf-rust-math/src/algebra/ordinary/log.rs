use std::ops::*;

use crate::algebra::concept::FloatingNumber;

pub fn ln<T: FloatingNumber>(x: &T) -> Option<T>
    where for<'a> &'a T: Sub<&'a T, Output=T> +
        Mul<&'a T, Output=T> +
        Div<&'a T, Output=T> {
    if x <= T::ZERO {
        T::NAN.clone()
    } else {
        let frac_e = T::ONE / T::E;

        let mut val = T::ZERO.clone();
        let mut xp = x.clone();
        if &xp < T::ONE {
            while xp <= frac_e {
                xp *= T::E;
                val -= T::ONE;
            }
        } else if &xp > T::ONE {
            while &xp >= T::E {
                xp /= T::E;
                val += T::ONE;
            }
        }
        let mut base = &xp - T::ONE;
        let mut signed = T::ONE.clone();
        let mut i = T::ONE.clone();
        loop {
            let this_item = &signed * &base / &i;
            val += this_item.clone();
            base *= &xp - T::ONE;
            signed = -signed;
            i += T::ONE;

            if &this_item <= T::EPSILON {
                break;
            }
        }
        Some(val)
    }
}

pub fn log<T: FloatingNumber + Clone>(nature: &T, x: &T) -> Option<T>
    where for<'a> &'a T: Sub<&'a T, Output=T> +
        Mul<&'a T, Output=T> +
        Div<&'a T, Output=T> {
    if let (Some(ln_nature), Some(ln_x)) = (ln(nature), ln(x)) {
        Some(ln_x / ln_nature)
    } else {
        None
    }
}

pub fn lg10<T: FloatingNumber + Clone>(x: &T) -> Option<T>
    where for<'a> &'a T: Sub<&'a T, Output=T> +
        Mul<&'a T, Output=T> +
        Div<&'a T, Output=T> {
    log(T::TEN, x)
}

pub fn lg2<T: FloatingNumber + Clone>(x: &T) -> Option<T>
    where for<'a> &'a T: Sub<&'a T, Output=T> +
        Mul<&'a T, Output=T> +
        Div<&'a T, Output=T> {
    log(T::TWO, x)
}
