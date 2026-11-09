//! HTTP/N-API 共用原生 BPP3D 单轮执行，不降级求解器或改写报告。 / Native BPP3D one-round execution shared by HTTP/N-API, without solver downgrade or report rewriting.

use std::sync::Arc;
use std::time::{Duration, Instant};
use ospf_rust_core::solver::{LinearSolver, SolveOptions};
use ospf_rust_framework::solver::LinearExecutionOptions;
use ospf_rust_framework_bpp3d::application::{
    Bpp3dOneRoundOptions, ColumnGenerationApplicationService, FinalPackingValidation,
};
use ospf_rust_bridge::{Bpp3dOptions, BridgeError, JobView, ModelArtifact, Result, SolveRequest, decode_bpp3d_problem, encode_bpp3d_execution_report};
use crate::{NativeEngine, create_solver};

fn execute(
    source: &ModelArtifact,
    configuration: Bpp3dOptions,
    solver: Arc<dyn LinearSolver>,
    options: &SolveOptions<'_>,
) -> Result<String> {
    let started = Instant::now();
    let time_limit = options.time_limit.unwrap_or(Duration::from_secs(60));
    let cancellation = options.cancellation_handle.ok_or_else(|| BridgeError::new("ExecutionFailure", "missing BPP3D cancellation handle"))?.clone();
    let (report_limit, max_placements) = configuration.budgets()?;
    // 非 Send 领域对象只在求解 worker 内构造，解码及三轮子求解共享同一时间预算。 / Construct non-Send domain objects only inside the solve worker; decoding and all three child solves share one time budget.
    let layer_generation = configuration.layer_generation()?;
    let (_, request, traces) = decode_bpp3d_problem(source)?;
    let evidence = ColumnGenerationApplicationService::default().run_csv_materialized_one_generation_round_with_evidence(
        request, traces, layer_generation, solver,
        Bpp3dOneRoundOptions {
            linear: LinearExecutionOptions {
                time_limit: time_limit.saturating_sub(started.elapsed()), cancellation,
                progress: options.progress_reporter.cloned(), report_limit, ..Default::default()
            },
            validation: FinalPackingValidation { max_placements, extra_geometry: None },
        },
    ).map_err(|error| BridgeError::new(error.solver_error_class().code(), &error))?;
    // 作业完成只表示执行报告已产出；报告内的 completed/failure 才是原流程的真实结局。 / Job completion means the execution report was produced; its completed/failure fields carry the real flow outcome.
    encode_bpp3d_execution_report(source, max_placements, &evidence)
}

impl NativeEngine {
    pub(crate) fn start_bpp3d(&self, request: SolveRequest) -> Result<JobView> {
        let configuration = request.bpp3d_options.clone().ok_or_else(|| BridgeError::new("InvalidInput", "missing BPP3D configuration"))?;
        configuration.budgets()?;
        decode_bpp3d_problem(&request.model)?;
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
