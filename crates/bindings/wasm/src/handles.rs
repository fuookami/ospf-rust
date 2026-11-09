//! 保留原生 token 与符号身份的模型句柄 / Model handles preserving native token and symbol identities.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use ospf_rust_math::symbol::{DynSymbol, OwnedSymbol, Quadratic as MathQuadratic, QuadraticMonomial as MathQuadraticMonomial, Symbol};
use ospf_rust_core::model::flatten::{LinearMonomial, QuadraticMonomial};
use ospf_rust_core::symbol::{IntermediateSymbol, LinearExpressionSymbol as RustSymbol, LinearIntermediateSymbol, QuadraticExpressionSymbol as RustQuadraticSymbol, QuadraticIntermediateSymbol};
use ospf_rust_core::token::Token as RustToken;
use ospf_rust_core::variable::{VariableId, VariableType};
use ospf_rust_bridge::BridgeError;
use crate::linear::{IndexedVariable, Linear, Scope};
use crate::quadratic::Quadratic;
use crate::{bridge_error, error};

/// 真实 Rust token 的稳定身份句柄，不是重新构造的变量副本。
/// Stable identity handle to a real Rust token, not a reconstructed variable copy.
#[wasm_bindgen]
pub struct Token {
    scope: Rc<Scope>,
    id: VariableId,
}

impl Token {
    pub(crate) fn new(scope: Rc<Scope>, index: usize) -> Result<Self, JsValue> {
        let id = scope.model()?.tokens().get(index).map(RustToken::id)
            .ok_or_else(|| error("token index out of bounds"))?;
        Ok(Self { scope, id })
    }

    fn read<T>(&self, read: impl FnOnce(&RustToken<f64>) -> T) -> Result<T, JsValue> {
        let source = self.scope.model()?;
        let token = source.find_token(self.id)
            .ok_or_else(|| bridge_error(BridgeError::new("InvalidReference", "token is no longer registered")))?;
        Ok(read(token))
    }
}

#[wasm_bindgen]
impl Token {
    /// 精确身份，JS 中为 bigint；不跨模型或进程复用。
    /// Exact identity exposed as bigint in JS; not reusable across models or processes.
    pub fn id(&self) -> Result<u64, JsValue> { self.read(|token| token.id().unique_id()) }

    /// 当前求解器顺序索引 / Current solver-order index.
    #[wasm_bindgen(js_name = solverIndex)]
    pub fn solver_index(&self) -> Result<u32, JsValue> {
        u32::try_from(self.read(|token| token.solver_index)?).map_err(error)
    }

    /// 变量名称 / Variable name.
    pub fn name(&self) -> Result<String, JsValue> { self.read(|token| token.name().into()) }

    /// 当前桥接支持的变量类型 / Variable kind supported by this bridge.
    pub fn kind(&self) -> Result<String, JsValue> {
        match self.read(RustToken::var_type)? {
            VariableType::Continuous => Ok("continuous".into()),
            VariableType::Integer => Ok("integer".into()),
            VariableType::Binary => Ok("binary".into()),
            _ => Err(error("token type is not exposed by this binding")),
        }
    }

    /// 有限下界；None 表示无界 / Finite lower bound; None means unbounded.
    #[wasm_bindgen(js_name = lowerBound)]
    pub fn lower_bound(&self) -> Result<Option<f64>, JsValue> { self.read(|token| token.range().lower_bound) }

    /// 有限上界；None 表示无界 / Finite upper bound; None means unbounded.
    #[wasm_bindgen(js_name = upperBound)]
    pub fn upper_bound(&self) -> Result<Option<f64>, JsValue> { self.read(|token| token.range().upper_bound) }

    /// 最近显式绑定的赋值，不是可行性或求解证明。
    /// Most recently bound explicit assignment, not a feasibility or solve certificate.
    pub fn result(&self) -> Result<Option<f64>, JsValue> { self.read(RustToken::get_result) }

    /// 以该 token 构造单位系数表达式 / Construct a unit-coefficient expression from this token.
    #[wasm_bindgen(js_name = toLinear)]
    pub fn to_linear(&self) -> Result<Linear, JsValue> {
        Linear::new(self.scope.clone(), &[self.solver_index()?], &[1.0], 0.0)
    }
}

