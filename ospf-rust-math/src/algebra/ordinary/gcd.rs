use std::mem::swap;
use std::ops::*;

use crate::{Arithmetic, TrailingZeros};

pub fn gcd_stein<I: Arithmetic +
    TrailingZeros +
    for<'a> SubAssign<&'a I> +
    ShrAssign<usize> +
    Shl<usize, Output=I>
>(mut x: I, mut y: I) -> I
    where for<'a> &'a I: Rem<&'a I, Output=I> +
        BitOr<&'a I, Output=I>
{
    debug_assert!(&x >= &Arithmetic::ZERO);
    debug_assert!(&y >= &Arithmetic::ZERO);

    if &x == Arithmetic::ZERO {
        return y;
    }
    if &y == Arithmetic::ZERO {
        return x;
    }

    let shift = I::trailing_zeros(&x | &y);
    x >>= shift;
    y >>= shift;
    x >>= I::trailing_zeros(x.clone());

    loop {
        y >>= I::trailing_zeros(y.clone());
        if x > y {
            swap(&mut x, &mut y);
        }
        y -= &x;
        if &y == Arithmetic::ZERO {
            break;
        }
    }
    x << shift
}

pub fn gcd_euclid<I: Arithmetic + for<'a> SubAssign<&'a I>>(mut x: I, mut y: I) -> I {
    debug_assert!(&x >= &Arithmetic::ZERO);
    debug_assert!(&y >= &Arithmetic::ZERO);

    if &x == Arithmetic::ZERO {
        return y;
    }
    if &y == Arithmetic::ZERO {
        return x;
    }

    loop {
        if x > y {
            swap(&mut x, &mut y);
        }
        y -= &x;
        if &y == Arithmetic::ZERO {
            break;
        }
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stein() {
        assert_eq!(gcd_stein::<i32>(4, 6), 2);
        assert_eq!(gcd_stein::<i32>(6, 9), 3);
        assert_eq!(gcd_stein::<i32>(24, 30), 6);
    }

    #[test]
    fn test_euclid() {
        assert_eq!(gcd_euclid::<i32>(4, 6), 2);
        assert_eq!(gcd_euclid::<i32>(6, 9), 3);
        assert_eq!(gcd_euclid::<i32>(24, 30), 6);
    }
}
