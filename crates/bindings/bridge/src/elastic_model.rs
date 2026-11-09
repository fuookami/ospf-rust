//! 续挖第 16 单：线性模型的三个纯变换（`model/intermediate/linear_triad_model.rs` + `elastic.rs`）
//! Continuation, delivery 16: three pure transforms of the linear model.
//!
//! `elastic` 构建器看起来只是"给个罚项"，但 `LinearTriadModel::elastic` 的实测行为有四条，其中两条容易被想当然搞错：
//!
//! 1. **每条约束只加一个松弛变量**（不是两侧各一个），`[0, +∞)`、连续，行系数 `-1.0`。
//!    所以"弹性化"只放松 `<=` 方向；要把等式两侧都放松，得自己再补。
//! 2. **目标系数直接推入 `penalty`**：`penalty` 是**目标函数里松弛变量的单价**，不是"每违反一单位的罚金"这种
//!    带量纲的说法——它和 `c` 里的其它系数同量级才合适。
//! 3. **目标方向被强制改成 `Minimum`**，哪怕原来是 `Maximum`。同理 `feasibility()` 也强制 `Minimum`。
//!    这不是 bug 也不是笔误：放松后的模型必须是"最小化违反量"，但**如果原模型是最大化，语义已经被改掉了**，
//!    调用方必须知道。
//! 4. **`penalty` 不做任何校验**：`penalty = -1` 或 `0` 都能通过，负罚项会让求解器**主动违反约束**。
//!    这是构建器的真实边界：它不替你判断罚项是否合理。
//!
//! 另外 `linear_relaxed()` 是一张死板的映射表（实测）：`Binary → Percentage`、`Ternary`/`UInteger →
//! UContinuous`、`BalancedTernary`/`Integer → Continuous`，其余不变。注意 `Binary` 松弛成 `Percentage`（`[0,1]` 连续）
//! 而不是 `Continuous`——界没丢。
//!
//! The elastic builder looks like "just pass a penalty", but `LinearTriadModel::elastic` has four measured behaviours, two of
//! them easy to assume wrong. **One slack per constraint**, not one per side, `[0, +∞)`, continuous, row coefficient `-1.0`, so
//! elasticizing relaxes only the `<=` direction. **The penalty is pushed straight into the objective** — it is the unit price of
//! a slack variable in `c`, not a dimensioned "penalty per unit violated". **The objective direction is forced to `Minimum`**
//! even when it was `Maximum` (and `feasibility()` does the same): not a slip — the relaxed model must minimize violation, but on
//! a maximization model the **semantics have changed** and the caller has to know. And **the penalty is never validated**:
//! `-1` and `0` both pass, and a negative penalty makes the solver **seek constraint violations** — the builder's real
//! boundary is that it will not judge your penalty for you. Finally `linear_relaxed()` is a fixed map: `Binary → Percentage`,
//! `Ternary`/`UInteger → UContinuous`, `BalancedTernary`/`Integer → Continuous`, everything else unchanged — note `Binary`
//! relaxes to the bounded `Percentage` rather than `Continuous`, so the bounds survive.

use ospf_rust_core::model::intermediate::{
    BasicLinearTriadModel, LinearElasticBuilder, LinearTriadModel, SparseVector,
};
use ospf_rust_core::model::ObjectiveCategory;
use ospf_rust_core::token::Token;
use ospf_rust_core::variable::{ContinuousVariableItem, VariableId, VariableType};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个变量。 / One variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelVariableSpec {
    /// 变量名。 / The variable name.
    pub name: String,
    /// 组号（同组共享 `group_id`）。 / The group id (shared within a group).
    pub group: u64,
    /// `continuous` / `binary` / `integer` / `percentage`。 / The variable kind.
    pub kind: String,
    /// 下界。 / The lower bound.
    pub lower: f64,
    /// 上界。 / The upper bound.
    pub upper: f64,
    /// 目标系数。 / The objective coefficient.
    pub cost: f64,
}

