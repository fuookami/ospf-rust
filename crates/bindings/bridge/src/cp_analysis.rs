//! 默认精确 MIP 降维预检，不查询后端运行状态 / Default exact MIP-lowering preflight, not backend readiness.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::model::constraint_programming::ConstraintProgrammingSnapshot;
use ospf_rust_core::solver::constraint_programming::{MipLoweringError, MipLoweringOptions, lower_constraint_programming_for_solve};

/// 默认预算下的完整模型降维结果，不代表可行性或后端求解保证。
/// Whole-model lowering under default budgets, not a feasibility or backend solve guarantee.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MipLoweringAnalysis {
    /// 完整模型通过精确降维 / The complete model passed exact lowering.
    Lowered {
        /// 源模型指纹 / Source model fingerprint.
        fingerprint: String,
        /// 源变量数 / Source variable count.
        source_variables: usize,
        /// 辅助变量数 / Auxiliary variable count.
        auxiliary_variables: usize,
        /// 源约束数 / Source constraint count.
        source_constraints: usize,
        /// 生成线性行数 / Generated linear-row count.
        generated_constraints: usize,
        /// Boolean 辅助变量数 / Boolean auxiliary count.
        boolean_auxiliaries: usize,
        /// table 元组数 / Table tuple count.
        table_tuples: usize,
    },
    /// 未实现或默认关闭的 formulation / Unimplemented or default-disabled formulation.
    Unsupported {
        /// 源约束 ID，模型级错误时为空 / Source constraint ID, absent for model-level failures.
        constraint: Option<String>,
        /// core formulation 名称 / Core formulation name.
        feature: String,
    },
    /// 无法精确转换到原生浮点 MIP / Cannot convert exactly to native floating-point MIP.
    Inexact {
        /// 数值字段 / Numeric field.
        field: String,
        /// 无损原值文本 / Lossless original value text.
        value: String,
    },
    /// 超出默认降维预算 / A default lowering budget was exceeded.
    BudgetExceeded {
        /// 预算资源 / Budgeted resource.
        resource: String,
        /// 默认预算上限 / Default budget limit.
        limit: usize,
        /// 所需资源数 / Required resource count.
        actual: usize,
    },
    /// 无效模型或 formulation / Invalid model or formulation.
    Invalid {
        /// 失败说明 / Failure description.
        message: String,
    },
}

/// 执行真实 core 降维并释放临时线性模型 / Perform real core lowering and drop the temporary linear model.
pub fn analyze_cp_mip_lowering(snapshot: &ConstraintProgrammingSnapshot) -> MipLoweringAnalysis {
    match lower_constraint_programming_for_solve(snapshot, MipLoweringOptions::default()) {
        Ok(result) => {
            let statistics = result.statistics;
            MipLoweringAnalysis::Lowered {
                fingerprint: result.source_fingerprint.value,
                source_variables: statistics.source_variables,
                auxiliary_variables: statistics.auxiliary_variables,
                source_constraints: statistics.source_constraints,
                generated_constraints: statistics.generated_constraints,
                boolean_auxiliaries: statistics.boolean_auxiliaries,
                table_tuples: statistics.table_tuples,
            }
        }
        Err(MipLoweringError::Unsupported { constraint, feature }) => MipLoweringAnalysis::Unsupported {
            constraint: constraint.map(|id| id.0), feature,
        },
        Err(MipLoweringError::Numeric { field, value }) => MipLoweringAnalysis::Inexact { field, value },
        Err(MipLoweringError::BudgetExceeded { resource, limit, actual }) => MipLoweringAnalysis::BudgetExceeded { resource, limit, actual },
        Err(MipLoweringError::Invalid { message }) => MipLoweringAnalysis::Invalid { message },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::model::constraint_programming::{ConstraintProgrammingModel, IntegerDomain, IntegerExpression, IntegerObjective, IntegerTerm, IntegerVariable};

    #[test]
    fn lowering_preflight_preserves_exact_failure_values_and_source_identity() {
        let mut model = ConstraintProgrammingModel::new("preflight");
        let variable = IntegerVariable::new("x");
        model.add_variable(variable, IntegerDomain::range(0, 2).unwrap()).unwrap();
        let snapshot = model.freeze().unwrap();
        match analyze_cp_mip_lowering(&snapshot) {
            MipLoweringAnalysis::Lowered { fingerprint, source_variables, .. } => {
                assert_eq!(fingerprint, snapshot.fingerprint.value);
                assert_eq!(source_variables, 1);
            }
            other => panic!("unexpected preflight: {other:?}"),
        }
        let mut model = ConstraintProgrammingModel::new("inexact");
        model.add_variable(IntegerVariable::new("x"), IntegerDomain::range(9_007_199_254_740_993, 9_007_199_254_740_993).unwrap()).unwrap();
        match analyze_cp_mip_lowering(&model.freeze().unwrap()) {
            MipLoweringAnalysis::Inexact { value, .. } => assert_eq!(value, "9007199254740993"),
            other => panic!("unexpected preflight: {other:?}"),
        }
    }

    #[test]
    fn preflight_uses_the_execution_objective_and_its_numeric_gate() {
        for (constant, expected) in [(7, "lowered"), (9_007_199_254_740_992, "inexact")] {
            let mut model = ConstraintProgrammingModel::new("objective-preflight");
            let variable = IntegerVariable::new("x");
            model.add_variable(variable.clone(), IntegerDomain::boolean()).unwrap();
            model.set_objective(IntegerObjective::minimize(IntegerExpression::linear(constant, [IntegerTerm {
                variable, coefficient: 1,
            }]).unwrap()));
            let snapshot = model.freeze().unwrap();
            match (expected, analyze_cp_mip_lowering(&snapshot)) {
                ("lowered", MipLoweringAnalysis::Lowered { fingerprint, auxiliary_variables, .. }) => {
                    assert_eq!(fingerprint, snapshot.fingerprint.value);
                    assert_eq!(auxiliary_variables, 3);
                }
                ("inexact", MipLoweringAnalysis::Inexact { field, value }) => {
                    assert_eq!(field, "objective.absoluteTermBounds");
                    assert_eq!(value, "9007199254740993");
                }
                (_, other) => panic!("unexpected preflight: {other:?}"),
            }
        }
        let mut model = ConstraintProgrammingModel::new("zero-constant-inexact");
        let x = IntegerVariable::new("x");
        let y = IntegerVariable::new("y");
        let one = IntegerDomain::range(1, 1).unwrap();
        model.add_variable(x.clone(), one.clone()).unwrap();
        model.add_variable(y.clone(), one).unwrap();
        model.set_objective(IntegerObjective::maximize(IntegerExpression::linear(0, [
            IntegerTerm { variable: x, coefficient: 9_007_199_254_740_992 },
            IntegerTerm { variable: y, coefficient: 1 },
        ]).unwrap()));
        match analyze_cp_mip_lowering(&model.freeze().unwrap()) {
            MipLoweringAnalysis::Inexact { field, value } => {
                assert_eq!(field, "objective.absoluteTermBounds");
                assert_eq!(value, "9007199254740993");
            }
            other => panic!("unexpected preflight: {other:?}"),
        }
    }
}
