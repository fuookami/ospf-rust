//! 批次 F 第 3 单：三个函数族的**即时展开核心关系表** / Batch F, delivery 3: the eager-expansion core relation tables of
//! three function families.
//!
//! 这一单交付的是 `inequality` / `imply` / `if_in` 里三个**公开且纯**的表函数——它们不持有模型、不建变量，只回答
//! "这套编码到底长什么样"：
//!
//! * `indicator_core_relations(kind)` —— 关系指示的取真/取假核心关系与两条 Big-M 松弛。**`equal` 与 `notEqual` 返回空**：
//!   它们的取假侧是**析取**（`s <= -boundary` 或 `s >= boundary`），两条指示约束表达不了。这条最容易被误以为"应该
//!   支持"，所以报告把 `supported` 直接标出来；
//! * `imply_coupling_indicators()` —— 蕴含耦合的三条指示约束（前提出发两条、结论出发一条）；
//! * `in_values_value_core_relations()` —— `InValues` 的 band 核心关系：取真时是 `[−tol, tol]` 的**一对**关系，
//!   另有 side 列的两个方向与 `strict_boundary`。
//!
//! 容差都是**公开常量**（`INDICATOR_TOLERANCE = 1e-10`、`STEP_EPSILON = 1e-8`、`STRICT_BOUNDARY = 2·STEP_EPSILON`），
//! 报告原样给出：这些是数值稳定性取值，TS 侧不该自己拍一组。
//!
//! These three table functions are **public and pure** — they hold no model and create no variable, and only answer "what
//! does this encoding actually look like": `indicator_core_relations(kind)` (the true/false core relations of a relational
//! indicator plus both big-M relaxations, where **`equal` and `notEqual` return nothing** because their false side is a
//! **disjunction** of two rows and cannot be expressed by two indicator constraints — the easiest thing to expect to be
//! supported, so `supported` is stated explicitly); `imply_coupling_indicators()` (the three indicator constraints of the
//! implication coupling, two keyed on the premise and one on the consequence); and `in_values_value_core_relations()` (the
//! InValues band: a **pair** of relations for `[-tol, tol]` when true, plus both side directions and `strict_boundary`). The
//! tolerances are **public constants** (`INDICATOR_TOLERANCE = 1e-10`, `STEP_EPSILON = 1e-8`, `STRICT_BOUNDARY =
//! 2·STEP_EPSILON`) and are reported verbatim: they are numerical-stability values TypeScript should not invent again.

use ospf_rust_core::model::ConstraintRelation;
use ospf_rust_core::symbol::function::if_in::{
    in_values_value_core_relations, STEP_EPSILON, STRICT_BOUNDARY,
};
use ospf_rust_core::symbol::function::imply::imply_coupling_indicators;
use ospf_rust_core::symbol::function::inequality::{
    indicator_core_relations, InequalityKind, INDICATOR_TOLERANCE,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Result;

/// 核心关系表的请求：**没有参数**。/ The core-relation-table request: **no parameters**.
///
/// 保留一个空对象而不是 `null`，是为了让 native 层的"每个 kind 有自己字段"约定保持一致：字段存在即表示调用方
/// 选了这个 kind，字段内容为空说明这个 kind 不需要输入。
/// An empty object instead of `null` keeps the native layer's "each kind has its own field" convention intact: the field
/// being present selects the kind, and its emptiness says the kind needs no input.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolCoreRelationsRequest {}

/// 一条核心关系 `s REL rhs`。 / One core relation `s REL rhs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolCoreRelation {
    /// 关系：`lessEqual` / `equal` / `greaterEqual`。 / The relation.
    pub relation: String,
    /// 右端项。 / The right-hand side.
    pub rhs: f64,
}

/// 关系指示的即时展开。 / The eager expansion of a relational indicator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolInequalityRow {
    /// 请求的关系种类（线格式代码）。 / The requested kind (wire code).
    pub kind: String,
    /// 这一层是否支持（`equal` / `notEqual` 不支持：取假侧是析取）。
    /// Whether this layer supports it (`equal` and `notEqual` are not: their false side is a disjunction).
    pub supported: bool,
    /// 指示列取真时的核心关系（不支持时为 `undefined`）。 / The core relation when true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub when_true: Option<SymbolCoreRelation>,
    /// 指示列取假时的核心关系（不支持时为 `undefined`）。 / The core relation when false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub when_false: Option<SymbolCoreRelation>,
    /// 下侧松弛右端项（不支持时为 `undefined`）。 / The lower relaxation right-hand side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub relaxed_lower_rhs: Option<f64>,
    /// 上侧松弛右端项（不支持时为 `undefined`）。 / The upper relaxation right-hand side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub relaxed_upper_rhs: Option<f64>,
}

