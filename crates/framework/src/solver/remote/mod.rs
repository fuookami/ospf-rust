//! 远程求解客户端
//! Remote solver client

#[cfg(feature = "remote-solver")]
pub mod client;
#[cfg(feature = "remote-solver")]
pub mod cp;
pub mod domain;
pub mod compiled_linear;
pub mod error;
#[cfg(feature = "remote-solver")]
pub mod http;
pub mod ospf_serializer;
pub mod port;
pub mod storage;

#[cfg(feature = "remote-solver")]
pub use client::*;
#[cfg(feature = "remote-solver")]
pub use cp::*;
pub use domain::*;
pub use compiled_linear::*;
pub use error::*;
#[cfg(feature = "remote-solver")]
pub use http::*;
pub use ospf_serializer::*;
pub use port::*;
pub use storage::*;