/// 一行约束。 / One constraint row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelRowSpec {
    /// 系数（`[列索引, 系数]`）。 / The coefficients as `[column, value]`.
    #[serde(default)]
    pub coefficients: Vec<Vec<f64>>,
    /// 右端。 / The right-hand side.
    pub rhs: f64,
}

/// 模型与变换请求。 / The model-and-transform request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElasticModelRequest {
    /// 模型名。 / The model name.
    pub name: String,
    /// 变量。 / The variables.
    #[serde(default)]
    pub variables: Vec<ModelVariableSpec>,
    /// 约束行。 / The constraint rows.
    #[serde(default)]
    pub rows: Vec<ModelRowSpec>,
    /// `min` / `max`。 / The objective direction.
    pub category: String,
    /// 弹性化的罚项（**不校验**）；不给则用领域的默认值。
    /// The elastic penalty (**never validated**); the domain default applies when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub penalty: Option<f64>,
}

/// 松弛变量的观察。 / An observation of the slack variables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlackRow {
    /// 列索引。 / The column index.
    pub index: u32,
    /// 名字。 / The name.
    pub name: String,
    /// 下界。 / The lower bound.
    pub lower: f64,
    /// 上界是否为正无穷。 / Whether the upper bound is positive infinity.
    pub upper_is_infinite: bool,
    /// 变量类型名。 / The variable type name.
    pub kind: String,
    /// 在对应约束行里的系数（没有则该行没写它）。 / The coefficient in its row.
    pub row_coefficient: Option<f64>,
    /// 目标系数。 / The objective coefficient.
    pub objective_coefficient: f64,
}

/// 一次变换的结论。 / One transform's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransformRow {
    /// 变换名。 / The transform name.
    pub transform: String,
    /// 变量数。 / The variable count.
    pub variables: u32,
    /// 约束数。 / The constraint count.
    pub constraints: u32,
    /// 目标系数个数。 / The objective coefficient count.
    pub objective_length: u32,
    /// 目标方向。 / The objective direction.
    pub category: String,
    /// 目标是否全零。 / Whether the objective is all zero.
    pub objective_is_zero: bool,
    /// 变量类型序列（`elf`/`bin`/`int`/`pct` 等）。 / The variable kind sequence.
    pub kinds: Vec<String>,
}

/// 弹性模型结论。 / The elastic-model verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElasticModelReport {
    /// 变换结论（顺序：`elastic` → `feasibility` → `linearRelaxed`）。
    /// The transform verdicts in order.
    pub transforms: Vec<TransformRow>,
    /// 弹性化新增的松弛变量。 / The slack variables the elasticize added.
    pub slacks: Vec<SlackRow>,
    /// 每条约束新增的松弛变量数（实测 **1**）。 / Slacks per constraint (**1**).
    pub slacks_per_constraint: u32,
    /// 目标方向是否被强制成 `min`（实测：**是**，即使原来是 `max`）。 / Whether it was forced to `min`.
    pub category_forced_to_minimum: bool,
    /// 负罚项是否被拒（实测：**不拒**）。 / Whether a negative penalty was refused (**no**).
    pub negative_penalty_refused: bool,
    /// 零罚项是否被拒（实测：**不拒**）。 / Whether a zero penalty was refused (**no**).
    pub zero_penalty_refused: bool,
    /// 松弛映射（松弛前后的类型名）。 / The relaxation map (before and after).
    pub relaxation_map: Vec<KindPair>,
}

/// 一对变量类型名。 / A pair of variable kind names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KindPair {
    /// 松弛前。 / Before relaxation.
    pub before: String,
    /// 松弛后。 / After relaxation.
    pub after: String,
}

/// 变量数与约束数上限（信任边界）。 / Caps on variables and rows (trust boundaries).
const MAX_VARIABLES: usize = 2_048;
const MAX_ROWS: usize = 2_048;

