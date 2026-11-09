//! 一维分切可移植建模与校验；不在 WASM 执行 LP/MILP。
//! Portable cutting-stock modeling and validation, without LP/MILP execution in WASM.

use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use ospf_rust_bridge::*;
use crate::{bridge_error, error};

fn parse<T: DeserializeOwned>(text: &str) -> std::result::Result<T, JsValue> {
    if text.len() > 8 * 1024 * 1024 { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "CSP1D input exceeds 8 MiB"))); }
    serde_json::from_str(text).map_err(error)
}

/// 独立源问题，单位、生成及约束校验全部委托 Rust。 / Independent source problem, delegating units, generation and constraint checks to Rust.
#[wasm_bindgen]
pub struct Csp1dCuttingContext { problem: Csp1dProblem, artifact: ModelArtifact }

#[wasm_bindgen]
impl Csp1dCuttingContext {
    /// 从生成契约的 DTO 构造。 / Construct from the generated-contract DTO.
    #[wasm_bindgen(constructor)]
    pub fn new(text: &str) -> std::result::Result<Csp1dCuttingContext, JsValue> {
        let artifact = encode_csp1d_problem(parse(text)?).map_err(bridge_error)?;
        let (problem, _) = decode_csp1d_problem(&artifact).map_err(bridge_error)?;
        Ok(Self { problem, artifact })
    }

    /// 复验无损源封装，不信任外部指纹。 / Revalidate the lossless source artifact, without trusting external fingerprints.
    #[wasm_bindgen(js_name = fromArtifact)]
    pub fn from_artifact(text: &str) -> std::result::Result<Csp1dCuttingContext, JsValue> {
        let artifact = parse(text)?;
        let (problem, _) = decode_csp1d_problem(&artifact).map_err(bridge_error)?;
        Ok(Self { problem, artifact })
    }

    /// 完整无损源封装。 / Complete lossless source artifact.
    #[wasm_bindgen(js_name = toArtifactJson)]
    pub fn to_artifact_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.artifact).map_err(error) }

    /// 复用受限原领域生成器，不声明源问题全局完备。 / Reuse a restricted original-domain generator, without claiming source-wide completeness.
    #[wasm_bindgen(js_name = generatePatterns)]
    pub fn generate_patterns(&self, text: &str) -> std::result::Result<Csp1dPatternGenerationResult, JsValue> {
        Ok(Csp1dPatternGenerationResult { value: generate_csp1d_patterns(&self.problem, &parse(text)?).map_err(bridge_error)? })
    }

    /// 检查源引用、刀数、实际单位、幅宽和设备。 / Check source references, knife count, actual units, width and machine.
    #[wasm_bindgen(js_name = validatePlan)]
    pub fn validate_plan(&self, text: &str) -> std::result::Result<(), JsValue> {
        decode_csp1d_pattern(&self.problem.domain().map_err(bridge_error)?, &parse(text)?, self.problem.length_unit).map_err(bridge_error)?;
        Ok(())
    }

    /// 复验需求、物料、设备及完整源模型，不逐件展开。 / Revalidate demand, materials, machines and the full source model, without piece expansion.
    #[wasm_bindgen(js_name = verifyUsages)]
    pub fn verify_usages(&self, text: &str) -> std::result::Result<Csp1dProductionResult, JsValue> {
        Ok(Csp1dProductionResult { value: verify_csp1d_usages(&self.problem, &parse::<Vec<Csp1dUsage>>(text)?).map_err(bridge_error)? })
    }

    /// 将实际整数子报告绑定到本源问题，生产量只能来自其真实方案列。 / Bind an actual integer child report to this source problem, deriving production only from its actual plan columns.
    #[wasm_bindgen(js_name = readProductionReport)]
    pub fn read_production_report(&self, text: &str) -> std::result::Result<Csp1dProductionReportResult, JsValue> {
        Ok(Csp1dProductionReportResult { value: decode_csp1d_production_report(text, &self.artifact).map_err(bridge_error)?, text: text.into() })
    }

    /// 读取完整原生流程，保留阶段和原 trace 文本。 / Read a complete native workflow, retaining stages and original trace text.
    #[wasm_bindgen(js_name = readExecutionReport)]
    pub fn read_execution_report(&self, text: &str) -> std::result::Result<Csp1dExecutionReportResult, JsValue> {
        Ok(Csp1dExecutionReportResult { value: decode_csp1d_execution_report(text, &self.artifact).map_err(bridge_error)?, text: text.into() })
    }
}

