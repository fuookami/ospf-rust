//! 续挖第 20 单：条件的三值语义与 `if_` / `if_legacy` 两条入口的类型分叉
//! （`symbol/functions/semantic.rs` + `conditional.rs` + `conditional_indicator.rs` + `binaryzation.rs`）
//! Continuation, delivery 20: three-valued condition semantics and the type split between `if_` and `if_legacy`.
//!
//! `semantic.rs` 一共六个入口，看起来是同一件事的别名，实测是**两条互不兼容的路**：
//!
//! 1. **`if_` / `if_named` 返回 `ConditionalIndicatorFunction`**：带 `relation` + `strict_boundary` +
//!    `ConditionBounds`，**三值**（真/假/**不可判定**）。结果变量名 `<符号名>_if`，条件指示变量名 `<符号名>_if_nz`。
//! 2. **`if_legacy` / `if_named_legacy` 返回 `BinaryzationFunction`**：只有阈值，**阈值被钉死成 `0.0`**
//!    （`auto_threshold(condition, 0.0)`）。它是**被限制过的**便利入口，不是"另一种写法"——
//!    这条路上根本没有 `relation` 与 `strict_boundary` 的概念。
//! 3. **`legacy_if` / `legacy_if_named` 是纯别名**：`legacy_if` 直接转发 `if_legacy`、`legacy_if_named` 转发
//!    `if_named_legacy`。形状完全一致，但**每个入口每次调用都新分配符号 ID 与变量组**，所以"等价"不等于"同一个对象"。
//!
//! 三值语义的真正规定在 `classify` 与 `validate_relation_bounds`，其中两条最容易踩：
//!
//! 4. **不可判定窗口随关系翻转**：`Greater` / `LessEqual` 的窗口是 `[0, boundary]`，
//!    `GreaterEqual` / `Less` 的窗口是 `[-boundary, 0]`。也就是**非严格关系照样留一段不可判定带，只是挪到了另一侧**——
//!    以为 `>=` 就没有死区是错的。
//! 5. **界完全落在窗口里才被拒**：判据是 `lower > undefined_lower && upper < undefined_upper`，
//!    所以**贴着端点**（如 `Greater` 时 `lower == 0`）是**接受**的。这是"半开拒绝"，不是"接触即拒"。
//!
//! `evaluate_difference` 把三值变成二值：真→`1`、假→`0`、**不可判定→无值**（不是 0）。而 `branch_coverage` 回答
//! "声明的界能否证明条件恒真/恒假"——恒定时急切展开会塌成两条**定值行**，这正是原生写入需要整体回退的原因。
//!
//! `semantic.rs` has six entries that look like aliases of one thing but are **two incompatible paths**. **`if_` /
//! `if_named` return `ConditionalIndicatorFunction`** with a relation, a strict boundary and bounds — **three-valued**
//! (true / false / **undecidable**) — naming the result `<symbol>_if` and the indicator `<symbol>_if_nz`. **`if_legacy` /
//! `if_named_legacy` return `BinaryzationFunction`** with **the threshold pinned to `0.0`**: a **restricted** convenience
//! entry, not another spelling — that path has no notion of a relation or a strict boundary at all. **`legacy_if` /
//! `legacy_if_named` are pure aliases** forwarding to the two legacy entries, identical in shape but **allocating a fresh
//! symbol id and variable group on every call**, so "equivalent" is not "the same object". The real three-valued rules live in
//! `classify` and `validate_relation_bounds`, with two easy mistakes: **the undecidable window flips with the relation** —
//! `[0, boundary]` for `Greater`/`LessEqual`, `[-boundary, 0]` for `GreaterEqual`/`Less`, so **a non-strict relation still
//! leaves a dead band, just on the other side** (assuming `>=` has no dead band is wrong) — and **bounds are refused only when
//! entirely inside the window** (`lower > undefined_lower && upper < undefined_upper`), so **touching an endpoint is
//! accepted**: a half-open refusal, not a contact refusal. `evaluate_difference` turns the three values into binary — true to
//! `1`, false to `0`, **undecidable to no value** (not `0`) — while `branch_coverage` answers whether the declared bounds prove
//! the condition constantly true or false; when constant, eager expansion collapses to two **fixed-value** rows, which is
//! exactly why a native write has to fall back as a whole.

