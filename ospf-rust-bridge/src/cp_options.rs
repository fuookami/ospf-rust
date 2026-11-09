//! 原生 CP 执行参数，无损整数提示 / Native CP execution options with lossless integer hints.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::solver::StableVariableId;
use crate::{BridgeError, Result};

/// 精确整数提示项，数值只按十进制文本跨边界 / Exact integer hint entry; values cross the boundary only as decimal text.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntegerSolutionHint {
    /// 接收模型中的稳定变量 ID / Stable variable ID in the receiving model.
    pub variable_id: String,
    /// i64 十进制文本 / Decimal i64 text.
    pub value: String,
}

/// 原生 CP 的显式参数；限制值零保留 core 的立即终止语义。
/// Explicit native CP options; zero limits retain core's immediate termination semantics.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintProgrammingOptions {
    /// 后端节点上限 / Backend node limit.
    #[ts(optional)]
    pub node_limit: Option<u32>,
    /// 后端可行解上限 / Backend feasible-solution limit.
    #[ts(optional)]
    pub solution_limit: Option<u32>,
    /// 完整或部分精确提示；未知 ID 和非法赋值由 core 拒绝 / Complete or partial exact hint; core rejects unknown IDs and invalid assignments.
    #[ts(optional)]
    pub solution_hint: Option<Vec<IntegerSolutionHint>>,
    /// 请求整个提交模型的经验证 conflict seed，不承诺最小性 / Request a verified whole-submitted-model conflict seed, without a minimality guarantee.
    #[ts(optional)]
    pub request_conflict: Option<bool>,
}

impl ConstraintProgrammingOptions {
    /// 校验提示编码、规模和重复身份，不替代 core 的模型语义校验。
    /// Validate hint encoding, size and duplicate identity without replacing core model-semantic validation.
    pub fn decode_hint(&self) -> Result<Option<BTreeMap<StableVariableId, i64>>> {
        let Some(entries) = &self.solution_hint else { return Ok(None); };
        if entries.len() > 100_000 { return Err(BridgeError::new("PayloadTooLarge", "CP solution hint exceeds 100000 entries")); }
        let mut result = BTreeMap::new();
        for entry in entries {
            if entry.variable_id.is_empty() || entry.variable_id.len() > 4096 || entry.value.len() > 20 {
                return Err(BridgeError::new("InvalidInput", "invalid CP solution hint entry"));
            }
            let value = entry.value.parse::<i64>().map_err(|_| BridgeError::new("InvalidInput", "CP solution hint value must be decimal i64 text"))?;
            if value.to_string() != entry.value {
                return Err(BridgeError::new("InvalidInput", "CP solution hint value must use canonical decimal text"));
            }
            if result.insert(StableVariableId::from(entry.variable_id.as_str()), value).is_some() {
                return Err(BridgeError::new("InvalidInput", "duplicate CP solution hint variable"));
            }
        }
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_codec_preserves_i64_values_and_rejects_lossy_or_ambiguous_inputs() {
        let options: ConstraintProgrammingOptions = serde_json::from_str(r#"{"nodeLimit":0,"solutionLimit":0,"solutionHint":[{"variableId":"x","value":"9007199254740993"}],"requestConflict":true}"#).unwrap();
        assert_eq!(options.node_limit, Some(0));
        assert_eq!(options.decode_hint().unwrap().unwrap()[&StableVariableId::from("x")], 9_007_199_254_740_993);
        for value in ["9007199254740993.0", "9223372036854775808", "-9223372036854775809", "+1", "01", "-0", " 1", "1e3"] {
            let options = ConstraintProgrammingOptions { solution_hint: Some(vec![IntegerSolutionHint { variable_id: "x".into(), value: value.into() }]), ..Default::default() };
            assert_eq!(options.decode_hint().unwrap_err().code, "InvalidInput");
        }
        let mut options = options;
        let duplicate = options.solution_hint.as_ref().unwrap()[0].clone();
        options.solution_hint.as_mut().unwrap().push(duplicate);
        assert_eq!(options.decode_hint().unwrap_err().code, "InvalidInput");
        for json in [r#"{"nodeLimit":-1}"#, r#"{"solutionLimit":4294967296}"#, r#"{"solutionHint":[{"variableId":"x","value":9007199254740993}]}"#, r#"{"requestConflict":"true"}"#, r#"{"shrinkConflict":true}"#] {
            assert!(serde_json::from_str::<ConstraintProgrammingOptions>(json).is_err());
        }
    }
}