/// 与 context 独立拥有的候选结果。 / Candidate result independently owned from its context.
#[wasm_bindgen]
pub struct Csp1dPatternGenerationResult { value: Csp1dPatterns }

#[wasm_bindgen]
impl Csp1dPatternGenerationResult {
    /// 精确统计计数保持文本。 / Exact statistical counters remain text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }
}

/// 独立拥有、通过源约束及精确整数复验的生产结果。 / Independently owned production revalidated against source constraints and exact integers.
#[wasm_bindgen]
pub struct Csp1dProductionResult { value: Csp1dProduction }

#[wasm_bindgen]
impl Csp1dProductionResult {
    /// 精确生产/物料/设备计数不经 JS number。 / Exact production/material/machine counts bypass JS number.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }
}

/// 已验证的源绑定整数报告；不运行求解器，也不认证来源或证明全局最优。 / Validated source-bound integer report, without running a solver, authenticating provenance or proving global optimality.
#[wasm_bindgen]
pub struct Csp1dProductionReportResult { value: Csp1dProductionReport, text: String }

#[wasm_bindgen]
impl Csp1dProductionReportResult {
    /// 原始无损封装文本。 / Original lossless artifact text.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String { self.text.clone() }

    /// Rust 生成的有限数值摘要；精确报告计数不经此入口。 / Rust-derived finite summary; exact report counters bypass this entry.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value.summary).map_err(error) }

    /// 原始子报告文本，u64 元数据不经过 JS number。 / Original child-report text, bypassing JS number for u64 metadata.
    #[wasm_bindgen(js_name = solverReportJson)]
    pub fn solver_report_json(&self) -> String { self.value.artifact.child.solver_report_json.clone() }

    /// 独立拥有的生产副本；无 incumbent 时返回 undefined。 / Independently owned production copy; undefined when no incumbent exists.
    pub fn production(&self) -> Option<Csp1dProductionResult> { self.value.production.clone().map(|value| Csp1dProductionResult { value }) }
}

/// 完整原生执行的独立报告，不在 WASM 执行求解。 / Independent complete native execution report, without solving in WASM.
#[wasm_bindgen]
pub struct Csp1dExecutionReportResult { value: Csp1dExecutionReport, text: String }

