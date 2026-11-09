//! 索引注册辅助绑定：按声明键注册变量并返回稳定键 → 模型索引映射。
//! Indexed-registration helper binding: register variables for declared keys and return a stable key → model-index map.
//!
//! 覆盖 `optional_array`（按需注册）、`appendable_pool`（可追加/退役）、`indexed_combination`（定维组合展开）。
//! Covers `optional_array` (register on demand), `appendable_pool` (append/retire) and `indexed_combination`
//! (fixed-arity combination expansion).
//!
//! 这三者是**构建期辅助**，没有独立的求解语义；它们的可移植部分正是"哪些键拿到了哪个模型索引"。
//! 因此桥接不搬运 Rust 侧的容器类型，而是让调用方声明键与注册策略，由 Rust 侧执行真实注册并把映射交回。
//! These three are **build-time helpers** without independent solve semantics; their portable part is exactly "which
//! key received which model index". The bridge therefore does not ship the Rust-side containers; callers declare keys
//! and a registration policy, Rust performs the real registration and hands the mapping back.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_core::model::MetaModel;
use ospf_rust_core::variable::{ContinuousVariableItem, VariableRange};

use crate::{BridgeError, ModelArtifact, Result};

/// 变量的整数性。 / The variable's integrality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum IndexedVariableKind {
    /// 连续。 / Continuous.
    Continuous,
    /// 整数。 / Integer.
    Integer,
    /// 二元。 / Binary.
    Binary,
}

/// 一个待注册的键及其取值范围。 / One key to register together with its value range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IndexedKey {
    /// 稳定的领域键；同一批次内不得重复。 / Stable domain key; it must not repeat within one batch.
    pub key: String,
    /// 可选下界；省略表示无下界。 / Optional lower bound; omission means unbounded below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lower_bound: Option<f64>,
    /// 可选上界；省略表示无上界。 / Optional upper bound; omission means unbounded above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub upper_bound: Option<f64>,
}

/// 注册策略。 / The registration policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum IndexedRegistrationMode {
    /// 对应 `optional_array`：只为声明的键注册，未声明的键没有索引。
    /// Mirrors `optional_array`: only declared keys get an index, and undeclared keys have none.
    Optional,
    /// 对应 `appendable_pool`：声明的键进入活跃集合，退役键保留历史但不进入映射。
    /// Mirrors `appendable_pool`: declared keys enter the active set, while retired keys stay historical without an index.
    Appendable,
    /// 对应 `indexed_combination`：按键列表顺序展开成定维组合，索引等于键的位置。
    /// Mirrors `indexed_combination`: keys expand into a fixed-arity combination in list order, with the index equal to
    /// the key's position.
    Combination,
}

/// 一次注册请求。 / One registration request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IndexedRegistrationRequest {
    /// 变量名前缀；注册出的变量名是 `<prefix>:<key>`。 / Variable-name prefix; a registered name is `<prefix>:<key>`.
    pub prefix: String,
    /// 注册策略。 / The registration policy.
    pub mode: IndexedRegistrationMode,
    /// 变量的整数性。 / The variable's integrality.
    pub kind: IndexedVariableKind,
    /// 待注册的键。 / The keys to register.
    pub keys: Vec<IndexedKey>,
    /// `appendable` 模式下的退役键；其它模式拒绝。 / Keys to retire under `appendable`; other modes refuse it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retired_keys: Vec<String>,
}

/// 一个键的注册结果。 / The registration result of one key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IndexedKeyIndex {
    /// 领域键。 / The domain key.
    pub key: String,
    /// 模型中的列索引。 / Column index in the model.
    pub index: u32,
}

/// 注册结果：注册后的模型 + 键映射 + 退役键。 / Registration result: the resulting model, the key mapping and retired keys.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IndexedRegistrationReport {
    /// 注册后的模型工件；调用方可直接用它求解。 / The resulting model artifact; callers may solve with it directly.
    pub model: ModelArtifact,
    /// 键 → 模型索引，按键稳定排序。 / Key → model index, stably sorted by key.
    pub indices: Vec<IndexedKeyIndex>,
    /// 已退役的键，按稳定排序；仅 `appendable` 可能非空。 / Retired keys, stably sorted; only `appendable` can be non-empty.
    pub retired_keys: Vec<String>,
    /// 注册后的变量总数。 / Total variable count after registration.
    pub variable_count: u32,
}

