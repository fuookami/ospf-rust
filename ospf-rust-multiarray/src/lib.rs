#![feature(generic_const_exprs)]
#![feature(associated_type_defaults)]
#![feature(coroutines, coroutine_trait)]
#![cfg_attr(debug_assertions, allow(dead_code, unused, incomplete_features))]

pub use shape::*;
pub use multi_array::*;
pub use multi_array_view::*;

#[macro_use]
pub mod dummy_vector;
#[macro_use]
pub mod map_vector;
pub mod shape;
pub mod multi_array;
pub mod multi_array_view;