use ospf_rust_core::model::flatten::{Linear, LinearMonomial};
use ospf_rust_core::symbol::functions::binaryzation::BinaryzationMethod;
use ospf_rust_core::symbol::functions::conditional::{ConditionRelation, ConditionBounds, TruthValue};
use ospf_rust_core::symbol::functions::semantic::{
    if_, if_legacy, if_named, if_named_legacy, legacy_if, legacy_if_named,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 条件多项式字面量。 / A condition polynomial literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConditionPolynomialSpec {
    /// 项（`[列索引, 系数]`）。 / The terms as `[column, coefficient]`.
    #[serde(default)]
    pub terms: Vec<Vec<f64>>,
    /// 常数项。 / The constant.
    pub constant: f64,
}

/// 条件界。 / The condition bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConditionBoundsSpec {
    pub lower: f64,
    pub upper: f64,
}

/// 三值请求。 / The three-valued request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConditionalSemanticsRequest {
    /// 条件多项式。 / The condition polynomial.
    pub condition: ConditionPolynomialSpec,
    /// 声明的界（用于 `branch_coverage`）。 / The declared bounds (for `branch_coverage`).
    pub bounds: ConditionBoundsSpec,
    /// 严格边界（必须有限且为正）。 / The strict boundary (finite and positive).
    pub strict_boundary: f64,
    /// 调用方符号名（不给则走自动命名）。 / The caller symbol name (auto-named when absent).
    #[serde(default)]
    pub name: Option<String>,
    /// 要分类的条件差值。 / The condition differences to classify.
    #[serde(default)]
    pub probes: Vec<f64>,
    /// 可分性探测的界（每条关系各跑一遍）。 / Bounds to probe for separability, per relation.
    #[serde(default)]
    pub bound_probes: Vec<ConditionBoundsSpec>,
}

/// 一次分类。 / One classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClassificationRow {
    /// 条件差值。 / The condition difference.
    pub difference: f64,
    /// `true` / `false` / `undefined`。 / `true` / `false` / `undefined`.
    pub truth: String,
    /// 二值化结果（**不可判定时无值**）。 / The binary result (**absent when undecidable**).
    #[serde(default)]
    pub binary: Option<f64>,
}

/// 一种关系的三值规定。 / One relation's three-valued rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationSemanticsRow {
    /// 关系名。 / The relation name.
    pub relation: String,
    /// 不可判定窗口的下端。 / The lower end of the undecidable window.
    pub undefined_lower: f64,
    /// 不可判定窗口的上端。 / The upper end of the undecidable window.
    pub undefined_upper: f64,
    /// 分类结果。 / The classifications.
    pub classifications: Vec<ClassificationRow>,
    /// 声明的界能否证明常量真值（无值表示不能）。 / The constant truth the bounds prove, if any.
    #[serde(default)]
    pub coverage: Option<String>,
}

/// 一次界探测。 / One bounds probe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundsProbeRow {
    pub relation: String,
    pub lower: f64,
    pub upper: f64,
    pub accepted: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

/// 一条入口的落地形状。 / One entry's shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntryShapeRow {
    /// 入口名。 / The entry name.
    pub entry: String,
    /// `conditionalIndicator` / `thresholdBinaryzation`。 / The kind.
    pub kind: String,
    /// 结果变量名。 / The result variable name.
    pub result_variable: String,
    /// 条件指示变量名（仅条件指示函数有）。 / The indicator variable name (conditional only).
    #[serde(default)]
    pub indicator_variable: Option<String>,
    /// 阈值（仅二值化有，**恒为 0**）。 / The threshold (binaryzation only, **always 0**).
    #[serde(default)]
    pub threshold: Option<f64>,
    /// 方法名（仅二值化有）。 / The method (binaryzation only).
    #[serde(default)]
    pub method: Option<String>,
    /// 结果变量组的唯一 ID（用来分辨"等价但不同对象"）。 / The result group's unique id.
    pub group_id: u64,
}

/// 一对别名。 / An alias pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AliasRow {
    /// 别名 / 目标。 / Alias and target.
    pub pair: String,
    /// 形状是否一致（阈值 / 方法 / 名字模式）。 / Whether the shapes agree.
    pub same_shape: bool,
    /// 是否同一个对象（组 ID 相同）。 / Whether it is the same object (same group id).
    pub same_identity: bool,
}

