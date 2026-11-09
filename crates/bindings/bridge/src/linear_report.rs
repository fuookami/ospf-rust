//! 运行时无关的实际线性模型／报告门禁，不改写原始身份或精确元数据。
//! Runtime-independent actual linear model/report gate, without rewriting original identities or exact metadata.

use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::{CURRENT_SOLVE_REPORT_SCHEMA_VERSION, SolveReport, evaluate_linear_solution, linear_model_fingerprint,
    linear_model_mapping, require_optimal_lp_certificate_for_linear_model};
use crate::{BridgeError, Result, invalid};

pub(crate) fn decode_linear_report(
    model: &LinearTriadModel,
    json: &str,
    exact_lp: bool,
) -> Result<SolveReport<f64, u64>> {
    if json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "linear report exceeds 8 MiB")); }
    let report: SolveReport<f64, u64> = serde_json::from_str(json).map_err(invalid)?;
    if report.schema_version != CURRENT_SOLVE_REPORT_SCHEMA_VERSION { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported linear report schema")); }
    report.validate().map_err(invalid)?;
    let mapping = linear_model_mapping(model).map_err(invalid)?;
    if report.fingerprints.model.as_ref() != Some(&linear_model_fingerprint(model).map_err(invalid)?) || report.model_mapping.as_ref() != Some(&mapping) {
        return Err(BridgeError::new("FingerprintMismatch", "report is not bound to its actual linear model"));
    }
    if exact_lp && model.has_integer_variables() { return Err(invalid("LP stage contains integer solver variables")); }
    if let Some(solution) = &report.solution {
        if solution.value.is_some() || solution.values.len() != model.num_variables() { return Err(invalid("linear incumbent requires a full variable vector")); }
        let diagnostics = evaluate_linear_solution(model, &solution.values, 1e-7).map_err(invalid)?;
        if diagnostics.constraint_evaluations.iter().any(|row| !row.satisfied) || diagnostics.variable_bound_evaluations.iter().any(|bound| !bound.satisfied)
            || solution.values.iter().zip(&model.var_types).any(|(value, kind)| kind.is_integer() && (value - value.round()).abs() > 1e-7) {
            return Err(invalid("linear incumbent violates actual rows, bounds or integer domains"));
        }
        let objective = ospf_rust_core::model::LinearObjectiveEvaluationFlt64::new(model.c.iter().copied().zip(solution.values.iter().copied()), 0.0).map_err(invalid)?;
        objective.validate_reported(solution.objective_value.or(solution.objective), 1e-7).map_err(invalid)?;
        if solution.objective.is_some() { objective.validate_reported(solution.objective, 1e-7).map_err(invalid)?; }
        for (id, value) in &solution.stable_values {
            let column = mapping.columns_by_variable.get(id).ok_or_else(|| invalid("foreign stable variable in linear report"))?;
            if *value != solution.values[*column] { return Err(invalid("linear stable and vector values disagree")); }
        }
    }
    if exact_lp && report.is_optimal() { require_optimal_lp_certificate_for_linear_model(&report, model).map_err(invalid)?; }
    Ok(report)
}
