use std::ops::Mul;

pub trait Cross<Rhs = Self> {
    type Output;

    fn cross(self, rhs: Rhs) -> Self::Output;
}

impl <T: Mul<U> + Clone, U> Cross<U> for &T {
    type Output = <T as Mul<U>>::Output;

    default fn cross(self, rhs: U) -> Self::Output {
        self.clone() * rhs
    }
}

fn cross<T: Cross<U>, U>(lhs: T, rhs: U) -> T::Output {
    lhs.cross(rhs)
}