/// 已注册 Rust 线性表达式符号；释放句柄不注销符号。
/// Registered Rust linear-expression symbol; releasing a handle does not unregister it.
#[wasm_bindgen]
pub struct LinearExpressionSymbol {
    scope: Rc<Scope>,
    value: Arc<RustSymbol<f64>>,
}

impl LinearExpressionSymbol {
    fn check(&self) -> Result<(), JsValue> {
        if !self.scope.model()?.symbols().iter().any(|symbol| symbol.id().id == self.value.id().id) {
            return Err(bridge_error(BridgeError::new("InvalidReference", "symbol is no longer registered")));
        }
        Ok(())
    }
}

#[wasm_bindgen]
impl LinearExpressionSymbol {
    /// 模型内精确符号身份 / Exact symbol identity within its model.
    pub fn id(&self) -> Result<u64, JsValue> { self.check()?; Ok(self.value.id().id) }

    /// 注册名称 / Registered name.
    pub fn name(&self) -> Result<String, JsValue> { self.check()?; Ok(self.value.name().into()) }

    /// 直接依赖的符号 ID 副本 / Copy of directly depended-on symbol IDs.
    #[wasm_bindgen(js_name = dependencyIds)]
    pub fn dependency_ids(&self) -> Result<Vec<u64>, JsValue> {
        self.check()?;
        Ok(self.scope.model()?.symbol_dependency_ids(self.value.id().id))
    }

    /// 经 Rust 符号特化接口平展，并保留此符号为后续注册的直接依赖。
    /// Flatten through the Rust symbol specialization, retaining this symbol as a direct dependency.
    #[wasm_bindgen(js_name = toLinear)]
    pub fn to_linear(&self) -> Result<Linear, JsValue> {
        self.check()?;
        let polynomial = self.value.to_linear_polynomial();
        let indices: Vec<_> = polynomial.monomials().iter().map(|term| u32::try_from(term.var_index()).map_err(error)).collect::<Result<_, _>>()?;
        let coefficients: Vec<_> = polynomial.monomials().iter().map(|term| *term.coefficient()).collect();
        let mut expression = Linear::new(self.scope.clone(), &indices, &coefficients, *polynomial.constant_term())?;
        expression.dependencies.insert(self.value.id().id);
        Ok(expression)
    }

    /// 用完整变量顺序求值，使用 Rust 符号 prepare 语义。
    /// Evaluate in complete variable order using Rust symbol prepare semantics.
    pub fn evaluate(&self, values: &[f64]) -> Result<f64, JsValue> {
        self.check()?;
        if values.len() != self.scope.variables.get() || values.iter().any(|value| !value.is_finite()) {
            return Err(error("invalid symbol evaluation shape or values"));
        }
        let values: HashMap<_, _> = values.iter().copied().enumerate().collect();
        let value = self.value.prepare(&values).ok_or_else(|| error("symbol evaluation is incomplete"))?;
        if !value.is_finite() { return Err(error("symbol evaluation overflow")); }
        Ok(value)
    }

    /// 从 token 赋值求值；缺失任一所需 token 时返回 None。
    /// Evaluate token assignments, returning None if a required token is unassigned.
    #[wasm_bindgen(js_name = evaluateSolution)]
    pub fn evaluate_solution(&self) -> Result<Option<f64>, JsValue> {
        self.check()?;
        let source = self.scope.model()?;
        let value = self.value.evaluate_from_tokens(source.as_basic(), false);
        if value.is_some_and(|value| !value.is_finite()) { return Err(error("symbol evaluation overflow")); }
        Ok(value)
    }
}

