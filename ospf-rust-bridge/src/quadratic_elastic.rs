//! 续挖第 58 单：二次弹性构建器的**开关语义**——只弹性化线性行、关掉后连目标方向都不动
//! （`model/intermediate/elastic.rs` 的 `QuadraticElasticBuilder`）
//! Continuation, delivery 58: the quadratic elastic builder's **switch semantics** — only linear rows are elasticized, and
//! turning it off leaves even the objective direction alone (`QuadraticElasticBuilder`).
//!
//! 第 33 单量过 `LinearElasticBuilder`；这一单量**二次**那个，它的差别全在 `linear_constraints(bool)` 这个开关上。
//! 七条实测：
//!
//! 1. **只有线性行被弹性化，每行一个松弛**（`one_slack_per_linear_row()`）：默认开关下变量数 = 原变量数 +
//!    **线性约束数 × 1**，二次约束数不变。
//! 2. **关掉开关就不加任何松弛**（`disabling_adds_no_slack()`）：变量数原样。
//! 3. **关掉开关连目标方向都不动**（`disabling_keeps_the_objective_category()`）：开着时 `objective_category`
//!    被**强制成 `Minimum`**，关掉时**保持原值**（例如 `Maximum`）——早退发生在改方向之前。
//! 4. **罚项进目标且后写覆盖**（`the_penalty_is_last_write_wins_and_enters_the_objective()`）：每个松弛在 `c`
//!    末尾追加一个 `penalty`（`penalty(5).penalty(9)` 等价于 `penalty(9)`）。
//! 5. **松弛只有负部一侧**（`slacks_only_have_the_negative_side()`）：每行只加**一个**松弛，系数 `-1.0`，
//!    界 `[0, +inf)`——所以 `>=` 与等式行的另一侧没有松弛，弹性化是**单侧**的。
//! 6. **默认罚项是 1000**（`the_default_penalty_is_one_thousand()`）：`new().finish()` 与
//!    `.penalty(1000.0).finish()` 完全一致，与 `.penalty(1.0)` 不一致。
//! 7. **关掉开关时 `c` 仍然被 resize**（`disabling_still_resizes_the_objective()`）：`c.resize(num_variables)`
//!    写在早退**之前**，所以把 `c` 人为截短后再关掉开关，`c` 依然被补齐；而原模型本身不被修改
//!    （`finish()` 是在 clone 上做）。
//!
//! Delivery 33 measured `LinearElasticBuilder`; this one measures the **quadratic** one, whose differences all live in the
//! `linear_constraints(bool)` switch. **Only linear rows are elasticized** (`only_linear_rows_get_slacks()`): with the switch
//! on, the variable count is the original plus **2× the linear constraint count**, and the quadratic constraint count is
//! unchanged. **Turning it off adds no slack at all** (`disabling_adds_no_slack()`). **Turning it off leaves even the objective
//! direction alone** (`disabling_keeps_the_objective_category()`): on, `objective_category` is **forced to `Minimum`**; off,
//! it keeps its original value (say `Maximum`) because the early return happens before the direction is changed. **The penalty
//! enters the objective and is last-write-wins** (`the_penalty_is_last_write_wins_and_enters_the_objective()`). **Slacks only
//! have the negative side** (`slacks_only_have_the_negative_side()`): each row gets **one** slack with coefficient `-1.0` and
//! bounds `[0, +inf)`, so the other side of a `>=` or equality row has no slack and elasticization is **one-sided**. **The
//! default penalty is 1000** (`the_default_penalty_is_one_thousand()`). And **`c` is resized even when the switch is off**
//! (`disabling_still_resizes_the_objective()`): `c.resize(num_variables)` runs **before** the early return, so a deliberately
//! truncated `c` is still padded; the source model itself is never modified since `finish()` works on a clone.

use ospf_rust_core::model::ObjectiveCategory;
use ospf_rust_core::model::intermediate::{
    BasicQuadraticTetradModel, QuadraticElasticBuilder, QuadraticTetradModel, SparseMatrix,
    SparseVector,
};
use ospf_rust_core::token::Token;
use ospf_rust_core::variable::{ContinuousVariableItem, VariableType};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 弹性构建请求。 / The elastic-builder request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuadraticElasticRequest {
    /// 罚项（缺省用构建器自己的 1000）。 / The penalty (defaults to the builder's own 1000).
    #[serde(default)]
    pub penalty: Option<f64>,
    /// `linear_constraints` 开关。 / The `linear_constraints` switch.
    #[serde(default = "default_true")]
    pub linear_constraints: bool,
    /// 线性约束行数（1..=4）。 / The number of linear rows (1..=4).
    #[serde(default)]
    pub rows: Option<u64>,
}

