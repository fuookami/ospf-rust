//! 模型作用域内的不可变二次表达式 / Immutable quadratic expressions within a model scope.

use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use ospf_rust_math::symbol::{OwnedSymbol, Quadratic as MathQuadratic};
use ospf_rust_math::symbol::operation::Evaluate;
use ospf_rust_core::model::{ConstraintRelation, ObjectiveCategory};
use ospf_rust_core::model::mechanism::SymbolicQuadraticInequality;
use ospf_rust_core::solver::quadratic_model_fingerprint;
use ospf_rust_bridge::{BridgeError, encode_quadratic_model, quadratic_objective_matrix};
use crate::linear::{IndexedVariable, Linear, Scope};
use crate::{MetaModel, bridge_error, error};

/// f64 二次多项式；交叉项只存一次，其系数不隐含二倍因子。
/// f64 quadratic polynomial; each cross term is stored once without an implicit factor of two.
#[wasm_bindgen]
pub struct Quadratic {
    scope: Rc<Scope>,
    pub(crate) value: MathQuadratic<f64>,
    pub(crate) dependencies: BTreeSet<u64>,
}

impl Quadratic {
    pub(crate) fn checked(
        scope: Rc<Scope>,
        mut value: MathQuadratic<f64>,
        dependencies: BTreeSet<u64>,
    ) -> Result<Self, JsValue> {
        scope.check()?;
        if !value.constant.is_finite() || value.monomials.iter().any(|term| !term.coefficient.is_finite()) {
            return Err(error("quadratic expression contains non-finite values"));
        }
        // 先规范交换项再交给 Rust 合并，保证 x*y 与 y*x 使用同一键。
        // Canonicalize commutative terms before Rust simplification so x*y and y*x share a key.
        for term in &mut value.monomials {
            if let Some(second) = &mut term.symbol2 {
                if term.symbol1.dyn_id().parent_id > second.dyn_id().parent_id {
                    std::mem::swap(&mut term.symbol1, second);
                }
            }
        }
        value.simplify();
        if value.monomials.iter().any(|term| !term.coefficient.is_finite()) {
            return Err(error("quadratic coefficient overflow"));
        }
        value.monomials.sort_by_key(|term| (
            term.symbol1.dyn_id().parent_id, term.symbol2.as_ref().map(|symbol| symbol.dyn_id().parent_id),
        ));
        Ok(Self { scope, value, dependencies })
    }

    fn check_scope(&self, other: &Self) -> Result<(), JsValue> {
        self.check_model(&other.scope)
    }

    pub(crate) fn check_model(&self, scope: &Rc<Scope>) -> Result<(), JsValue> {
        self.scope.check()?;
        scope.check()?;
        if !Rc::ptr_eq(&self.scope, scope) {
            return Err(bridge_error(BridgeError::new("InvalidReference", "expression belongs to another model")));
        }
        Ok(())
    }

    fn quadratic_terms(&self) -> impl Iterator<Item = (usize, usize, f64)> + '_ {
        self.value.monomials.iter().filter_map(|term| term.symbol2.as_ref().map(|second| (
            term.symbol1.dyn_id().parent_id, second.dyn_id().parent_id, term.coefficient,
        )))
    }
}

#[wasm_bindgen]
impl Quadratic {
    /// 捕获二次关系快照，将常数移到右端并拒绝溢出。
    /// Capture a quadratic relation snapshot, moving constants to the RHS and rejecting overflow.
    pub fn compare(&self, relation: &str, rhs: f64) -> Result<QuadraticInequality, JsValue> {
        self.scope.check()?;
        if !rhs.is_finite() { return Err(error("quadratic constraint RHS must be finite")); }
        let relation = match relation {
            "le" => ConstraintRelation::LessEqual,
            "ge" => ConstraintRelation::GreaterEqual,
            "eq" => ConstraintRelation::Equal,
            _ => return Err(error("unknown quadratic constraint relation")),
        };
        let rhs = rhs - self.value.constant;
        if !rhs.is_finite() { return Err(error("quadratic constraint normalization overflow")); }
        Ok(QuadraticInequality {
            expression: Self::checked(self.scope.clone(), self.value.clone(), self.dependencies.clone())?, relation, rhs,
        })
    }

    /// 同模型二次式相加 / Add quadratic expressions from the same model.
    pub fn add(&self, other: &Quadratic) -> Result<Quadratic, JsValue> {
        self.check_scope(other)?;
        let mut dependencies = self.dependencies.clone();
        dependencies.extend(&other.dependencies);
        Self::checked(self.scope.clone(), self.value.clone() + other.value.clone(), dependencies)
    }