/// 校验请求：空前缀、重复键、非有限边界、跨模式字段混用都在这里拒绝。
/// Validate the request: blank prefixes, repeated keys, non-finite bounds and cross-mode field mixing are refused here.
fn validate(request: &IndexedRegistrationRequest) -> Result<()> {
    if request.prefix.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput", "indexed registration prefix must not be blank"));
    }
    if request.keys.is_empty() {
        return Err(BridgeError::new("InvalidInput", "indexed registration requires at least one key"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for key in &request.keys {
        if key.key.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "indexed registration key must not be blank"));
        }
        if !seen.insert(key.key.as_str()) {
            return Err(BridgeError::new("InvalidInput", "indexed registration repeats a key"));
        }
        for bound in [key.lower_bound, key.upper_bound].into_iter().flatten() {
            if !bound.is_finite() {
                return Err(BridgeError::new("InvalidInput", "indexed registration bounds must be finite"));
            }
        }
        if let (Some(lower), Some(upper)) = (key.lower_bound, key.upper_bound)
            && lower > upper {
            return Err(BridgeError::new("InvalidInput", "indexed registration lower bound exceeds the upper bound"));
        }
    }
    if request.kind == IndexedVariableKind::Binary {
        // 二元变量的边界必须落在 0..=1 内，否则声明的范围与整数性矛盾。
        // Binary bounds must lie inside 0..=1; otherwise the declared range contradicts the integrality.
        for key in &request.keys {
            for bound in [key.lower_bound, key.upper_bound].into_iter().flatten() {
                if !(0.0..=1.0).contains(&bound) {
                    return Err(BridgeError::new("InvalidInput", "binary indexed registration bounds must lie in [0, 1]"));
                }
            }
        }
    }
    match request.mode {
        IndexedRegistrationMode::Appendable => {
            let mut retired = std::collections::BTreeSet::new();
            for key in &request.retired_keys {
                if !seen.contains(key.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "retired key is not part of the registration"));
                }
                if !retired.insert(key.as_str()) {
                    return Err(BridgeError::new("InvalidInput", "indexed registration repeats a retired key"));
                }
            }
        }
        _ => {
            if !request.retired_keys.is_empty() {
                return Err(BridgeError::new("InvalidInput",
                    "only appendable registration accepts retired keys"));
            }
        }
    }
    Ok(())
}

