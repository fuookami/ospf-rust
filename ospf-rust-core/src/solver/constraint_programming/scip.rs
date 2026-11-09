//! 基于严格有限降维的 SCIP CP facade / SCIP CP facade backed by strict finite lowering.
//!
//! `russcip` 的现有安全线性入口不能承载通用 CP AST。此 facade 因此只暴露
//! `MipBackedConstraintProgrammingSolver<SCIPSolver>`，并明确把支持等级报告为
//! `ExactLowering`，不宣称 SCIP 原生 CP 能力。
//! The current safe linear entry points in `russcip` do not carry a general CP AST. This facade
//! therefore exposes only `MipBackedConstraintProgrammingSolver<SCIPSolver>` and never claims
//! native SCIP CP support.

use super::{
    ConstraintProgrammingSession, ConstraintProgrammingSolveOptions, ConstraintProgrammingSolver,
    ConstraintProgrammingSupportReport, MipBackedConstraintProgrammingSolver, MipLoweringOptions,
};
use crate::error::Result;
use crate::model::constraint_programming::ConstraintProgrammingSnapshot;
use crate::solver::solvers::scip::{SCIPConfig, SCIPSolver};
use crate::solver::{
    ConstraintProgrammingAssumption, SolveReport, SolverCapability, SolverDescriptor, SolverInfo,
};

/// SCIP 的 CP facade / SCIP constraint-programming facade.
///
/// 当前实现通过已验证的有限 MIP formulation 使用 SCIP，不能把未支持的全局约束静默
/// 丢弃，也不宣称 `Native` CP 能力。/ The implementation uses SCIP through a verified finite
/// MIP formulation. Unsupported global constraints are rejected and native CP support is not
/// claimed.
#[derive(Clone)]
pub struct ScipConstraintProgrammingSolver {
    inner: MipBackedConstraintProgrammingSolver<SCIPSolver>,
}

impl ScipConstraintProgrammingSolver {
    /// 创建默认 SCIP CP facade / Create a default SCIP CP facade.
    pub fn new() -> Self {
        Self {
            inner: MipBackedConstraintProgrammingSolver::new(SCIPSolver::new()),
        }
    }

    /// 使用 SCIP 配置创建 facade / Create the facade with SCIP configuration.
    pub fn with_config(config: SCIPConfig) -> Self {
        Self {
            inner: MipBackedConstraintProgrammingSolver::new(SCIPSolver::with_config(config)),
        }
    }

    /// 设置严格降维预算 / Set strict lowering budgets.
    pub fn with_lowering_options(mut self, options: MipLoweringOptions) -> Self {
        self.inner = self.inner.with_lowering_options(options);
        self
    }

    /// 返回底层 SCIP solver / Return the wrapped SCIP solver.
    pub fn backend(&self) -> &SCIPSolver {
        self.inner.backend()
    }

    /// 返回严格降维 solver / Return the strict-lowering solver.
    pub fn inner(&self) -> &MipBackedConstraintProgrammingSolver<SCIPSolver> {
        &self.inner
    }
}

impl Default for ScipConstraintProgrammingSolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SolverInfo for ScipConstraintProgrammingSolver {
    fn name(&self) -> &str {
        "scip-constraint-programming-exact-lowering"
    }

    fn capabilities(&self) -> Vec<SolverCapability> {
        self.inner.capabilities()
    }

    fn descriptor(&self) -> SolverDescriptor {
        let mut descriptor = self.inner.descriptor();
        descriptor.solver_id = self.name().to_owned();
        descriptor.display_name = "SCIP CP (exact finite lowering)".to_owned();
        descriptor.capabilities.levels.insert(
            "constraint_programming".to_owned(),
            crate::solver::CapabilitySupport::Conditional,
        );
        descriptor.warnings.push(
            "SCIP CP uses the exact finite MIP lowering path; native CP constraints are not exposed"
                .to_owned(),
        );
        descriptor
    }
}

impl ConstraintProgrammingSolver for ScipConstraintProgrammingSolver {
    fn analyze_support(
        &self,
        snapshot: &ConstraintProgrammingSnapshot,
    ) -> ConstraintProgrammingSupportReport {
        self.inner.analyze_support(snapshot)
    }

    fn solve_constraint_programming(
        &self,
        snapshot: &ConstraintProgrammingSnapshot,
        options: &ConstraintProgrammingSolveOptions<'_>,
    ) -> Result<SolveReport<i64>> {
        self.inner.solve_constraint_programming(snapshot, options)
    }

    fn create_session(
        &self,
        snapshot: &ConstraintProgrammingSnapshot,
    ) -> Result<Box<dyn ConstraintProgrammingSession>> {
        self.inner.create_session(snapshot)
    }

