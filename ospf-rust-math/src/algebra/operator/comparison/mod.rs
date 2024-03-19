// pub use compare::*;
pub use equal::{Equal, EqualFlt, EqualInt};
// pub use greater::*;
// pub use greater_equal::*;
pub use less::{Less, LessFlt, LessInt};
pub use less_equal::{LessEqual, LessEqualFlt, LessEqualInt};
pub use unequal::{Unequal, UnequalFlt, UnequalInt};
pub use zero::{Zero, ZeroFlt, ZeroInt};

// pub mod compare;
pub mod equal;
// pub mod greater;
// pub mod greater_equal;
pub mod less;
pub mod less_equal;
pub mod unequal;
pub mod zero;
