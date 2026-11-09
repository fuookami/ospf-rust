//! 不可变 assumptions 派生与默认降维预检 / Immutable assumption derivation and default lowering preflight.

use wasm_bindgen::prelude::*;
use ospf_rust_core::model::constraint_programming::{BooleanLiteral, IntegerVariable};
use ospf_rust_core::solver::constraint_programming::{ConstraintProgrammingAssumption, snapshot_with_assumptions};
use ospf_rust_bridge::analyze_cp_mip_lowering;
use crate::cp::{ConstraintProgrammingSnapshot, IntegerDomain, integer};
use crate::error;

/// 内部临时批次，复制快照及假设，不持有 JS 回调 / Internal temporary batch, copying snapshot and assumptions without JS callbacks.
#[wasm_bindgen]
pub struct ConstraintProgrammingAssumptions {
    snapshot: ConstraintProgrammingSnapshot,
    values: Vec<ConstraintProgrammingAssumption>,
}

impl ConstraintProgrammingAssumptions {
    fn variable(&self, id: &str) -> Result<IntegerVariable, JsValue> {
        self.snapshot.value.variables.iter().find(|entry| entry.variable.stable_id.0 == id)
            .map(|entry| entry.variable.clone()).ok_or_else(|| error("assumption references an unknown snapshot variable"))
    }
}

#[wasm_bindgen]
impl ConstraintProgrammingAssumptions {
    /// 要求 Boolean literal 为真 / Require a Boolean literal to be true.
    pub fn literal(&mut self, id: &str, negated: bool) -> Result<(), JsValue> {
        self.values.push(ConstraintProgrammingAssumption::Literal(BooleanLiteral { variable: self.variable(id)?, negated }));
        Ok(())
    }

    /// 固定精确整数 / Fix an exact integer.
    pub fn equal(&mut self, id: &str, value: &str) -> Result<(), JsValue> {
        self.values.push(ConstraintProgrammingAssumption::Equal(self.variable(id)?, integer(value)?));
        Ok(())
    }

    /// 收紧精确下界 / Tighten an exact lower bound.
    #[wasm_bindgen(js_name = lowerBound)]
    pub fn lower_bound(&mut self, id: &str, value: &str) -> Result<(), JsValue> {
        self.values.push(ConstraintProgrammingAssumption::LowerBound(self.variable(id)?, integer(value)?));
        Ok(())
    }

    /// 收紧精确上界 / Tighten an exact upper bound.
    #[wasm_bindgen(js_name = upperBound)]
    pub fn upper_bound(&mut self, id: &str, value: &str) -> Result<(), JsValue> {
        self.values.push(ConstraintProgrammingAssumption::UpperBound(self.variable(id)?, integer(value)?));
        Ok(())
    }

    /// 复制域；core 要求其为源域的子集 / Copy the domain; core requires it to be a subset of the source domain.
    #[wasm_bindgen(js_name = sparseDomain)]
    pub fn sparse_domain(&mut self, id: &str, domain: &IntegerDomain) -> Result<(), JsValue> {
        self.values.push(ConstraintProgrammingAssumption::SparseDomain(self.variable(id)?, domain.value.clone()));
        Ok(())
    }

    /// 一次性校验完整假设并生成独立快照 / Validate the complete assumption set and create an independent snapshot.
    pub fn freeze(&self) -> Result<ConstraintProgrammingSnapshot, JsValue> {
        Ok(ConstraintProgrammingSnapshot { value: snapshot_with_assumptions(&self.snapshot.value, &self.values).map_err(error)? })
    }
}

#[wasm_bindgen]
impl ConstraintProgrammingSnapshot {
    /// 创建临时假设批次；调用方负责释放 / Create a temporary assumption batch; callers must free it.
    pub fn assumptions(&self) -> ConstraintProgrammingAssumptions {
        ConstraintProgrammingAssumptions { snapshot: Self { value: self.value.clone() }, values: vec![] }
    }

    /// 同步执行默认预算降维预检，不调用求解器 / Synchronously preflight default-budget lowering without invoking a solver.
    #[wasm_bindgen(js_name = analyzeMipLowering)]
    pub fn analyze_mip_lowering(&self) -> Result<String, JsValue> {
        serde_json::to_string(&analyze_cp_mip_lowering(&self.value)).map_err(error)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use ospf_rust_core::model::constraint_programming::ConstraintProgrammingModel;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn assumption_batches_copy_source_and_reuse_core_identity() {
        let high = 9_007_199_254_740_993;
        let mut model = ConstraintProgrammingModel::new("assumptions");
        model.add_variable(IntegerVariable::new("x"), ospf_rust_core::model::constraint_programming::IntegerDomain::range(high, high + 2).unwrap()).unwrap();
        let source = ConstraintProgrammingSnapshot { value: model.freeze().unwrap() };
        let mut batch = source.assumptions();
        batch.equal("x", &high.to_string()).unwrap();
        let derived = batch.freeze().unwrap();
        let expected = snapshot_with_assumptions(&source.value, &[ConstraintProgrammingAssumption::Equal(IntegerVariable::new("x"), high)]).unwrap();
        assert_eq!(derived.value.fingerprint, expected.fingerprint);
        assert_ne!(derived.value.fingerprint, source.value.fingerprint);
        assert!(derived.validate_assignment(&[high]).is_ok());
        assert!(derived.validate_assignment(&[high + 1]).is_err());
        assert!(source.validate_assignment(&[high + 1]).is_ok());
        drop(source);
        assert_eq!(batch.freeze().unwrap().to_json().unwrap(), derived.to_json().unwrap());
        assert!(derived.analyze_mip_lowering().unwrap().contains("9007199254740993"));
    }
}
