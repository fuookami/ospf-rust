//! 精确整数 CP builder 与独立快照 / Exact-integer CP builder and independent snapshots.

use std::cell::{Ref, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use ospf_rust_core::model::constraint_programming as core;
use ospf_rust_core::solver::{SolveReport, StableVariableId};
use ospf_rust_framework::solver::remote::SerializedIntervalValue;
use ospf_rust_bridge::{BridgeError, ConstraintOptions, ModelArtifact, decode_cp_report, decode_cp_snapshot, encode_cp_snapshot};
use crate::{bridge_error, error};

mod assumptions;
mod global;
pub use assumptions::ConstraintProgrammingAssumptions;
pub use global::IntegerExpressions;

fn integer(value: &str) -> Result<i64, JsValue> { value.parse().map_err(error) }

#[derive(Clone)]
struct State {
    model: core::ConstraintProgrammingModel,
    variables: Vec<core::IntegerVariable>,
    intervals: Vec<core::IntervalVariableId>,
}

struct Scope(RefCell<Option<State>>);

impl Scope {
    fn state(&self) -> Result<Ref<'_, State>, JsValue> {
        let state = self.0.try_borrow().map_err(|_| error("CP model is already borrowed"))?;
        Ref::filter_map(state, Option::as_ref).map_err(|_| bridge_error(BridgeError::new("Disposed", "CP model has been disposed")))
    }

    fn mutate<T>(&self, operation: impl FnOnce(&mut State) -> Result<T, JsValue>) -> Result<T, JsValue> {
        let mut state = self.0.try_borrow_mut().map_err(|_| error("CP model is already borrowed"))?;
        let current = state.as_ref().ok_or_else(|| bridge_error(BridgeError::new("Disposed", "CP model has been disposed")))?;
        // core builder 没有 transaction；只提交通过 freeze 校验的独立副本。
        // The core builder has no transaction; commit only an independent copy that passes freeze validation.
        let mut next = current.clone();
        let result = operation(&mut next)?;
        next.model.freeze().map_err(error)?;
        *state = Some(next);
        Ok(result)
    }

    fn check(&self, other: &Rc<Self>) -> Result<(), JsValue> {
        self.state()?;
        other.state()?;
        if !std::ptr::eq(self, Rc::as_ptr(other)) {
            return Err(bridge_error(BridgeError::new("InvalidReference", "CP object belongs to another model")));
        }
        Ok(())
    }

    fn variable(&self, index: u32) -> Result<core::IntegerVariable, JsValue> {
        self.state()?.variables.get(index as usize).cloned().ok_or_else(|| error("CP variable index is out of bounds"))
    }
}

/// 精确 i64 值域，不将稀疏值域扩展为区间 / Exact i64 domain, never widening sparse values into a range.
#[wasm_bindgen]
pub struct IntegerDomain { value: core::IntegerDomain }

#[wasm_bindgen]
impl IntegerDomain {
    /// 闭整数区间，文本输入避免 JS 到 i64 的截断 / Closed range, using text input to avoid JS-to-i64 truncation.
    pub fn range(lower: &str, upper: &str) -> Result<IntegerDomain, JsValue> {
        Ok(Self { value: core::IntegerDomain::range(integer(lower)?, integer(upper)?).map_err(error)? })
    }

    /// 排序去重后的稀疏域 / Sorted and deduplicated sparse domain.
    pub fn values(values: &[i64]) -> Result<IntegerDomain, JsValue> {
        Ok(Self { value: core::IntegerDomain::values(values.iter().copied()).map_err(error)? })
    }

    /// 精确布尔域 / Exact Boolean domain.
    pub fn boolean() -> IntegerDomain { Self { value: core::IntegerDomain::boolean() } }

    /// 下界 / Lower bound.
    pub fn lower(&self) -> i64 { self.value.lower() }

    /// 上界 / Upper bound.
    pub fn upper(&self) -> i64 { self.value.upper() }

