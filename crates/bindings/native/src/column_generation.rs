//! 共享原生列生成执行边界。 / Shared native column-generation execution boundary.

use ospf_rust_core::solver::SolveOptions;
use ospf_rust_framework::solver::{ProjectedColumnGeneration, ProjectedColumnGenerationConfig};
use ospf_rust_bridge::{BridgeError, JobView, Result, SolveRequest, decode_meta_model, decode_model, encode_column_generation_result};
use crate::{NativeEngine, create_solver};

impl NativeEngine {
    pub(crate) fn start_column_generation(&self, request: SolveRequest) -> Result<JobView> {
        let configuration = request.column_generation_options.clone().ok_or_else(|| BridgeError::new("InvalidInput", "missing column-generation configuration"))?;
        configuration.validate()?;
        let solver = create_solver(configuration.backend.id(), None)?;
        #[cfg(feature = "scip")]
        let solver: std::sync::Arc<dyn ospf_rust_core::solver::Solver> = if configuration.backend == ospf_rust_bridge::NativeBackend::Scip {
            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
            std::sync::Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults()))
        } else { solver };
        decode_model(&request.model)?;
        let artifact = request.model.clone();
        self.start_encoded(request, move |options| {
            // 所有非 Send 模型与列池均在此 worker 内构造。 / Construct all non-Send models and column pools inside this worker.
            let source = decode_meta_model(&artifact)?;
            let decisions = configuration.decisions(&source)?;
            let defaults = ProjectedColumnGenerationConfig::default();
            let config = ProjectedColumnGenerationConfig {
                iteration_limit: configuration.iteration_limit.map_or(defaults.iteration_limit, |value| value as usize),
                column_limit: configuration.column_limit.map_or(defaults.column_limit, |value| value as usize),
                time_limit: options.time_limit.unwrap_or(defaults.time_limit), tolerance: configuration.tolerance.unwrap_or(defaults.tolerance),
            };
            let handle = options.cancellation_handle.ok_or_else(|| BridgeError::new("ExecutionFailure", "missing cancellation handle"))?;
            let result = ProjectedColumnGeneration::solve(&source, decisions, configuration.blocks.iter().map(|block| block.id.clone()).collect(), config, handle,
                |model, remaining, handle| solver.solve_linear_report_with_options(model, &SolveOptions {
                    time_limit: Some(remaining), cancellation_handle: Some(handle), progress_reporter: options.progress_reporter, ..Default::default()
                })).map_err(|error| BridgeError::new(error.solver_error_class().code(), error))?;
            encode_column_generation_result(&result, &artifact, &source, config.tolerance)
        })
    }
}