pub(crate) fn register_symbol(scope: Rc<Scope>, name: &str, expression: &Linear) -> Result<LinearExpressionSymbol, JsValue> {
    expression.check_scope(&scope)?;
    if name.trim().is_empty() { return Err(error("symbol name is blank")); }
    let id = scope.next_symbol.get();
    let next = id.checked_add(1).ok_or_else(|| error("symbol identity exhausted"))?;
    let monomials = expression.terms().map(|(index, coefficient)| LinearMonomial::new(coefficient, index)).collect();
    let value = Arc::new(RustSymbol::new_with_dependencies(id, name, monomials, expression.constant()?, expression.dependencies.iter().copied().collect()));
    commit_symbol(&scope, name, value.clone(), next)?;
    Ok(LinearExpressionSymbol { scope, value })
}

/// 已注册 Rust 二次表达式符号；释放句柄不注销符号。
/// Registered Rust quadratic-expression symbol; releasing a handle does not unregister it.
#[wasm_bindgen]
pub struct QuadraticExpressionSymbol {
    scope: Rc<Scope>,
    value: Arc<RustQuadraticSymbol<f64>>,
}

impl QuadraticExpressionSymbol {
    fn check(&self) -> Result<(), JsValue> {
        if !self.scope.model()?.symbols().iter().any(|symbol| symbol.id().id == self.value.id().id) {
            return Err(bridge_error(BridgeError::new("InvalidReference", "symbol is no longer registered")));
        }
        Ok(())
    }
}

#[wasm_bindgen]
impl QuadraticExpressionSymbol {
    /// 模型内精确符号身份 / Exact symbol identity within its model.
    pub fn id(&self) -> Result<u64, JsValue> { self.check()?; Ok(self.value.id().id) }

    /// 注册名称 / Registered name.
    pub fn name(&self) -> Result<String, JsValue> { self.check()?; Ok(self.value.name().into()) }

    /// 直接依赖的符号 ID 副本 / Copy of directly depended-on symbol IDs.
    #[wasm_bindgen(js_name = dependencyIds)]
    pub fn dependency_ids(&self) -> Result<Vec<u64>, JsValue> {
        self.check()?;
        Ok(self.scope.model()?.symbol_dependency_ids(self.value.id().id))
    }

    /// 平展为二次表达式并保留当前符号为直接依赖。
    /// Flatten to a quadratic expression retaining this symbol as a direct dependency.
    #[wasm_bindgen(js_name = toQuadratic)]
    pub fn to_quadratic(&self) -> Result<Quadratic, JsValue> {
        self.check()?;
        let polynomial = self.value.to_quadratic_polynomial();
        let monomials = polynomial.monomials().iter().map(|term| MathQuadraticMonomial::new(
            *term.coefficient(),
            OwnedSymbol::new(IndexedVariable(term.var_index1())),
            term.var_index2().map(|index| OwnedSymbol::new(IndexedVariable(index))),
        )).collect();
        Quadratic::checked(
            self.scope.clone(),
            MathQuadratic::new(monomials, *polynomial.constant()),
            [self.value.id().id].into_iter().collect(),
        )
    }

    /// 用完整变量顺序求值 / Evaluate using the complete variable order.
    pub fn evaluate(&self, values: &[f64]) -> Result<f64, JsValue> {
        self.check()?;
        if values.len() != self.scope.variables.get() || values.iter().any(|value| !value.is_finite()) {
            return Err(error("invalid symbol evaluation shape or values"));
        }
        let values: HashMap<_, _> = values.iter().copied().enumerate().collect();
        let value = self.value.prepare(&values).ok_or_else(|| error("symbol evaluation is incomplete"))?;
        if !value.is_finite() { return Err(error("symbol evaluation overflow")); }
        Ok(value)
    }

    /// 从 token 赋值求值；缺失任一所需 token 时返回 None。
    /// Evaluate token assignments, returning None if a required token is unassigned.
    #[wasm_bindgen(js_name = evaluateSolution)]
    pub fn evaluate_solution(&self) -> Result<Option<f64>, JsValue> {
        self.check()?;
        let source = self.scope.model()?;
        let value = self.value.evaluate_from_tokens(source.as_basic(), false);
        if value.is_some_and(|value| !value.is_finite()) { return Err(error("symbol evaluation overflow")); }
        Ok(value)
    }
}