    /// 可超过 u64 的基数文本 / Cardinality text, potentially exceeding u64.
    pub fn cardinality(&self) -> String { self.value.cardinality().to_string() }

    /// 精确成员测试 / Exact membership test.
    pub fn contains(&self, value: &str) -> Result<bool, JsValue> { Ok(self.value.contains(integer(value)?)) }
}

/// 绑定到模型的精确整数表达式 / Model-owned exact integer expression.
#[wasm_bindgen]
pub struct IntegerExpression {
    scope: Rc<Scope>,
    value: core::IntegerExpression,
}

#[wasm_bindgen]
impl IntegerExpression {
    /// 相加并交给 core 规范化 / Add and normalize through core.
    pub fn add(&self, other: &IntegerExpression) -> Result<IntegerExpression, JsValue> {
        self.scope.check(&other.scope)?;
        let constant = self.value.constant.checked_add(other.value.constant).ok_or_else(|| error("integer constant overflow"))?;
        let value = core::IntegerExpression::linear(constant, self.value.terms.iter().chain(&other.value.terms).cloned()).map_err(error)?;
        Ok(Self { scope: self.scope.clone(), value })
    }

    /// 增加精确常数 / Add an exact constant.
    pub fn offset(&self, value: &str) -> Result<IntegerExpression, JsValue> {
        self.scope.state()?;
        let constant = self.value.constant.checked_add(integer(value)?).ok_or_else(|| error("integer constant overflow"))?;
        let value = core::IntegerExpression::linear(constant, self.value.terms.clone()).map_err(error)?;
        Ok(Self { scope: self.scope.clone(), value })
    }

    /// checked i64 标量乘法，不转浮点 / Checked i64 scalar multiplication without floating conversion.
    pub fn mul(&self, value: &str) -> Result<IntegerExpression, JsValue> {
        self.scope.state()?;
        let multiplier = integer(value)?;
        let constant = self.value.constant.checked_mul(multiplier).ok_or_else(|| error("integer constant overflow"))?;
        let terms = self.value.terms.iter().map(|term| {
            Ok(core::IntegerTerm { variable: term.variable.clone(), coefficient: term.coefficient.checked_mul(multiplier).ok_or_else(|| error("integer coefficient overflow"))? })
        }).collect::<Result<Vec<_>, JsValue>>()?;
        let value = core::IntegerExpression::linear(constant, terms).map_err(error)?;
        Ok(Self { scope: self.scope.clone(), value })
    }

    /// 求值输入使用 builder 注册顺序，不是快照顺序。
    /// Evaluate in builder registration order, not snapshot canonical order.
    pub fn evaluate(&self, values: &[i64]) -> Result<i64, JsValue> {
        let state = self.scope.state()?;
        if values.len() != state.variables.len() { return Err(error("invalid integer expression evaluation shape")); }
        let values = state.variables.iter().zip(values).map(|(variable, value)| (variable.stable_id.clone(), *value)).collect();
        self.value.evaluate(&values).map_err(error)
    }

    /// 创建精确关系快照 / Create an exact relation snapshot.
    pub fn compare(&self, relation: &str, rhs: &str) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.state()?;
        let relation = match relation {
            "le" => core::IntegerRelation::LessOrEqual,
            "ge" => core::IntegerRelation::GreaterOrEqual,
            "eq" => core::IntegerRelation::Equal,
            "ne" => core::IntegerRelation::NotEqual,
            _ => return Err(error("unknown integer relation")),
        };
        Ok(ConstraintProgrammingConstraint { scope: self.scope.clone(), value: core::ConstraintProgrammingConstraint::Integer {
            expression: self.value.clone(), relation, rhs: integer(rhs)?,
        } })
    }
}

/// CP 约束 AST 的模型归属句柄 / Model-owned handle to a CP constraint AST.
#[wasm_bindgen]
pub struct ConstraintProgrammingConstraint {
    scope: Rc<Scope>,
    value: core::ConstraintProgrammingConstraint,
}

