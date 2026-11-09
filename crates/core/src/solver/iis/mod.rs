//! IIS (Irreducible Inconsistent Subsystem) 计算模块
//! IIS (Irreducible Inconsistent Subsystem) Computation Module
//!
//! 当优化模型不可行时，IIS 用于识别导致不可行的最小约束和变量边界集合。
//! When an optimization model is infeasible, IIS is used to identify the minimal
//! set of constraints and variable bounds causing infeasibility.
//!
//! # 核心概念 / Core Concepts
//!
//! - **IIS**: 不可约不一致子系统，即删除其中任何一个约束或边界后系统变为可行
//! - **IIS**: Irreducible Inconsistent Subsystem, meaning removing any constraint
//!   or bound from it makes the system feasible
//!
//! # 算法 / Algorithms
//!
//! - **弹性过滤 (Elastic Filtering)**: 通过添加松弛变量来识别 IIS
//! - **Elastic Filtering**: Identifies IIS by adding slack variables
//! - **删除过滤 (Deletion Filtering)**: 逐个删除约束来识别 IIS
//! - **Deletion Filtering**: Identifies IIS by removing constraints one by one
//!
//! 算法通过 [`LinearSolver`] 注入可行性 oracle；Gurobi 的便捷入口位于
//! `ospf-rust-core-plugin-gurobi::iis`。
//! The algorithms receive a feasibility oracle through [`LinearSolver`]; the Gurobi convenience
//! entry point lives in `ospf-rust-core-plugin-gurobi::iis`.

pub mod deletion_filtering;
pub mod elastic_filtering;
pub mod iis_config;
pub mod iis_model;

pub use deletion_filtering::*;
pub use elastic_filtering::*;
pub use iis_config::*;
pub use iis_model::*;

use crate::error::Result;
use crate::solver::LinearSolver;

/// 使用给定求解器计算 IIS / Compute IIS with the given solver
///
/// 对给定的线性模型计算不可约不一致子系统。求解器只承担可行性 oracle 的角色。
/// Computes the irreducible inconsistent subsystem for the given linear model. The solver
/// only acts as the feasibility oracle.
pub fn compute_iis_with_solver<M>(
    model: &M,
    config: &IISConfig,
    solver: &dyn LinearSolver,
) -> Result<LinearIISModel>
where
    M: LinearTriadModelIISSource + ?Sized,
{
    match config.algorithm {
        IISAlgorithm::ElasticFiltering => {
            elastic_filtering::compute_iis_elastic(model, config, solver)
        }
        IISAlgorithm::DeletionFiltering => {
            deletion_filtering::compute_iis_deletion(model, config, solver)
        }
    }
}