pub(crate) fn register_quadratic_symbol(scope: Rc<Scope>, name: &str, expression: &Quadratic) -> Result<QuadraticExpressionSymbol, JsValue> {
    expression.check_model(&scope)?;
    if name.trim().is_empty() { return Err(error("symbol name is blank")); }
    let id = scope.next_symbol.get();
    let next = id.checked_add(1).ok_or_else(|| error("symbol identity exhausted"))?;
    let monomials = expression.value.monomials.iter().map(|term| match &term.symbol2 {
        Some(second) => QuadraticMonomial::new_quadratic(term.coefficient, term.symbol1.dyn_id().parent_id, second.dyn_id().parent_id),
        None => QuadraticMonomial::new_linear(term.coefficient, term.symbol1.dyn_id().parent_id),
    }).collect();
    let value = Arc::new(RustQuadraticSymbol::new_with_dependencies(
        id, name, monomials, expression.constant()?, expression.dependencies.iter().copied().collect(),
    ));
    commit_symbol(&scope, name, value.clone(), next)?;
    Ok(QuadraticExpressionSymbol { scope, value })
}

fn commit_symbol(scope: &Rc<Scope>, name: &str, registered: Arc<dyn IntermediateSymbol<f64>>, next: u64) -> Result<(), JsValue> {
    {
        let mut source = scope.model_mut()?;
        if source.symbols().iter().any(|symbol| symbol.id().name == name) {
            return Err(bridge_error(BridgeError::new("DuplicateName", "symbol name is already registered")));
        }
        source.transaction(|source| {
            source.add_symbols([registered]).map_err(error)?;
            source.try_to_quadratic_tetrad_model().map_err(error)?;
            source.clear_solution();
            Ok::<_, JsValue>(())
        })?;
    }
    scope.next_symbol.set(next);
    Ok(())
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use crate::MetaModel;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn tokens_and_registered_symbols_retain_native_identity() {
        let mut model = MetaModel::new("handles").unwrap();
        model.add_variable("x", "continuous", Some(0.0), Some(10.0)).unwrap();
        let token = model.token(0).unwrap();
        let id = token.id().unwrap();
        let x = token.to_linear().unwrap();
        let input = x.mul(2.0).unwrap().offset(3.0).unwrap();
        let symbol = model.add_linear_symbol("cost", &input).unwrap();
        assert_eq!(symbol.name().unwrap(), "cost");
        assert_eq!(symbol.evaluate(&[4.0]).unwrap(), 11.0);
        assert_eq!(symbol.evaluate_solution().unwrap(), None);
        let reference = symbol.to_linear().unwrap();
        let derived = model.add_linear_symbol("derived", &reference.mul(2.0).unwrap()).unwrap();
        assert_eq!(derived.dependency_ids().unwrap(), vec![symbol.id().unwrap()]);
        assert_eq!(model.scope.model().unwrap().symbols().len(), 2);
        model.set_solution(&[4.0]).unwrap();
        assert_eq!(token.result().unwrap(), Some(4.0));
        assert_eq!(symbol.evaluate_solution().unwrap(), Some(11.0));
        assert_eq!(derived.evaluate_solution().unwrap(), Some(22.0));
        assert!(model.set_solution(&[11.0]).is_err());
        assert_eq!(token.result().unwrap(), Some(4.0));
        let fingerprint = model.to_linear_triad_model().unwrap();
        assert!(model.add_linear_symbol("cost", &input).is_err());
        assert_eq!(model.to_linear_triad_model().unwrap(), fingerprint);
        assert_eq!(symbol.evaluate_solution().unwrap(), Some(11.0));
        let inequality = symbol.to_linear().unwrap().compare("le", 11.0).unwrap();
        model.add_inequality(&inequality, "capacity").unwrap();
        assert_eq!(symbol.evaluate_solution().unwrap(), None);
        assert_eq!(token.id().unwrap(), id);
        model.add_variable("y", "binary", Some(0.0), Some(1.0)).unwrap();
        assert_eq!(token.id().unwrap(), id);
        assert_eq!(model.token(0).unwrap().id().unwrap(), id);
        assert_eq!(token.kind().unwrap(), "continuous");
        assert_eq!(model.token(1).unwrap().kind().unwrap(), "binary");
        assert_eq!(model.token(1).unwrap().solver_index().unwrap(), 1);
        assert!(model.set_solution(&[1.0, 0.5]).is_err());
        model.set_solution(&[1.0, 1.0]).unwrap();
        model.clear_solution().unwrap();
        assert_eq!(token.result().unwrap(), None);
        let mut other = MetaModel::new("other").unwrap();
        assert!(other.add_linear_symbol("foreign", &reference).is_err());
        drop(model);
        assert!(token.name().is_err());
        assert!(symbol.id().is_err());
        assert!(derived.evaluate_solution().is_err());
    }

    #[wasm_bindgen_test]
    fn quadratic_symbols_retain_native_terms_dependencies_and_transactional_registration() {
        let mut model = MetaModel::new("quadratic-symbols").unwrap();
        model.add_variable("x", "continuous", None, None).unwrap();
        model.add_variable("y", "continuous", None, None).unwrap();
        let x = model.linear_expression(&[0], &[1.0], 0.0).unwrap();
        let linear = model.linear_expression(&[0, 1], &[2.0, 1.0], 1.0).unwrap();
        let linear_symbol = model.add_linear_symbol("linear", &linear).unwrap();
        let expression = linear_symbol.to_linear().unwrap().product(&x).unwrap().offset(3.0).unwrap();
        let symbol = model.add_quadratic_symbol("quadratic", &expression).unwrap();
        assert_eq!(symbol.name().unwrap(), "quadratic");
        assert_eq!(symbol.dependency_ids().unwrap(), vec![linear_symbol.id().unwrap()]);
        assert_eq!(symbol.evaluate(&[2.0, 3.0]).unwrap(), 19.0);
        assert_eq!(symbol.evaluate_solution().unwrap(), None);
        let reference = symbol.to_quadratic().unwrap();
        assert_eq!(reference.evaluate(&[2.0, 3.0]).unwrap(), 19.0);
        assert_eq!(reference.linear_coefficients().unwrap(), vec![1.0, 0.0]);
        assert_eq!(reference.row_indices().unwrap(), vec![0, 0]);
        assert_eq!(reference.column_indices().unwrap(), vec![0, 1]);
        assert_eq!(reference.quadratic_coefficients().unwrap(), vec![2.0, 1.0]);
        let derived = model.add_quadratic_symbol("derived", &reference.mul(2.0).unwrap()).unwrap();
        assert_eq!(derived.dependency_ids().unwrap(), vec![symbol.id().unwrap()]);
        model.set_solution(&[2.0, 3.0]).unwrap();
        assert_eq!(symbol.evaluate_solution().unwrap(), Some(19.0));
        assert_eq!(derived.evaluate_solution().unwrap(), Some(38.0));
        let before = model.to_quadratic_tetrad_model().unwrap();
        assert!(model.add_quadratic_symbol("linear", &reference).is_err());
        assert!(model.add_linear_symbol("quadratic", &linear).is_err());
        assert_eq!(model.to_quadratic_tetrad_model().unwrap(), before);
        assert_eq!(symbol.evaluate_solution().unwrap(), Some(19.0));
        assert!(symbol.evaluate(&[2.0]).is_err());
        assert!(symbol.evaluate(&[f64::NAN, 0.0]).is_err());
        assert!(symbol.evaluate(&[f64::MAX, f64::MAX]).is_err());
        let mut other = MetaModel::new("other-quadratic").unwrap();
        assert!(other.add_quadratic_symbol("foreign", &reference).is_err());
        model.add_variable("later", "continuous", None, None).unwrap();
        assert_eq!(symbol.evaluate_solution().unwrap(), None);
        assert!(symbol.evaluate(&[2.0, 3.0]).is_err());
        model.set_solution(&[2.0, 3.0, 0.0]).unwrap();
        assert_eq!(derived.evaluate_solution().unwrap(), Some(38.0));
        drop(model);
        assert!(symbol.id().is_err());
        assert!(derived.evaluate_solution().is_err());
        assert!(reference.evaluate(&[2.0, 3.0, 0.0]).is_err());
    }
}
