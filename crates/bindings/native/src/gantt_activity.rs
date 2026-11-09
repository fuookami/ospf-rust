//! Gantt 活动调度单 MILP 原生执行，保留原报告并独立复验方案。 / Native single-MILP Gantt activity execution retaining the original report and revalidating placements independently.

use std::time::{Duration, Instant};
use ospf_rust_core::solver::{ProblemStatus, TerminationReason};
use ospf_rust_framework::solver::FrameworkSolveOptions;
use ospf_rust_framework_gantt_scheduling::{
    application::service::ActivitySchedulingApplicationService,
    domain::activity::{ActivityChoiceKey, ActivityPreparation, ActivitySchedulingContext},
    domain::common::TaskId,
    domain::task::F64SolverValueAdapter,
};
use ospf_rust_bridge::{
    BridgeError, GanttActivityExecutionReport, GanttActivityOptions, JobView, ModelArtifact, Result, SolveRequest,
    decode_gantt_activity_problem, encode_gantt_activity_execution_report, gantt_activity_schedule,
};
use crate::NativeEngine;

/// 报告状态文本，直接取自原枚举而不是自造描述。 / Report status text taken from the original enum rather than invented descriptions.
fn status_text(status: ProblemStatus) -> String {
    serde_json::to_value(status).ok().and_then(|value| value.as_str().map(str::to_owned)).unwrap_or_default()
}

/// 终止原因文本，直接取自原枚举。 / Termination text taken from the original enum.
fn reason_text(reason: TerminationReason) -> String {
    serde_json::to_value(reason).ok().and_then(|value| value.as_str().map(str::to_owned)).unwrap_or_default()
}

/// 逐后端构造原框架求解器，不在求解器选择上做隐式回退。 / Construct the original framework solver per backend without implicit solver fallback.
macro_rules! execute_with {
    ($solver:expr, $source:expr, $configuration:expr, $options:expr) => {{
        let started = Instant::now();
        let time_limit = $options.time_limit.unwrap_or(Duration::from_secs(60));
        let cancellation = $options.cancellation_handle.ok_or_else(|| BridgeError::new("ExecutionFailure", "missing Gantt activity cancellation handle"))?.clone();
        let progress = $options.progress_reporter.cloned();
        let (_, problem) = decode_gantt_activity_problem($source)?;
        let solver = $solver;
        let adapter = F64SolverValueAdapter;
        let (_, revalidate_problem) = decode_gantt_activity_problem($source)?;
        let report = ActivitySchedulingApplicationService::new(solver)
            .solve(move || ActivitySchedulingContext::new(problem, &adapter, None, Default::default()), FrameworkSolveOptions {
                time_limit: Some(time_limit.saturating_sub(started.elapsed())),
                cancellation_handle: Some(cancellation), progress_reporter: progress, ..Default::default()
            })
            .map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
        // incumbent 只有通过原域独立复验才交付分配，否则保留报告与失败原因。 / Deliver placements only after native revalidation, otherwise retain the report and the failure reason.
        let preparation = ActivityPreparation::new(revalidate_problem, &F64SolverValueAdapter, None)
            .map_err(|error| BridgeError::new("ExecutionFailure", &error))?;
        let mut failure = None;
        let mut schedule = None;
        if let Some(solution) = report.solution.as_ref().and_then(|solution| solution.value.as_ref()) {
            let keys: Vec<ActivityChoiceKey> = solution.placements.iter()
                .map(|placement| ActivityChoiceKey { task: TaskId::from(placement.key.task.as_str()), mode: placement.key.mode.clone(), start: placement.key.start })
                .collect();
            match preparation.validate(&keys) {
                Ok(validation) if validation.feasible() => {
                    let (source_problem, _) = decode_gantt_activity_problem($source)?;
                    schedule = Some(gantt_activity_schedule(&source_problem, solution)?);
                }
                Ok(validation) => failure = Some(format!("Gantt activity incumbent fails native validation: {:?}", validation.violations)),
                Err(error) => failure = Some(format!("Gantt activity incumbent revalidation failed: {error}")),
            }
        }
        encode_gantt_activity_execution_report(&GanttActivityExecutionReport {
            source_fingerprint: $source.fingerprint.clone(), problem_status: status_text(report.problem_status),
            termination_reason: reason_text(report.termination_reason),
            objective_value: report.solution.as_ref().and_then(|solution| solution.objective_value),
            solver_model_only: preparation.solver_model_only(), schedule, failure,
        })
    }};
}

fn execute(source: &ModelArtifact, configuration: GanttActivityOptions, options: &ospf_rust_core::solver::SolveOptions<'_>) -> Result<String> {
    match configuration.backend {
        #[cfg(feature = "scip")]
        ospf_rust_bridge::NativeBackend::Scip => {
            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
            use ospf_rust_core_plugin_scip::extension::ScipColumnGenerationSolver;
            execute_with!(ScipColumnGenerationSolver::with_solver(SCIPSolver::with_config(SCIPConfig::default().with_output(false))), source, configuration, options)
        }
        #[cfg(any(feature = "gurobi10", feature = "gurobi11", feature = "gurobi12"))]
        ospf_rust_bridge::NativeBackend::Gurobi => {
            use ospf_rust_core_plugin_gurobi::{GurobiConfig, GurobiSolver};
            use ospf_rust_core_plugin_gurobi::extension::GurobiColumnGenerationSolver;
            execute_with!(GurobiColumnGenerationSolver::with_solver(GurobiSolver::with_config(GurobiConfig::new().with_output(false))), source, configuration, options)
        }
        backend => Err(BridgeError::new("BackendNotCompiled", &format!("{} is not compiled into the native host", backend.id()))),
    }
}

impl NativeEngine {
    pub(crate) fn start_gantt_activity(&self, request: SolveRequest) -> Result<JobView> {
        let configuration = request.gantt_activity_options.clone().ok_or_else(|| BridgeError::new("InvalidInput", "missing Gantt activity configuration"))?;
        decode_gantt_activity_problem(&request.model)?;
        let source = request.model.clone();
        self.start_encoded(request, move |options| execute(&source, configuration, options))
    }
}

#[cfg(test)]
mod tests;
