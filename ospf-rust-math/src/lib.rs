#![feature(associated_type_defaults)]
#![feature(specialization)]
#![feature(unboxed_closures, fn_traits)]
#![feature(coroutines, coroutine_trait)]

pub use algebra::*;
pub use combinatorics::*;
pub use functional::*;
pub use geometry::*;
pub use symbol::*;

pub mod algebra;
pub mod combinatorics;
pub mod functional;
pub mod geometry;
pub mod symbol;