/// 精确 CP builder；与 f64 MetaModel 明确分离 / Exact CP builder, explicitly separate from the f64 MetaModel.
#[wasm_bindgen]
pub struct ConstraintProgrammingModel { scope: Rc<Scope> }

impl Drop for ConstraintProgrammingModel {
    fn drop(&mut self) { self.scope.0.borrow_mut().take(); }
}

impl ConstraintProgrammingModel {
    fn register_interval(&mut self, interval: core::IntervalVariable) -> Result<u32, JsValue> {
        self.scope.mutate(|state| {
            let index = u32::try_from(state.intervals.len()).map_err(error)?;
            state.model.register_interval(interval.clone()).map_err(error)?;
            state.intervals.push(interval.id);
            Ok(index)
        })
    }
}

#[wasm_bindgen]
impl ConstraintProgrammingModel {
    /// 使用显式稳定身份命名空间创建 builder / Create a builder with an explicit stable identity namespace.
    #[wasm_bindgen(constructor)]
    pub fn new(name: &str, namespace: &str) -> Result<ConstraintProgrammingModel, JsValue> {
        if name.trim().is_empty() || namespace.trim().is_empty() { return Err(error("CP name and namespace must not be blank")); }
        Ok(Self { scope: Rc::new(Scope(RefCell::new(Some(State {
            model: core::ConstraintProgrammingModel::new(name).with_identity_metadata(namespace, "1.0"), variables: Vec::new(), intervals: Vec::new(),
        })))) })
    }

    /// 注册稳定 ID 的变量，失败保持 builder 不变 / Register a stable-ID variable, preserving the builder on failure.
    #[wasm_bindgen(js_name = addVariable)]
    pub fn add_variable(&mut self, id: &str, domain: &IntegerDomain) -> Result<u32, JsValue> {
        if id.trim().is_empty() { return Err(error("CP variable ID is blank")); }
        self.scope.mutate(|state| {
            let index = u32::try_from(state.variables.len()).map_err(error)?;
            let variable = core::IntegerVariable::new(id);
            state.model.add_variable(variable.clone(), domain.value.clone()).map_err(error)?;
            state.variables.push(variable);
            Ok(index)
        })
    }

    /// 注册顺序变量构造精确表达式 / Construct an exact expression using registration-order variables.
    pub fn expression(
        &self,
        indices: &[u32],
        coefficients: &[i64],
        constant: &str,
    ) -> Result<IntegerExpression, JsValue> {
        self.scope.state()?;
        if indices.len() != coefficients.len() { return Err(error("invalid integer expression shape")); }
        let terms = indices.iter().zip(coefficients).map(|(&index, &coefficient)| {
            Ok(core::IntegerTerm { variable: self.scope.variable(index)?, coefficient })
        }).collect::<Result<Vec<_>, JsValue>>()?;
        Ok(IntegerExpression { scope: self.scope.clone(), value: core::IntegerExpression::linear(integer(constant)?, terms).map_err(error)? })
    }

    /// 注册原始约束及稳定元数据 / Register the source constraint and stable metadata.
    #[wasm_bindgen(js_name = addConstraint)]
    pub fn add_constraint(&mut self, constraint: &ConstraintProgrammingConstraint, options_json: &str) -> Result<(), JsValue> {
        self.scope.check(&constraint.scope)?;
        let options: ConstraintOptions = serde_json::from_str(options_json).map_err(error)?;
        if options.id.trim().is_empty() { return Err(error("CP constraint ID is blank")); }
        let mut definition = core::ConstraintDefinition::new(options.id, constraint.value.clone());
        if let Some(name) = options.name { definition.name = name; }
        definition.group = options.group;
        definition.origin = options.origin;
        self.scope.mutate(|state| state.model.add_constraint(definition).map_err(error))
    }

