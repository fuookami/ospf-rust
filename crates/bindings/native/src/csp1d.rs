//! HTTP/N-API 共用原 CSP1D 编排、总预算和实际报告；不降级启发式。
//! HTTP/N-API share original CSP1D orchestration, total budget and actual reports, without heuristic downgrade.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use ospf_rust_bridge::{BridgeError, Csp1dAlgorithm, Csp1dExecutionStop, Csp1dOptions, JobView, ModelArtifact, Result, SolveRequest,
    decode_csp1d_problem, encode_csp1d_execution_report};
use ospf_rust_core::solver::{LinearSolver, SolveHandle, SolveOptions, TerminationReason};
use ospf_rust_framework_csp1d as domain;
use domain::Csp1dInitialCuttingPlanGenerator;
use crate::{NativeEngine, create_solver};

struct BoundedGenerator {
    delegate: Box<dyn Csp1dInitialCuttingPlanGenerator<f64>>,
    pool_limit: usize,
    output_limit: usize,
    cancellation: SolveHandle,
    started: Instant,
    time_limit: Duration,
}

impl Csp1dInitialCuttingPlanGenerator<f64> for BoundedGenerator {
    fn generate(&self, input: &domain::CuttingPlanGenerationInput<f64>) -> Vec<domain::CuttingPlan<f64>> { self.generate_with_report(input).plans }

    fn generate_with_report(&self, input: &domain::CuttingPlanGenerationInput<f64>) -> domain::CuttingPlanGenerationReport<f64> {
        let remaining = self.pool_limit.saturating_sub(input.existing_plans.len()).min(self.output_limit);
        if self.cancellation.is_cancelled() || self.started.elapsed() >= self.time_limit || remaining == 0 {
            return domain::CuttingPlanGenerationReport { plans: Vec::new(), statistics: domain::CuttingPlanGenerationStatistics {
                stop_reason: if self.started.elapsed() >= self.time_limit { domain::CuttingPlanGenerationStopReason::Timeout }
                    else if remaining == 0 { domain::CuttingPlanGenerationStopReason::MaxPlans } else { domain::CuttingPlanGenerationStopReason::Exhausted }, ..Default::default()
            } };
        }
        let mut report = self.delegate.generate_with_report(input);
        let mut ids = input.existing_plans.iter().map(|plan| plan.id.clone()).collect::<BTreeSet<_>>();
        let mut keys = input.existing_plans.iter().map(|plan| plan.canonical_key()).collect::<BTreeSet<_>>();
        report.plans.retain(|plan| ids.insert(plan.id.clone()) && keys.insert(plan.canonical_key()));
        if report.plans.len() > remaining { report.plans.truncate(remaining); report.statistics.stop_reason = domain::CuttingPlanGenerationStopReason::MaxPlans; }
        report.statistics.accepted_plans = report.plans.len() as i64;
        report
    }
}

fn execute(
    source: &ModelArtifact,
    configuration: Csp1dOptions,
    solver: Arc<dyn LinearSolver>,
    options: &SolveOptions<'_>,
) -> Result<String> {
    let started = Instant::now();
    let config = configuration.config()?;
    let time_limit = options.time_limit.unwrap_or(Duration::from_secs(60));
    let cancellation = options.cancellation_handle.ok_or_else(|| BridgeError::new("ExecutionFailure", "missing CSP1D cancellation handle"))?.clone();
    let backend = Arc::new(domain::Csp1dNativeBackend::new(solver, domain::Csp1dExecutionOptions {
        time_limit, cancellation: cancellation.clone(), progress: options.progress_reporter.cloned(), report_limit: 3 * config.iteration_limit as usize + 1, ..Default::default()
    }).map_err(|error| BridgeError::new(error.solver_error_class().code(), error))?);
    // 领域对象和非 Send 注册只在 worker 内构造；不是把现成 MetaModel 移进线程。
    // Construct domain objects and non-Send registration inside the worker, never move an existing MetaModel into it.
    let (_, problem) = decode_csp1d_problem(source)?;
    let generation = configuration.generation.clone().unwrap_or_default();
    let pool_limit = configuration.pool_limit.unwrap_or(512) as usize;
    let initial = BoundedGenerator { delegate: generation.generator()?, pool_limit, output_limit: config.max_initial_plans as usize,
        cancellation: cancellation.clone(), started, time_limit };
    let solve_config = Some(domain::Csp1dSolveConfig { column_generation: config, ..Default::default() });
    let result = match configuration.algorithm {
        Csp1dAlgorithm::Milp => domain::Csp1dMilp { initial_generator: Box::new(initial), milp_solver: Some(backend.clone()), ..Default::default() }
            .solve_with_trace(problem, solve_config),
        Csp1dAlgorithm::ColumnGeneration => {
            let mut pricing_options = generation; pricing_options.max_plans = Some(pool_limit as u32);
            let pricing = BoundedGenerator { delegate: pricing_options.generator()?, pool_limit, output_limit: pool_limit,
                cancellation: cancellation.clone(), started, time_limit };
            domain::Csp1dColumnGeneration { initial_generator: Box::new(initial), pricing_generator: Box::new(domain::ReducedCostPricingGenerator::new(pricing)),
                lp_solver: Some(backend.clone()), milp_solver: Some(backend.clone()), ..Default::default() }.solve_with_trace(problem, solve_config)
        }
    };
    let records = backend.reports();
    // 已完成子求解赢得竞争时保留完成；最后取消/限时报告则保留其真实语义。
    // Preserve completion when the completed child won the race; otherwise retain the last child's actual cancellation/timeout semantics.
    let stop = match records.last().map(|record| record.report.termination_reason) {
        Some(TerminationReason::Cancelled | TerminationReason::Interrupted) => Csp1dExecutionStop::Cancelled,
        Some(TerminationReason::TimeLimit) => Csp1dExecutionStop::TimeLimit,
        Some(_) => Csp1dExecutionStop::Completed,
        None if cancellation.is_cancelled() => Csp1dExecutionStop::Cancelled,
        None if started.elapsed() >= time_limit => Csp1dExecutionStop::TimeLimit,
        None => Csp1dExecutionStop::Completed,
    };
    encode_csp1d_execution_report(source, configuration, &result, &records, stop)
}

impl NativeEngine {
    pub(crate) fn start_csp1d(&self, request: SolveRequest) -> Result<JobView> {
        let configuration = request.csp1d_options.clone().ok_or_else(|| BridgeError::new("InvalidInput", "missing CSP1D configuration"))?;
        configuration.config()?;
        decode_csp1d_problem(&request.model)?;
        let solver = create_solver(configuration.backend.id(), None)?;
        #[cfg(feature = "scip")]
        let solver: Arc<dyn ospf_rust_core::solver::Solver> = if configuration.backend == ospf_rust_bridge::NativeBackend::Scip {
            use ospf_rust_core::solver::solvers::scip::{SCIPConfig, SCIPSolver};
            Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults()))
        } else { solver };
        let solver: Arc<dyn LinearSolver> = solver;
        let source = request.model.clone();
        self.start_encoded(request, move |options| execute(&source, configuration, solver, options))
    }
}

#[cfg(test)]
mod tests;
