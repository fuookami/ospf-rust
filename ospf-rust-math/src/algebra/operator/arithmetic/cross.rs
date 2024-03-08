use std::ops::Mul;

pub trait Cross<Rhs = Self> {
    type Output;

    fn cross(self, rhs: Rhs) -> Self::Output;
}

impl<T: Cross<U> + Clone, U> Cross<U> for &T {
    type Output = <T as Cross<U>>::Output;

    fn cross(self, rhs: U) -> Self::Output {
        self.clone().cross(rhs)
    }
}

pub fn cross<T: Cross<U>, U>(lhs: T, rhs: U) -> T::Output {
    lhs.cross(rhs)
}

macro_rules! scalar_cross_template {
    ($($type:ident)*) => ($(
        impl <U> Cross<U> for $type where $type: Mul<U> {
            type Output = <$type as Mul<U>>::Output;

            fn cross(self, rhs: U) -> Self::Output {
                self * rhs
            }
        }
    )*)
}
scalar_cross_template! { bool i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize f32 f64 }

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use crate::algebra::concept::RealNumber;
    use super::*;

    fn test_scalar<T: RealNumber + Cross<Output=T> + for<'a> Cross<&'a T, Output=T> + Debug>() {
        assert_eq!(&(T::ZERO.clone().cross(T::ZERO.clone())), T::ZERO);
        assert_eq!(&((T::ZERO).cross(T::ZERO.clone())), T::ZERO);
        assert_eq!(&(T::ZERO.clone().cross(T::ZERO)), T::ZERO);
        assert_eq!(&((T::ZERO).cross(T::ZERO)), T::ZERO);

        assert_eq!(&(T::ONE.clone().cross(T::TWO.clone())), T::TWO);
        assert_eq!(&((T::ONE).cross(T::TWO.clone())), T::TWO);
        assert_eq!(&(T::ONE.clone().cross(T::TWO)), T::TWO);
        assert_eq!(&((T::ONE).cross(T::TWO)), T::TWO);

        assert_eq!(&(T::TWO.clone().cross(T::ONE.clone())), T::TWO);
        assert_eq!(&((T::TWO).cross(T::ONE.clone())), T::TWO);
        assert_eq!(&(T::TWO.clone().cross(T::ONE)), T::TWO);
        assert_eq!(&((T::TWO).cross(T::ONE)), T::TWO);
    }

    #[test]
    fn test() {
        test_scalar::<i8>();
        test_scalar::<i16>();
        test_scalar::<i32>();
        test_scalar::<i64>();
        test_scalar::<i128>();
        test_scalar::<u8>();
        test_scalar::<u16>();
        test_scalar::<u32>();
        test_scalar::<u64>();
        test_scalar::<u128>();
        test_scalar::<f32>();
        test_scalar::<f64>();
    }
}