    /// 设置原生整数目标，包含精确常数 / Set a native integer objective, including an exact constant.
    #[wasm_bindgen(js_name = setObjective)]
    pub fn set_objective(&mut self, expression: &IntegerExpression, id: &str, maximize: bool) -> Result<(), JsValue> {
        self.scope.check(&expression.scope)?;
        let objective = if maximize { core::IntegerObjective::maximize_with_id(id, expression.value.clone()) }
            else { core::IntegerObjective::minimize_with_id(id, expression.value.clone()) };
        self.scope.mutate(|state| { state.model.set_objective(objective); Ok(()) })
    }

    /// 切换为 satisfaction / Switch to satisfaction.
    #[wasm_bindgen(js_name = clearObjective)]
    pub fn clear_objective(&mut self) -> Result<(), JsValue> {
        self.scope.mutate(|state| { state.model.clear_objective(); Ok(()) })
    }

    /// 对变量创建全异约束 / Create an all-different constraint over variables.
    #[wasm_bindgen(js_name = allDifferent)]
    pub fn all_different(&self, indices: &[u32]) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.state()?;
        let expressions = indices.iter().map(|&index| self.scope.variable(index).map(core::IntegerExpression::variable)).collect::<Result<_, _>>()?;
        Ok(ConstraintProgrammingConstraint { scope: self.scope.clone(), value: core::ConstraintProgrammingConstraint::AllDifferent { expressions } })
    }

    /// 布尔文字/AND/OR/XOR/至多一个/恰好一个 / Boolean literal, AND, OR, XOR, at-most-one or exactly-one.
    #[wasm_bindgen(js_name = booleanConstraint)]
    pub fn boolean_constraint(&self, indices: &[u32], negated: &[u8], operation: &str) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.state()?;
        if indices.len() != negated.len() || negated.iter().any(|&value| value > 1) { return Err(error("invalid Boolean literal shape")); }
        let literals = indices.iter().zip(negated).map(|(&index, &negated)| {
            Ok(core::BooleanLiteral { variable: self.scope.variable(index)?, negated: negated == 1 })
        }).collect::<Result<Vec<_>, JsValue>>()?;
        use core::ConstraintProgrammingConstraint as C;
        let value = match operation {
            "literal" if literals.len() == 1 => C::Boolean { literal: literals[0].clone() },
            "and" => C::And { literals }, "or" => C::Or { literals }, "xor" => C::Xor { literals },
            "atMostOne" => C::AtMostOne { literals }, "exactlyOne" => C::ExactlyOne { literals },
            _ => return Err(error("unknown Boolean operation or invalid literal count")),
        };
        Ok(ConstraintProgrammingConstraint { scope: self.scope.clone(), value })
    }

    /// 注册固定时长区间；可选 presence 必须是布尔变量 / Register a fixed-duration interval, with an optional Boolean presence variable.
    #[wasm_bindgen(js_name = addInterval)]
    pub fn add_interval(
        &mut self,
        id: &str,
        start: &IntegerExpression,
        duration: &str,
        end: &IntegerExpression,
        presence: Option<u32>,
    ) -> Result<u32, JsValue> {
        self.scope.check(&start.scope)?;
        self.scope.check(&end.scope)?;
        if id.trim().is_empty() { return Err(error("CP interval ID is blank")); }
        let presence = presence.map(|index| self.scope.variable(index).map(core::BooleanLiteral::positive)).transpose()?;
        let interval = core::IntervalVariable::new(id, start.value.clone(), core::IntervalDuration::Fixed(integer(duration)?), end.value.clone(), presence).map_err(error)?;
        self.register_interval(interval)
    }

    /// 创建不重叠约束，保持区间身份 / Create a no-overlap constraint preserving interval identities.
    #[wasm_bindgen(js_name = noOverlap)]
    pub fn no_overlap(&self, indices: &[u32]) -> Result<ConstraintProgrammingConstraint, JsValue> {
        let state = self.scope.state()?;
        let intervals = indices.iter().map(|&index| state.intervals.get(index as usize).cloned().ok_or_else(|| error("CP interval index out of bounds"))).collect::<Result<_, _>>()?;
        Ok(ConstraintProgrammingConstraint { scope: self.scope.clone(), value: core::ConstraintProgrammingConstraint::NoOverlap { intervals } })
    }

    /// 冻结为独立不可变快照；不依赖 builder 后续生命周期。
    /// Freeze an independent immutable snapshot, unaffected by the builder's later lifetime.
    pub fn freeze(&self) -> Result<ConstraintProgrammingSnapshot, JsValue> {
        Ok(ConstraintProgrammingSnapshot { value: self.scope.state()?.model.freeze().map_err(error)? })
    }
}