/// 三值结论。 / The three-valued verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConditionalSemanticsReport {
    /// 严格边界。 / The strict boundary.
    pub strict_boundary: f64,
    /// 四种关系的规定。 / The four relations' rules.
    pub relations: Vec<RelationSemanticsRow>,
    /// 界探测。 / The bounds probes.
    pub bounds_probes: Vec<BoundsProbeRow>,
    /// 六个入口的落地形状。 / The six entries' shapes.
    pub entries: Vec<EntryShapeRow>,
    /// 两对别名。 / The two alias pairs.
    pub aliases: Vec<AliasRow>,
    /// 不可判定窗口随关系翻转（**是**）。 / Whether the window flips with the relation (**yes**).
    pub window_flips_with_relation: bool,
    /// 贴着端点被接受（**是**）。 / Whether touching an endpoint is accepted (**yes**).
    pub touching_endpoint_is_accepted: bool,
    /// 界完全落在窗口内被拒（**是**）。 / Whether bounds inside the window are refused (**yes**).
    pub interior_is_refused: bool,
    /// 不可判定在二值化里是"无值"而不是 0（**是**）。 / Whether undecidable is absent, not zero (**yes**).
    pub undefined_has_no_binary: bool,
    /// `if_legacy` 的阈值恒为 0（**是**）。 / Whether the legacy threshold is always zero (**yes**).
    pub legacy_threshold_is_zero: bool,
    /// 别名形状一致但不是同一个对象（**是**）。 / Whether aliases agree in shape but not identity (**yes**).
    pub aliases_are_not_shared: bool,
}

/// 探测上限（信任边界）。 / The probe cap (a trust boundary).
const MAX_PROBES: usize = 1_024;