/// 一条蕴含耦合指示约束。 / One implication coupling indicator constraint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolImplyRow {
    /// 指示列取自前提（`true`）还是结论（`false`）。 / Whether the indicator is keyed on the premise.
    pub keyed_on_premise: bool,
    /// 指示列取值。 / The indicator column's value.
    pub indicator_value: bool,
    /// 条件关系。 / The condition's relation.
    pub relation: String,
    /// 对侧子指示器结果列的系数。 / The opposite sub-indicator's coefficient.
    pub other_coefficient: f64,
    /// 条件右端项。 / The condition's right-hand side.
    pub rhs: f64,
}

/// `InValues` 的 band 展开。 / The InValues band expansion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolInValuesTable {
    /// 候选值指示列取真时的**一对**核心关系。 / The **pair** of core relations when the indicator is true.
    pub when_indicator_true: Vec<SymbolCoreRelation>,
    /// side 列取真时的核心关系。 / The core relation when the side column is true.
    pub when_side_true: SymbolCoreRelation,
    /// side 列取假时的核心关系。 / The core relation when the side column is false.
    pub when_side_false: SymbolCoreRelation,
    /// 下侧松弛右端项。 / The lower relaxation right-hand side.
    pub relaxed_lower_rhs: f64,
    /// 上侧松弛右端项。 / The upper relaxation right-hand side.
    pub relaxed_upper_rhs: f64,
    /// band 行使用的容差。 / The tolerance the band rows use.
    pub band_tolerance: f64,
    /// 「不在集合内」一侧与候选值之间的最小距离。 / The minimum distance from a candidate when outside the set.
    pub strict_boundary: f64,
}

/// 核心关系表的结论。 / The core-relation-table verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolCoreRelationsReport {
    /// 关系种类词表（桥接定义，6 种）。 / The kind vocabulary (bridge-defined, six entries).
    pub kind_vocabulary: Vec<String>,
    /// 指示容差（`INDICATOR_TOLERANCE`）。 / The indicator tolerance.
    pub indicator_tolerance: f64,
    /// band 步进容差（`STEP_EPSILON`）。 / The band step tolerance.
    pub step_epsilon: f64,
    /// InValues 的严格边界（`STRICT_BOUNDARY`）。 / The InValues strict boundary.
    pub strict_boundary: f64,
    /// 支持的关系种类数。 / How many kinds are supported.
    pub supported_kind_count: u32,
    /// 关系指示的逐种类展开。 / The per-kind indicator expansions.
    pub inequalities: Vec<SymbolInequalityRow>,
    /// 蕴含耦合的三条指示约束。 / The three implication coupling constraints.
    pub imply: Vec<SymbolImplyRow>,
    /// `InValues` 的 band 展开。 / The InValues band expansion.
    pub in_values: SymbolInValuesTable,
}

