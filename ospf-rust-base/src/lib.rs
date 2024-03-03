#![feature(specialization)]
#![feature(entry_insert)]
#![feature(coroutines, coroutine_trait)]

#[macro_use]
extern crate strum;

pub mod meta_programming;
pub mod error;
mod generator_iterator;
pub mod indexed_type;

pub use meta_programming::*;
pub use error::*;
pub use generator_iterator::GeneratorIterator;
pub use indexed_type::{IndexGenerator, Indexed};
