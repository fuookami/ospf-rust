//! 原分区列生成共享原生执行；记录有界预算观察，供同一 service 无时钟复验。
//! Shared native execution of original partition CG, recording bounded budget observations for clock-free revalidation by the same service.

use std::cell::RefCell;
use std::time::{Duration, Instant};
use ospf_rust_core::solver::{SolveHandle, SolveOptions, TerminationReason};
use ospf_rust_framework_bpp::application::{PartitionColumnGenerationApplicationService, PartitionColumnGenerationControl,
    DiscreteLoadingApplicationService, DiscreteLoadingCertificationService, DiscreteLoadingCertificationResult,
    DiscreteLoadingSolveOptions, DiscreteLoadingSolveResult};
use ospf_rust_bridge::{BppLoadingContext, BppLoadingPartitionCall, BppLoadingPartitionEnvelope, BppLoadingPartitionObservation,
    BridgeError, JobView, Result, SolveRequest, encode_bpp_loading_partition, encode_bpp_loading_partition_certification, encode_compiled_model};
use crate::{NativeEngine, create_solver};
use crate::linear_identity::bind_cancelled_identity;

struct Control<'a> {
    started: Instant,
    limit: Duration,
    handle: &'a SolveHandle,
    observations: RefCell<Vec<BppLoadingPartitionObservation>>,
}

impl Control<'_> {
    fn observe(&self, terminal_check: bool) -> ospf_rust_core::Result<(Duration, bool)> {
        let mut observations = self.observations.borrow_mut();
        if observations.len() >= 16384 { return Err(ospf_rust_core::error::CoreError::NotImplemented("partition observation budget exhausted".into())); }
        let remaining = self.limit.saturating_sub(self.started.elapsed()); let cancelled = self.handle.is_cancelled();
        observations.push(BppLoadingPartitionObservation { terminal_check, remaining_nanos: remaining.as_nanos().to_string(), cancelled });
        Ok((remaining, cancelled))
    }
}

impl PartitionColumnGenerationControl for Control<'_> {
    fn remaining(&self) -> ospf_rust_core::Result<Duration> { Ok(self.observe(false)?.0) }
    fn terminal(&self) -> ospf_rust_core::Result<Option<TerminationReason>> {
        let (remaining, cancelled) = self.observe(true)?;
        Ok(if cancelled { Some(TerminationReason::Cancelled) } else if remaining.is_zero() { Some(TerminationReason::TimeLimit) } else { None })
    }
}