/// 执行一次真实的键注册，返回注册后的模型与键映射。
/// Perform one real key registration and return the resulting model together with the key mapping.
pub fn register_indexed_keys(request: &IndexedRegistrationRequest) -> Result<IndexedRegistrationReport> {
    validate(request)?;
    let mut model = MetaModel::<f64>::new(&request.prefix);
    let retired: std::collections::BTreeSet<&str> = request.retired_keys.iter().map(|key| key.as_str()).collect();
    let mut indices = BTreeMap::new();
    for key in &request.keys {
        // 退役键仍会注册进模型（`appendable_pool` 的语义就是"保留但不活跃"），只是不进入映射。
        // Retired keys are still registered into the model (that is exactly `appendable_pool`'s "kept but inactive"
        // semantics); they simply do not appear in the mapping.
        let name = format!("{}:{}", request.prefix, key.key);
        let range = VariableRange::new(key.lower_bound, key.upper_bound);
        let registered = match request.kind {
            IndexedVariableKind::Continuous => {
                model.add_variable(ContinuousVariableItem::auto_with_range(&name, range))
                    .map_err(|error| BridgeError::new("InvalidInput", &error.to_string()))?
            }
            IndexedVariableKind::Integer => {
                model.add_variable(ospf_rust_core::variable::IntegerVariableItem::auto_with_range(&name, range))
                    .map_err(|error| BridgeError::new("InvalidInput", &error.to_string()))?
            }
            IndexedVariableKind::Binary => {
                model.add_variable(ospf_rust_core::variable::BinaryVariableItem::auto(&name))
                    .map_err(|error| BridgeError::new("InvalidInput", &error.to_string()))?
            }
        };
        if !retired.contains(key.key.as_str()) {
            indices.insert(key.key.clone(), registered);
        }
    }
    let linear = model.try_to_linear_triad_model()
        .map_err(|error| BridgeError::new("InvalidInput", &error.to_string()))?;
    let artifact = crate::encode_model(&linear)?;
    Ok(IndexedRegistrationReport {
        model: artifact,
        indices: indices.into_iter()
            .map(|(key, index)| Ok(IndexedKeyIndex { key,
                index: u32::try_from(index).map_err(|_| BridgeError::new("InvalidInput", "model index overflow"))? }))
            .collect::<Result<Vec<_>>>()?,
        retired_keys: retired.iter().map(|key| (*key).to_owned()).collect(),
        variable_count: u32::try_from(linear.num_variables())
            .map_err(|_| BridgeError::new("InvalidInput", "variable count overflow"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(mode: IndexedRegistrationMode, kind: IndexedVariableKind) -> IndexedRegistrationRequest {
        IndexedRegistrationRequest { prefix: "x".into(), mode, kind,
            keys: vec![IndexedKey { key: "a".into(), lower_bound: Some(0.0), upper_bound: Some(4.0) },
                IndexedKey { key: "b".into(), lower_bound: Some(0.0), upper_bound: Some(4.0) }],
            retired_keys: Vec::new() }
    }

    #[test]
    fn registration_returns_a_solvable_model_and_a_stable_key_map() {
        for (mode, kind) in [(IndexedRegistrationMode::Optional, IndexedVariableKind::Continuous),
            (IndexedRegistrationMode::Appendable, IndexedVariableKind::Integer),
            (IndexedRegistrationMode::Combination, IndexedVariableKind::Continuous)] {
            let report = register_indexed_keys(&request(mode, kind)).unwrap();
            assert_eq!(report.indices.len(), 2, "{mode:?}");
            assert_eq!(report.variable_count, 2, "{mode:?}");
            // 映射按键稳定排序，索引与注册顺序一致。 / The mapping is sorted by key and indices follow registration order.
            assert_eq!(report.indices[0].key, "a");
            assert_eq!(report.indices[0].index, 0);
            assert_eq!(report.indices[1].key, "b");
            assert_eq!(report.indices[1].index, 1);
            assert!(report.retired_keys.is_empty());
            // 产出的模型必须能往返编解码。 / The produced model must round-trip through the codec.
            assert_eq!(crate::decode_model(&report.model).unwrap().num_variables(), 2);
        }
    }

    #[test]
    fn binary_registration_accepts_zero_one_bounds() {
        let request = IndexedRegistrationRequest { keys: vec![IndexedKey { key: "a".into(), lower_bound: Some(0.0),
            upper_bound: Some(1.0) }], ..request(IndexedRegistrationMode::Optional, IndexedVariableKind::Binary) };
        let report = register_indexed_keys(&request).unwrap();
        assert_eq!(report.variable_count, 1);
        assert_eq!(report.indices[0].index, 0);
    }

    #[test]
    fn retired_keys_are_kept_in_the_model_but_absent_from_the_map() {
        let mut request = request(IndexedRegistrationMode::Appendable, IndexedVariableKind::Continuous);
        request.retired_keys = vec!["b".into()];
        let report = register_indexed_keys(&request).unwrap();
        // 退役键仍占一列，但不给出索引。 / The retired key still occupies a column but gets no index.
        assert_eq!(report.variable_count, 2);
        assert_eq!(report.indices.len(), 1);
        assert_eq!(report.indices[0].key, "a");
        assert_eq!(report.retired_keys, vec!["b".to_owned()]);
    }

    #[test]
    fn bad_requests_are_refused_before_registration() {
        let broken: Vec<(&str, IndexedRegistrationRequest)> = vec![
            ("blank prefix", IndexedRegistrationRequest { prefix: "  ".into(), ..request(IndexedRegistrationMode::Optional, IndexedVariableKind::Continuous) }),
            ("no keys", IndexedRegistrationRequest { keys: Vec::new(), ..request(IndexedRegistrationMode::Optional, IndexedVariableKind::Continuous) }),
            ("blank key", IndexedRegistrationRequest { keys: vec![IndexedKey { key: " ".into(), lower_bound: None, upper_bound: None }],
                ..request(IndexedRegistrationMode::Optional, IndexedVariableKind::Continuous) }),
            ("repeated key", IndexedRegistrationRequest { keys: vec![
                IndexedKey { key: "a".into(), lower_bound: None, upper_bound: None },
                IndexedKey { key: "a".into(), lower_bound: None, upper_bound: None }],
                ..request(IndexedRegistrationMode::Optional, IndexedVariableKind::Continuous) }),
            ("inverted bounds", IndexedRegistrationRequest { keys: vec![IndexedKey { key: "a".into(), lower_bound: Some(4.0),
                upper_bound: Some(0.0) }], ..request(IndexedRegistrationMode::Optional, IndexedVariableKind::Continuous) }),
            ("binary out of range", IndexedRegistrationRequest { keys: vec![IndexedKey { key: "a".into(),
                lower_bound: Some(0.0), upper_bound: Some(4.0) }],
                ..request(IndexedRegistrationMode::Optional, IndexedVariableKind::Binary) }),
            ("retired under optional", IndexedRegistrationRequest { retired_keys: vec!["a".into()],
                ..request(IndexedRegistrationMode::Optional, IndexedVariableKind::Continuous) }),
            ("retired key not declared", IndexedRegistrationRequest { retired_keys: vec!["ghost".into()],
                ..request(IndexedRegistrationMode::Appendable, IndexedVariableKind::Continuous) }),
        ];
        for (label, case) in broken {
            assert!(register_indexed_keys(&case).is_err(), "{label} must be refused");
        }
    }
}
