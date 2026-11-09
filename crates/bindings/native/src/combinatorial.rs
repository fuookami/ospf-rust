//! 复用真实 Rust framework 组合编排 / Reuse actual Rust framework combinatorial orchestration.

use std::sync::Arc;
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{CombinatorialSolveReport, LinearSolver as CoreLinearSolver, SolveReport, linear_model_fingerprint};
use ospf_rust_framework::solver::column_generation_solver::{CombinatorialFallbackPolicy, FeasibleSolution};
use ospf_rust_framework::solver::parallel_combinatorial_linear_solver::{LinearSolver, ParallelCombinatorialLinearSolver};
use ospf_rust_framework::solver::serial_combinatorial_linear_solver::SerialCombinatorialLinearSolver;
use ospf_rust_framework::solver::{FrameworkSolveOptions, ParallelCombinatorialMode};
use ospf_rust_bridge::{BridgeError, JobView, Result, SerialFallbackPolicy, SolveRequest, decode_combinatorial_report, decode_model};
use crate::{NativeEngine, create_solver};

struct BackendAdapter { id: String, solver: Arc<dyn CoreLinearSolver> }

impl LinearSolver for BackendAdapter {
    fn name(&self) -> &str { &self.id }

    fn solve(&self, model: &LinearTriadModel) -> ospf_rust_core::Result<FeasibleSolution> {
        let report = self.solver.solve_linear_report_with_options(model, &Default::default())?;
        FeasibleSolution::try_from_report(&report)
    }

    fn solve_report_with_options(&self, model: &LinearTriadModel, options: FrameworkSolveOptions) -> ospf_rust_core::Result<SolveReport<f64>> {
        self.solver.solve_linear_report_with_options(model, &options.to_core_solve_options(None))
    }
}

fn encode_report(mut aggregate: CombinatorialSolveReport<f64>, model: &LinearTriadModel) -> Result<String> {
    // 无 incumbent 的预取消或全失败聚合可能没有指纹；仅在已认证执行边界绑定源身份。
    // Pre-cancelled or all-failed aggregates without incumbents may lack a fingerprint; bind source identity only at this authenticated execution boundary.
    if aggregate.report.fingerprints.model.is_none() && aggregate.report.solution.is_none() {
        aggregate.report.fingerprints.model = Some(linear_model_fingerprint(model).map_err(|err| BridgeError::new("InvalidInput", err))?);
    }
    let json = serde_json::to_string(&aggregate).map_err(|err| BridgeError::new("CodecFailure", err))?;
    decode_combinatorial_report(&json, model)?;
    Ok(json)
}

