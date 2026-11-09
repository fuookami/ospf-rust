//! 求解器接口模块 / Solver Interface Module
//!
//! 本模块提供优化求解器的统一接口。
//! This module provides unified interface for optimization solvers.
//!
//! # 核心概念 / Core Concepts
//!
//! - [`SolverInfo`] - 求解器基础信息 trait
//! - [`LinearSolver`] - 线性模型求解 trait
//! - [`QuadraticSolver`] - 二次模型求解 trait
//! - [`Solver`] - 组合求解器 trait（线性 + 二次）
//! - [`SolverConfig`] - 求解器配置
//! - [`SolverOutput`] - 求解结果
//! - [`SolverStatus`] - 求解状态
//!
//! # 支持的求解器 / Supported Solvers
//!
//! - `ospf-rust-core-plugin-gurobi` - Gurobi 求解器（独立 plugin crate）
//! - `ospf-rust-core-plugin-scip` - SCIP 求解器（独立 plugin crate）
//!
//! # IIS 计算 / IIS Computation
//!
//! - [`iis::compute_iis`] - 计算 IIS
//! - [`iis::IISConfig`] - IIS 配置
//! - [`iis::LinearIISModel`] - IIS 模型
//!
//! # 启发式算法 / Heuristic Algorithms
//!
//! 元启发式与选择型超启发式已迁移到 `ospf-rust-core-plugin-heuristic`。
//! The metaheuristic framework and selection hyper-heuristic moved to
//! `ospf-rust-core-plugin-heuristic`.

pub mod audit;
pub mod cancellation;
pub mod checkpoint;
pub mod config;
pub mod constraint_programming;
pub mod fingerprint;
pub mod iis;
pub mod linear_certificate;
pub mod output;
pub mod progress;
pub mod report;
#[allow(clippy::module_inception)]
pub mod solver;
#[doc(hidden)]
pub mod solver_config;
pub mod solver_ext;
#[doc(hidden)]
pub mod solver_output;
pub mod value;

pub use audit::*;
pub use cancellation::*;
pub use checkpoint::*;
pub use config::*;
pub use constraint_programming::*;
pub use fingerprint::*;
pub use output::*;
pub use linear_certificate::*;
pub use progress::*;
pub use report::*;
pub use solver::*;
pub use solver_ext::*;
pub use value::*;