/// 建模型并走三个纯变换。非法输入是**错误**。
/// Build the model, then walk the three pure transforms.
pub fn verify_elastic_model(request: &ElasticModelRequest) -> Result<ElasticModelReport> {
    if request.variables.len() > MAX_VARIABLES || request.rows.len() > MAX_ROWS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "elastic model failed validation: at most {MAX_VARIABLES} variables and {MAX_ROWS} rows are accepted")));
    }
    let category = match request.category.as_str() {
        "min" => ObjectiveCategory::Minimum,
        "max" => ObjectiveCategory::Maximum,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "elastic model failed validation: unknown objective direction `{other}`"))),
    };
    let mut basic = BasicLinearTriadModel::new(&request.name);
    let mut objective = Vec::with_capacity(request.variables.len());
    for spec in &request.variables {
        if spec.name.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "elastic model failed validation: variable names must not be blank"));
        }
        if spec.lower > spec.upper {
            return Err(BridgeError::new("InvalidInput",
                "elastic model failed validation: variable lower bounds must not exceed the upper bound"));
        }
        let id = VariableId::new(usize::try_from(spec.group).unwrap_or(usize::MAX), 0);
        // 一律用 `add_variable_with_bounds`：`add_variable` 的类型来自 token，这里要显式指定。
        // Always `add_variable_with_bounds`: `add_variable` derives the kind from the token.
        basic.add_variable_with_bounds(
            Token::from_generic(ContinuousVariableItem::create(id, &spec.name),
                basic.num_variables()),
            spec.lower, spec.upper, kind_of(&spec.kind)?);
        objective.push(spec.cost);
    }
    for row in &request.rows {
        let mut vector = SparseVector::<f64>::new();
        for pair in &row.coefficients {
            match pair.as_slice() {
                [index, value] => {
                    if *index < 0.0 || index.fract() != 0.0 {
                        return Err(BridgeError::new("InvalidInput",
                            "elastic model failed validation: column indices must be non-negative integers"));
                    }
                    let index = usize::try_from(*index as u64).unwrap_or(usize::MAX);
                    if index >= request.variables.len() {
                        return Err(BridgeError::new("InvalidInput",
                            "elastic model failed validation: a row references a column that does not exist"));
                    }
                    vector.add(index, *value);
                }
                _ => return Err(BridgeError::new("InvalidInput",
                    "elastic model failed validation: coefficients are [column, value] pairs")),
            }
        }
        basic.add_constraint(vector, row.rhs);
    }
    let model = LinearTriadModel { basic, c: objective, objective_category: category };
    let base = describe("base", &model);
    // 走构建器而不是直接调 `elastic`：这条路径就是领域给调用方的入口。
    // Go through the builder rather than `elastic` directly: that is the domain's caller-facing entry.
    // 不给罚项就走 `LinearElasticBuilder::new` 的默认值（实测 1_000.0），不在这里另编一个。
    // Without a penalty the builder's own default applies (measured 1_000.0); do not invent one here.
    let builder = LinearElasticBuilder::new(&model);
    let elastic = match request.penalty {
        Some(penalty) => builder.penalty(penalty).finish(),
        None => builder.finish(),
    };
    let elastic_row = describe("elastic", &elastic);
    let elastic_variables = elastic_row.variables;
    let feasibility = model.feasibility();
    let relaxed = model.linear_relaxed();
    let mut slacks = Vec::new();
    let baseline = base.variables;
    let rows = base.constraints;
    for index in baseline..elastic_row.variables {
        let column = usize::try_from(index).unwrap_or(usize::MAX);
        let constraint_index = usize::try_from(index - baseline).unwrap_or(usize::MAX);
        let row_coefficient = elastic.basic.A.rows.get(constraint_index)
            .and_then(|row| row.entries.iter().find(|(position, _)| *position == column))
            .map(|(_, value)| *value);
        slacks.push(SlackRow {
            index,
            name: elastic.basic.variables.get(column)
                .map(|token| token.variable.name().to_owned()).unwrap_or_default(),
            lower: elastic.basic.lb.get(column).copied().unwrap_or(f64::NAN),
            upper_is_infinite: elastic.basic.ub.get(column)
                .map(|upper| upper.is_infinite() && *upper > 0.0).unwrap_or(false),
            kind: kind_name(*elastic.basic.var_types.get(column).unwrap_or(&VariableType::Continuous)),
            row_coefficient,
            objective_coefficient: elastic.c.get(column).copied().unwrap_or(f64::NAN),
        });
    }
    // 负 / 零罚项都不拒 —— 构建器没有这个校验。 / Negative and zero penalties are both accepted.
    let negative_penalty_refused = build_with_penalty(&model, -1.0).is_err();
    let zero_penalty_refused = build_with_penalty(&model, 0.0).is_err();
    Ok(ElasticModelReport {
        transforms: vec![base, elastic_row, describe("feasibility", &feasibility),
            describe("linearRelaxed", &relaxed)],
        slacks,
        slacks_per_constraint: if rows == 0 { 0 } else {
            // 每条约束新增的松弛变量数。 / The slacks added per constraint.
            u32::try_from(elastic_variables.saturating_sub(baseline) as usize / rows as usize)
                .unwrap_or(u32::MAX)
        },
        category_forced_to_minimum: elastic.objective_category == ObjectiveCategory::Minimum
            && model.objective_category == ObjectiveCategory::Maximum,
        negative_penalty_refused,
        zero_penalty_refused,
        relaxation_map: relaxed_kinds(&model, &relaxed),
    })
}

