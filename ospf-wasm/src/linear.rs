//! 模型作用域内的不可变线性表达式 / Immutable linear expressions within a model scope.

use std::any::Any;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::{BTreeSet, HashMap};
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use ospf_rust_math::symbol::{DynSymbol, Linear as MathLinear, LinearMonomial, OwnedSymbol, SymbolDynId};
use ospf_rust_math::symbol::operation::Evaluate;
use ospf_rust_core::model::MetaModel as RustMetaModel;
use ospf_rust_framework::solver::remote::SerializedConstraintSign;
use ospf_rust_bridge::BridgeError;
use crate::{bridge_error, error};
use crate::quadratic::Quadratic;

pub(crate) struct Scope {
    pub(crate) disposed: Cell<bool>,
    pub(crate) variables: Cell<usize>,
    pub(crate) next_symbol: Cell<u64>,
    pub(crate) source: RefCell<Option<RustMetaModel<f64>>>,
    pub(crate) quadratic_objective: RefCell<Vec<(usize, usize, f64)>>,
}

impl Scope {
    pub(crate) fn new(name: &str) -> Self {
        Self {
            disposed: Cell::new(false), variables: Cell::new(0), next_symbol: Cell::new(1),
            source: RefCell::new(Some(RustMetaModel::new(name))),
            quadratic_objective: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn check(&self) -> Result<(), JsValue> {
        if self.disposed.get() {
            Err(bridge_error(BridgeError::new("Disposed", "the owning model has been disposed")))
        } else { Ok(()) }
    }

    pub(crate) fn model(&self) -> Result<Ref<'_, RustMetaModel<f64>>, JsValue> {
        self.check()?;
        let source = self.source.try_borrow().map_err(|_| error("model is already borrowed"))?;
        Ref::filter_map(source, Option::as_ref).map_err(|_| error("model is unavailable"))
    }

    pub(crate) fn model_mut(&self) -> Result<RefMut<'_, RustMetaModel<f64>>, JsValue> {
        self.check()?;
        let source = self.source.try_borrow_mut().map_err(|_| error("model is already borrowed"))?;
        RefMut::filter_map(source, Option::as_mut).map_err(|_| error("model is unavailable"))
    }

    pub(crate) fn compile_quadratic(&self) -> Result<ospf_rust_core::model::intermediate::QuadraticTetradModel, JsValue> {
        let mut model = self.model()?.try_to_quadratic_tetrad_model().map_err(error)?;
        model.Q = ospf_rust_bridge::quadratic_objective_matrix(model.num_variables(), self.quadratic_objective.borrow().iter().copied()).map_err(bridge_error)?;
        Ok(model)
    }
}

// 索引只在经 Rc 身份校验的模型作用域内有效，不复用用户可变名称作为身份。
// Indices are local to an Rc-checked model scope; mutable user names are not identities.
#[derive(Clone, Debug)]
pub(crate) struct IndexedVariable(pub(crate) usize);

impl Display for IndexedVariable {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { write!(f, "x{}", self.0) }
}

impl DynSymbol for IndexedVariable {
    fn name(&self) -> &str { "variable" }
    fn display_name(&self) -> &str { "variable" }
    fn dyn_id(&self) -> SymbolDynId<'_> { SymbolDynId::standalone(self.0) }
    fn as_any(&self) -> &dyn Any { self }
}

/// f64 线性多项式；运算返回独立对象，不修改操作数。
/// f64 linear polynomial; operations return independent objects without mutating operands.
#[wasm_bindgen]
pub struct Linear {
    pub(crate) scope: Rc<Scope>,
    pub(crate) value: MathLinear<f64>,
    pub(crate) dependencies: BTreeSet<u64>,
}

impl Linear {
    pub(crate) fn new(
        scope: Rc<Scope>,
        indices: &[u32],
        coefficients: &[f64],
        constant: f64,
    ) -> Result<Self, JsValue> {
        scope.check()?;
        if indices.len() != coefficients.len()
            || indices.iter().any(|&index| index as usize >= scope.variables.get())
        { return Err(error("invalid expression shape or variable index")); }
        let monomials = indices.iter().zip(coefficients).map(|(&index, &coefficient)| {
            LinearMonomial::new(coefficient, OwnedSymbol::new(IndexedVariable(index as usize)))
        }).collect();
        Self::checked(scope, MathLinear::new(monomials, constant))
    }

    fn checked(scope: Rc<Scope>, mut value: MathLinear<f64>) -> Result<Self, JsValue> {
        scope.check()?;
        if !value.constant.is_finite() || value.monomials.iter().any(|term| !term.coefficient.is_finite()) {
            return Err(error("expression contains non-finite values"));
        }
        value.simplify();
        if value.monomials.iter().any(|term| !term.coefficient.is_finite()) {
            return Err(error("expression coefficient overflow"));
        }
        // 固定求值和交换顺序，避免 HashMap 简化顺序影响浮点结果与指纹。
        // Stabilize evaluation and exchange order after HashMap-based simplification.
        value.monomials.sort_by_key(|term| term.symbol.dyn_id().parent_id);
        Ok(Self { scope, value, dependencies: BTreeSet::new() })
    }

    fn derived(&self, value: MathLinear<f64>) -> Result<Self, JsValue> {
        let mut result = Self::checked(self.scope.clone(), value)?;
        result.dependencies = self.dependencies.clone();
        Ok(result)
    }