/// 给出三个函数族的即时展开核心关系表。纯读：不建变量、不生成约束。
/// Report the eager-expansion core relation tables of the three families. Pure reads: no variable and no constraint.
pub fn verify_symbol_core_relations() -> Result<SymbolCoreRelationsReport> {
    // 顺序与领域枚举声明一致：非严格两型、严格两型、然后 `equal` / `notEqual`。
    // The order matches the domain enum's declaration: the two non-strict kinds, the two strict ones, then
    // `equal` / `notEqual`.
    let kinds = [InequalityKind::LessEqual, InequalityKind::GreaterEqual, InequalityKind::Less,
        InequalityKind::Greater, InequalityKind::Equal, InequalityKind::NotEqual];
    let mut inequalities = Vec::with_capacity(kinds.len());
    let mut supported_kind_count = 0u32;
    for kind in kinds {
        let table = indicator_core_relations(kind);
        if table.is_some() { supported_kind_count = supported_kind_count.saturating_add(1); }
        inequalities.push(SymbolInequalityRow {
            kind: kind_code(kind),
            supported: table.is_some(),
            when_true: table.as_ref().map(|table| relation(table.when_true)),
            when_false: table.as_ref().map(|table| relation(table.when_false)),
            relaxed_lower_rhs: table.as_ref().map(|table| table.relaxed_lower_rhs),
            relaxed_upper_rhs: table.as_ref().map(|table| table.relaxed_upper_rhs),
        });
    }
    let in_values = in_values_value_core_relations();
    Ok(SymbolCoreRelationsReport {
        kind_vocabulary: kinds.iter().map(|kind| kind_code(*kind)).collect(),
        indicator_tolerance: INDICATOR_TOLERANCE,
        step_epsilon: STEP_EPSILON,
        strict_boundary: STRICT_BOUNDARY,
        supported_kind_count,
        inequalities,
        imply: imply_coupling_indicators().into_iter().map(|coupling| SymbolImplyRow {
            keyed_on_premise: coupling.keyed_on_premise,
            indicator_value: coupling.indicator_value,
            relation: relation_code(coupling.relation),
            other_coefficient: coupling.other_coefficient,
            rhs: coupling.rhs,
        }).collect(),
        in_values: SymbolInValuesTable {
            when_indicator_true: in_values.when_indicator_true.iter()
                .map(|pair| relation(*pair)).collect(),
            when_side_true: relation(in_values.when_side_true),
            when_side_false: relation(in_values.when_side_false),
            relaxed_lower_rhs: in_values.relaxed_lower_rhs,
            relaxed_upper_rhs: in_values.relaxed_upper_rhs,
            band_tolerance: in_values.band_tolerance,
            strict_boundary: in_values.strict_boundary,
        },
    })
}

/// 组装一条核心关系。 / Assemble one core relation.
fn relation(pair: (ConstraintRelation, f64)) -> SymbolCoreRelation {
    SymbolCoreRelation { relation: relation_code(pair.0), rhs: pair.1 }
}

/// 关系种类的线格式代码。 / The wire code of a kind.
fn kind_code(kind: InequalityKind) -> String {
    match kind {
        InequalityKind::LessEqual => "lessEqual".to_owned(),
        InequalityKind::GreaterEqual => "greaterEqual".to_owned(),
        InequalityKind::Less => "less".to_owned(),
        InequalityKind::Greater => "greater".to_owned(),
        InequalityKind::Equal => "equal".to_owned(),
        InequalityKind::NotEqual => "notEqual".to_owned(),
    }
}