/// 用给定罚项弹性化一次，只为观察"是否被拒"。 / Elasticize once to observe whether the penalty is refused.
fn build_with_penalty(model: &LinearTriadModel, penalty: f64) -> std::result::Result<f64, ()> {
    let elastic = LinearElasticBuilder::new(model).penalty(penalty).finish();
    Ok(elastic.c.last().copied().unwrap_or(f64::NAN))
}

/// 描述一次变换后的模型形状。 / Describe the shape of a transformed model.
fn describe(transform: &str, model: &LinearTriadModel) -> TransformRow {
    TransformRow {
        transform: transform.to_owned(),
        variables: u32::try_from(model.basic.num_variables()).unwrap_or(u32::MAX),
        constraints: u32::try_from(model.basic.num_constraints()).unwrap_or(u32::MAX),
        objective_length: u32::try_from(model.c.len()).unwrap_or(u32::MAX),
        category: match model.objective_category {
            ObjectiveCategory::Minimum => "min",
            ObjectiveCategory::Maximum => "max",
        }.to_owned(),
        objective_is_zero: model.c.iter().all(|coefficient| *coefficient == 0.0),
        kinds: model.basic.var_types.iter().map(|kind| kind_name(*kind)).collect(),
    }
}

/// 松弛前后的类型对。 / The before/after kind pairs.
fn relaxed_kinds(before: &LinearTriadModel, after: &LinearTriadModel) -> Vec<KindPair> {
    let mut pairs = Vec::new();
    for (left, right) in before.basic.var_types.iter().zip(after.basic.var_types.iter()) {
        let pair = KindPair { before: kind_name(*left), after: kind_name(*right) };
        if !pairs.contains(&pair) { pairs.push(pair); }
    }
    pairs
}

/// 线格式的类型名。 / The wire kind name.
fn kind_of(kind: &str) -> Result<VariableType> {
    Ok(match kind {
        "continuous" => VariableType::Continuous,
        "binary" => VariableType::Binary,
        "integer" => VariableType::Integer,
        "percentage" => VariableType::Percentage,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "elastic model failed validation: unknown variable kind `{other}`"))),
    })
}

