pub use bound::*;
pub use interval::*;
// pub use value_range::*;
// pub use value_range_stc::*;
use value_wrapper::ValueWrapper;

pub mod bound;
pub mod interval;
// pub mod value_range;
// pub mod value_range_stc;
pub mod value_wrapper;

pub struct IllegalArgumentError {
    msg: String,
}
