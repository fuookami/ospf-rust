//! 子报告身份只绑定实际模型；源 context 用于独立数学复验。 / Child identity binds only the actual model; source contexts independently revalidate mathematics.

use std::collections::BTreeMap;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_core::model::{MetaModel, intermediate::LinearTriadModel};
use ospf_rust_core::solver::{ProblemStatus, SolveReport, require_infeasibility_certificate_for_model, require_optimal_lp_certificate_for_linear_model};
use ospf_rust_framework_bpp2d::{application::Bpp2dColumnGenerationInput, domain::{item::model::Layer, layer_assignment::{Bpp2dDemandShadowPriceMap, LayerAssignmentContext, LayerAssignmentModel, LayerAssignmentModelInput}}};
use crate::{Csp1dStageKind, Csp1dStageReportArtifact, Result, decode_csp1d_stage_report, invalid};
use crate::bpp2d::BPP2D_TOLERANCE;
use super::{Bpp2dStageKind, Bpp2dStageReportArtifact, Bpp2dStageSummary};

pub(super) struct DecodedStage {
    pub actual: LinearTriadModel,
    pub report: SolveReport<f64, u64>,
    pub source: MetaModel<f64>,
    pub assignment: LayerAssignmentModel,
    pub permutation: Vec<usize>,
    pub prices: Option<Bpp2dDemandShadowPriceMap>,
    pub summary: Bpp2dStageSummary,
}

pub(super) fn decode(
    record: &Bpp2dStageReportArtifact,
    input: &Bpp2dColumnGenerationInput,
    layers: &[Layer<f64, Meter>],
) -> Result<DecodedStage> {
    let prefix = layers.get(..record.pool_size as usize).ok_or_else(|| invalid("BPP2D stage exceeds the complete pool"))?;
    // 共享原始线性报告门禁，不修复导入身份；u64 元数据保持原样。 / Share raw linear-report gates without repairing imported identity; retain u64 metadata.
    let (actual, report) = decode_csp1d_stage_report(&Csp1dStageReportArtifact { schema_version: "1.0".into(),
        stage: if record.stage == Bpp2dStageKind::Integer { Csp1dStageKind::Milp } else { Csp1dStageKind::Lp },
        model: record.model.clone(), solver_report_json: record.solver_report_json.clone(), objective_constant: record.objective_constant })?;
    if report.problem_status == ProblemStatus::Infeasible {
        require_infeasibility_certificate_for_model(&report, &record.model.model_fingerprint).map_err(invalid)?;
    }
    let mut source = MetaModel::new("bpp2d-source-report-check");
    let assignment = LayerAssignmentContext::new(input.objective.clone()).register_model(&mut source, LayerAssignmentModelInput {
        layers: prefix, demands: &input.demands, stage: record.stage.domain(),
    }).map_err(invalid)?;
    let snapshot = source.linear_model_snapshot().map_err(invalid)?;
    let compiled = snapshot.compile_flt64().map_err(invalid)?;
    if record.objective_constant != compiled.objective_constant { return Err(invalid("BPP2D child objective constant differs from its source")); }
    let permutation = crate::source_model::compare_source_model(&actual, &compiled.model)?;
    let objective = if let Some(solution) = &report.solution {
        let values = compiled.model.variables.iter().enumerate().map(|(column, token)| (token.id(), solution.values[permutation[column]])).collect();
        snapshot.validate_solution(&values, &BPP2D_TOLERANCE).map_err(invalid)?;
        assignment.extract_copies(&source, &values, BPP2D_TOLERANCE).map_err(invalid)?;
        assignment.extract_deficits(&source, &values, BPP2D_TOLERANCE).map_err(invalid)?;
        Some(snapshot.evaluate_objective_flt64(&values).map_err(invalid)?.total_value().map_err(invalid)?)
    } else { None };
    let prices = if record.stage != Bpp2dStageKind::Integer && report.is_optimal() {
        let certificate = require_optimal_lp_certificate_for_linear_model(&report, &actual).map_err(invalid)?;
        let mut by_origin = BTreeMap::<&str, f64>::new();
        for (row, dual) in compiled.rows.iter().zip(certificate.dual) {
            let price = by_origin.entry(&row.source).or_default();
            *price += f64::from(row.multiplier) * dual;
            if !price.is_finite() { return Err(invalid("BPP2D source-row dual overflows")); }
        }
        let duals = snapshot.rows.iter().zip(source.constraints()).map(|(row, constraint)| {
            if row.source_name != constraint.name { return Err(invalid("BPP2D source-row order changed")); }
            by_origin.get(row.id.as_str()).copied().ok_or_else(|| invalid("BPP2D source row lacks a dual"))
        }).collect::<Result<Vec<_>>>()?;
        Some(assignment.extract_shadow_prices(&source, &duals).map_err(invalid)?)
    } else { None };
    let summary = Bpp2dStageSummary { stage: record.stage, pool_size: record.pool_size, model_fingerprint: record.model.model_fingerprint.value.clone(),
        problem_status: serde_json::to_value(report.problem_status).map_err(invalid)?.as_str().ok_or_else(|| invalid("invalid BPP2D status"))?.into(),
        termination_reason: serde_json::to_value(report.termination_reason).map_err(invalid)?.as_str().ok_or_else(|| invalid("invalid BPP2D termination"))?.into(),
        restricted_optimal: report.is_optimal(), restricted_infeasible: report.problem_status == ProblemStatus::Infeasible,
        has_incumbent: report.solution.is_some(), objective_value: objective };
    Ok(DecodedStage { actual, report, source, assignment, permutation, prices, summary })
}