impl NativeEngine {
    pub(crate) fn start_parallel(&self, request: SolveRequest) -> Result<JobView> {
        let configuration = request.parallel_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "missing parallel options"))?;
        configuration.validate()?;
        let solvers = configuration.backends.iter().map(|backend| {
            let solver = create_solver(backend.id(), None)?;
            Ok(Arc::new(BackendAdapter { id: backend.id().into(), solver }) as Arc<dyn LinearSolver>)
        }).collect::<Result<Vec<_>>>()?;
        let mode = match configuration.mode.unwrap_or_default() {
            ospf_rust_bridge::ParallelCombinatorialMode::First => ParallelCombinatorialMode::First,
            ospf_rust_bridge::ParallelCombinatorialMode::Best => ParallelCombinatorialMode::Best,
        };
        let model = decode_model(&request.model)?;
        let solver = ParallelCombinatorialLinearSolver::new(solvers, mode);
        self.start_encoded(request, move |options| {
            let aggregate = solver.solve_combinatorial_report_with_options(&model, FrameworkSolveOptions {
                time_limit: options.time_limit,
                cancellation_handle: options.cancellation_handle.cloned(),
                progress_reporter: options.progress_reporter.cloned(),
                ..Default::default()
            }).map_err(|err| BridgeError::new(err.solver_error_class().code(), err))?;
            encode_report(aggregate, &model)
        })
    }

    pub(crate) fn start_serial(&self, request: SolveRequest) -> Result<JobView> {
        let configuration = request.serial_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "missing serial options"))?;
        configuration.validate()?;
        let solvers = configuration.backends.iter().map(|backend| {
            let solver = create_solver(backend.id(), None)?;
            Ok(Arc::new(BackendAdapter { id: backend.id().into(), solver }) as Arc<dyn LinearSolver>)
        }).collect::<Result<Vec<_>>>()?;
        let fallback_policy = match configuration.fallback_policy.unwrap_or_default() {
            SerialFallbackPolicy::StopOnCertifiedTerminal => CombinatorialFallbackPolicy::StopOnCertifiedTerminal,
            SerialFallbackPolicy::ContinueOnTerminal => CombinatorialFallbackPolicy::ContinueOnTerminal,
        };
        let model = decode_model(&request.model)?;
        let solver = SerialCombinatorialLinearSolver::new(solvers);
        self.start_encoded(request, move |options| {
            let aggregate = solver.solve_combinatorial_report_with_options(&model, FrameworkSolveOptions {
                time_limit: options.time_limit,
                cancellation_handle: options.cancellation_handle.cloned(),
                progress_reporter: options.progress_reporter.cloned(), fallback_policy,
                ..Default::default()
            }).map_err(|err| BridgeError::new(err.solver_error_class().code(), err))?;
            encode_report(aggregate, &model)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::error::{CoreError, SolverError};
    use ospf_rust_core::solver::{ProblemStatus, SolverCapability, SolverInfo, SolverOutput, TerminationReason};

    struct Fake { fail: bool }
    impl SolverInfo for Fake {
        fn name(&self) -> &str { if self.fail { "fake-failure" } else { "fake-report" } }
        fn capabilities(&self) -> Vec<SolverCapability> { vec![] }
    }
    impl ospf_rust_core::solver::LinearSolver for Fake {
        fn solve_linear(&self, _: &LinearTriadModel) -> ospf_rust_core::Result<SolverOutput> { unreachable!("report path required") }
        fn solve_linear_report_with_options(&self, _: &LinearTriadModel, _: &ospf_rust_core::solver::SolveOptions<'_>) -> ospf_rust_core::Result<SolveReport<f64>> {
            if self.fail { return Err(CoreError::Solver(SolverError::LicenseError("fake missing license".into()))); }
            SolveReport::builder(ProblemStatus::Unknown, TerminationReason::NodeLimit).build()
        }
    }

    #[test]
    fn actual_framework_preserves_failed_attempts_and_authenticated_model_identity() {
        let model = LinearTriadModel::new("serial-contract");
        let adapters: Vec<Arc<dyn LinearSolver>> = [true, false].into_iter().map(|fail| {
            Arc::new(BackendAdapter { id: if fail { "first".into() } else { "second".into() }, solver: Arc::new(Fake { fail }) }) as Arc<dyn LinearSolver>
        }).collect();
        let solver = SerialCombinatorialLinearSolver::new(adapters);
        let aggregate = solver.solve_combinatorial_report_with_options(&model, Default::default()).unwrap();
        assert_eq!(aggregate.attempts.len(), 2);
        assert!(aggregate.attempts[0].error.is_some());
        assert_eq!(aggregate.report.termination_reason, TerminationReason::BackendFailure);
        let json = encode_report(aggregate, &model).unwrap();
        assert_eq!(decode_combinatorial_report(&json, &model).unwrap().attempts.len(), 2);
    }

    #[test]
    fn parallel_adapter_preserves_failed_attempts_in_both_modes() {
        let model = LinearTriadModel::new("parallel-failure-contract");
        for mode in [ParallelCombinatorialMode::First, ParallelCombinatorialMode::Best] {
            let adapters: Vec<Arc<dyn LinearSolver>> = [true, false].into_iter().map(|fail| {
                Arc::new(BackendAdapter { id: if fail { "first".into() } else { "second".into() }, solver: Arc::new(Fake { fail }) }) as Arc<dyn LinearSolver>
            }).collect();
            let solver = ParallelCombinatorialLinearSolver::new(adapters, mode);
            let aggregate = solver.solve_combinatorial_report_with_options(&model, Default::default()).unwrap();
            assert_eq!(aggregate.attempts.len(), 2);
            assert!(aggregate.attempts.iter().any(|attempt| attempt.error.is_some()));
            assert_eq!(aggregate.report.termination_reason, TerminationReason::BackendFailure);
            encode_report(aggregate, &model).unwrap();
        }
    }

    #[test]
    fn parallel_first_cancels_and_joins_loser_without_cancelling_parent() {
        use std::sync::{Barrier, atomic::{AtomicBool, Ordering}};
        use ospf_rust_core::solver::{CancellationOrigin, SolveHandle, SolveSolution, SolverProvenance, cancelled_solve_report};
        struct Worker { winner: bool, barrier: Arc<Barrier>, finished: Arc<AtomicBool> }
        impl SolverInfo for Worker {
            fn name(&self) -> &str { if self.winner { "winner" } else { "loser" } }
            fn capabilities(&self) -> Vec<SolverCapability> { vec![] }
        }
        impl CoreLinearSolver for Worker {
            fn solve_linear(&self, _: &LinearTriadModel) -> ospf_rust_core::Result<SolverOutput> { unreachable!("report path required") }
            fn solve_linear_report_with_options(&self, model: &LinearTriadModel, options: &ospf_rust_core::solver::SolveOptions<'_>) -> ospf_rust_core::Result<SolveReport<f64>> {
                self.barrier.wait();
                if self.winner {
                    return SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
                        .solution(SolveSolution::vector(vec![]))
                        .fingerprints(ospf_rust_core::solver::SolveFingerprints { model: Some(linear_model_fingerprint(model).unwrap()), ..Default::default() }).build();
                }
                let handle = options.cancellation_handle.unwrap();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while !handle.is_cancelled() {
                    assert!(std::time::Instant::now() < deadline, "loser must receive framework cancellation");
                    std::thread::yield_now();
                }
                assert_eq!(handle.cancellation().unwrap().origin, CancellationOrigin::FrameworkLoser);
                self.finished.store(true, Ordering::SeqCst);
                cancelled_solve_report(SolverProvenance::default(), handle)
            }
        }
        let parent = SolveHandle::new();
        let barrier = Arc::new(Barrier::new(2));
        let finished = Arc::new(AtomicBool::new(false));
        let adapters = [false, true].into_iter().map(|winner| {
            let solver = Arc::new(Worker { winner, barrier: barrier.clone(), finished: finished.clone() });
            Arc::new(BackendAdapter { id: solver.name().into(), solver }) as Arc<dyn LinearSolver>
        }).collect();
        let model = LinearTriadModel::new("parallel-first-contract");
        let aggregate = ParallelCombinatorialLinearSolver::new(adapters, ParallelCombinatorialMode::First)
            .solve_combinatorial_report_with_options(&model, FrameworkSolveOptions::default().with_cancellation_handle(Some(parent.clone()))).unwrap();
        assert!(finished.load(Ordering::SeqCst));
        assert!(!parent.is_cancelled());
        assert_eq!(aggregate.attempts.len(), 2);
        assert!(aggregate.selected_attempt_id.as_ref().unwrap().contains("winner"));
        assert!(aggregate.attempts.iter().any(|attempt| attempt.cancellation_reason.is_some()));
        encode_report(aggregate, &model).unwrap();
    }

    #[test]
    fn parallel_best_uses_model_direction_and_precancellation_starts_no_workers() {
        use ospf_rust_core::solver::{CancellationOrigin, SolveHandle, SolveSolution};
        struct Valued { value: f64, must_not_run: bool }
        impl CoreLinearSolver for Valued {
            fn solve_linear(&self, _: &LinearTriadModel) -> ospf_rust_core::Result<SolverOutput> { unreachable!("report path required") }
            fn solve_linear_report_with_options(&self, model: &LinearTriadModel, _: &ospf_rust_core::solver::SolveOptions<'_>) -> ospf_rust_core::Result<SolveReport<f64>> {
                assert!(!self.must_not_run, "pre-cancellation must not start workers");
                SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
                    .solution(SolveSolution { objective: Some(self.value), objective_value: Some(self.value), ..SolveSolution::vector(vec![]) })
                    .fingerprints(ospf_rust_core::solver::SolveFingerprints { model: Some(linear_model_fingerprint(model).unwrap()), ..Default::default() }).build()
            }
        }
        impl SolverInfo for Valued {
            fn name(&self) -> &str { "valued-fake" }
            fn capabilities(&self) -> Vec<SolverCapability> { vec![] }
        }
        for maximize in [false, true] {
            let mut model = LinearTriadModel::new("parallel-best-contract");
            if maximize { model.objective_category = ospf_rust_core::model::ObjectiveCategory::Maximum; }
            let adapters = [1.0, 2.0].into_iter().map(|value| {
                Arc::new(BackendAdapter { id: value.to_string(), solver: Arc::new(Valued { value, must_not_run: false }) }) as Arc<dyn LinearSolver>
            }).collect();
            let aggregate = ParallelCombinatorialLinearSolver::with_solvers(adapters)
                .solve_combinatorial_report_with_options(&model, Default::default()).unwrap();
            assert_eq!(aggregate.report.solution.as_ref().unwrap().objective_value, Some(if maximize { 2.0 } else { 1.0 }));
            encode_report(aggregate, &model).unwrap();
        }
        let handle = SolveHandle::new(); handle.cancel(CancellationOrigin::User);
        let solver = ParallelCombinatorialLinearSolver::with_solvers(vec![Arc::new(BackendAdapter { id: "not-started".into(), solver: Arc::new(Valued { value: 1.0, must_not_run: true }) })]);
        let model = LinearTriadModel::new("parallel-precancel-contract");
        let aggregate = solver.solve_combinatorial_report_with_options(&model, FrameworkSolveOptions::default().with_cancellation_handle(Some(handle))).unwrap();
        assert_eq!(aggregate.report.termination_reason, TerminationReason::Cancelled);
        encode_report(aggregate, &model).unwrap();
    }

    #[test]
    fn actual_framework_preserves_timeout_and_precancellation_without_starting_later_backends() {
        struct Timed { must_not_run: bool }
        impl LinearSolver for Timed {
            fn name(&self) -> &str { if self.must_not_run { "must-not-run" } else { "timed" } }
            fn solve(&self, _: &LinearTriadModel) -> ospf_rust_core::Result<FeasibleSolution> { unreachable!("report path required") }
            fn solve_report_with_options(&self, _: &LinearTriadModel, _: FrameworkSolveOptions) -> ospf_rust_core::Result<SolveReport<f64>> {
                assert!(!self.must_not_run, "control terminals must stop the serial chain");
                SolveReport::builder(ProblemStatus::Unknown, TerminationReason::TimeLimit).build()
            }
        }
        let model = LinearTriadModel::new("serial-timeout");
        for policy in [CombinatorialFallbackPolicy::StopOnCertifiedTerminal, CombinatorialFallbackPolicy::ContinueOnTerminal] {
            let solver = SerialCombinatorialLinearSolver::new(vec![Arc::new(Timed { must_not_run: false }), Arc::new(Timed { must_not_run: true })]);
            let aggregate = solver.solve_combinatorial_report_with_options(&model, FrameworkSolveOptions::default().with_fallback_policy(policy)).unwrap();
            assert_eq!(aggregate.attempts.len(), 1);
            assert_eq!(aggregate.report.termination_reason, TerminationReason::TimeLimit);
            assert!(aggregate.selected_attempt_id.is_some());
            encode_report(aggregate, &model).unwrap();
        }
        let handle = ospf_rust_core::solver::SolveHandle::new();
        handle.cancel(ospf_rust_core::solver::CancellationOrigin::User);
        let solver = SerialCombinatorialLinearSolver::new(vec![Arc::new(Timed { must_not_run: true })]);
        let aggregate = solver.solve_combinatorial_report_with_options(&model, FrameworkSolveOptions::default().with_cancellation_handle(Some(handle))).unwrap();
        assert_eq!(aggregate.report.termination_reason, TerminationReason::Cancelled);
        encode_report(aggregate, &model).unwrap();
    }
}