impl NativeEngine {
    pub(crate) fn start_bpp_partition(&self, request: SolveRequest) -> Result<JobView> {
        let configuration = request.bpp_loading_options.as_ref().ok_or_else(|| BridgeError::new("InvalidInput", "missing BPP loading configuration"))?;
        let settings = configuration.partition.clone().ok_or_else(|| BridgeError::new("InvalidInput", "missing BPP partition configuration"))?;
        let context = BppLoadingContext::from_artifact(&request.model)?; settings.validate(&context)?;
        let solver = create_solver(configuration.backend.id(), None)?;
        #[cfg(feature = "scip")]
        let solver: std::sync::Arc<dyn ospf_rust_core::solver::Solver> = if configuration.backend == ospf_rust_bridge::NativeBackend::Scip {
            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
            std::sync::Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults()))
        } else { solver };
        let source = request.model.clone(); let time_limit_ms = request.time_limit_ms.unwrap_or(60_000);
        let accelerator_budget = settings.accelerator_budget(time_limit_ms)?;
        self.start_encoded(request, move |options| {
            let handle = options.cancellation_handle.ok_or_else(|| BridgeError::new("ExecutionFailure", "missing partition cancellation handle"))?;
            let started = Instant::now(); let total = Duration::from_millis(u64::from(time_limit_ms));
            let control = Control { started, limit: accelerator_budget, handle, observations: Default::default() };
            let context = BppLoadingContext::from_artifact(&source)?; let domain = settings.partition.domain(&context)?;
            let registration = context.into_registration().map_err(|error| BridgeError::new(error.solver_error_class().code(), error))?;
            let mut calls = Vec::new(); let mut bytes = 0usize;
            let result = PartitionColumnGenerationApplicationService::solve_with_control(&registration, domain, settings.config(accelerator_budget.as_millis() as u32), handle, &control,
                |model, remaining, handle| {
                    let artifact = encode_compiled_model(model).map_err(|error| ospf_rust_core::SolverError::ContractViolation(error.to_string()))?;
                    let outcome = if calls.len() >= settings.report_limit.unwrap_or(256) as usize || bytes.saturating_add(artifact.model_json.len()) > 32 * 1024 * 1024 {
                        Err(ospf_rust_core::error::CoreError::NotImplemented("partition child report count/text budget exhausted".into()))
                    } else {
                        solver.solve_linear_report_with_options(model, &SolveOptions { time_limit: Some(remaining), cancellation_handle: Some(handle),
                            progress_reporter: options.progress_reporter, ..Default::default() }).and_then(|mut report| { bind_cancelled_identity(&mut report, model)?; Ok(report) })
                    };
                    let report_json = outcome.as_ref().ok().map(serde_json::to_string).transpose().map_err(|error| ospf_rust_core::SolverError::ContractViolation(error.to_string()))?;
                    if report_json.as_ref().is_some_and(|text| text.len() > 8 * 1024 * 1024 || bytes.saturating_add(artifact.model_json.len()).saturating_add(text.len()) > 32 * 1024 * 1024) {
                        return Err(ospf_rust_core::SolverError::ContractViolation("partition child report exceeds text budget".into()).into());
                    }
                    bytes = bytes.saturating_add(artifact.model_json.len()).saturating_add(report_json.as_ref().map_or(0, String::len));
                    calls.push(BppLoadingPartitionCall { model: artifact, report_json, error: outcome.as_ref().err().map(|error| BridgeError::new(error.solver_error_class().code(), error)) }); outcome
                }).map_err(|error| BridgeError::new(error.solver_error_class().code(), error))?;
            let acceleration = encode_bpp_loading_partition(&source, BppLoadingPartitionEnvelope { schema_version: "1.0".into(), source_fingerprint: source.fingerprint.clone(),
                options: settings.clone(), time_limit_ms, observations: control.observations.into_inner(), calls, application_report_json: String::new(), certification: None }, &result)?;
            if settings.source_milp_finishing.is_none() { return Ok(acceleration); }
            let bound = result.report.fingerprints.model.is_some();
            let previous = DiscreteLoadingSolveResult { report: result.report, registration: bound.then_some(registration), solution: result.solution, objective: result.objective };
            let remaining = total.saturating_sub(started.elapsed());
            let factory = || BppLoadingContext::from_artifact(&source)
                .map_err(|error| ospf_rust_core::SolverError::ContractViolation(error.to_string()))?.into_registration();
            let solve = |model: &ospf_rust_core::model::intermediate::LinearTriadModel, remaining, handle: &SolveHandle| {
                let mut report = solver.solve_linear_report_with_options(model, &SolveOptions { time_limit: Some(remaining), cancellation_handle: Some(handle),
                    progress_reporter: options.progress_reporter, ..Default::default() })?;
                bind_cancelled_identity(&mut report, model)?; Ok(report)
            };
            // 只有实际源身份才能进入 certification；根前置停止不伪造旧注册。
            // Only an actual source identity enters certification; a root pre-solve stop never fabricates a previous registration.
            let finished = if previous.registration.is_some() {
                DiscreteLoadingCertificationService::solve(previous, factory, DiscreteLoadingSolveOptions { time_limit: remaining, ..Default::default() }, handle, solve)
            } else if !handle.is_cancelled() && !remaining.is_zero() {
                DiscreteLoadingApplicationService::solve(factory, DiscreteLoadingSolveOptions { time_limit: remaining, ..Default::default() }, handle, solve)
                    .map(|result| DiscreteLoadingCertificationResult { certification_report: result.registration.as_ref().map(|_| result.report.clone()), result, certification_error: None })
            } else {
                let mut previous = previous;
                if handle.is_cancelled() { previous.report.termination_reason = TerminationReason::Cancelled; }
                Ok(DiscreteLoadingCertificationResult { result: previous, certification_report: None, certification_error: None })
            }
                .map_err(|error| BridgeError::new(error.solver_error_class().code(), error))?;
            encode_bpp_loading_partition_certification(&acceleration, &finished, &source)
        })
    }
}
