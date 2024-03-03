#![feature(associated_type_defaults)]
#![feature(fn_traits)]
#![feature(unboxed_closures)]
#![feature(specialization)]
#![feature(coroutines, coroutine_trait)]

pub mod algebra;
pub mod combinatorics;
pub mod functional;
pub mod geometry;
pub mod symbol;

pub use algebra::*;
pub use combinatorics::*;
pub use functional::*;
pub use geometry::*;
pub use symbol::*;
