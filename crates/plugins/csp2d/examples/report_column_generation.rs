//! 显式选择原生后端，展示原 shelf 的真实报告驱动列生成。 / Explicitly select a native backend and demonstrate actual report-driven column generation with the original shelf.

use std::sync::Arc;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_core::solver::LinearSolver;
use ospf_rust_framework_csp2d::application::{Csp2dColumnGenerationInput, solve_csp2d_column_generation};
use ospf_rust_framework_csp2d::domain::material::{DemandValue2, Material2, MaterialId2, Product2, ProductId2};
use ospf_rust_framework_csp2d::domain::pattern_generation::{PatternGenerationRequest2, ShelfPatternGenerator2};
use ospf_rust_framework_csp2d::domain::produce::Csp2dMasterDemand;

fn meters(value: f64) -> Quantity<f64, Meter> { Quantity::new_ct(value) }

fn backend(name: &str) -> Result<Arc<dyn LinearSolver>, Box<dyn std::error::Error>> {
    match name {
        #[cfg(any(feature = "scip", feature = "scip-bundled", feature = "scip-from-source"))]
        "scip" => {
            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
            Ok(Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults())))
        },
        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
        "gurobi" => {
            use ospf_rust_core::solver::solvers::gurobi::{GurobiConfig, GurobiSolver};
            Ok(Arc::new(GurobiSolver::with_config(GurobiConfig::default().with_output(false))))
        },
        _ => Err(format!("Backend '{name}' is unknown or not compiled; explicitly enable its solver feature and runtime").into()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selected = std::env::args().nth(1).ok_or("Specify scip or gurobi; no backend is selected automatically")?;
    let solver = backend(&selected)?;
    let material = Material2::sheet(MaterialId2::new("sheet"), "4m x 2m", meters(4.0), meters(2.0), 10);
    let product = Product2::new(ProductId2::new("part"), "1m x 1m", meters(1.0), meters(1.0)).with_allow_rotate(false);
    let input = Csp2dColumnGenerationInput {
        requests: vec![PatternGenerationRequest2::new(material, vec![(product, 4)], meters(0.0))],
        demands: vec![Csp2dMasterDemand::new(ProductId2::new("part"), DemandValue2::Amount(3))],
        initial_columns: Vec::new(), objective: Default::default(), config: Default::default(),
    };
    let result = solve_csp2d_column_generation(solver, Default::default(), &input, &[Box::new(ShelfPatternGenerator2::new())])?;
    println!("stop={:?}, columns={}, children={}, candidate_complete={}", result.stop, result.columns.len(), result.stages.len(), result.candidate_complete);
    let final_result = &result.stages[result.final_stage.ok_or("No final integer stage was executed")?].result;
    println!("restricted_status={:?}, usages={:?}, objective={:?}", final_result.stage_report.report.problem_status, final_result.usages, final_result.objective);
    if final_result.usages.is_none() { return Err("Final restricted master has no incumbent; inspect the original child report".into()); }
    Ok(())
}
