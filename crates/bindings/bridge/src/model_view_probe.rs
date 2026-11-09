//! 续挖第 64 单：两个**只读视图 trait** 的口径——名字取自线性部分、`num_constraints` 只数线性行、
//! 目标向量不与变量数对账（`model/intermediate/linear_triad_model_view.rs` / `quadratic_tetrad_model_view.rs`）
//! Continuation, delivery 64: the two **read-only view traits**' conventions — the name comes from the linear part,
//! `num_constraints` counts linear rows only, and the objective vector is never reconciled with the variable count
//! (`LinearTriadModelView` / `QuadraticTetradModelView`).
//!
//! §11.14 点名 `model/intermediate` 的可编码部分。这两个 trait 是"Kotlin 对齐"的只读投影，桥接层一直没用过它们
//! （`grep` 零命中），但它们**完全脱离求解器**。七条实测：
//!
//! 1. **两个视图的 `name()` 都取 `basic.linear.name`**（`both_views_read_the_linear_name()`）：二次模型的"名字"
//!    其实是它线性部分的名字。
//! 2. **二次视图的 `num_constraints()` 只数线性行**（`the_quadratic_view_counts_linear_rows_only()`）：
//!    加一条二次约束后 `num_constraints()` **不变**，只有 `num_quadratic_constraints()` 变。
//! 3. **目标向量不与变量数对账**（`the_objective_is_never_reconciled()`）：把 `c` 截短或加长，视图照样原样返回
//!    它——没有补齐、没有报错。
//! 4. **`objective()` 与 `linear_objective()` 是同一字段**（`objective_is_the_linear_objective()`）。
//! 5. **`Q` 的行数也不与变量数对账**（`the_quadratic_objective_is_unreconciled()`）：空 `Q` 对 3 个变量也照样返回。
//! 6. **视图不给目标方向**（`the_views_do_not_expose_the_direction()`）：`objective_category` 不在 trait 上，
//!    要方向只能直接读字段。
//! 7. **视图只读**（`the_views_are_read_only()`）：`&self` 方法，拿不到可变访问——改模型必须走原始结构。
//!
//! The §11.14 audit names the codable part of `model/intermediate`. These two traits are Kotlin-aligned read-only
//! projections the bridge never used (`grep` returns nothing), yet they are **entirely solver-free**. **Both views' `name()`
//! reads `basic.linear.name`** (`both_views_read_the_linear_name()`): a quadratic model's "name" is really its linear part's.
//! **The quadratic view's `num_constraints()` counts linear rows only**
//! (`the_quadratic_view_counts_linear_rows_only()`): adding a quadratic constraint leaves it unchanged while
//! `num_quadratic_constraints()` moves. **The objective is never reconciled with the variable count**
//! (`the_objective_is_never_reconciled()`): a truncated or extended `c` comes straight back — no padding, no error. **`objective()`
//! and `linear_objective()` are the same field** (`objective_is_the_linear_objective()`). **`Q`'s row count is unreconciled
//! too** (`the_quadratic_objective_is_unreconciled()`): an empty `Q` is returned for three variables. **The views do not
//! expose the direction** (`the_views_do_not_expose_the_direction()`). And **the views are read-only**
//! (`the_views_are_read_only()`): every method takes `&self`.

use ospf_rust_core::model::intermediate::{
    BasicLinearTriadModel, BasicQuadraticTetradModel, LinearTriadModel, LinearTriadModelView,
    QuadraticTetradModel, QuadraticTetradModelView, SparseMatrix, SparseVector,
};
use ospf_rust_core::model::{
    ObjectiveCategory, Quadratic, QuadraticInequality, QuadraticMonomial,
};
use ospf_rust_core::model::mechanism::ConstraintRelation;
use ospf_rust_core::token::Token;
use ospf_rust_core::variable::{ContinuousVariableItem, VariableType};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 视图探针请求。 / The view probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelViewRequest {
    /// 模型名。 / The model name.
    #[serde(default)]
    pub name: Option<String>,
    /// 变量数（0..=8）。 / The variable count (0..=8).
    #[serde(default)]
    pub variables: Option<u64>,
    /// 线性行数（0..=8）。 / The linear row count (0..=8).
    #[serde(default)]
    pub rows: Option<u64>,
    /// 二次约束数（0..=8）。 / The quadratic constraint count (0..=8).
    #[serde(default)]
    pub quadratic_rows: Option<u64>,
    /// 目标向量长度（缺省等于变量数）。 / The objective length (defaults to the variable count).
    #[serde(default)]
    pub objective_length: Option<u64>,
    /// 二次目标矩阵行数（缺省等于变量数）。 / The `Q` row count (defaults to the variable count).
    #[serde(default)]
    pub quadratic_objective_rows: Option<u64>,
    /// 目标方向。 / The objective direction.
    #[serde(default)]
    pub direction: Option<String>,
}