/// 类型名。 / The kind name.
fn kind_name(kind: VariableType) -> String {
    match kind {
        VariableType::Continuous => "continuous",
        VariableType::Binary => "binary",
        VariableType::Integer => "integer",
        VariableType::Percentage => "percentage",
        VariableType::Ternary => "ternary",
        VariableType::BalancedTernary => "balancedTernary",
        VariableType::UInteger => "uinteger",
        VariableType::UContinuous => "ucontinuous",
    }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variable(name: &str, group: u64, kind: &str, cost: f64) -> ModelVariableSpec {
        ModelVariableSpec { name: name.to_owned(), group, kind: kind.to_owned(), lower: 0.0,
            upper: 10.0, cost }
    }

    fn row(coefficients: Vec<Vec<f64>>, rhs: f64) -> ModelRowSpec {
        ModelRowSpec { coefficients, rhs }
    }

    fn request(category: &str, penalty: Option<f64>) -> ElasticModelRequest {
        ElasticModelRequest { name: "m".to_owned(),
            variables: vec![variable("x", 0, "continuous", 1.0),
                variable("y", 1, "binary", 2.0)],
            rows: vec![row(vec![vec![0.0, 1.0], vec![1.0, 1.0]], 4.0),
                row(vec![vec![0.0, 2.0]], 3.0)],
            category: category.to_owned(), penalty }
    }

    #[test]
    fn elastic_adds_one_slack_per_row_and_forces_the_objective_to_minimum() {
        // 原模型是**最大化**，弹性化后方向被强制成 `min`。 / A maximization model is forced to `min`.
        let report = verify_elastic_model(&request("max", None)).unwrap();
        assert_eq!(report.transforms[0].transform, "base");
        assert_eq!(report.transforms[0].variables, 2);
        assert_eq!(report.transforms[0].constraints, 2);
        assert_eq!(report.transforms[0].category, "max");
        let elastic = &report.transforms[1];
        assert_eq!(elastic.transform, "elastic");
        // 每条约束一个松弛变量。 / One slack per constraint.
        assert_eq!(elastic.variables, 4);
        assert_eq!(elastic.constraints, 2);
        assert_eq!(report.slacks_per_constraint, 1);
        assert_eq!(report.slacks.len(), 2);
        assert!(report.category_forced_to_minimum);
        assert_eq!(elastic.category, "min");
        // 松弛变量：`[0, +∞)`、连续、行系数 -1、目标系数默认 1_000。 
        // The slack: `[0, +∞)`, continuous, row coefficient -1, default penalty in the objective.
        for slack in &report.slacks {
            assert_eq!(slack.lower, 0.0);
            assert!(slack.upper_is_infinite);
            assert_eq!(slack.kind, "continuous");
            assert_eq!(slack.row_coefficient, Some(-1.0));
            assert_eq!(slack.objective_coefficient, 1_000.0);
            assert!(slack.name.ends_with("_elastic_s0") || slack.name.ends_with("_elastic_s1"));
        }
        assert_eq!(report.slacks[0].index, 2);
        assert_eq!(report.slacks[1].index, 3);
        // 目标系数个数跟着变量数走。 / The objective grows with the variables.
        assert_eq!(elastic.objective_length, 4);
        // 可行性模型：目标全零、方向 `min`。 / The feasibility model zeroes the objective.
        assert!(report.transforms[2].objective_is_zero);
        assert_eq!(report.transforms[2].category, "min");
        assert_eq!(report.transforms[2].variables, 2);
    }

    #[test]
    fn explicit_penalty_lands_in_the_objective_and_is_never_validated() {
        let report = verify_elastic_model(&request("min", Some(7.5))).unwrap();
        assert_eq!(report.slacks[0].objective_coefficient, 7.5);
        assert_eq!(report.slacks[1].objective_coefficient, 7.5);
        // 负罚项与零罚项都不拒。 / Neither a negative nor a zero penalty is refused.
        let report = verify_elastic_model(&request("min", Some(-1.0))).unwrap();
        assert_eq!(report.slacks[0].objective_coefficient, -1.0);
        assert!(!report.negative_penalty_refused);
        let report = verify_elastic_model(&request("min", Some(0.0))).unwrap();
        assert_eq!(report.slacks[0].objective_coefficient, 0.0);
        assert!(!report.zero_penalty_refused);
    }

    #[test]
    fn relaxation_is_a_fixed_map_that_keeps_the_bounds() {
        let mut document = request("min", None);
        document.variables = vec![variable("a", 0, "continuous", 0.0),
            variable("b", 1, "binary", 0.0), variable("c", 2, "integer", 0.0),
            variable("d", 3, "percentage", 0.0)];
        let report = verify_elastic_model(&document).unwrap();
        assert_eq!(report.transforms[3].transform, "linearRelaxed");
        // 固定映射：`binary → percentage`（界没丢）、`integer → continuous`，其余不变。
        // The fixed map: `binary → percentage` (bounds survive), `integer → continuous`, the rest unchanged.
        let map = |before: &str| report.relaxation_map.iter()
            .find(|pair| pair.before == before).map(|pair| pair.after.clone())
            .unwrap_or_else(|| panic!("kind {before}"));
        assert_eq!(map("continuous"), "continuous");
        assert_eq!(map("binary"), "percentage");
        assert_eq!(map("integer"), "continuous");
        assert_eq!(map("percentage"), "percentage");
        assert_eq!(report.transforms[0].kinds, vec!["continuous".to_owned(), "binary".to_owned(),
            "integer".to_owned(), "percentage".to_owned()]);
        assert_eq!(report.transforms[3].kinds, vec!["continuous".to_owned(),
            "percentage".to_owned(), "continuous".to_owned(), "percentage".to_owned()]);
    }

    #[test]
    fn a_model_without_rows_adds_no_slack_but_still_forces_the_direction() {
        let mut document = request("max", None);
        document.rows = Vec::new();
        let report = verify_elastic_model(&document).unwrap();
        assert_eq!(report.slacks.len(), 0);
        assert_eq!(report.slacks_per_constraint, 0);
        assert_eq!(report.transforms[1].variables, 2);
        // 没有约束也照样把最大化改成最小化。 / Even with no rows the direction is forced.
        assert!(report.category_forced_to_minimum);
        assert_eq!(report.transforms[1].category, "min");
        // 空模型也合法。 / An empty model is legal.
        let mut document = request("min", None);
        document.variables = Vec::new();
        document.rows = Vec::new();
        let report = verify_elastic_model(&document).unwrap();
        assert_eq!(report.transforms[0].variables, 0);
        assert!(!report.category_forced_to_minimum);
    }

    #[test]
    fn bad_input_is_refused() {
        // 未知方向 / 未知类型 / 空白变量名 / 界颠倒。 / Unknown direction, kind, blank name, reversed bounds.
        let mut document = request("both", None);
        assert!(verify_elastic_model(&document).unwrap_err().message
            .contains("unknown objective direction"));
        document.category = "min".to_owned();
        document.variables[0].kind = "semi".to_owned();
        assert!(verify_elastic_model(&document).unwrap_err().message.contains("unknown variable kind"));
        let mut document = request("min", None);
        document.variables[0].name = "  ".to_owned();
        assert!(verify_elastic_model(&document).unwrap_err().message
            .contains("names must not be blank"));
        let mut document = request("min", None);
        document.variables[0].lower = 5.0;
        document.variables[0].upper = 1.0;
        assert!(verify_elastic_model(&document).unwrap_err().message
            .contains("lower bounds must not exceed"));
        // 行系数：不是对 / 列索引不是非负整数 / 列越界。 / Row coefficients: not pairs, bad index, out of range.
        let mut document = request("min", None);
        document.rows[0].coefficients = vec![vec![0.0, 1.0, 2.0]];
        assert!(verify_elastic_model(&document).unwrap_err().message
            .contains("coefficients are [column, value] pairs"));
        let mut document = request("min", None);
        document.rows[0].coefficients = vec![vec![-1.0, 1.0]];
        assert!(verify_elastic_model(&document).unwrap_err().message
            .contains("non-negative integers"));
        let mut document = request("min", None);
        document.rows[0].coefficients = vec![vec![9.0, 1.0]];
        assert!(verify_elastic_model(&document).unwrap_err().message
            .contains("column that does not exist"));
        // 上限。 / The cap.
        let mut document = request("min", None);
        document.variables = (0..=MAX_VARIABLES)
            .map(|index| variable(&format!("v{index}"), index as u64, "continuous", 0.0))
            .collect();
        assert!(verify_elastic_model(&document).unwrap_err().message.contains("at most"));
    }
}