#[wasm_bindgen]
impl Csp1dExecutionReportResult {
    /// 从无损源 artifact 复验报告。 / Revalidate a report against a lossless source artifact.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(text: &str, source: &str) -> std::result::Result<Csp1dExecutionReportResult, JsValue> {
        Ok(Self { value: decode_csp1d_execution_report(text, &parse(source)?).map_err(bridge_error)?, text: text.into() })
    }

    /// 原始完整封装。 / Original complete envelope.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String { self.text.clone() }

    /// Rust 派生摘要。 / Rust-derived summary.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> std::result::Result<String, JsValue> { serde_json::to_string(&self.value.summary).map_err(error) }

    /// 原 trace canonical 文本，精确计数不经 JS number。 / Original canonical trace text, bypassing JS number for exact counters.
    #[wasm_bindgen(js_name = traceJson)]
    pub fn trace_json(&self) -> String { self.value.artifact.trace_json.clone() }

    /// 独立阶段封装，保留实际模型及原始报告。 / Independent stage envelope retaining the actual model and raw report.
    #[wasm_bindgen(js_name = stageJson)]
    pub fn stage_json(&self, index: u32) -> std::result::Result<Option<String>, JsValue> {
        self.value.artifact.stages.get(index as usize).map(serde_json::to_string).transpose().map_err(error)
    }

    /// 阶段原始报告文本。 / Raw stage-report text.
    #[wasm_bindgen(js_name = stageReportJson)]
    pub fn stage_report_json(&self, index: u32) -> Option<String> { self.value.artifact.stages.get(index as usize).map(|stage| stage.solver_report_json.clone()) }

    /// 最终整数报告的独立副本。 / Independent copy of the final integer report.
    #[wasm_bindgen(js_name = integerReport)]
    pub fn integer_report(&self) -> std::result::Result<Option<Csp1dProductionReportResult>, JsValue> {
        self.value.integer.as_ref().map(|value| Ok(Csp1dProductionReportResult {
            text: serde_json::to_string(&value.artifact).map_err(error)?, value: value.clone(),
        })).transpose()
    }

    /// 独立生产副本，前置/终态无 incumbent 时不存在。 / Independent production copy, absent on early/terminal failure without an incumbent.
    pub fn production(&self) -> Option<Csp1dProductionResult> { self.value.production.clone().map(|value| Csp1dProductionResult { value }) }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn complete_workflow_codec_keeps_trace_stages_and_independent_integer_results_on_wasm32() {
        use std::sync::{Arc, Mutex};
        use ospf_rust_core::model::MetaModel;
        use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
        use ospf_rust_framework_csp1d as domain;
        #[derive(Debug, Default)]
        struct Backend(Mutex<Vec<domain::Csp1dStageReport>>);
        impl domain::Csp1dMilpBackend for Backend {
            fn solve_milp(&self, model: &MetaModel<f64>, _: bool) -> ospf_rust_core::Result<SolveReport<f64>> {
                let compiled = model.linear_model_snapshot()?.compile_flt64()?;
                let mut values = vec![0.0; compiled.model.num_variables()]; values[2] = 2.0;
                let mut solution = SolveSolution::vector(values); solution.objective_value = Some(2.0);
                let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution)
                    .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model)?), ..Default::default() })
                    .model_mapping(Some(linear_model_mapping(&compiled.model)?)).build()?;
                self.0.lock().unwrap().push(domain::Csp1dStageReport { stage: domain::Csp1dSolveStage::Milp, model: compiled.model,
                    report: report.clone(), objective_constant: compiled.objective_constant });
                Ok(report)
            }
        }
        let context = Csp1dCuttingContext::new(r#"{"products":[{"id":"p","name":"p","width":[{"value":30,"unit":"mm"}]}],"materials":[{"id":"m","name":"m","widthRange":{"lowerBound":{"value":0,"unit":"cm"},"upperBound":{"value":10,"unit":"cm"}},"machineId":null,"availableBatches":"3"}],"machines":[],"demands":[{"productId":"p","mode":"roll","count":"5"}],"lengthUnit":"cm"}"#).unwrap();
        let configuration = Csp1dOptions { backend: NativeBackend::Scip, algorithm: Csp1dAlgorithm::Milp, generation: None,
            iteration_limit: None, columns_per_iteration: None, pool_limit: None };
        let backend = Arc::new(Backend::default());
        let result = domain::Csp1dMilp { initial_generator: Csp1dGenerationOptions::default().generator().unwrap(), milp_solver: Some(backend.clone()), ..Default::default() }
            .solve_with_trace(context.problem.domain().unwrap(), Some(domain::Csp1dSolveConfig { column_generation: configuration.config().unwrap(), ..Default::default() }));
        let text = encode_csp1d_execution_report(&context.artifact, configuration, &result, &backend.0.lock().unwrap(), Csp1dExecutionStop::Completed).unwrap();
        let mut artifact: Csp1dExecutionReportArtifact = serde_json::from_str(&text).unwrap();
        let mut portable: SolveReport<f64, u64> = serde_json::from_str(&artifact.stages[0].solver_report_json).unwrap();
        portable.statistics.nodes = Some(u64::MAX); artifact.stages[0].solver_report_json = serde_json::to_string(&portable).unwrap();
        let text = serde_json::to_string(&artifact).unwrap();
        let report = context.read_execution_report(&text).unwrap();
        assert_eq!(report.value.summary.stage_count, 1); assert_eq!(report.to_json(), text); assert_eq!(report.trace_json(), artifact.trace_json);
        assert!(report.stage_report_json(0).unwrap().contains("18446744073709551615")); assert!(report.stage_json(1).unwrap().is_none());
        let integer = report.integer_report().unwrap().unwrap(); let production = report.production().unwrap();
        drop(context); drop(report); drop(integer);
        assert_eq!(production.value.batch_count, "2"); assert_eq!(production.value.produced["p"], "6");
    }

    #[wasm_bindgen_test]
    fn csp_portable_calls_do_not_invoke_native_clocks_and_keep_independent_exact_results() {
        let context = Csp1dCuttingContext::new(r#"{"products":[{"id":"p","name":"p","width":[{"value":30,"unit":"mm"}]}],"materials":[{"id":"m","name":"m","widthRange":{"lowerBound":{"value":0,"unit":"cm"},"upperBound":{"value":10,"unit":"cm"}},"machineId":null,"availableBatches":null}],"machines":[],"demands":[{"productId":"p","mode":"sheet","count":"5"}],"lengthUnit":"cm"}"#).unwrap();
        let patterns = context.generate_patterns("{}").unwrap();
        assert_eq!(patterns.value.plans.len(), 3);
        let usage = Csp1dUsage { plan: patterns.value.plans[2].clone(), count: "9007199254740991".into() };
        let result = context.verify_usages(&serde_json::to_string(&vec![usage]).unwrap()).unwrap();
        assert_eq!(result.value.produced["p"], "27021597764222973");
        let artifact = context.to_artifact_json().unwrap();
        let restored = Csp1dCuttingContext::from_artifact(&artifact).unwrap();
        assert!(restored.generate_patterns(r#"{"maxKnives":0}"#).is_err());
        drop(context); drop(restored);
        assert!(result.to_json().unwrap().contains("27021597764222973"));
        assert!(!patterns.value.candidate_complete);
    }

    #[wasm_bindgen_test]
    fn actual_child_codec_and_lp_certificate_revalidation_keep_u64_on_wasm32() {
        use ospf_rust_core::model::{MetaModel, SubObjective, flatten::{Linear, LinearMonomial}};
        use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason,
            linear_model_fingerprint, linear_model_mapping};
        use ospf_rust_core::variable::{Continuous, VariableRange};
        use ospf_rust_framework::solver::remote::encode_compiled_linear_model;

        let mut source = MetaModel::new("wasm-child-contract");
        let x = source.register_auto_variable_with_range::<Continuous>("x", VariableRange::bounded(0.0, 10.0)).unwrap();
        source.add_ge_constraint(&[(x, 1.0)], 2.0, "demand").unwrap();
        source.add_sub_objective(SubObjective::minimize(Linear::new(vec![LinearMonomial::new(1.0, x)], 7.0), "cost"));
        let mut compiled = source.linear_model_snapshot().unwrap().compile_flt64().unwrap();
        compiled.model.basic.constraint_group_ids[0] = Some(u64::MAX);
        let mut solution = SolveSolution::vector(vec![2.0]);
        solution.objective_value = Some(2.0); solution.dual_solution = Some(vec![-1.0]);
        let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
            .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() })
            .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap())).build().unwrap();
        let mut artifact = Csp1dStageReportArtifact { schema_version: "1.0".into(), stage: Csp1dStageKind::Lp,
            model: encode_compiled_linear_model(&compiled.model).unwrap(), solver_report_json: serde_json::to_string(&report).unwrap(),
            objective_constant: compiled.objective_constant };
        let mut exact: SolveReport<f64, u64> = serde_json::from_str(&artifact.solver_report_json).unwrap();
        exact.statistics.nodes = Some(u64::MAX); exact.provenance.thread_count = Some(9_007_199_254_740_993);
        artifact.solver_report_json = serde_json::to_string(&exact).unwrap();
        drop(source); drop(compiled); drop(report);
        let (model, report) = decode_csp1d_stage_report(&artifact).unwrap();
        assert_eq!(model.constraint_group_ids[0], Some(u64::MAX));
        assert_eq!(report.statistics.nodes, Some(u64::MAX));
        assert_eq!(report.provenance.thread_count, Some(9_007_199_254_740_993));
        exact.solution.as_mut().unwrap().dual_solution = Some(vec![0.0]);
        artifact.solver_report_json = serde_json::to_string(&exact).unwrap();
        assert!(decode_csp1d_stage_report(&artifact).is_err());
    }

    #[wasm_bindgen_test]
    fn source_bound_integer_reader_derives_independent_production_and_preserves_exact_child_text() {
        use ospf_rust_core::model::MetaModel;
        use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolveSolution, TerminationReason, linear_model_fingerprint, linear_model_mapping};
        use ospf_rust_framework_csp1d::{Csp1dModelContext, Csp1dModelingMode, Csp1dProduceContextBuilder, Csp1dSolveStage, Csp1dStageReport, ProduceInput};

        let context = Csp1dCuttingContext::new(r#"{"products":[{"id":"p","name":"p","width":[{"value":30,"unit":"mm"}]}],"materials":[{"id":"m","name":"m","widthRange":{"lowerBound":{"value":0,"unit":"cm"},"upperBound":{"value":10,"unit":"cm"}},"machineId":null,"availableBatches":null}],"machines":[],"demands":[{"productId":"p","mode":"sheet","count":"5"}],"lengthUnit":"cm"}"#).unwrap();
        let source = context.problem.domain().unwrap();
        let patterns = generate_csp1d_patterns(&context.problem, &Default::default()).unwrap();
        let plans = patterns.plans.iter().map(|plan| decode_csp1d_pattern(&source, plan, context.problem.length_unit).unwrap()).collect::<Vec<_>>();
        let mut builder = Csp1dProduceContextBuilder::new(ProduceInput { cutting_plans: plans.clone(), demands: source.demands,
            materials: source.materials, machines: source.machines, warm_start_plan_usages: Vec::new() });
        builder.mode(Csp1dModelingMode::MILP);
        let mut original = builder.build().unwrap();
        let mut model = MetaModel::new("actual-wasm-report-test-child"); original.register(&mut model).unwrap();
        let compiled = model.linear_model_snapshot().unwrap().compile_flt64().unwrap();
        let mut solution = SolveSolution::vector(vec![0.0, 0.0, 2.0]); solution.objective_value = Some(2.0);
        let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed).solution(solution).proof(SolveProof::optimality())
            .fingerprints(SolveFingerprints { model: Some(linear_model_fingerprint(&compiled.model).unwrap()), ..Default::default() })
            .model_mapping(Some(linear_model_mapping(&compiled.model).unwrap())).build().unwrap();
        let record = Csp1dStageReport { stage: Csp1dSolveStage::Milp, model: compiled.model, report, objective_constant: compiled.objective_constant };
        let mut artifact: Csp1dProductionReportArtifact = serde_json::from_str(&encode_csp1d_production_report(&context.artifact, &plans, &record).unwrap()).unwrap();
        let mut portable: SolveReport<f64, u64> = serde_json::from_str(&artifact.child.solver_report_json).unwrap();
        portable.statistics.nodes = Some(u64::MAX);
        artifact.child.solver_report_json = serde_json::to_string(&portable).unwrap();
        let text = serde_json::to_string(&artifact).unwrap();
        let result = context.read_production_report(&text).unwrap();
        assert_eq!(result.to_json(), text); assert_eq!(result.solver_report_json(), artifact.child.solver_report_json);
        assert!(result.solver_report_json().contains("18446744073709551615"));
        assert!(!result.value.summary.candidate_complete); assert!(result.value.summary.restricted_optimal);
        let first = result.production().unwrap(); let second = result.production().unwrap();
        drop(context); drop(result); drop(first);
        assert_eq!(second.value.batch_count, "2"); assert_eq!(second.value.produced["p"], "6");
    }
}