    fn solve_constraint_programming_with_assumptions_and_conflict(
        &self,
        snapshot: &ConstraintProgrammingSnapshot,
        assumptions: &[ConstraintProgrammingAssumption],
        options: &ConstraintProgrammingSolveOptions<'_>,
    ) -> Result<SolveReport<i64>> {
        self.inner
            .solve_constraint_programming_with_assumptions_and_conflict(
                snapshot,
                assumptions,
                options,
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::constraint_programming::{
        ConstraintDefinition, ConstraintProgrammingConstraint, ConstraintProgrammingModel,
        IntegerDomain, IntegerExpression, IntegerObjective, IntegerRelation, IntegerTerm, IntegerVariable,
    };

    fn snapshot_with_constraint(
        constraint_factory: impl FnOnce(IntegerVariable) -> ConstraintProgrammingConstraint,
    ) -> ConstraintProgrammingSnapshot {
        let mut model = ConstraintProgrammingModel::new("scip-cp-facade");
        let x = IntegerVariable::new("x");
        model
            .register_variable(x.clone(), IntegerDomain::boolean())
            .expect("variable");
        model
            .add_constraint(ConstraintDefinition::new(
                "constraint",
                constraint_factory(x),
            ))
            .expect("constraint");
        model.freeze().expect("snapshot")
    }

    #[test]
    fn facade_descriptor_does_not_claim_native_cp() {
        let solver = ScipConstraintProgrammingSolver::new();
        let descriptor = solver.descriptor();
        assert_eq!(
            descriptor.capabilities.levels.get("constraint_programming"),
            Some(&crate::solver::CapabilitySupport::Conditional)
        );
        assert!(
            descriptor
                .warnings
                .iter()
                .any(|warning| warning.contains("exact finite MIP lowering"))
        );
    }

    #[test]
    fn unsupported_global_constraint_is_reported_per_constraint() {
        let supported = snapshot_with_constraint(|x| {
            ConstraintProgrammingConstraint::integer(
                IntegerExpression::variable(x),
                IntegerRelation::GreaterOrEqual,
                0,
            )
        });
        let solver = ScipConstraintProgrammingSolver::new();
        let report = solver.analyze_support(&supported);
        assert_eq!(
            report
                .constraints
                .get(&crate::solver::StableConstraintId::from("constraint")),
            Some(&super::super::ConstraintProgrammingSupport::ExactLowering)
        );
    }

    #[test]
    fn scip_executes_cp_objective_constants_in_both_directions() {
        for (maximize, constant, expected) in [(false, -7, -7), (true, 7, 10)] {
            let mut model = ConstraintProgrammingModel::new("scip-cp-objective-constant");
            let x = IntegerVariable::new("x");
            model.register_variable(x.clone(), IntegerDomain::range(0, 3).unwrap()).unwrap();
            let expression = IntegerExpression::linear(constant, [IntegerTerm { variable: x, coefficient: 1 }]).unwrap();
            model.set_objective(if maximize { IntegerObjective::maximize(expression) } else { IntegerObjective::minimize(expression) });
            let snapshot = model.freeze().unwrap();
            let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let captured = std::sync::Arc::clone(&observed);
            let reporter: crate::solver::SolveProgressReporter = std::sync::Arc::new(move |progress| {
                captured.lock().unwrap().push(progress.clone());
                Ok(())
            });
            let options = ConstraintProgrammingSolveOptions::builder().progress_reporter(Some(&reporter)).finish();
            let solver = ScipConstraintProgrammingSolver::with_config(SCIPConfig::default().with_output(false));
            let report = solver.solve_constraint_programming(&snapshot, &options).expect("SCIP solve");
            assert!(report.is_optimal());
            assert_eq!(report.solution.as_ref().and_then(|solution| solution.objective), Some(expected));
            assert_eq!(report.solution.as_ref().unwrap().values.len(), 1);
            assert_eq!(report.statistics.best_bound, Some(expected));
            assert_eq!(report.statistics.best_bound_value, Some(expected as f64));
            let observed = observed.lock().unwrap();
            let terminal = observed.iter().find(|progress| progress.terminal).expect("terminal progress");
            assert_eq!(terminal.objective_value, Some(expected as f64));
            assert_eq!(terminal.best_bound, Some(expected as f64));
        }
    }

    #[test]
    fn scip_cp_intermediate_progress_includes_the_objective_constant() {
        use crate::solver::ProgressValue;
        use crate::solver::solvers::scip::PresolvingMode;
        use std::sync::{Arc, Mutex};

        let mut model = ConstraintProgrammingModel::new("scip-cp-progress");
        let mut objective_terms = Vec::new();
        let mut limit_terms = Vec::new();
        for index in 0..48 {
            let variable = IntegerVariable::new(format!("x{index}"));
            model.register_variable(variable.clone(), IntegerDomain::boolean()).unwrap();
            objective_terms.push(IntegerTerm { variable: variable.clone(), coefficient: index + 2 });
            limit_terms.push(IntegerTerm { variable, coefficient: 1 });
        }
        model.add_constraint(ConstraintDefinition::new(
            "limit",
            ConstraintProgrammingConstraint::integer(
                IntegerExpression::linear(0, limit_terms).unwrap(),
                IntegerRelation::LessOrEqual,
                17,
            ),
        )).unwrap();
        model.set_objective(IntegerObjective::maximize(IntegerExpression::linear(137, objective_terms).unwrap()));
        let snapshot = model.freeze().unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let recorded = events.clone();
        let reporter: crate::solver::SolveProgressReporter = Arc::new(move |event| {
            recorded.lock().unwrap().push(event.clone());
            Ok(())
        });
        let options = ConstraintProgrammingSolveOptions::builder().progress_reporter(Some(&reporter)).finish();
        let config = SCIPConfig::default().with_output(false).with_presolving(PresolvingMode::Off)
            .with_heuristics_priority(0).with_telemetry_min_interval(1e-9);
        let report = ScipConstraintProgrammingSolver::with_config(config)
            .solve_constraint_programming(&snapshot, &options).unwrap();
        assert!(report.is_optimal());
        let events = events.lock().unwrap();
        let intermediate: Vec<_> = events.iter().filter(|event| event.stage_progress == ProgressValue::Indeterminate).collect();
        assert!(!intermediate.is_empty());
        assert!(intermediate.iter().any(|event| event.objective_value.is_some() || event.best_bound.is_some()));
        for event in intermediate {
            assert!(event.objective_value.is_none_or(|value| value >= 137.0));
            assert!(event.best_bound.is_none_or(|value| value >= 137.0));
        }
        let terminal = events.last().unwrap();
        assert!(terminal.terminal);
        assert_eq!(terminal.objective_value, report.solution.as_ref().and_then(|solution| solution.objective).map(|value| value as f64));
    }
}
