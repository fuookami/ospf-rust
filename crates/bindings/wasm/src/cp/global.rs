//! 完整 CP 约束族与区间工厂 / Factories for all core CP constraint families and intervals.

use std::collections::BTreeSet;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use ospf_rust_core::model::constraint_programming as core;
use crate::error;
use super::{ConstraintProgrammingConstraint, ConstraintProgrammingModel, IntegerExpression, Scope, integer};

/// 同模型表达式的临时批量容器；push 时复制 AST / Temporary same-model expression batch, copying ASTs on push.
#[wasm_bindgen]
pub struct IntegerExpressions {
    scope: Rc<Scope>,
    values: Vec<core::IntegerExpression>,
}

#[wasm_bindgen]
impl IntegerExpressions {
    /// 校验模型身份并捕获不可变表达式副本 / Check ownership and capture an immutable expression copy.
    pub fn push(&mut self, expression: &IntegerExpression) -> Result<(), JsValue> {
        self.scope.check(&expression.scope)?;
        self.values.push(expression.value.clone());
        Ok(())
    }
}

impl ConstraintProgrammingModel {
    fn constraint(&self, value: core::ConstraintProgrammingConstraint) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.state()?;
        Ok(ConstraintProgrammingConstraint { scope: self.scope.clone(), value })
    }

    fn literal(&self, index: u32, negated: bool) -> Result<core::BooleanLiteral, JsValue> {
        Ok(core::BooleanLiteral { variable: self.scope.variable(index)?, negated })
    }

    fn interval_ids(&self, indices: &[u32]) -> Result<Vec<core::IntervalVariableId>, JsValue> {
        let state = self.scope.state()?;
        indices.iter().map(|&index| state.intervals.get(index as usize).cloned().ok_or_else(|| error("CP interval index out of bounds"))).collect()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;
    use crate::cp::IntegerDomain;

    #[wasm_bindgen_test]
    fn expression_batches_capture_exact_ast_and_keep_model_identity() {
        let high = 9_007_199_254_740_993i64;
        let mut model = ConstraintProgrammingModel::new("global", "sdk-tests").unwrap();
        model.add_variable("i", &IntegerDomain::boolean()).unwrap();
        model.add_variable("v", &IntegerDomain::values(&[high, high + 2]).unwrap()).unwrap();
        let index = model.expression(&[0], &[1], "0").unwrap();
        let target = model.expression(&[1], &[1], "0").unwrap();
        let element = model.element(&index, &[high, high + 2], &target).unwrap();
        let mut batch = model.expressions().unwrap();
        batch.push(&index).unwrap();
        batch.push(&target).unwrap();
        let table = model.assignment_table(&batch, &[0, high, 1, high + 2], 2, true).unwrap();
        assert_eq!(table.value, core::ConstraintProgrammingConstraint::AllowedAssignments {
            expressions: vec![index.value.clone(), target.value.clone()], tuples: vec![vec![0, high], vec![1, high + 2]],
        });
        assert!(model.assignment_table(&batch, &[0], 1, true).is_err());
        let other = ConstraintProgrammingModel::new("other", "sdk-tests").unwrap();
        assert!(other.all_different_expressions(&batch).is_err());
        let relation = target.compare("eq", &(high + 2).to_string()).unwrap();
        let reified = model.reification(0, false, &relation, "equivalent").unwrap();
        drop(index);
        drop(target);
        drop(batch);
        drop(relation);
        model.add_constraint(&element, r#"{"id":"element"}"#).unwrap();
        model.add_constraint(&table, r#"{"id":"table"}"#).unwrap();
        model.add_constraint(&reified, r#"{"id":"reified"}"#).unwrap();
        let snapshot = model.freeze().unwrap();
        snapshot.validate_assignment(&[0, high]).unwrap();
        snapshot.validate_assignment(&[1, high + 2]).unwrap();
        assert!(snapshot.validate_assignment(&[0, high + 2]).is_err());
        let mut invalidated = model.expressions().unwrap();
        drop(model);
        assert!(invalidated.push(&other.expression(&[], &[], "0").unwrap()).is_err());
        snapshot.validate_assignment(&[1, high + 2]).unwrap();
    }
}

#[wasm_bindgen]
impl ConstraintProgrammingModel {
    /// 创建同模型表达式批量容器 / Create a same-model expression batch.
    pub fn expressions(&self) -> Result<IntegerExpressions, JsValue> {
        self.scope.state()?;
        Ok(IntegerExpressions { scope: self.scope.clone(), values: Vec::new() })
    }

    /// 表达式全异，不限于单变量 / All-different over expressions, not only variables.
    #[wasm_bindgen(js_name = allDifferentExpressions)]
    pub fn all_different_expressions(&self, expressions: &IntegerExpressions) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.check(&expressions.scope)?;
        self.constraint(core::ConstraintProgrammingConstraint::AllDifferent { expressions: expressions.values.clone() })
    }

    /// 文字蕴含任意原始约束，复制后件 AST / Literal implication over any source constraint, copying the consequent AST.
    pub fn implication(
        &self,
        index: u32,
        negated: bool,
        constraint: &ConstraintProgrammingConstraint,
    ) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.check(&constraint.scope)?;
        self.constraint(core::ConstraintProgrammingConstraint::Implication { enforcement: self.literal(index, negated)?, constraint: Box::new(constraint.value.clone()) })
    }

    /// 任意原始约束的三向重化 / Three reification directions for any source constraint.
    pub fn reification(
        &self,
        index: u32,
        negated: bool,
        constraint: &ConstraintProgrammingConstraint,
        direction: &str,
    ) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.check(&constraint.scope)?;
        let direction = match direction {
            "equivalent" => core::ReificationDirection::Equivalent,
            "impliedByLiteral" => core::ReificationDirection::ImpliedByLiteral,
            "impliesLiteral" => core::ReificationDirection::ImpliesLiteral,
            _ => return Err(error("unknown reification direction")),
        };
        self.constraint(core::ConstraintProgrammingConstraint::Reification { literal: self.literal(index, negated)?, constraint: Box::new(constraint.value.clone()), direction })
    }

    /// 零基索引的精确常量 Element 表 / Exact constant Element table with a zero-based index.
    pub fn element(
        &self,
        index: &IntegerExpression,
        values: &[i64],
        target: &IntegerExpression,
    ) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.check(&index.scope)?;
        self.scope.check(&target.scope)?;
        self.constraint(core::ConstraintProgrammingConstraint::Element { index: index.value.clone(), values: values.to_vec(), target: target.value.clone() })
    }

    /// 行优先精确整数表，保留零列空元组语义 / Row-major exact-integer table, retaining zero-column empty-tuple semantics.
    #[wasm_bindgen(js_name = assignmentTable)]
    pub fn assignment_table(
        &self,
        expressions: &IntegerExpressions,
        values: &[i64],
        rows: u32,
        allowed: bool,
    ) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.check(&expressions.scope)?;
        let width = expressions.values.len();
        if rows > 1_048_576 || (rows as usize).checked_mul(width) != Some(values.len()) { return Err(error("invalid CP table shape or row budget")); }
        let tuples = (0..rows as usize).map(|row| values[row * width..(row + 1) * width].to_vec()).collect();
        let expressions = expressions.values.clone();
        self.constraint(if allowed { core::ConstraintProgrammingConstraint::AllowedAssignments { expressions, tuples } }
            else { core::ConstraintProgrammingConstraint::ForbiddenAssignments { expressions, tuples } })
    }

    /// 固定或变量时长区间，以及正/负 presence literal / Fixed or variable duration interval with a positive/negative presence literal.
    #[wasm_bindgen(js_name = addGeneralInterval)]
    pub fn add_general_interval(
        &mut self,
        id: &str,
        start: &IntegerExpression,
        fixed_duration: &str,
        duration_variable: Option<u32>,
        end: &IntegerExpression,
        presence: Option<u32>,
        negated: bool,
    ) -> Result<u32, JsValue> {
        self.scope.check(&start.scope)?;
        self.scope.check(&end.scope)?;
        if id.trim().is_empty() || (presence.is_none() && negated) { return Err(error("invalid CP interval identity or presence")); }
        let duration = match duration_variable {
            Some(index) if fixed_duration.is_empty() => core::IntervalDuration::Variable(self.scope.variable(index)?),
            None => core::IntervalDuration::Fixed(integer(fixed_duration)?),
            _ => return Err(error("interval duration must have exactly one representation")),
        };
        let presence = presence.map(|index| self.literal(index, negated)).transpose()?;
        let interval = core::IntervalVariable::new(id, start.value.clone(), duration, end.value.clone(), presence).map_err(error)?;
        self.register_interval(interval)
    }

    /// 原始固定需求累计约束 / Source fixed-demand cumulative constraint.
    pub fn cumulative(&self, indices: &[u32], demands: &[i64], capacity: &str) -> Result<ConstraintProgrammingConstraint, JsValue> {
        if indices.len() != demands.len() { return Err(error("invalid cumulative task shape")); }
        let tasks = self.interval_ids(indices)?.into_iter().zip(demands).map(|(interval, &demand)| core::CumulativeTask { interval, demand }).collect();
        self.constraint(core::ConstraintProgrammingConstraint::Cumulative { tasks, capacity: integer(capacity)? })
    }

    /// 原始后继表达式的单回路约束 / Source single-circuit constraint over successor expressions.
    pub fn circuit(&self, successors: &IntegerExpressions) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.check(&successors.scope)?;
        self.constraint(core::ConstraintProgrammingConstraint::Circuit { successors: successors.values.clone() })
    }

    /// 确定性自动机，状态为 i32，输入标签为精确 i64 / Deterministic automaton with i32 states and exact i64 input labels.
    pub fn automaton(
        &self,
        expressions: &IntegerExpressions,
        initial_state: i32,
        final_states: &[i32],
        from_states: &[i32],
        values: &[i64],
        to_states: &[i32],
    ) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.check(&expressions.scope)?;
        if from_states.len() != values.len() || to_states.len() != values.len() { return Err(error("invalid automaton transition shape")); }
        let transitions = from_states.iter().zip(values).zip(to_states).map(|((&from_state, &value), &to_state)| core::AutomatonTransition { from_state, value, to_state }).collect();
        self.constraint(core::ConstraintProgrammingConstraint::Automaton {
            expressions: expressions.values.clone(), initial_state, final_states: final_states.iter().copied().collect::<BTreeSet<_>>(), transitions,
        })
    }

    /// 精确时间与增量表达式的 Reservoir / Reservoir with exact time and level-change expressions.
    pub fn reservoir(
        &self,
        times: &IntegerExpressions,
        changes: &IntegerExpressions,
        initial: &str,
        minimum: &str,
        maximum: &str,
    ) -> Result<ConstraintProgrammingConstraint, JsValue> {
        self.scope.check(&times.scope)?;
        self.scope.check(&changes.scope)?;
        if times.values.len() != changes.values.len() { return Err(error("invalid reservoir event shape")); }
        let events = times.values.iter().zip(&changes.values).map(|(time, level_change)| core::ReservoirEvent { time: time.clone(), level_change: level_change.clone() }).collect();
        self.constraint(core::ConstraintProgrammingConstraint::Reservoir { events, initial_level: integer(initial)?, minimum_level: integer(minimum)?, maximum_level: integer(maximum)? })
    }
}
