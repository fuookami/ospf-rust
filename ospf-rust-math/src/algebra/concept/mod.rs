pub mod arithmetic;
pub mod bounded;
pub mod precision;
pub mod scalar;
pub mod signed;
pub mod tensor;
pub mod variant;
pub mod vector;

pub use arithmetic::*;
pub use bounded::*;
use num::bigint::ToBigInt;
pub use precision::*;
pub use scalar::*;
pub use signed::*;
pub use tensor::*;
pub use variant::*;
pub use vector::*;

pub type dec = bigdecimal::BigDecimal;
pub type ix = num::BigInt;
pub type uix = num::BigUint;

pub trait DecFrom<T>: Sized {
    fn from_ix(value: T) -> Self;
}

impl DecFrom<ix> for dec {
    fn from_ix(value: ix) -> Self {
        dec::new(value, 1)
    }   
}

impl DecFrom<uix> for dec {
    fn from_ix(value: uix) -> Self {
        dec::new(value.to_bigint().unwrap(), 1)
    }
}