    /// 同模型二次式相减 / Subtract quadratic expressions from the same model.
    pub fn sub(&self, other: &Quadratic) -> Result<Quadratic, JsValue> {
        self.check_scope(other)?;
        let mut dependencies = self.dependencies.clone();
        dependencies.extend(&other.dependencies);
        Self::checked(self.scope.clone(), self.value.clone() - other.value.clone(), dependencies)
    }

    /// 增加有限常数 / Add a finite constant.
    pub fn offset(&self, value: f64) -> Result<Quadratic, JsValue> {
        self.scope.check()?;
        if !value.is_finite() { return Err(error("offset must be finite")); }
        let mut result = self.value.clone();
        result.constant += value;
        Self::checked(self.scope.clone(), result, self.dependencies.clone())
    }

    /// 仅乘有限标量，不隐式生成三次或四次式。
    /// Multiply only by a finite scalar, never implicitly generating cubic or quartic expressions.
    pub fn mul(&self, value: f64) -> Result<Quadratic, JsValue> {
        self.scope.check()?;
        if !value.is_finite() { return Err(error("multiplier must be finite")); }
        Self::checked(self.scope.clone(), self.value.clone() * value, self.dependencies.clone())
    }

    /// 常数项 / Constant term.
    pub fn constant(&self) -> Result<f64, JsValue> { self.scope.check()?; Ok(self.value.constant) }

    /// 当前变量顺序的线性系数副本 / Copy of linear coefficients in current variable order.
    #[wasm_bindgen(js_name = linearCoefficients)]
    pub fn linear_coefficients(&self) -> Result<Vec<f64>, JsValue> {
        self.scope.check()?;
        let mut result = vec![0.0; self.scope.variables.get()];
        for term in self.value.monomials.iter().filter(|term| term.symbol2.is_none()) {
            result[term.symbol1.dyn_id().parent_id] = term.coefficient;
        }
        Ok(result)
    }

    /// 二次项的首变量索引副本，按 (row, column) 排序。
    /// Copy of first-variable indices, sorted by (row, column).
    #[wasm_bindgen(js_name = rowIndices)]
    pub fn row_indices(&self) -> Result<Vec<u32>, JsValue> {
        self.scope.check()?;
        self.quadratic_terms().map(|(row, _, _)| u32::try_from(row).map_err(error)).collect()
    }

    /// 二次项的次变量索引副本，row 不大于 column。
    /// Copy of second-variable indices, with row no greater than column.
    #[wasm_bindgen(js_name = columnIndices)]
    pub fn column_indices(&self) -> Result<Vec<u32>, JsValue> {
        self.scope.check()?;
        self.quadratic_terms().map(|(_, column, _)| u32::try_from(column).map_err(error)).collect()
    }

    /// 对齐两个索引数组的多项式系数副本 / Copy of polynomial coefficients aligned with both index arrays.
    #[wasm_bindgen(js_name = quadraticCoefficients)]
    pub fn quadratic_coefficients(&self) -> Result<Vec<f64>, JsValue> {
        self.scope.check()?;
        Ok(self.quadratic_terms().map(|(_, _, coefficient)| coefficient).collect())
    }

    /// 用当前完整变量顺序在 Rust 中求值 / Evaluate in Rust using all current variables in order.
    pub fn evaluate(&self, values: &[f64]) -> Result<f64, JsValue> {
        self.scope.check()?;
        if values.len() != self.scope.variables.get() || values.iter().any(|value| !value.is_finite()) {
            return Err(error("invalid quadratic evaluation shape or values"));
        }
        let mut assignments = HashMap::new();
        for term in &self.value.monomials {
            assignments.insert(term.symbol1.clone(), values[term.symbol1.dyn_id().parent_id]);
            if let Some(second) = &term.symbol2 {
                assignments.insert(second.clone(), values[second.dyn_id().parent_id]);
            }
        }
        let result = self.value.evaluate(&assignments);
        if !result.is_finite() { return Err(error("quadratic evaluation overflow")); }
        Ok(result)
    }