/// 缺省为真。 / Defaults to true.
fn default_true() -> bool {
    true
}

/// 弹性构建结论。 / The elastic-builder verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuadraticElasticReport {
    pub variables_before: u64,
    pub variables_after: u64,
    pub constraints_before: u64,
    pub constraints_after: u64,
    pub quadratic_constraints: u64,
    /// 目标系数向量。 / The objective vector.
    pub objective: Vec<f64>,
    /// 构建后的目标方向名。 / The objective direction name after building.
    pub objective_category: String,
    /// 原模型的目标方向名（`finish()` 之后仍未变）。 / The source model's direction name (unchanged by `finish()`).
    pub source_objective_category: String,
    /// 追加变量的 group id。 / The group IDs of the appended variables.
    pub slack_group_ids: Vec<u64>,
    /// 首行追加的松弛系数。 / The appended slack coefficient in the first row.
    #[serde(default)]
    pub slack_coefficient: Option<f64>,
    /// 每行一个松弛（**是**）。 / Whether each row gets one slack (**yes**).
    pub one_slack_per_linear_row: bool,
    /// 关掉开关不加松弛（**是**）。 / Whether disabling adds no slack (**yes**).
    pub disabling_adds_no_slack: bool,
    /// 关掉开关连方向都不动（**是**）。 / Whether disabling keeps the objective direction (**yes**).
    pub disabling_keeps_the_objective_category: bool,
    /// 罚项进目标且后写覆盖（**是**）。 / Whether the penalty enters the objective and is last-write-wins (**yes**).
    pub the_penalty_is_last_write_wins_and_enters_the_objective: bool,
    /// 松弛只有负部一侧（**是**）。 / Whether slacks only have the negative side (**yes**).
    pub slacks_only_have_the_negative_side: bool,
    /// 默认罚项是 1000（**是**）。 / Whether the default penalty is 1000 (**yes**).
    pub the_default_penalty_is_one_thousand: bool,
    /// 关掉开关时 `c` 仍被 resize（**是**）。 / Whether `c` is resized even when disabled (**yes**).
    pub disabling_still_resizes_the_objective: bool,
}

/// 造一个带 `rows` 条线性约束的二次模型。 / Build a quadratic model with `rows` linear constraints.
fn probe_model(rows: usize) -> QuadraticTetradModel {
    let mut basic = BasicQuadraticTetradModel::new("elastic_probe");
    for index in 0..rows.max(1) {
        basic.linear.add_variable_with_bounds(
            Token::from_generic(ContinuousVariableItem::auto(&format!("x{index}")), index),
            0.0,
            10.0,
            VariableType::Continuous,
        );
        basic.linear.add_constraint(SparseVector::new(), 1.0);
    }
    let mut model = QuadraticTetradModel::from_basic(basic);
    let width = model.num_variables();
    model.set_objective(vec![3.0; width], SparseMatrix::new(), ObjectiveCategory::Maximum);
    model
}

/// 方向名。 / The direction name.
fn category_name(category: ObjectiveCategory) -> String {
    format!("{category:?}")
}

/// 追加变量的 group id。 / The group IDs of appended variables.
fn slack_group_ids(model: &QuadraticTetradModel, from: usize) -> Vec<u64> {
    model.basic.linear.variables.iter().skip(from)
        .map(|token| token.id().group_id as u64).collect()
}

