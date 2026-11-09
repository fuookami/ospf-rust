//! Gurobi-backed IIS convenience entry points / 基于 Gurobi 的 IIS 便捷入口
//!
//! 通用 IIS 算法位于 `ospf-rust-core`，本模块只负责注入 `GurobiSolver`。
//! The generic IIS algorithms live in `ospf-rust-core`; this module only injects `GurobiSolver`.

use ospf_rust_core::error::Result;
use ospf_rust_core::solver::iis::{IISConfig, LinearIISModel, LinearTriadModelIISSource};

use crate::{GurobiConfig, GurobiSolver};

/// 使用 Gurobi 计算 IIS / Compute IIS with Gurobi
pub fn compute_iis<M>(model: &M, config: &IISConfig) -> Result<LinearIISModel>
where
    M: LinearTriadModelIISSource + ?Sized,
{
    let solver = GurobiSolver::with_config(GurobiConfig::new().with_output(false));
    ospf_rust_core::solver::iis::compute_iis_with_solver(model, config, &solver)
}
