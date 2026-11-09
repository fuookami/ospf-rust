//! HTTP/N-API 共用原生 CSP2D 列生成，不降级启发式求解或改写报告。 / Native CSP2D column generation shared by HTTP/N-API, without heuristic solver downgrade or report rewriting.

use std::sync::Arc;
use std::time::{Duration, Instant};
use ospf_rust_core::solver::{LinearSolver, SolveOptions};
use ospf_rust_framework_csp2d::application::{Csp2dExecutionOptions, solve_csp2d_column_generation};
use ospf_rust_bridge::{Csp2dOptions, BridgeError, JobView, ModelArtifact, Result, SolveRequest, decode_csp2d_problem, encode_csp2d_execution_report};
use crate::{NativeEngine, create_solver};

fn execute(
    source: &ModelArtifact,
    configuration: Csp2dOptions,
    solver: Arc<dyn LinearSolver>,
    options: &SolveOptions<'_>,
) -> Result<String> {
    let started = Instant::now();
    let time_limit = options.time_limit.unwrap_or(Duration::from_secs(60));
    let cancellation = options.cancellation_handle.ok_or_else(|| BridgeError::new("ExecutionFailure", "missing CSP2D cancellation handle"))?.clone();
    // 非 Send 领域对象只在求解 worker 内构造，解码及生成共享同一时间预算。 / Construct non-Send domain objects only inside the solve worker; decoding and generation share one time budget.
    let (_, mut input) = decode_csp2d_problem(source)?;
    input.config = configuration.config()?;
    let generators = configuration.generators()?;
    let result = solve_csp2d_column_generation(solver, Csp2dExecutionOptions {
        time_limit: time_limit.saturating_sub(started.elapsed()), cancellation, progress: options.progress_reporter.cloned(),
        report_limit: configuration.report_limit.unwrap_or(196) as usize, ..Default::default()
    }, &input, &generators).map_err(|error| BridgeError::new(error.solver_error_class().code(), error))?;
    encode_csp2d_execution_report(source, configuration, &result)
}

impl NativeEngine {
    pub(crate) fn start_csp2d(&self, request: SolveRequest) -> Result<JobView> {
        let configuration = request.csp2d_options.clone().ok_or_else(|| BridgeError::new("InvalidInput", "missing CSP2D configuration"))?;
        let config = configuration.config()?;
        let (_, input) = decode_csp2d_problem(&request.model)?;
        if input.initial_columns.len() > config.column_limit { return Err(BridgeError::new("InvalidInput", "CSP2D initial pool exceeds poolLimit")); }
        let solver = create_solver(configuration.backend.id(), None)?;
        #[cfg(feature = "scip")]
        let solver: Arc<dyn ospf_rust_core::solver::Solver> = if configuration.backend == ospf_rust_bridge::NativeBackend::Scip {
            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
            Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults()))
        } else { solver };
        let solver: Arc<dyn LinearSolver> = solver;
        let source = request.model.clone();
        self.start_encoded(request, move |options| execute(&source, configuration, solver, options))
    }
}

#[cfg(test)]
mod tests;