/// 走一遍条件三值语义。非法输入是**错误**。 / Walk the three-valued semantics.
pub fn verify_conditional_semantics(
    request: &ConditionalSemanticsRequest,
) -> Result<ConditionalSemanticsReport> {
    if request.probes.len() > MAX_PROBES || request.bound_probes.len() > MAX_PROBES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "conditional semantics failed validation: at most {MAX_PROBES} probes are accepted")));
    }
    let condition = build_polynomial(&request.condition)?;
    let bounds = ConditionBounds { lower: request.bounds.lower, upper: request.bounds.upper };
    // 先按 `if_` 建一次：它会把 `strict_boundary` 与界的可分性一起校验。
    // Build once through `if_`: that validates the boundary and the separability of the bounds.
    let indicator = if_::<f64>(condition.clone(), ConditionRelation::Greater, request.strict_boundary,
        ConditionBounds { lower: bounds.lower, upper: bounds.upper })
        .map_err(|error| BridgeError::new("InvalidInput",
            &format!("conditional semantics failed validation: {error}")))?;
    let mut relations = Vec::with_capacity(4);
    let mut window_flips_with_relation = false;
    let mut undefined_has_no_binary = true;
    for relation in RELATIONS {
        let row = if_::<f64>(condition.clone(), relation, request.strict_boundary,
            ConditionBounds { lower: bounds.lower, upper: bounds.upper });
        // 同一组界在非严格关系下可能被拒，这时仍然把三值规定报出来（用 `classify` 不需要建对象）。
        let classifications = request.probes.iter().map(|difference| {
            let truth = classify(*difference, relation, request.strict_boundary)?;
            let binary = match truth {
                TruthValue::True => Some(1.0),
                TruthValue::False => Some(0.0),
                TruthValue::Undefined => None,
            };
            undefined_has_no_binary &= (truth != TruthValue::Undefined) || binary.is_none();
            Ok(ClassificationRow { difference: *difference, truth: truth_name(truth), binary })
        }).collect::<Result<Vec<ClassificationRow>>>()?;
        let (undefined_lower, undefined_upper) = undefined_window(relation, request.strict_boundary);
        let coverage = match &row {
            Ok(function) => function.branch_coverage().ok().flatten().map(truth_name),
            Err(_) => None,
        };
        relations.push(RelationSemanticsRow { relation: relation_name(relation),
            undefined_lower, undefined_upper, classifications, coverage });
    }
    // 窗口确实随关系翻转：`[0, b]` 与 `[-b, 0]` 同时在表里。
    window_flips_with_relation = relations.iter()
        .any(|row| row.undefined_lower == 0.0 && row.undefined_upper > 0.0)
        && relations.iter().any(|row| row.undefined_lower < 0.0 && row.undefined_upper == 0.0);
    // 界探测：完全在窗口外、贴端点、完全在窗口内三档。 / Three tiers: outside, touching, inside.
    let mut bounds_probes = Vec::new();
    let mut touching_endpoint_is_accepted = true;
    let mut interior_is_refused = true;
    for probe in &request.bound_probes {
        for relation in RELATIONS {
            match if_::<f64>(condition.clone(), relation, request.strict_boundary,
                ConditionBounds { lower: probe.lower, upper: probe.upper })
            {
                Ok(_) => bounds_probes.push(BoundsProbeRow { relation: relation_name(relation),
                    lower: probe.lower, upper: probe.upper, accepted: true, reason: None }),
                Err(error) => bounds_probes.push(BoundsProbeRow { relation: relation_name(relation),
                    lower: probe.lower, upper: probe.upper, accepted: false,
                    reason: Some(error.to_string()) }),
            }
        }
    }
    // 六个入口的落地形状。 / The six entries' shapes.
    let name = request.name.as_deref().unwrap_or("probe");
    let conditional_auto = if_(condition.clone(), ConditionRelation::Greater,
        request.strict_boundary, bounds.clone()).map_err(as_bridge)?;
    let conditional_named = if_named(name, condition.clone(), ConditionRelation::Greater,
        request.strict_boundary, bounds.clone()).map_err(as_bridge)?;
    let binary_auto = if_legacy(condition.clone());
    let binary_named = if_named_legacy(name, condition.clone());
    let alias_auto = legacy_if(condition.clone());
    let alias_named = legacy_if_named(name, condition.clone());
    let entries = vec![
        EntryShapeRow { entry: "if_".to_owned(), kind: "conditionalIndicator".to_owned(),
            result_variable: conditional_auto.result_variable().name().to_owned(),
            indicator_variable: Some(conditional_auto.condition_indicator_variable().name().to_owned()),
            threshold: None, method: None,
            group_id: conditional_auto.result_variable().id().unique_id() },
        EntryShapeRow { entry: "if_named".to_owned(), kind: "conditionalIndicator".to_owned(),
            result_variable: conditional_named.result_variable().name().to_owned(),
            indicator_variable: Some(conditional_named.condition_indicator_variable().name().to_owned()),
            threshold: None, method: None,
            group_id: conditional_named.result_variable().id().unique_id() },
        EntryShapeRow { entry: "if_legacy".to_owned(), kind: "thresholdBinaryzation".to_owned(),
            result_variable: binary_auto.result_variable().name().to_owned(),
            indicator_variable: None, threshold: Some(*binary_auto.threshold()),
            method: Some(method_name(binary_auto.method())),
            group_id: binary_auto.result_variable().id().unique_id() },
        EntryShapeRow { entry: "if_named_legacy".to_owned(), kind: "thresholdBinaryzation".to_owned(),
            result_variable: binary_named.result_variable().name().to_owned(),
            indicator_variable: None, threshold: Some(*binary_named.threshold()),
            method: Some(method_name(binary_named.method())),
            group_id: binary_named.result_variable().id().unique_id() },
        EntryShapeRow { entry: "legacy_if".to_owned(), kind: "thresholdBinaryzation".to_owned(),
            result_variable: alias_auto.result_variable().name().to_owned(),
            indicator_variable: None, threshold: Some(*alias_auto.threshold()),
            method: Some(method_name(alias_auto.method())),
            group_id: alias_auto.result_variable().id().unique_id() },
        EntryShapeRow { entry: "legacy_if_named".to_owned(), kind: "thresholdBinaryzation".to_owned(),
            result_variable: alias_named.result_variable().name().to_owned(),
            indicator_variable: None, threshold: Some(*alias_named.threshold()),
            method: Some(method_name(alias_named.method())),
            group_id: alias_named.result_variable().id().unique_id() },
    ];
    let legacy_threshold_is_zero = entries.iter()
        .filter(|row| row.kind == "thresholdBinaryzation")
        .all(|row| row.threshold == Some(0.0));
    // 别名对：形状一致，身份不同。 / The alias pairs agree in shape and differ in identity.
    let aliases = vec![
        AliasRow { pair: "legacy_if/if_legacy".to_owned(),
            same_shape: binary_auto.threshold() == alias_auto.threshold()
                && binary_auto.method() == alias_auto.method(),
            same_identity: binary_auto.result_variable().id().unique_id()
                == alias_auto.result_variable().id().unique_id() },
        AliasRow { pair: "legacy_if_named/if_named_legacy".to_owned(),
            same_shape: binary_named.threshold() == alias_named.threshold()
                && binary_named.method() == alias_named.method(),
            same_identity: binary_named.result_variable().id().unique_id()
                == alias_named.result_variable().id().unique_id() },
    ];
    let aliases_are_not_shared = aliases.iter()
        .all(|row| row.same_shape && !row.same_identity);
    // 两条"半开拒绝"的实测：贴端点接受、完全在窗口内被拒。
    for probe in &request.bound_probes {
        for relation in RELATIONS {
            let (lower, upper) = undefined_window(relation, request.strict_boundary);
            // 完全在窗口内部（严格内缩）→ 必须被拒。
            if probe.lower > lower && probe.upper < upper {
                interior_is_refused &= if_::<f64>(condition.clone(), relation,
                    request.strict_boundary, ConditionBounds { lower: probe.lower, upper: probe.upper })
                    .is_err();
            }
            // 贴着窗口端点（一端相等、另一端在外）→ 必须被接受。
            if (probe.lower == lower && probe.upper >= upper)
                || (probe.upper == upper && probe.lower <= lower)
            {
                touching_endpoint_is_accepted &= if_::<f64>(condition.clone(), relation,
                    request.strict_boundary, ConditionBounds { lower: probe.lower, upper: probe.upper })
                    .is_ok();
            }
        }
    }
    // 空探测表时这两条无从验证，按"规则未被推翻"处理。 / With no probes, the two findings stand unrefuted.
    let _ = &indicator;
    Ok(ConditionalSemanticsReport {
        strict_boundary: request.strict_boundary,
        relations,
        bounds_probes,
        entries,
        aliases,
        window_flips_with_relation,
        touching_endpoint_is_accepted,
        interior_is_refused,
        undefined_has_no_binary,
        legacy_threshold_is_zero,
        aliases_are_not_shared,
    })
}