/// 走一遍二次弹性构建。非法输入是**错误**。 / Walk the quadratic elastic builder.
pub fn verify_quadratic_elastic(request: &QuadraticElasticRequest) -> Result<QuadraticElasticReport> {
    let rows = usize::try_from(request.rows.unwrap_or(2)).unwrap_or(2);
    if !(1..=4).contains(&rows) {
        return Err(BridgeError::new("InvalidInput",
            "quadratic elastic failed validation: rows must be between 1 and 4"));
    }
    let model = probe_model(rows);
    let variables_before = model.num_variables() as u64;
    let constraints_before = model.num_constraints() as u64;
    let source_objective_category = category_name(model.objective_category);
    let builder = QuadraticElasticBuilder::new(&model)
        .linear_constraints(request.linear_constraints);
    let built = match request.penalty {
        Some(penalty) => {
            if !penalty.is_finite() || penalty < 0.0 {
                return Err(BridgeError::new("InvalidInput",
                    "quadratic elastic failed validation: the penalty must be finite and \
                     non-negative"));
            }
            builder.penalty(penalty).finish()
        }
        None => builder.finish(),
    };
    let findings = canonical_findings();
    Ok(QuadraticElasticReport { variables_before,
        variables_after: built.num_variables() as u64,
        constraints_before, constraints_after: built.num_constraints() as u64,
        quadratic_constraints: built.num_quadratic_constraints() as u64,
        objective: built.c.clone(), objective_category: category_name(built.objective_category),
        source_objective_category,
        slack_group_ids: slack_group_ids(&built, variables_before as usize),
        slack_coefficient: built.basic.linear.A.rows.first()
            .and_then(|row| row.entries.last().map(|(index, value)| (*index, *value)))
            .and_then(|(index, value)| (index >= variables_before as usize).then_some(value)),
        one_slack_per_linear_row: findings.one_slack_per_linear_row,
        disabling_adds_no_slack: findings.disabling_adds_no_slack,
        disabling_keeps_the_objective_category: findings.disabling_keeps_the_objective_category,
        the_penalty_is_last_write_wins_and_enters_the_objective:
            findings.the_penalty_is_last_write_wins_and_enters_the_objective,
        slacks_only_have_the_negative_side: findings.slacks_only_have_the_negative_side,
        the_default_penalty_is_one_thousand: findings.the_default_penalty_is_one_thousand,
        disabling_still_resizes_the_objective: findings.disabling_still_resizes_the_objective })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    one_slack_per_linear_row: bool,
    disabling_adds_no_slack: bool,
    disabling_keeps_the_objective_category: bool,
    the_penalty_is_last_write_wins_and_enters_the_objective: bool,
    slacks_only_have_the_negative_side: bool,
    the_default_penalty_is_one_thousand: bool,
    disabling_still_resizes_the_objective: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 每行一个松弛：变量数 +行数，二次约束数不变。 
    // 1) One slack per row: variables grow by rows, quadratic constraints do not.
    let one_slack_per_linear_row = {
        let model = probe_model(3);
        let built = QuadraticElasticBuilder::new(&model).finish();
        let enabled = built.num_variables() == model.num_variables() + 3
            && built.num_constraints() == model.num_constraints()
            && built.num_quadratic_constraints() == 0;
        let single = QuadraticElasticBuilder::new(&probe_model(1)).finish();
        enabled && single.num_variables() == probe_model(1).num_variables() + 1
    };
    // 2) 关掉开关不加任何松弛。 / Disabling adds no slack.
    let disabling_adds_no_slack = {
        let model = probe_model(2);
        let built = QuadraticElasticBuilder::new(&model).linear_constraints(false).finish();
        built.num_variables() == model.num_variables()
            && built.num_constraints() == model.num_constraints()
    };
    // 3) 关掉开关保持原方向；开着被强制 `Minimum`。 
    // 3) Disabling keeps the direction; enabling forces `Minimum`.
    let disabling_keeps_the_objective_category = {
        let model = probe_model(2);
        let enabled = QuadraticElasticBuilder::new(&model).finish();
        let disabled = QuadraticElasticBuilder::new(&model).linear_constraints(false).finish();
        model.objective_category == ObjectiveCategory::Maximum
            && enabled.objective_category == ObjectiveCategory::Minimum
            && disabled.objective_category == ObjectiveCategory::Maximum
    };
    // 4) 罚项进目标，且后写覆盖。 / The penalty enters the objective and is last-write-wins.
    let the_penalty_is_last_write_wins_and_enters_the_objective = {
        let model = probe_model(2);
        let built = QuadraticElasticBuilder::new(&model).penalty(7.5).finish();
        let width = model.num_variables();
        let overwritten = QuadraticElasticBuilder::new(&model).penalty(5.0).penalty(9.0).finish();
        let expected = QuadraticElasticBuilder::new(&model).penalty(9.0).finish();
        built.c.len() == width + 2 && built.c[..width].iter().all(|value| *value == 3.0)
            && built.c[width..].iter().all(|value| *value == 7.5)
            && overwritten.c == expected.c && overwritten.c[width] == 9.0
    };
    // 5) 松弛只有负部一侧：每行只加一个系数 -1.0 的松弛。 
    // 5) Slacks only have the negative side: each row gains one slack with coefficient -1.0.
    let slacks_only_have_the_negative_side = {
        let model = probe_model(2);
        let built = QuadraticElasticBuilder::new(&model).finish();
        let width = model.num_variables();
        let appended = built.basic.linear.A.rows.iter().enumerate().all(|(position, row)| {
            let tail = row.entries.iter()
                .filter(|(index, _)| *index >= width).collect::<Vec<_>>();
            tail.len() == 1 && tail[0].0 == width + position && tail[0].1 == -1.0
        });
        appended && built.num_variables() == width + 2
    };
    // 6) 默认罚项是 1000。 / The default penalty is 1000.
    let the_default_penalty_is_one_thousand = {
        let model = probe_model(1);
        let plain = QuadraticElasticBuilder::new(&model).finish();
        let explicit = QuadraticElasticBuilder::new(&model).penalty(1_000.0).finish();
        let other = QuadraticElasticBuilder::new(&model).penalty(1.0).finish();
        plain.c == explicit.c && plain.c != other.c
    };
    // 7) 关掉开关时 `c` 仍被 resize；原模型不被修改。 
    // 7) `c` is resized even when disabled; the source model is untouched.
    let disabling_still_resizes_the_objective = {
        let mut model = probe_model(2);
        let width = model.num_variables();
        model.c.clear();
        let built = QuadraticElasticBuilder::new(&model).linear_constraints(false).finish();
        let source_untouched = model.num_variables() == width && model.c.is_empty();
        built.c.len() == width && built.c.iter().all(|value| *value == 0.0) && source_untouched
    };
    Findings { one_slack_per_linear_row, disabling_adds_no_slack,
        disabling_keeps_the_objective_category,
        the_penalty_is_last_write_wins_and_enters_the_objective,
        slacks_only_have_the_negative_side, the_default_penalty_is_one_thousand,
        disabling_still_resizes_the_objective }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> QuadraticElasticRequest {
        QuadraticElasticRequest { penalty: None, linear_constraints: true, rows: Some(2) }
    }

    #[test]
    fn only_linear_rows_get_slacks() {
        let report = verify_quadratic_elastic(&request()).unwrap();
        assert_eq!(report.variables_before, 2);
        assert_eq!(report.variables_after, 4);
        assert_eq!(report.constraints_before, 2);
        assert_eq!(report.constraints_after, 2);
        assert_eq!(report.quadratic_constraints, 0);
        assert_eq!(report.objective.len(), 4);
        assert_eq!(report.objective[0], 3.0);
        assert_eq!(report.objective[2], 1_000.0);
        assert_eq!(report.objective_category, "Minimum");
        assert_eq!(report.source_objective_category, "Maximum");
        assert_eq!(report.slack_group_ids.len(), 2);
        assert_eq!(report.slack_coefficient, Some(-1.0));
        assert!(report.slack_group_ids.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(report.one_slack_per_linear_row);
        assert!(report.disabling_adds_no_slack);
        assert!(report.disabling_keeps_the_objective_category);
        assert!(report.the_penalty_is_last_write_wins_and_enters_the_objective);
        assert!(report.slacks_only_have_the_negative_side);
        assert!(report.the_default_penalty_is_one_thousand);
        assert!(report.disabling_still_resizes_the_objective);
    }

    #[test]
    fn disabling_adds_no_slack_and_keeps_the_direction() {
        let report = verify_quadratic_elastic(&QuadraticElasticRequest {
            linear_constraints: false, ..request() }).unwrap();
        assert_eq!(report.variables_after, report.variables_before);
        assert_eq!(report.objective.len(), 2);
        assert_eq!(report.objective_category, "Maximum");
        assert!(report.slack_group_ids.is_empty());
    }

    #[test]
    fn an_explicit_penalty_reaches_the_objective() {
        let report = verify_quadratic_elastic(&QuadraticElasticRequest { penalty: Some(4.5),
            ..request() }).unwrap();
        assert_eq!(report.objective[2], 4.5);
        assert_eq!(report.objective[3], 4.5);
    }

    #[test]
    fn bad_input_is_refused() {
        for rows in [0, 5] {
            assert!(verify_quadratic_elastic(&QuadraticElasticRequest { rows: Some(rows),
                ..request() }).unwrap_err().message.contains("between 1 and 4"));
        }
        assert!(verify_quadratic_elastic(&QuadraticElasticRequest { penalty: Some(-1.0),
            ..request() }).unwrap_err().message.contains("penalty"));
        assert!(verify_quadratic_elastic(&QuadraticElasticRequest { penalty: Some(f64::NAN),
            ..request() }).unwrap_err().message.contains("penalty"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_quadratic_elastic(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"variablesBefore\""), "{json}");
        assert!(json.contains("\"oneSlackPerLinearRow\""), "{json}");
        assert!(json.contains("\"disablingStillResizesTheObjective\""), "{json}");
        assert!(!json.contains("only_linear"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<QuadraticElasticReport>(&copy.to_string()).is_err());
    }
}
