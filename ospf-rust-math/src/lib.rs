#![feature(associated_type_defaults)]
#![feature(specialization)]
#![feature(concat_idents)]
#![feature(unboxed_closures, fn_traits)]
#![feature(coroutines, coroutine_trait)]
#![feature(tuple_trait)]
#![feature(trait_upcasting)]
#![cfg_attr(debug_assertions, allow(dead_code, unused, incomplete_features))]

pub use algebra::*;
pub use combinatorics::*;
pub use functional::*;

// pub use geometry::*;
// pub use symbol::*;

pub mod algebra;
pub mod combinatorics;
pub mod functional;
// pub mod geometry;
// pub mod symbol;