/// 四种关系。 / The four relations.
const RELATIONS: [ConditionRelation; 4] = [
    ConditionRelation::Greater,
    ConditionRelation::GreaterEqual,
    ConditionRelation::Less,
    ConditionRelation::LessEqual,
];

/// 三值分类（与 `ConditionalIndicatorFunction::classify` 同源）。 / Classify a difference.
fn classify(difference: f64, relation: ConditionRelation, boundary: f64) -> Result<TruthValue> {
    if !difference.is_finite() {
        return Err(BridgeError::new("InvalidInput",
            "conditional semantics failed validation: a probe must be finite"));
    }
    Ok(match relation {
        ConditionRelation::Greater => {
            if difference >= boundary { TruthValue::True }
            else if difference <= 0.0 { TruthValue::False }
            else { TruthValue::Undefined }
        }
        ConditionRelation::GreaterEqual => {
            if difference >= 0.0 { TruthValue::True }
            else if difference <= -boundary { TruthValue::False }
            else { TruthValue::Undefined }
        }
        ConditionRelation::Less => {
            if difference <= -boundary { TruthValue::True }
            else if difference >= 0.0 { TruthValue::False }
            else { TruthValue::Undefined }
        }
        ConditionRelation::LessEqual => {
            if difference <= 0.0 { TruthValue::True }
            else if difference >= boundary { TruthValue::False }
            else { TruthValue::Undefined }
        }
    })
}

/// 不可判定窗口。 / The undecidable window.
fn undefined_window(relation: ConditionRelation, boundary: f64) -> (f64, f64) {
    match relation {
        ConditionRelation::Greater | ConditionRelation::LessEqual => (0.0, boundary),
        ConditionRelation::GreaterEqual | ConditionRelation::Less => (-boundary, 0.0),
    }
}

/// 关系名。 / The relation name.
fn relation_name(relation: ConditionRelation) -> String {
    match relation {
        ConditionRelation::Greater => "greater",
        ConditionRelation::GreaterEqual => "greaterEqual",
        ConditionRelation::Less => "less",
        ConditionRelation::LessEqual => "lessEqual",
    }.to_owned()
}

/// 三值名。 / The truth value name.
fn truth_name(truth: TruthValue) -> String {
    match truth {
        TruthValue::True => "true",
        TruthValue::False => "false",
        TruthValue::Undefined => "undefined",
    }.to_owned()
}

/// 二值化方法名。 / The binaryzation method name.
fn method_name(method: BinaryzationMethod) -> String {
    format!("{method:?}")
}

/// 错误转换。 / The error conversion.
fn as_bridge(error: ospf_rust_core::error::CoreError) -> BridgeError {
    BridgeError::new("InvalidInput", &format!("conditional semantics failed validation: {error}"))
}