/// 独立不可变快照，canonical ID 顺序可与注册顺序不同。
/// Independent immutable snapshot, whose canonical ID order can differ from registration order.
#[wasm_bindgen]
pub struct ConstraintProgrammingSnapshot { pub(crate) value: core::ConstraintProgrammingSnapshot }

/// 独立精确 CP 统一报告，读取时复验源快照 / Independent exact CP unified report, verified against its source on import.
#[wasm_bindgen]
pub struct ConstraintProgrammingReport {
    value: SolveReport<i64, u64>,
    variables: Vec<String>,
}

#[wasm_bindgen]
impl ConstraintProgrammingReport {
    /// 校验报告契约、源指纹、完整赋值与精确目标 / Validate report contract, source fingerprint, complete assignment and exact objective.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str, snapshot: &ConstraintProgrammingSnapshot) -> Result<ConstraintProgrammingReport, JsValue> {
        Ok(Self { value: decode_cp_report(json, &snapshot.value).map_err(bridge_error)?, variables: snapshot.variable_ids() })
    }

    /// 保留所有精确整数的报告文本 / Report text retaining all exact integers.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value).map_err(error) }

    /// 原始 CP 稳定变量的 canonical 顺序 / Canonical order of original CP stable variables.
    #[wasm_bindgen(js_name = variableIds)]
    pub fn variable_ids(&self) -> Vec<String> { self.variables.clone() }

    /// canonical 顺序解的独立副本；无 incumbent 时为空 / Independent canonical-order solution copy, empty without an incumbent.
    pub fn values(&self) -> Vec<i64> { self.value.solution.as_ref().map(|solution| solution.values.clone()).unwrap_or_default() }

    /// 按稳定 ID 读取精确值，无 incumbent 时为 None / Read an exact stable-ID value, or None without an incumbent.
    pub fn value(&self, id: &str) -> Result<Option<i64>, JsValue> {
        if !self.variables.iter().any(|variable| variable == id) { return Err(error("unknown CP report variable ID")); }
        Ok(self.value.solution.as_ref().and_then(|solution| solution.stable_values.get(&StableVariableId::from(id)).copied()))
    }

    /// typed 精确整数目标，不使用浮点快照 / Typed exact integer objective, never the floating snapshot.
    #[wasm_bindgen(js_name = objectiveValue)]
    pub fn objective_value(&self) -> Option<i64> { self.value.solution.as_ref().and_then(|solution| solution.objective) }

    /// typed 精确整数界 / Typed exact integer bound.
    #[wasm_bindgen(js_name = bestBound)]
    pub fn best_bound(&self) -> Option<i64> { self.value.statistics.best_bound }

    /// 是否携带 incumbent / Whether an incumbent is present.
    #[wasm_bindgen(js_name = hasSolution)]
    pub fn has_solution(&self) -> bool { self.value.solution.is_some() }

    /// 读取诊断扩展，不解析包含精确数值的整份报告 / Read a diagnostic extension without parsing the full exact-numeric report.
    #[wasm_bindgen(js_name = diagnosticExtension)]
    pub fn diagnostic_extension(&self, key: &str) -> Option<String> { self.value.diagnostics.extensions.get(key).cloned() }

    /// 统一诊断问题码 / Unified diagnostic issue codes.
    #[wasm_bindgen(js_name = diagnosticIssueCodes)]
    pub fn diagnostic_issue_codes(&self) -> Vec<String> { self.value.diagnostics.issues.iter().map(|issue| issue.code.clone()).collect() }

    /// 返回原始不可行证据文本，不宣称不可约性 / Return original infeasibility-evidence text, without claiming irreducibility.
    #[wasm_bindgen(js_name = infeasibilityEvidenceJson)]
    pub fn infeasibility_evidence_json(&self) -> Result<Option<String>, JsValue> {
        self.value.diagnostics.infeasibility_evidence.as_ref().map(serde_json::to_string).transpose().map_err(error)
    }

    /// 统一问题结论 / Unified problem conclusion.
    #[wasm_bindgen(js_name = problemStatus)]
    pub fn problem_status(&self) -> Result<String, JsValue> {
        serde_json::to_value(self.value.problem_status).map_err(error)?.as_str().map(str::to_owned).ok_or_else(|| error("invalid problem status"))
    }

    /// 统一执行终止原因 / Unified execution termination reason.
    #[wasm_bindgen(js_name = terminationReason)]
    pub fn termination_reason(&self) -> Result<String, JsValue> {
        serde_json::to_value(self.value.termination_reason).map_err(error)?.as_str().map(str::to_owned).ok_or_else(|| error("invalid termination reason"))
    }
}

