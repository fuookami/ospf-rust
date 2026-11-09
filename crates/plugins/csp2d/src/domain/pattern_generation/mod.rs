//! 二维图案生成领域 / 2D pattern generation domain
//!
//! 映射 Kotlin `csp2d-domain-pattern-generation-context`：图案生成请求、货架式生成器与
//! 断头台切分树构造，以及默认行语义下的物理列身份。报告驱动定价由 produce context 与 application 组合，
//! 启发式生成器不提供精确定价或完整候选证明。
//! Maps the Kotlin `csp2d-domain-pattern-generation-context` submodule: the generation request, the shelf
//! generator, guillotine cut-tree construction and physical-column identity under default-row semantics.
//! Report-driven pricing composes the produce context/application; heuristic generators provide neither
//! exact-pricing nor complete-candidate proofs.

pub mod costar_filler;
pub mod request;
pub mod shelf_generator;
pub(crate) mod catalog;

pub use costar_filler::{CostarFiller, CostarFillingResult};
pub use request::{PatternGenerationRequest2, PatternGenerator2};
pub use shelf_generator::{ShelfPatternGenerator2, shelf_pattern};
pub use catalog::PatternColumnKey;
