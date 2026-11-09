//! 重放有界阶段及定价生命周期，不把启发式轨迹当作完整搜索证明。 / Replay bounded stages and pricing lifecycles without treating heuristic traces as complete-search proofs.

use std::collections::BTreeSet;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_core::solver::{ProblemStatus, TerminationReason};
use ospf_rust_framework_csp2d::{application::Csp2dColumnGenerationInput, domain::{material::CuttingPlan2, produce::{ProduceContext, ProducePricingInput, ProduceStage}}};
use crate::{Result, encode_csp2d_column, invalid};
use crate::csp2d::CSP2D_TOLERANCE;
use super::{Csp2dExecutionReportArtifact, Csp2dExecutionStop as Stop, Csp2dStageKind as Kind, stage::DecodedStage};

pub(super) fn validate(
    artifact: &Csp2dExecutionReportArtifact,
    input: &Csp2dColumnGenerationInput,
    layers: &[CuttingPlan2<f64, Meter>],
    stages: &[DecodedStage],
) -> Result<()> {
    let initial = &input.initial_columns;
    if layers.len() < initial.len() { return Err(invalid("CSP2D complete pool lost initial columns")); }
    for (original, actual) in initial.iter().zip(layers) {
        if encode_csp2d_column(original)? != encode_csp2d_column(actual)? { return Err(invalid("CSP2D initial pool prefix was altered or reordered")); }
    }
    let keys = input.column_keys(layers).map_err(invalid)?;
    let mut unique = keys[..initial.len()].iter().cloned().collect::<BTreeSet<_>>();
    if keys[initial.len()..].iter().any(|key| !unique.insert(key.clone())) { return Err(invalid("CSP2D generated columns contain physical duplicates")); }
    let final_index = artifact.final_stage.map(|index| index as usize);
    if let Some(index) = final_index {
        if index.checked_add(1) != Some(stages.len()) || !input.config.final_integer_enabled || stages[index].summary.stage != Kind::Integer {
            return Err(invalid("CSP2D final stage must be the enabled last integer child"));
        }
    }
    let lp_count = final_index.unwrap_or(stages.len());
    let mut pool = initial.len();
    let mut next = if pool == 0 { Kind::FeasibilityLp } else { Kind::Rmp };
    let mut recovered = None;
    let mut cursor = 0;
    let mut batches = 0;
    let external = matches!(artifact.stop, Stop::Cancelled | Stop::TimeLimit);
    let report_cap = artifact.configuration.report_limit.unwrap_or(196) as usize;
    let reason = loop {
        if cursor == lp_count && final_index.is_none() {
            if external { break artifact.stop; }
            if artifact.stop == Stop::ReportLimit && stages.len() == report_cap { break Stop::ReportLimit; }
        }
        let stage = stages.get(cursor).filter(|_| cursor < lp_count).ok_or_else(|| invalid("CSP2D trace omitted a required LP stage"))?;
        if stage.summary.stage != next || stage.summary.pool_size as usize != pool { return Err(invalid("CSP2D phase or pool prefix differs from lifecycle")); }
        let index = cursor;
        cursor += 1;
        match stage.report.termination_reason {
            TerminationReason::Cancelled => break Stop::Cancelled,
            TerminationReason::TimeLimit => break Stop::TimeLimit,
            TerminationReason::Completed => {},
            _ => break Stop::MasterStopped,
        }
        let batch = artifact.iterations.get(batches).filter(|batch| batch.stage_index as usize == index);
        if cursor == lp_count && final_index.is_none() && external && batch.is_none() { break artifact.stop; }
        if stage.report.problem_status == ProblemStatus::Infeasible {
            if next == Kind::Rmp && recovered != Some(pool) { next = Kind::FeasibilityLp; continue; }
            break Stop::MasterStopped;
        }
        let Some(prices) = &stage.prices else { break Stop::MasterStopped; };
        if next == Kind::FeasibilityLp && stage.deficits.iter().all(|value| value.abs() <= CSP2D_TOLERANCE) {
            recovered = Some(pool);
            next = Kind::Rmp;
            continue;
        }
        if batches >= input.config.iteration_limit { break Stop::IterationLimit; }
        if pool >= input.config.column_limit { break Stop::ColumnLimit; }
        let batch = batch.ok_or_else(|| invalid("CSP2D eligible LP lacks its pricing batch"))?;
        let observed = batch.candidates_evaluated as usize;
        if observed > input.config.max_candidates_per_iteration || batch.candidate_limit_reached != (observed == input.config.max_candidates_per_iteration)
            || batch.added_columns.len() > observed { return Err(invalid("CSP2D candidate observation counts are inconsistent")); }
        let mut previous = f64::NEG_INFINITY;
        for added in &batch.added_columns {
            if added.index as usize != pool || pool >= input.config.column_limit { return Err(invalid("CSP2D added indices must be a contiguous pool suffix")); }
            let layer = layers.get(pool).ok_or_else(|| invalid("CSP2D pricing references an absent column"))?;
            let cost = ProduceContext { objective: input.objective.clone() }.reduced_cost(ProducePricingInput { candidate: layer, demands: &input.demands, prices, stage: if next == Kind::FeasibilityLp { ProduceStage::FeasibilityLp } else { ProduceStage::Rmp } }).map_err(invalid)?;
            if !cost.is_finite() || cost != added.reduced_cost || cost >= -input.config.reduced_cost_tolerance || cost < previous {
                return Err(invalid("CSP2D reduced costs differ from certified source pricing or admission order"));
            }
            previous = cost;
            pool += 1;
        }
        batches += 1;
        if batch.added_columns.is_empty() { break Stop::NoImprovingColumns; }
    };
    if cursor != lp_count || batches != artifact.iterations.len() || pool != layers.len() { return Err(invalid("CSP2D trace has unconsumed stages, batches or columns")); }
    let normal = matches!(reason, Stop::NoImprovingColumns | Stop::IterationLimit | Stop::ColumnLimit);
    if normal && input.config.final_integer_enabled {
        if let Some(index) = final_index {
            if stages[index].summary.pool_size as usize != pool { return Err(invalid("CSP2D final did not use the complete pool")); }
            if artifact.stop != reason { return Err(invalid("CSP2D pricing stop differs from its lifecycle")); }
        } else if !external && !(artifact.stop == Stop::ReportLimit && stages.len() == report_cap) {
            return Err(invalid("CSP2D enabled final stage was omitted without an external budget stop"));
        }
    } else if final_index.is_some() || artifact.stop != reason {
        return Err(invalid("CSP2D final execution or stop contradicts its lifecycle"));
    }
    if artifact.stop == Stop::ReportLimit && stages.len() != report_cap { return Err(invalid("CSP2D report limit was not reached")); }
    Ok(())
}