impl ConstraintProgrammingSnapshot {
    pub(crate) fn assignment(&self, values: &[i64]) -> Result<BTreeMap<StableVariableId, i64>, JsValue> {
        if values.len() != self.value.variables.len() { return Err(error("CP assignment must cover canonical variable order")); }
        Ok(self.value.variables.iter().zip(values).map(|(entry, &value)| (entry.variable.stable_id.clone(), value)).collect())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn cp_snapshots_retain_exact_integers_canonical_order_and_independent_lifetimes() {
        let high = 9_007_199_254_740_993i64;
        let mut model = ConstraintProgrammingModel::new("exact", "sdk-tests").unwrap();
        let domain = IntegerDomain::range(&high.to_string(), &(high + 5).to_string()).unwrap();
        model.add_variable("z", &domain).unwrap();
        model.add_variable("a", &IntegerDomain::boolean()).unwrap();
        let expression = model.expression(&[0], &[1], "3").unwrap();
        assert_eq!(expression.evaluate(&[high, 1]).unwrap(), high + 3);
        model.set_objective(&expression, "cost", false).unwrap();
        let relation = expression.compare("ge", &(high + 3).to_string()).unwrap();
        model.add_constraint(&relation, r#"{"id":"minimum"}"#).unwrap();
        let boolean = model.boolean_constraint(&[1], &[0], "literal").unwrap();
        model.add_constraint(&boolean, r#"{"id":"enabled"}"#).unwrap();
        let snapshot = model.freeze().unwrap();
        assert!(snapshot.has_objective_constant());
        assert_eq!(snapshot.variable_ids(), vec!["a", "z"]);
        snapshot.validate_assignment(&[1, high]).unwrap();
        assert!(snapshot.validate_assignment(&[0, high]).is_err());
        assert_eq!(snapshot.objective_value(&[1, high]).unwrap(), Some(high + 3));
        let json = snapshot.to_json().unwrap();
        assert!(json.contains(&high.to_string()));
        assert_eq!(ConstraintProgrammingSnapshot::from_json(&json).unwrap().to_json().unwrap(), json);
        let mut other = ConstraintProgrammingModel::new("exact", "sdk-tests").unwrap();
        assert!(other.add_constraint(&relation, r#"{"id":"foreign"}"#).is_err());
        assert!(model.add_variable("z", &domain).is_err());
        assert_eq!(model.freeze().unwrap().to_json().unwrap(), json);
        drop(model);
        assert!(expression.evaluate(&[high, 1]).is_err());
        snapshot.validate_assignment(&[1, high]).unwrap();
        assert_eq!(IntegerDomain::range(&i64::MIN.to_string(), &i64::MAX.to_string()).unwrap().cardinality(), "18446744073709551616");
    }

    #[wasm_bindgen_test]
    fn cp_intervals_and_activity_reuse_original_semantics_and_exact_evidence() {
        use crate::analysis::ConstraintActivityReport;

        let high = 9_007_199_254_740_993i64;
        let mut model = ConstraintProgrammingModel::new("schedule", "sdk-tests").unwrap();
        let domain = IntegerDomain::range(&high.to_string(), &(high + 10).to_string()).unwrap();
        model.add_variable("a", &domain).unwrap();
        model.add_variable("b", &domain).unwrap();
        model.add_variable("p", &IntegerDomain::boolean()).unwrap();
        let a = model.expression(&[0], &[1], "0").unwrap();
        let b = model.expression(&[1], &[1], "0").unwrap();
        model.add_interval("first", &a, "2", &a.offset("2").unwrap(), None).unwrap();
        model.add_interval("second", &b, "2", &b.offset("2").unwrap(), Some(2)).unwrap();
        model.add_constraint(&model.no_overlap(&[0, 1]).unwrap(), r#"{"id":"no-overlap"}"#).unwrap();
        model.add_constraint(&a.compare("le", &high.to_string()).unwrap(), r#"{"id":"limit"}"#).unwrap();
        let snapshot = model.freeze().unwrap();
        snapshot.validate_assignment(&[high, high + 2, 1]).unwrap();
        assert!(snapshot.validate_assignment(&[high, high + 1, 1]).is_err());
        let absent = snapshot.interval_values_json(&[high, high, 0]).unwrap();
        assert!(absent.contains(&high.to_string()));
        assert!(!absent.contains("second"));
        let report = ConstraintActivityReport::analyze_cp(&snapshot, &[high + 1, high + 3, 1], "{}").unwrap();
        assert_eq!(report.status("limit").unwrap(), "VIOLATED");
        assert_eq!(report.integer_lhs("limit").unwrap(), Some(high + 1));
        assert_eq!(report.integer_rhs("limit").unwrap(), Some(high));
        let json = report.to_json().unwrap();
        assert_eq!(ConstraintActivityReport::from_json(&json).unwrap().integer_rhs("limit").unwrap(), Some(high));
        assert!(ConstraintActivityReport::from_json(&json.replace("\"1.0\"", "\"99.0\"")).is_err());
    }

    #[wasm_bindgen_test]
    fn cp_report_handles_preserve_exact_values_without_source_lifetime() {
        use ospf_rust_core::solver::{ProblemStatus, SolveFingerprints, SolveSolution, TerminationReason};

        let high = 9_007_199_254_740_993i64;
        let mut model = ConstraintProgrammingModel::new("exact-report", "sdk-tests").unwrap();
        model.add_variable("x", &IntegerDomain::range(&high.to_string(), &(high + 5).to_string()).unwrap()).unwrap();
        let expression = model.expression(&[0], &[1], "0").unwrap();
        model.set_objective(&expression, "cost", false).unwrap();
        let snapshot = model.freeze().unwrap();
        let mut solution = SolveSolution::vector(vec![high]);
        solution.stable_values.insert(StableVariableId::from("x"), high);
        solution.objective = Some(high);
        let report = SolveReport::builder(ProblemStatus::Feasible, TerminationReason::Completed)
            .solution(solution).fingerprints(SolveFingerprints { model: Some(snapshot.value.fingerprint.clone()), ..Default::default() }).build().unwrap();
        let json = serde_json::to_string(&report).unwrap();
        let handle = ConstraintProgrammingReport::from_json(&json, &snapshot).unwrap();
        assert_eq!(handle.values(), vec![high]);
        assert_eq!(handle.objective_value(), Some(high));
        assert_eq!(handle.value("x").unwrap(), Some(high));
        drop(model);
        drop(snapshot);
        assert!(handle.to_json().unwrap().contains(&high.to_string()));
        assert_eq!(handle.problem_status().unwrap(), "FEASIBLE");
        assert!(handle.value("foreign").is_err());
    }
}

#[wasm_bindgen]
impl ConstraintProgrammingSnapshot {
    /// 从带摘要的 canonical 文本读取并校验 / Read and validate digest-bearing canonical text.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<ConstraintProgrammingSnapshot, JsValue> {
        if json.len() > 8 * 1024 * 1024 { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "CP snapshot exceeds 8 MiB"))); }
        Ok(Self { value: core::ConstraintProgrammingSnapshot::from_canonical_json(json.as_bytes()).map_err(error)? })
    }

    /// 校验外层模型指纹和内层摘要 / Validate the outer model fingerprint and inner digest.
    #[wasm_bindgen(js_name = fromArtifact)]
    pub fn from_artifact(json: &str) -> Result<ConstraintProgrammingSnapshot, JsValue> {
        let artifact: ModelArtifact = serde_json::from_str(json).map_err(error)?;
        Ok(Self { value: decode_cp_snapshot(&artifact).map_err(bridge_error)? })
    }

    /// 不经 JS JSON 解析的精确文本 / Exact text without JS JSON parsing.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> { Ok(encode_cp_snapshot(&self.value).map_err(bridge_error)?.model_json) }

    /// 带模型指纹的封装 / Envelope with the model fingerprint.
    #[wasm_bindgen(js_name = toArtifact)]
    pub fn to_artifact(&self) -> Result<String, JsValue> { serde_json::to_string(&encode_cp_snapshot(&self.value).map_err(bridge_error)?).map_err(error) }

    /// core 模型指纹 / Core model fingerprint.
    pub fn fingerprint(&self) -> String { self.value.fingerprint.value.clone() }

    /// 是否需要后端保留非零目标常数 / Whether the backend must preserve a nonzero objective constant.
    #[wasm_bindgen(js_name = hasObjectiveConstant)]
    pub fn has_objective_constant(&self) -> bool {
        self.value.objective.as_ref().is_some_and(|objective| objective.expression.constant != 0)
    }

    /// 完整赋值必须采用此稳定 ID 顺序 / Complete assignments must use this stable-ID order.
    #[wasm_bindgen(js_name = variableIds)]
    pub fn variable_ids(&self) -> Vec<String> { self.value.variables.iter().map(|entry| entry.variable.stable_id.0.clone()).collect() }

    /// 校验完整赋值的域、区间一致性和全部约束 / Validate complete assignment domains, interval consistency and all constraints.
    #[wasm_bindgen(js_name = validateAssignment)]
    pub fn validate_assignment(&self, values: &[i64]) -> Result<(), JsValue> {
        self.value.validate_assignment(&self.assignment(values)?).map_err(error)?;
        Ok(())
    }

    /// 精确目标求值，不声明可行性或最优性 / Exact objective evaluation, not a feasibility or optimality claim.
    #[wasm_bindgen(js_name = objectiveValue)]
    pub fn objective_value(&self, values: &[i64]) -> Result<Option<i64>, JsValue> {
        self.value.objective_value(&self.assignment(values)?).map_err(error)
    }

    /// 校验后返回精确区间文本，不转为 JS number / Return exact interval text after validation, never converting to JS number.
    #[wasm_bindgen(js_name = intervalValuesJson)]
    pub fn interval_values_json(&self, values: &[i64]) -> Result<String, JsValue> {
        let intervals = self.value.validate_assignment(&self.assignment(values)?).map_err(error)?;
        let intervals: BTreeMap<_, _> = intervals.into_iter().map(|(id, value)| (id, SerializedIntervalValue {
            start: value.start, size: value.duration, end: value.end, present: true,
        })).collect();
        serde_json::to_string(&intervals).map_err(error)
    }
}