    pub(crate) fn check_scope(&self, scope: &Rc<Scope>) -> Result<(), JsValue> {
        self.scope.check()?;
        scope.check()?;
        if !Rc::ptr_eq(&self.scope, scope) {
            return Err(bridge_error(BridgeError::new("InvalidReference", "expression belongs to another model")));
        }
        Ok(())
    }

    pub(crate) fn terms(&self) -> impl Iterator<Item = (usize, f64)> + '_ {
        self.value.monomials.iter().map(|term| (term.symbol.dyn_id().parent_id, term.coefficient))
    }
}

#[wasm_bindgen]
impl Linear {
    /// 同模型表达式相加 / Add expressions from the same model.
    pub fn add(&self, other: &Linear) -> Result<Linear, JsValue> {
        self.check_scope(&other.scope)?;
        let mut result = self.derived(&self.value + &other.value)?;
        result.dependencies.extend(&other.dependencies);
        Ok(result)
    }

    /// 同模型表达式相减 / Subtract expressions from the same model.
    pub fn sub(&self, other: &Linear) -> Result<Linear, JsValue> {
        self.check_scope(&other.scope)?;
        let mut result = self.derived(&self.value - &other.value)?;
        result.dependencies.extend(&other.dependencies);
        Ok(result)
    }

    /// 增加有限常数 / Add a finite constant.
    pub fn offset(&self, value: f64) -> Result<Linear, JsValue> {
        self.scope.check()?;
        if !value.is_finite() { return Err(error("offset must be finite")); }
        self.derived(&self.value + value)
    }

    /// 乘有限标量 / Multiply by a finite scalar.
    pub fn mul(&self, value: f64) -> Result<Linear, JsValue> {
        self.scope.check()?;
        if !value.is_finite() { return Err(error("multiplier must be finite")); }
        self.derived(&self.value * value)
    }

    /// 两个线性式相乘，复用 Rust 二次多项式运算。
    /// Multiply two linear expressions through Rust quadratic polynomial arithmetic.
    pub fn product(&self, other: &Linear) -> Result<Quadratic, JsValue> {
        self.check_scope(&other.scope)?;
        let mut dependencies = self.dependencies.clone();
        dependencies.extend(&other.dependencies);
        Quadratic::checked(self.scope.clone(), self.value.clone() * &other.value, dependencies)
    }

    /// 无损提升为二次表达式 / Promote losslessly to a quadratic expression.
    #[wasm_bindgen(js_name = toQuadratic)]
    pub fn to_quadratic(&self) -> Result<Quadratic, JsValue> {
        use ospf_rust_math::symbol::operation::ToQuadratic;
        self.scope.check()?;
        Quadratic::checked(self.scope.clone(), (&self.value).to_quadratic(), self.dependencies.clone())
    }

    /// 读取常数项 / Read the constant term.
    pub fn constant(&self) -> Result<f64, JsValue> {
        self.scope.check()?;
        Ok(self.value.constant)
    }

    /// 读取当前变量顺序的稠密系数副本 / Read a dense coefficient copy in current variable order.
    pub fn coefficients(&self) -> Result<Vec<f64>, JsValue> {
        self.scope.check()?;
        let mut values = vec![0.0; self.scope.variables.get()];
        for (index, coefficient) in self.terms() { values[index] = coefficient; }
        Ok(values)
    }

    /// 以当前完整变量顺序求值，不允许缺失值或非有限值。
    /// Evaluate using all current variables, rejecting missing and non-finite values.
    pub fn evaluate(&self, values: &[f64]) -> Result<f64, JsValue> {
        self.scope.check()?;
        if values.len() != self.scope.variables.get() || values.iter().any(|value| !value.is_finite()) {
            return Err(error("invalid evaluation shape or values"));
        }
        let values: HashMap<_, _> = self.value.monomials.iter().map(|term| {
            (term.symbol.clone(), values[term.symbol.dyn_id().parent_id])
        }).collect();
        let value = self.value.evaluate(&values);
        if !value.is_finite() { return Err(error("expression evaluation overflow")); }
        Ok(value)
    }

    /// 创建相对于零的关系，右表达式使用 sub 显式归一化。
    /// Create a relation to a scalar; use sub to normalize an expression right-hand side.
    pub fn compare(&self, relation: &str, rhs: f64) -> Result<LinearInequality, JsValue> {
        self.scope.check()?;
        if !rhs.is_finite() { return Err(error("constraint RHS must be finite")); }
        let sign = match relation {
            "le" => SerializedConstraintSign::LessEqual,
            "ge" => SerializedConstraintSign::GreaterEqual,
            "eq" => SerializedConstraintSign::Equal,
            _ => return Err(error("unknown constraint relation")),
        };
        let rhs = rhs - self.value.constant;
        if !rhs.is_finite() { return Err(error("constraint normalization overflow")); }
        Ok(LinearInequality { expression: self.derived(self.value.clone())?, sign, rhs })
    }
}

/// 捕获表达式快照的线性关系 / Linear relation capturing an expression snapshot.
#[wasm_bindgen]
pub struct LinearInequality {
    pub(crate) expression: Linear,
    pub(crate) sign: SerializedConstraintSign,
    pub(crate) rhs: f64,
}