    /// 仅当所有二次项已消去时降为线性式，不丢弃任何项。
    /// Demote only after every quadratic term cancels, without discarding any terms.
    #[wasm_bindgen(js_name = tryToLinear)]
    pub fn try_to_linear(&self) -> Result<Linear, JsValue> {
        self.scope.check()?;
        if self.quadratic_terms().next().is_some() {
            return Err(bridge_error(BridgeError::new("Unsupported", "expression still contains quadratic terms")));
        }
        let indices: Vec<_> = self.value.monomials.iter().map(|term| u32::try_from(term.symbol1.dyn_id().parent_id).map_err(error)).collect::<Result<_, _>>()?;
        let coefficients: Vec<_> = self.value.monomials.iter().map(|term| term.coefficient).collect();
        let mut result = Linear::new(self.scope.clone(), &indices, &coefficients, self.value.constant)?;
        result.dependencies = self.dependencies.clone();
        Ok(result)
    }
}

/// 保留原模型身份与表达式快照的二次关系 / Quadratic relation preserving model ownership and an expression snapshot.
#[wasm_bindgen]
pub struct QuadraticInequality {
    expression: Quadratic,
    relation: ConstraintRelation,
    rhs: f64,
}

#[wasm_bindgen]
impl MetaModel {
    /// 二次目标的线性部分注册到 MetaModel，二次部分在 Rust 四角模型编译边界写入。
    /// Register the linear part in MetaModel and apply the quadratic part at the Rust tetrad compilation boundary.
    #[wasm_bindgen(js_name = setQuadraticObjective)]
    pub fn set_quadratic_objective(&mut self, expression: &Quadratic, maximize: bool) -> Result<(), JsValue> {
        expression.check_model(&self.scope)?;
        if expression.constant()? != 0.0 {
            return Err(bridge_error(BridgeError::new("Unsupported", "nonzero objective constants are not yet supported")));
        }
        let terms: Vec<_> = expression.quadratic_terms().collect();
        let coefficients = expression.linear_coefficients()?;
        self.scope.model_mut()?.transaction(|source| {
            source.set_linear_objective(coefficients, if maximize { ObjectiveCategory::Maximum } else { ObjectiveCategory::Minimum });
            let mut model = source.try_to_quadratic_tetrad_model().map_err(error)?;
            model.Q = quadratic_objective_matrix(model.num_variables(), terms.iter().copied()).map_err(bridge_error)?;
            quadratic_model_fingerprint(&model).map_err(error)?;
            source.clear_solution();
            Ok::<_, JsValue>(())
        })?;
        *self.scope.quadratic_objective.borrow_mut() = terms;
        Ok(())
    }

    /// 将二次关系真正注册到 Rust MetaModel，而不是仅在导出时追加 DTO。
    /// Truly register the quadratic relation in Rust MetaModel, rather than appending a DTO only on export.
    #[wasm_bindgen(js_name = addQuadraticInequality)]
    pub fn add_quadratic_inequality(&mut self, inequality: &QuadraticInequality, name: &str) -> Result<(), JsValue> {
        inequality.expression.check_model(&self.scope)?;
        if name.trim().is_empty() { return Err(error("quadratic constraint name is blank")); }
        let mut source = self.scope.model_mut()?;
        let mut polynomial = inequality.expression.value.clone();
        for term in &mut polynomial.monomials {
            let first = term.symbol1.dyn_id().parent_id;
            term.symbol1 = OwnedSymbol::new(IndexedVariable(source.tokens()[first].id().unique_id() as usize));
            if let Some(second) = &mut term.symbol2 {
                let index = second.dyn_id().parent_id;
                *second = OwnedSymbol::new(IndexedVariable(source.tokens()[index].id().unique_id() as usize));
            }
        }
        polynomial.constant = 0.0;
        source.transaction(|source| {
            source.add_symbolic_quadratic_inequality(SymbolicQuadraticInequality::new(polynomial, inequality.relation, inequality.rhs), name);
            let model = source.try_to_quadratic_tetrad_model().map_err(error)?;
            quadratic_model_fingerprint(&model).map_err(error)?;
            source.clear_solution();
            Ok::<_, JsValue>(())
        })?;
        Ok(())
    }