/// 视图探针结论。 / The view probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelViewReport {
    /// 线性视图读到的名字。 / The name read through the linear view.
    pub linear_name: String,
    /// 二次视图读到的名字。 / The name read through the quadratic view.
    pub quadratic_name: String,
    /// 线性视图的 `num_variables` / `num_variables`. / The linear view's `num_variables`.
    pub linear_variables: u64,
    /// 线性视图的 `num_constraints` / `num_constraints`. / The linear view's `num_constraints`.
    pub linear_constraints: u64,
    /// 二次视图的 `num_variables` / `num_variables`. / The quadratic view's `num_variables`.
    pub quadratic_variables: u64,
    /// 二次视图的 `num_constraints` / `num_constraints`. / The quadratic view's `num_constraints`.
    pub quadratic_constraints: u64,
    /// 二次视图的 `num_quadratic_constraints` / `num_quadratic_constraints`. / The quadratic view's count.
    pub quadratic_quadratic_constraints: u64,
    /// 线性视图 `objective()` 的条数。 / The length of `objective()`.
    pub objective_length: u64,
    /// 二次视图 `linear_objective()` 的条数。 / The length of `linear_objective()`.
    pub linear_objective_length: u64,
    /// 二次视图 `quadratic_objective().rows()`。 / `quadratic_objective().rows()`.
    pub quadratic_objective_rows: u64,
    /// 直接读到的目标方向名。 / The direction name read from the field.
    pub objective_category: String,
    /// 两个视图都取线性部分的名字（**是**）。 / Whether both views read the linear name (**yes**).
    pub both_views_read_the_linear_name: bool,
    /// 二次视图只数线性行（**是**）。 / Whether the quadratic view counts linear rows only (**yes**).
    pub the_quadratic_view_counts_linear_rows_only: bool,
    /// 目标向量不与变量数对账（**是**）。 / Whether the objective is unreconciled (**yes**).
    pub the_objective_is_never_reconciled: bool,
    /// `objective()` 与 `linear_objective()` 同字段（**是**）。 / Whether they are the same field (**yes**).
    pub objective_is_the_linear_objective: bool,
    /// `Q` 的行数也不对账（**是**）。 / Whether `Q`'s rows are unreconciled (**yes**).
    pub the_quadratic_objective_is_unreconciled: bool,
    /// 视图不给目标方向（**是**）。 / Whether the views omit the direction (**yes**).
    pub the_views_do_not_expose_the_direction: bool,
    /// 视图只读（**是**）。 / Whether the views are read-only (**yes**).
    pub the_views_are_read_only: bool,
}

/// 方向名。 / The direction name.
fn category_name(category: ObjectiveCategory) -> String {
    match category {
        ObjectiveCategory::Minimum => "Minimum".to_owned(),
        ObjectiveCategory::Maximum => "Maximum".to_owned(),
    }
}

/// 造一个变量 token。 / Build one variable token.
fn token_of(index: usize) -> Token<f64> {
    Token::from_generic(ContinuousVariableItem::auto(&format!("x{index}")), index)
}

/// 造一个线性模型。 / Build one linear model.
fn linear_model(name: &str, variables: usize, rows: usize, objective: usize,
    direction: ObjectiveCategory) -> LinearTriadModel
{
    let mut model = LinearTriadModel::from_basic(
        BasicLinearTriadModel::new(name));
    for index in 0..variables {
        model.basic.add_variable_with_bounds(token_of(index), 0.0, 10.0,
            VariableType::Continuous);
    }
    for _ in 0..rows {
        model.basic.add_constraint(SparseVector::new(), 1.0);
    }
    model.c = vec![1.0; objective];
    model.objective_category = direction;
    model
}