/// 约束关系的线格式代码。 / The wire code of a constraint relation.
fn relation_code(relation: ConstraintRelation) -> String {
    match relation {
        ConstraintRelation::LessEqual => "lessEqual".to_owned(),
        ConstraintRelation::Equal => "equal".to_owned(),
        ConstraintRelation::GreaterEqual => "greaterEqual".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicator_tables_are_reported_with_the_unsupported_kinds() {
        let report = verify_symbol_core_relations().unwrap();
        assert_eq!(report.kind_vocabulary,
            vec!["lessEqual".to_owned(), "greaterEqual".to_owned(), "less".to_owned(),
                "greater".to_owned(), "equal".to_owned(), "notEqual".to_owned()]);
        assert_eq!(report.indicator_tolerance, 1.0e-10);
        assert_eq!(report.step_epsilon, 1.0e-8);
        assert_eq!(report.strict_boundary, 2.0e-8);
        // 六种里支持四种：`equal` / `notEqual` 的取假侧是析取。 / Four of six are supported.
        assert_eq!(report.supported_kind_count, 4);
        assert_eq!(report.inequalities.len(), 6);
        let row = |kind: &str| report.inequalities.iter()
            .find(|row| row.kind == kind).unwrap_or_else(|| panic!("missing {kind}"));
        // 不支持的两条：没有关系、也没有松弛量。 / The two unsupported kinds carry neither relations nor relaxations.
        for kind in ["equal", "notEqual"] {
            let unsupported = row(kind);
            assert!(!unsupported.supported, "{kind}");
            assert_eq!(unsupported.when_true, None, "{kind}");
            assert_eq!(unsupported.when_false, None, "{kind}");
            assert_eq!(unsupported.relaxed_lower_rhs, None, "{kind}");
            assert_eq!(unsupported.relaxed_upper_rhs, None, "{kind}");
        }
        // 非严格 `lessEqual`：取真 `s <= 0`，取假外扩 ε 得 `s >= ε`。 / Non-strict lessEqual expands outward.
        let less_equal = row("lessEqual");
        assert!(less_equal.supported);
        assert_eq!(less_equal.when_true, Some(SymbolCoreRelation {
            relation: "lessEqual".to_owned(), rhs: 0.0 }));
        assert_eq!(less_equal.when_false, Some(SymbolCoreRelation {
            relation: "greaterEqual".to_owned(), rhs: report.indicator_tolerance }));
        assert_eq!(less_equal.relaxed_lower_rhs, Some(report.indicator_tolerance));
        assert_eq!(less_equal.relaxed_upper_rhs, Some(0.0));
        // 严格 `less`：取真**内缩** ε 得 `s <= -ε`，取假为非严格 `s >= 0`。
        // Strict `less` shrinks inward: `s <= -ε` when true and the non-strict `s >= 0` when false.
        let strict_less = row("less");
        assert_eq!(strict_less.when_true, Some(SymbolCoreRelation {
            relation: "lessEqual".to_owned(), rhs: -report.indicator_tolerance }));
        assert_eq!(strict_less.when_false, Some(SymbolCoreRelation {
            relation: "greaterEqual".to_owned(), rhs: 0.0 }));
        // `greaterEqual` / `greater` 是前两者的镜像。 / The two `greater` kinds mirror them.
        assert_eq!(row("greaterEqual").when_true, Some(SymbolCoreRelation {
            relation: "greaterEqual".to_owned(), rhs: 0.0 }));
        assert_eq!(row("greater").when_true, Some(SymbolCoreRelation {
            relation: "greaterEqual".to_owned(), rhs: report.indicator_tolerance }));
    }

    #[test]
    fn coupling_and_band_tables_are_reported() {
        let report = verify_symbol_core_relations().unwrap();
        // 蕴含耦合：前提两条 + 结论一条。 / Implication coupling: two on the premise plus one on the consequence.
        assert_eq!(report.imply.len(), 3);
        assert_eq!(report.imply.iter().filter(|row| row.keyed_on_premise).count(), 2);
        let premise_true = report.imply.iter()
            .find(|row| row.keyed_on_premise && row.indicator_value).unwrap();
        assert_eq!(premise_true.relation, "lessEqual");
        assert_eq!(premise_true.other_coefficient, -1.0);
        assert_eq!(premise_true.rhs, 0.0);
        let premise_false = report.imply.iter()
            .find(|row| row.keyed_on_premise && !row.indicator_value).unwrap();
        assert_eq!(premise_false.relation, "greaterEqual");
        assert_eq!(premise_false.rhs, 1.0);
        // band：取真时是一对关系，两个方向关于 0 对称，容差就是 `STEP_EPSILON`。
        // The band is a pair when true, symmetric about zero, with `STEP_EPSILON` as the tolerance.
        let band = &report.in_values;
        assert_eq!(band.when_indicator_true.len(), 2);
        assert_eq!(band.when_indicator_true[0].relation, "lessEqual");
        assert_eq!(band.when_indicator_true[0].rhs, report.step_epsilon);
        assert_eq!(band.when_indicator_true[1].relation, "greaterEqual");
        assert_eq!(band.when_indicator_true[1].rhs, -report.step_epsilon);
        assert_eq!(band.band_tolerance, report.step_epsilon);
        // side 两个方向用 `STRICT_BOUNDARY`，且它同时是两条松弛的右端。
        // Both side directions use `STRICT_BOUNDARY`, which is also both relaxation right-hand sides.
        assert_eq!(band.when_side_true, SymbolCoreRelation {
            relation: "greaterEqual".to_owned(), rhs: report.strict_boundary });
        assert_eq!(band.when_side_false, SymbolCoreRelation {
            relation: "lessEqual".to_owned(), rhs: -report.strict_boundary });
        assert_eq!(band.relaxed_lower_rhs, report.strict_boundary);
        assert_eq!(band.relaxed_upper_rhs, report.strict_boundary);
        assert_eq!(band.strict_boundary, report.strict_boundary);
        // 严格边界是步进容差的两倍——公开常量的关系，不是巧合。
        // The strict boundary is twice the step epsilon — a relation between public constants, not a coincidence.
        assert_eq!(report.strict_boundary, report.step_epsilon * 2.0);
    }
}