    /// 导出无损二次模型及 core 指纹 / Export a lossless quadratic model and core fingerprint.
    #[wasm_bindgen(js_name = toQuadraticTetradModel)]
    pub fn to_quadratic_tetrad_model(&self) -> Result<String, JsValue> {
        let model = self.scope.compile_quadratic()?;
        serde_json::to_string(&encode_quadratic_model(&model).map_err(bridge_error)?).map_err(error)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use crate::MetaModel;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn quadratic_arithmetic_reuses_rust_polynomials_without_dropping_terms() {
        let mut model = MetaModel::new("quadratic").unwrap();
        model.add_variable("x", "continuous", None, None).unwrap();
        model.add_variable("y", "continuous", None, None).unwrap();
        let x = model.linear_expression(&[0], &[1.0], 2.0).unwrap();
        let y = model.linear_expression(&[1], &[3.0], 4.0).unwrap();
        let product = x.product(&y).unwrap();
        assert_eq!(product.constant().unwrap(), 8.0);
        assert_eq!(product.linear_coefficients().unwrap(), vec![4.0, 6.0]);
        assert_eq!(product.row_indices().unwrap(), vec![0]);
        assert_eq!(product.column_indices().unwrap(), vec![1]);
        assert_eq!(product.quadratic_coefficients().unwrap(), vec![3.0]);
        assert_eq!(product.evaluate(&[2.0, 5.0]).unwrap(), 76.0);
        let reversed = y.product(&x).unwrap();
        let cancelled = product.sub(&reversed).unwrap();
        assert_eq!(cancelled.try_to_linear().unwrap().coefficients().unwrap(), vec![0.0, 0.0]);
        assert!(product.try_to_linear().is_err());
        assert_eq!(product.add(&reversed).unwrap().quadratic_coefficients().unwrap(), vec![6.0]);
        assert!(product.evaluate(&[f64::NAN, 1.0]).is_err());
        assert!(product.evaluate(&[f64::MAX, f64::MAX]).is_err());
        assert!(product.mul(f64::MAX).is_err());
        assert_eq!(x.to_quadratic().unwrap().try_to_linear().unwrap().constant().unwrap(), 2.0);
        drop(model);
        assert!(product.constant().is_err());
    }

    #[wasm_bindgen_test]
    fn quadratic_model_compilation_preserves_objectives_constraints_and_identity() {
        let mut model = MetaModel::new("quadratic-model").unwrap();
        model.add_variable("x", "continuous", Some(0.0), Some(5.0)).unwrap();
        model.add_variable("y", "binary", Some(0.0), Some(1.0)).unwrap();
        let token = model.token(0).unwrap();
        let id = token.id().unwrap();
        let sum = model.linear_expression(&[0, 1], &[1.0, 1.0], 0.0).unwrap();
        let square = sum.product(&sum).unwrap();
        model.set_quadratic_objective(&square, false).unwrap();
        assert!(model.to_linear_triad_model().is_err());
        let relation = square.offset(3.0).unwrap().compare("le", 7.0).unwrap();
        model.add_quadratic_inequality(&relation, "quadratic-capacity").unwrap();
        model.add_linear_constraint(&[1.0, 0.0], "le", 4.0, "linear-capacity").unwrap();
        model.add_linear_symbol("sum", &sum).unwrap();
        model.set_solution(&[1.0, 1.0]).unwrap();
        let artifact = model.to_quadratic_tetrad_model().unwrap();
        let decoded = ospf_rust_bridge::decode_quadratic_model(&serde_json::from_str(&artifact).unwrap()).unwrap();
        assert_eq!(decoded.Q.rows[0].entries, vec![(0, 1.0), (1, 2.0)]);
        assert_eq!(decoded.quadratic_constraints.len(), 1);
        assert_eq!(decoded.quadratic_constraints[0].rhs, 4.0);
        assert_eq!(*decoded.quadratic_constraints[0].polynomial.constant(), 0.0);
        assert!(model.set_quadratic_objective(&square.offset(1.0).unwrap(), false).is_err());
        assert_eq!(model.to_quadratic_tetrad_model().unwrap(), artifact);
        assert_eq!(token.result().unwrap(), Some(1.0));
        model.add_variable("later", "continuous", None, None).unwrap();
        let later = ospf_rust_bridge::decode_quadratic_model(&serde_json::from_str(&model.to_quadratic_tetrad_model().unwrap()).unwrap()).unwrap();
        assert_eq!(later.Q.rows.len(), 3);
        assert_eq!(token.id().unwrap(), id);
        assert_eq!(token.result().unwrap(), None);
        model.set_linear_objective(&[1.0, 0.0, 0.0], true).unwrap();
        let switched = model.scope.compile_quadratic().unwrap();
        assert!(switched.Q.rows.iter().all(|row| row.entries.is_empty()));
        assert_eq!(switched.quadratic_constraints.len(), 1);
        assert!(model.to_linear_triad_model().is_err());
    }
}