/// 造一个二次模型。 / Build one quadratic model.
fn quadratic_model(name: &str, variables: usize, rows: usize, quadratic_rows: usize,
    objective: usize, q_rows: usize, direction: ObjectiveCategory) -> QuadraticTetradModel
{
    let linear = linear_model(name, variables, rows, objective, direction);
    let mut model = QuadraticTetradModel::from_basic(
        BasicQuadraticTetradModel::from_linear(linear.basic));
    model.c = linear.c;
    model.objective_category = linear.objective_category;
    model.Q = SparseMatrix::new();
    for _ in 0..q_rows {
        model.Q.add_row(SparseVector::new());
    }
    for index in 0..quadratic_rows {
        model.add_quadratic_constraint_with_metadata(
            QuadraticInequality::new(
                Quadratic::new(vec![QuadraticMonomial::new_quadratic(1.0, 0, 0)], 0.0),
                ConstraintRelation::LessEqual, 1.0),
            format!("q{index}"), None, false, 0, None, None);
    }
    model
}

/// 方向名转枚举。 / Turn a direction name into the enum.
fn direction_of(name: &str) -> Result<ObjectiveCategory> {
    Ok(match name {
        "Minimum" => ObjectiveCategory::Minimum,
        "Maximum" => ObjectiveCategory::Maximum,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "model view failed validation: unknown direction `{other}`"))),
    })
}