/// 建一个 `Linear<f64>`。 / Build a `Linear<f64>`.
fn build_polynomial(spec: &ConditionPolynomialSpec) -> Result<Linear<f64>> {
    if !spec.constant.is_finite() {
        return Err(BridgeError::new("InvalidInput",
            "conditional semantics failed validation: the constant must be finite"));
    }
    let mut monomials = Vec::with_capacity(spec.terms.len());
    for term in &spec.terms {
        if term.len() != 2 {
            return Err(BridgeError::new("InvalidInput",
                "conditional semantics failed validation: terms are [column, coefficient] pairs"));
        }
        let column = term[0];
        if !column.is_finite() || column < 0.0 || column.fract() != 0.0 {
            return Err(BridgeError::new("InvalidInput",
                "conditional semantics failed validation: the column must be a non-negative integer"));
        }
        if !term[1].is_finite() {
            return Err(BridgeError::new("InvalidInput",
                "conditional semantics failed validation: the coefficient must be finite"));
        }
        monomials.push(LinearMonomial::new(term[1], column as usize));
    }
    Ok(Linear::new(monomials, spec.constant))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(probes: &[f64], bound_probes: &[(f64, f64)]) -> ConditionalSemanticsRequest {
        ConditionalSemanticsRequest {
            condition: ConditionPolynomialSpec { terms: vec![vec![0.0, 1.0]], constant: 0.0 },
            bounds: ConditionBoundsSpec { lower: -5.0, upper: 5.0 },
            strict_boundary: 1.0,
            name: None,
            probes: probes.to_vec(),
            bound_probes: bound_probes.iter()
                .map(|(lower, upper)| ConditionBoundsSpec { lower: *lower, upper: *upper })
                .collect(),
        }
    }

    fn relation<'a>(report: &'a ConditionalSemanticsReport, name: &str)
        -> &'a RelationSemanticsRow
    {
        report.relations.iter().find(|row| row.relation == name)
            .unwrap_or_else(|| panic!("relation {name}"))
    }

    fn truth(row: &RelationSemanticsRow, difference: f64) -> String {
        row.classifications.iter().find(|cell| cell.difference == difference)
            .unwrap_or_else(|| panic!("difference {difference}")).truth.clone()
    }

    #[test]
    fn the_undecidable_window_flips_with_the_relation() {
        let report = verify_conditional_semantics(&request(&[], &[])).unwrap();
        assert!(report.window_flips_with_relation);
        // `Greater` / `LessEqual` 的窗口是 `[0, b]`。 / `[0, b]`.
        assert_eq!(relation(&report, "greater").undefined_lower, 0.0);
        assert_eq!(relation(&report, "greater").undefined_upper, 1.0);
        assert_eq!(relation(&report, "lessEqual").undefined_lower, 0.0);
        assert_eq!(relation(&report, "lessEqual").undefined_upper, 1.0);
        // `GreaterEqual` / `Less` 的窗口是 `[-b, 0]`——非严格关系照样有死区。 
        // `GreaterEqual` / `Less` use `[-b, 0]`: a non-strict relation still has a dead band.
        assert_eq!(relation(&report, "greaterEqual").undefined_lower, -1.0);
        assert_eq!(relation(&report, "greaterEqual").undefined_upper, 0.0);
        assert_eq!(relation(&report, "less").undefined_lower, -1.0);
        assert_eq!(relation(&report, "less").undefined_upper, 0.0);
    }

    #[test]
    fn classification_and_the_missing_binary_value() {
        let report = verify_conditional_semantics(
            &request(&[-2.0, -0.5, 0.0, 0.5, 1.0, 2.0], &[])).unwrap();
        assert!(report.undefined_has_no_binary);
        // `Greater`：`>= 1` 真、`<= 0` 假、中间不可判定。 / `Greater`: `>= 1` true, `<= 0` false, the gap undefined.
        let greater = relation(&report, "greater");
        assert_eq!(truth(greater, 1.0), "true");
        assert_eq!(truth(greater, 2.0), "true");
        assert_eq!(truth(greater, 0.0), "false");
        assert_eq!(truth(greater, -2.0), "false");
        assert_eq!(truth(greater, 0.5), "undefined");
        // `GreaterEqual`：`>= 0` 真、`<= -1` 假、`(-1, 0)` 不可判定。 
        // `GreaterEqual`: `>= 0` true, `<= -1` false, `(-1, 0)` undefined.
        let greater_equal = relation(&report, "greaterEqual");
        assert_eq!(truth(greater_equal, 0.0), "true");
        assert_eq!(truth(greater_equal, 2.0), "true");
        assert_eq!(truth(greater_equal, -2.0), "false");
        assert_eq!(truth(greater_equal, -0.5), "undefined");
        // `Less`：`<= -1` 真、`>= 0` 假。 / `Less`: `<= -1` true, `>= 0` false.
        let less = relation(&report, "less");
        assert_eq!(truth(less, -2.0), "true");
        assert_eq!(truth(less, 0.0), "false");
        assert_eq!(truth(less, -0.5), "undefined");
        // 不可判定的二值化结果是**无值**，不是 0。 / The binary result of an undecidable value is absent, not zero.
        let cell = greater.classifications.iter().find(|cell| cell.difference == 0.5).unwrap();
        assert_eq!(cell.binary, None);
        let cell = greater.classifications.iter().find(|cell| cell.difference == 1.0).unwrap();
        assert_eq!(cell.binary, Some(1.0));
        let cell = greater.classifications.iter().find(|cell| cell.difference == 0.0).unwrap();
        assert_eq!(cell.binary, Some(0.0));
    }

    #[test]
    fn touching_an_endpoint_is_accepted_but_the_interior_is_refused() {
        // `[-5, 5]` 远在窗口外 → 四条关系都接受。 / `[-5, 5]` is far outside: every relation accepts.
        // `[0, 2]` 贴着 `Greater` 窗口下端 → 接受。 / `[0, 2]` touches the lower end: accepted.
        // `(0.1, 0.9)` 完全落在 `Greater` 窗口内 → 只有 `Greater` 被拒。 
        // `(0.1, 0.9)` sits inside the `Greater` window: `Greater` is refused.
        let report = verify_conditional_semantics(&request(&[],
            &[(-5.0, 5.0), (0.0, 2.0), (0.1, 0.9)])).unwrap();
        assert!(report.touching_endpoint_is_accepted);
        assert!(report.interior_is_refused);
        let probe = |relation: &str, lower: f64, upper: f64| report.bounds_probes.iter()
            .find(|row| row.relation == relation && row.lower == lower && row.upper == upper)
            .unwrap_or_else(|| panic!("{relation} [{lower}, {upper}]"));
        assert!(probe("greater", -5.0, 5.0).accepted);
        assert!(probe("greaterEqual", -5.0, 5.0).accepted);
        // 贴端点接受。 / Touching is accepted.
        assert!(probe("greater", 0.0, 2.0).accepted);
        // 完全在窗口内被拒（`Greater` 的窗口是 `[0, 1]`）。 / Inside the `[0, 1]` window: refused.
        assert!(!probe("greater", 0.1, 0.9).accepted);
        assert!(probe("greater", 0.1, 0.9).reason.as_deref().unwrap().contains("Undefined interval"));
        // 同样的界对 `GreaterEqual`（窗口 `[-1, 0]`）是接受的。 / The same bounds are fine for `GreaterEqual`.
        assert!(probe("greaterEqual", 0.1, 0.9).accepted);
    }

    #[test]
    fn the_two_paths_return_different_types_and_the_legacy_threshold_is_zero() {
        let report = verify_conditional_semantics(&request(&[], &[])).unwrap();
        assert_eq!(report.entries.len(), 6);
        let entry = |name: &str| report.entries.iter().find(|row| row.entry == name)
            .unwrap_or_else(|| panic!("entry {name}"));
        // `if_` / `if_named` → 条件指示函数，带结果变量与指示变量。 
        // `if_` / `if_named` give the conditional indicator with both variables.
        assert_eq!(entry("if_").kind, "conditionalIndicator");
        // 按实测纠正：`if_` **不用调用方的名字**（它没有这个参数），自动名是 `conditional_indicator_<id>`；
        // 只有 `if_named` 才把调用方名字接上去。
        // Corrected by measurement: `if_` has no caller name and auto-names as `conditional_indicator_<id>`; only
        // `if_named` prepends the caller name.
        let auto = &entry("if_").result_variable;
        assert!(auto.starts_with("conditional_indicator_"), "{auto}");
        assert!(auto.ends_with("_if"), "{auto}");
        assert_eq!(entry("if_").indicator_variable.as_deref(),
            Some(format!("{}_nz", auto).as_str()));
        assert_eq!(entry("if_named").result_variable, "probe_if");
        assert_eq!(entry("if_named").indicator_variable.as_deref(), Some("probe_if_nz"));
        // `if_legacy` 家族 → 阈值二值化，**阈值恒为 0**、没有指示变量。 
        // The legacy family is threshold binaryzation: **threshold always 0**, no indicator variable.
        assert!(report.legacy_threshold_is_zero);
        for name in ["if_legacy", "if_named_legacy", "legacy_if", "legacy_if_named"] {
            let row = entry(name);
            assert_eq!(row.kind, "thresholdBinaryzation", "{name}");
            assert_eq!(row.threshold, Some(0.0), "{name}");
            assert_eq!(row.indicator_variable, None, "{name}");
            assert!(row.method.is_some(), "{name}");
        }
        // 别名形状一致，但组 ID 不同 → 每次都是新对象。 
        // The aliases agree in shape but differ in group id: a fresh object every call.
        assert_eq!(report.aliases.len(), 2);
        assert!(report.aliases_are_not_shared);
        for row in &report.aliases {
            assert!(row.same_shape, "{}", row.pair);
            assert!(!row.same_identity, "{}", row.pair);
        }
        // 六个入口的组 ID 全不相同。 / All six group ids differ.
        let mut ids: Vec<u64> = report.entries.iter().map(|row| row.group_id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 6);
    }

    #[test]
    fn coverage_reports_a_constant_condition() {
        // 界 `[2, 5]` 让 `Greater`（边界 1）恒真。 / The bounds `[2, 5]` make `Greater` constantly true.
        let mut document = request(&[], &[]);
        document.bounds = ConditionBoundsSpec { lower: 2.0, upper: 5.0 };
        let report = verify_conditional_semantics(&document).unwrap();
        assert_eq!(relation(&report, "greater").coverage.as_deref(), Some("true"));
        // 界 `[-5, -2]` 让 `Greater` 恒假。 / `[-5, -2]` makes it constantly false.
        let mut document = request(&[], &[]);
        document.bounds = ConditionBoundsSpec { lower: -5.0, upper: -2.0 };
        let report = verify_conditional_semantics(&document).unwrap();
        assert_eq!(relation(&report, "greater").coverage.as_deref(), Some("false"));
        // 跨过窗口的界无法证明常量真值。 / Bounds spanning the window prove nothing.
        let report = verify_conditional_semantics(&request(&[], &[])).unwrap();
        assert_eq!(relation(&report, "greater").coverage, None);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut document = request(&[], &[]);
        document.strict_boundary = 0.0;
        assert!(verify_conditional_semantics(&document).unwrap_err().message
            .contains("finite and positive"));
        let mut document = request(&[], &[]);
        document.strict_boundary = -1.0;
        assert!(verify_conditional_semantics(&document).unwrap_err().message
            .contains("finite and positive"));
        // 按实测纠正：`NaN` 先被 `finite_value` 拦下，措辞是"必须有限"，不是"有限且为正"。
        // Corrected: `NaN` trips `finite_value` first, so the wording is "must be finite", not "finite and positive".
        let mut document = request(&[], &[]);
        document.strict_boundary = f64::NAN;
        let message = verify_conditional_semantics(&document).unwrap_err().message;
        assert!(message.contains("strict boundary"), "{message}");
        assert!(message.contains("must be finite"), "{message}");
        let mut document = request(&[], &[]);
        document.condition.terms = vec![vec![0.0]];
        assert!(verify_conditional_semantics(&document).unwrap_err().message.contains("pairs"));
        let mut document = request(&[], &[]);
        document.condition.constant = f64::INFINITY;
        assert!(verify_conditional_semantics(&document).unwrap_err().message.contains("constant"));
        assert!(verify_conditional_semantics(&request(&[f64::NAN], &[])).unwrap_err().message
            .contains("must be finite"));
        let mut document = request(&[], &[]);
        document.probes = vec![0.0; MAX_PROBES + 1];
        assert!(verify_conditional_semantics(&document).unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。第四次踩到同一个坑，这条守住。
    /// A guard: DTOs must be camelCase and reject unknown fields. Missed four times; this holds the line.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_conditional_semantics(&request(&[], &[])).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"strictBoundary\""), "{json}");
        assert!(json.contains("\"undefinedLower\""), "{json}");
        assert!(json.contains("\"boundsProbes\""), "{json}");
        assert!(json.contains("\"resultVariable\""), "{json}");
        assert!(!json.contains("strict_boundary"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ConditionalSemanticsReport>(&copy.to_string()).is_err());
        // 嵌套结构也必须拒绝未知字段。 / Nested structures must reject unknown fields too.
        let mut nested = copy.clone();
        nested.as_object_mut().unwrap().remove("unexpected");
        nested["relations"][0]["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ConditionalSemanticsReport>(&nested.to_string()).is_err());
    }
}
