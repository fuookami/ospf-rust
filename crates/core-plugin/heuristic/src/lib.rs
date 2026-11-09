//! 启发式求解插件
//! Heuristic solver plugin
//!
//! 本 crate 承载 core 的元启发式框架与选择型超启发式，对应 Kotlin 的
//! `ospf-kotlin-core-plugin-heuristic`（其包结构同样是 `core/solver/heuristic` 与
//! `core/solver/hyper_heuristic`）。
//! This crate hosts the core metaheuristic framework and the selection hyper-heuristic, mirroring
//! Kotlin's `ospf-kotlin-core-plugin-heuristic`, whose package layout is the same two subtrees.

pub mod heuristic;
pub mod hyper_heuristic;