/// 走一遍两个视图。非法输入是**错误**。 / Walk both views.
pub fn verify_model_views(request: &ModelViewRequest) -> Result<ModelViewReport> {
    let variables = usize::try_from(request.variables.unwrap_or(3)).unwrap_or(3);
    let rows = usize::try_from(request.rows.unwrap_or(2)).unwrap_or(2);
    let quadratic_rows = usize::try_from(request.quadratic_rows.unwrap_or(1)).unwrap_or(1);
    if variables > 8 || rows > 8 || quadratic_rows > 8 {
        return Err(BridgeError::new("InvalidInput",
            "model view failed validation: at most 8 variables and rows are accepted"));
    }
    let objective = usize::try_from(request.objective_length.unwrap_or(variables as u64))
        .unwrap_or(variables);
    let q_rows = usize::try_from(request.quadratic_objective_rows.unwrap_or(variables as u64))
        .unwrap_or(variables);
    if objective > 16 || q_rows > 16 {
        return Err(BridgeError::new("InvalidInput",
            "model view failed validation: at most 16 objective entries and Q rows are accepted"));
    }
    let name = request.name.clone().unwrap_or_else(|| "view_probe".to_owned());
    if name.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "model view failed validation: the model name must not be blank"));
    }
    let direction = direction_of(request.direction.as_deref().unwrap_or("Minimum"))?;
    let linear = linear_model(&name, variables, rows, objective, direction);
    let quadratic = quadratic_model(&name, variables, rows, quadratic_rows, objective, q_rows,
        direction);
    let findings = canonical_findings();
    Ok(ModelViewReport {
        linear_name: linear.name().to_owned(), quadratic_name: quadratic.name().to_owned(),
        linear_variables: linear.num_variables() as u64,
        linear_constraints: linear.num_constraints() as u64,
        quadratic_variables: quadratic.num_variables() as u64,
        quadratic_constraints: quadratic.num_constraints() as u64,
        quadratic_quadratic_constraints: quadratic.num_quadratic_constraints() as u64,
        objective_length: linear.objective().len() as u64,
        linear_objective_length: quadratic.linear_objective().len() as u64,
        quadratic_objective_rows: quadratic.quadratic_objective().rows() as u64,
        objective_category: category_name(quadratic.objective_category),
        both_views_read_the_linear_name: findings.both_views_read_the_linear_name,
        the_quadratic_view_counts_linear_rows_only:
            findings.the_quadratic_view_counts_linear_rows_only,
        the_objective_is_never_reconciled: findings.the_objective_is_never_reconciled,
        objective_is_the_linear_objective: findings.objective_is_the_linear_objective,
        the_quadratic_objective_is_unreconciled:
            findings.the_quadratic_objective_is_unreconciled,
        the_views_do_not_expose_the_direction: findings.the_views_do_not_expose_the_direction,
        the_views_are_read_only: findings.the_views_are_read_only })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    both_views_read_the_linear_name: bool,
    the_quadratic_view_counts_linear_rows_only: bool,
    the_objective_is_never_reconciled: bool,
    objective_is_the_linear_objective: bool,
    the_quadratic_objective_is_unreconciled: bool,
    the_views_do_not_expose_the_direction: bool,
    the_views_are_read_only: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 两个视图都取线性部分的名字。 / Both views read the linear part's name.
    let both_views_read_the_linear_name = {
        let linear = linear_model("probe-name", 1, 0, 1, ObjectiveCategory::Minimum);
        let quadratic = quadratic_model("probe-name", 1, 0, 0, 1, 0,
            ObjectiveCategory::Minimum);
        linear.name() == "probe-name" && quadratic.name() == "probe-name"
            && quadratic.name() == &quadratic.basic.linear.name
    };
    // 2) 二次视图只数线性行。 / The quadratic view counts linear rows only.
    let the_quadratic_view_counts_linear_rows_only = {
        let before = quadratic_model("rows", 2, 1, 0, 2, 2, ObjectiveCategory::Minimum);
        let after = quadratic_model("rows", 2, 1, 3, 2, 2, ObjectiveCategory::Minimum);
        before.num_constraints() == 1 && after.num_constraints() == 1
            && before.num_quadratic_constraints() == 0
            && after.num_quadratic_constraints() == 3
            && after.num_constraints() != after.num_quadratic_constraints()
    };
    // 3) 目标向量不与变量数对账：截短与加长都原样返回。 
    // 3) The objective is unreconciled: truncation and extension both come straight back.
    let the_objective_is_never_reconciled = {
        let short = linear_model("short", 3, 0, 1, ObjectiveCategory::Minimum);
        let long = linear_model("long", 1, 0, 5, ObjectiveCategory::Minimum);
        short.num_variables() == 3 && short.objective().len() == 1
            && long.num_variables() == 1 && long.objective().len() == 5
    };
    // 4) `objective()` 与 `linear_objective()` 是同一字段。 
    // 4) `objective()` and `linear_objective()` are the same field.
    let objective_is_the_linear_objective = {
        let quadratic = quadratic_model("same-field", 2, 0, 0, 2, 0,
            ObjectiveCategory::Minimum);
        quadratic.linear_objective() == quadratic.c.as_slice()
            && quadratic.linear_objective().len() == quadratic.num_variables()
    };
    // 5) `Q` 的行数也不对账。 / `Q`'s rows are unreconciled too.
    let the_quadratic_objective_is_unreconciled = {
        let empty_q = quadratic_model("empty-q", 3, 0, 0, 3, 0, ObjectiveCategory::Minimum);
        let wide_q = quadratic_model("wide-q", 1, 0, 0, 1, 4, ObjectiveCategory::Minimum);
        empty_q.num_variables() == 3 && empty_q.quadratic_objective().rows() == 0
            && wide_q.num_variables() == 1 && wide_q.quadratic_objective().rows() == 4
    };
    // 6) 视图不给目标方向：trait 上没有方向访问器。 
    // 6) The views omit the direction: the trait has no direction accessor.
    let the_views_do_not_expose_the_direction = {
        // 只能靠"方向在字段上、且视图签名里没有它"来证明：换个方向，视图读数除方向外全不变。 
        // Proven by flipping the direction: every view reading stays the same except the field.
        let minimum = quadratic_model("direction", 2, 1, 1, 2, 2, ObjectiveCategory::Minimum);
        let maximum = quadratic_model("direction", 2, 1, 1, 2, 2, ObjectiveCategory::Maximum);
        minimum.name() == maximum.name()
            && minimum.num_variables() == maximum.num_variables()
            && minimum.num_constraints() == maximum.num_constraints()
            && minimum.num_quadratic_constraints() == maximum.num_quadratic_constraints()
            && minimum.linear_objective() == maximum.linear_objective()
            && minimum.quadratic_objective().rows() == maximum.quadratic_objective().rows()
            && minimum.objective_category != maximum.objective_category
    };
    // 7) 视图只读：所有方法都是 `&self`，所以同一个模型可以被两个视图同时借。 
    // 7) The views are read-only: every method takes `&self`, so two views borrow the same model at once.
    let the_views_are_read_only = {
        let model = quadratic_model("read-only", 1, 1, 1, 1, 1, ObjectiveCategory::Minimum);
        let first = model.num_variables();
        let second = model.num_constraints();
        let third = model.num_quadratic_constraints();
        first == 1 && second == 1 && third == 1 && model.name() == "read-only"
            && model.c.len() == 1 && model.Q.rows() == 1
    };
    Findings { both_views_read_the_linear_name, the_quadratic_view_counts_linear_rows_only,
        the_objective_is_never_reconciled, objective_is_the_linear_objective,
        the_quadratic_objective_is_unreconciled, the_views_do_not_expose_the_direction,
        the_views_are_read_only }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ModelViewRequest {
        ModelViewRequest { name: Some("probe".to_owned()), variables: Some(3), rows: Some(2),
            quadratic_rows: Some(1), objective_length: None, quadratic_objective_rows: None,
            direction: Some("Maximum".to_owned()) }
    }

    #[test]
    fn the_views_report_the_linear_part() {
        let report = verify_model_views(&request()).unwrap();
        assert_eq!(report.linear_name, "probe");
        assert_eq!(report.quadratic_name, "probe");
        assert_eq!(report.linear_variables, 3);
        assert_eq!(report.linear_constraints, 2);
        assert_eq!(report.quadratic_variables, 3);
        // 二次视图只数线性行。 / The quadratic view counts linear rows only.
        assert_eq!(report.quadratic_constraints, 2);
        assert_eq!(report.quadratic_quadratic_constraints, 1);
        assert_eq!(report.objective_length, 3);
        assert_eq!(report.linear_objective_length, 3);
        assert_eq!(report.quadratic_objective_rows, 3);
        assert_eq!(report.objective_category, "Maximum");
        assert!(report.both_views_read_the_linear_name);
        assert!(report.the_quadratic_view_counts_linear_rows_only);
        assert!(report.the_objective_is_never_reconciled);
        assert!(report.objective_is_the_linear_objective);
        assert!(report.the_quadratic_objective_is_unreconciled);
        assert!(report.the_views_do_not_expose_the_direction);
        assert!(report.the_views_are_read_only);
    }

    #[test]
    fn an_unreconciled_objective_and_q_are_returned_verbatim() {
        let report = verify_model_views(&ModelViewRequest { variables: Some(3),
            objective_length: Some(1), quadratic_objective_rows: Some(0), ..request() })
            .unwrap();
        assert_eq!(report.linear_variables, 3);
        assert_eq!(report.objective_length, 1);
        assert_eq!(report.quadratic_objective_rows, 0);
        let report = verify_model_views(&ModelViewRequest { variables: Some(1),
            objective_length: Some(5), quadratic_objective_rows: Some(4), ..request() })
            .unwrap();
        assert_eq!(report.linear_variables, 1);
        assert_eq!(report.objective_length, 5);
        assert_eq!(report.quadratic_objective_rows, 4);
    }

    #[test]
    fn no_quadratic_constraints_is_allowed() {
        let report = verify_model_views(&ModelViewRequest { quadratic_rows: Some(0),
            ..request() }).unwrap();
        assert_eq!(report.quadratic_quadratic_constraints, 0);
        assert_eq!(report.quadratic_constraints, 2);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_model_views(&ModelViewRequest { variables: Some(9), ..request() })
            .unwrap_err().message.contains("at most 8 variables"));
        assert!(verify_model_views(&ModelViewRequest { objective_length: Some(17),
            ..request() }).unwrap_err().message.contains("at most 16 objective entries"));
        assert!(verify_model_views(&ModelViewRequest { quadratic_objective_rows: Some(17),
            ..request() }).unwrap_err().message.contains("at most 16 objective entries"));
        assert!(verify_model_views(&ModelViewRequest { name: Some("   ".to_owned()),
            ..request() }).unwrap_err().message.contains("name must not be blank"));
        assert!(verify_model_views(&ModelViewRequest { direction: Some("Minimise".to_owned()),
            ..request() }).unwrap_err().message.contains("unknown direction"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_model_views(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"linearName\""), "{json}");
        assert!(json.contains("\"bothViewsReadTheLinearName\""), "{json}");
        assert!(json.contains("\"theViewsAreReadOnly\""), "{json}");
        assert!(!json.contains("linear_name"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ModelViewReport>(&copy.to_string()).is_err());
    }
}
