//! 批次 D 第 1 单：线性模型的稳定 solver 映射查询 / Batch D, delivery 1: the stable solver mapping of a linear model.
//!
//! 求解器按**列号 / 行号**索引变量与约束，报告里的证据也用下标指路。TS 侧拿到列号却叫不出名字，等于拿到了地址却没有
//! 街道名录。这一单把 `linear_model_mapping` 直接暴露成一个查询：先用 `decode_model` 把 `ModelArtifact` 按指纹解回
//! `LinearTriadModel`，再校验模型结构（各向量长度一致、边界有限、稀疏下标不越界、系数有限），然后按**确定性规则**
//! 生成稳定 ID——用的是**长度前缀分帧**（实测形如 `linear-variable:5#probe:1#0:1#x`，即
//! `类别:前缀长度#分段:长度#分段`），变量带模型名/列号/变量名，约束带模型名/行号/约束名/来源符号，
//! 目标带模型名与目标类别——最后用
//! `LinearModelMapping::validate` 证明正反两向**双射**。
//!
//! 关键点：稳定 ID 里**带列号/行号**，所以它不是"名字"，而是"位置的身份"。同一个变量一旦换了位置 ID 就变了；
//! 想让 ID 在重排后不变，得靠模型名与变量名保持稳定。SDK 文档里明确写了这一点，避免被当成可重排的键。
//!
//! Solvers index variables and constraints by **column and row**, and reports point at evidence by index. A column number
//! without a directory is an address without street names. This delivery exposes `linear_model_mapping` as a query: decode
//! the `ModelArtifact` back into a `LinearTriadModel` through its fingerprint, validate the structure (matching vector
//! lengths, finite bounds, in-range sparse indices, finite coefficients), derive stable IDs by **deterministic rules** that
//! use **length-prefixed framing** (measured shape: `linear-variable:5#probe:1#0:1#x`, i.e. `kind:len#part:len#part`), and
//! prove both directions are a **bijection**. Note the IDs **embed the position**: an ID is the identity of a slot,
//! not a name, so reordering changes it. Only stable model and variable names keep it, which the SDK docs state plainly.

use ospf_rust_core::solver::audit::LinearModelMapping;
use ospf_rust_core::solver::linear_model_mapping;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{decode_model, BridgeError, ModelArtifact, Result};

/// 映射查询请求。模型来自 `SolveRequest.model`，这里只放可选的"声称的映射"。
/// The mapping request. The model comes from `SolveRequest.model`; only an optional claimed mapping lives here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearMappingRequest {
    /// 可选：调用方声称的映射（JSON 文本），校验后必须与重算结果一致。
    /// Optional: a caller-claimed mapping (JSON text) that must match the recomputation after validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub claimed_mapping: Option<String>,
}

/// 稳定映射结论。 / The stable mapping verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearMappingReport {
    /// 稳定目标 ID。 / The stable objective ID.
    pub objective_id: String,
    /// 变量数（等于列数）。 / The variable count (equal to the column count).
    pub variable_count: u32,
    /// 约束数（等于行数）。 / The constraint count (equal to the row count).
    pub constraint_count: u32,
    /// 二次约束数（线性模型恒为零）。 / The quadratic constraint count (always zero for a linear model).
    pub quadratic_constraint_count: u32,
    /// 按列顺序排列的稳定变量 ID。 / Stable variable IDs in column order.
    pub variables_by_column: Vec<String>,
    /// 按行顺序排列的稳定约束 ID。 / Stable constraint IDs in row order.
    pub constraints_by_row: Vec<String>,
    /// 按顺序排列的稳定二次约束 ID。 / Stable quadratic constraint IDs in order.
    pub quadratic_constraints_by_row: Vec<String>,
    /// 重算映射的规范文本（可作为"声称的映射"再提交，必须逐字一致）。
    /// The canonical text of the recomputed mapping (re-submittable as a claimed mapping and must match verbatim).
    pub canonical_json: String,
}

/// 解码模型、构建并校验稳定映射；若给出"声称的映射"，还必须与重算结果一致。
/// Decode the model, build and validate the stable mapping; a claimed mapping must match the recomputation.
pub fn verify_linear_mapping(artifact: &ModelArtifact, request: &LinearMappingRequest) -> Result<LinearMappingReport> {
    // 指纹先过关：`decode_model` 拒绝与指纹不符的模型文本，避免在错误模型上给出"合法"映射。
    // The fingerprint gates everything: `decode_model` refuses text that disagrees with its fingerprint, so a "legal"
    // mapping is never produced for the wrong model.
    let model = decode_model(artifact).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("linear model could not be decoded: {error}"))
    })?;
    // 领域入口：校验模型结构 → 生成稳定 ID → 证明双射。桥接不重新实现任何一步。
    // The domain entry: validate the structure, derive the IDs, prove the bijection. The bridge re-implements none of it.
    let mapping = linear_model_mapping(&model).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("linear model mapping failed validation: {error}"))
    })?;
    if let Some(claimed) = request.claimed_mapping.as_deref() {
        if claimed.len() > 32 * 1024 * 1024 {
            return Err(BridgeError::new("InvalidInput", "the claimed mapping exceeds 32 MiB"));
        }
        let claimed: LinearModelMapping = serde_json::from_str(claimed).map_err(|error| {
            BridgeError::new("InvalidInput", &format!("claimed mapping is not decodable: {error}"))
        })?;
        // 先让声称的映射自己自洽（双射、维度一致、目标 ID 非空），再和重算结果比。
        // First let the claimed mapping prove itself consistent, then compare it with the recomputation.
        claimed.validate().map_err(|error| {
            BridgeError::new("InvalidInput", &format!("claimed mapping failed validation: {error}"))
        })?;
        if claimed.objective_id != mapping.objective_id {
            return Err(BridgeError::new("InvalidInput",
                "claimed mapping objective disagrees with the recomputation"));
        }
        if claimed.variables_by_column != mapping.variables_by_column {
            return Err(BridgeError::new("InvalidInput",
                "claimed mapping variables disagree with the recomputation"));
        }
        if claimed.constraints_by_row != mapping.constraints_by_row {
            return Err(BridgeError::new("InvalidInput",
                "claimed mapping constraints disagree with the recomputation"));
        }
        if claimed.quadratic_constraints_by_row != mapping.quadratic_constraints_by_row {
            return Err(BridgeError::new("InvalidInput",
                "claimed mapping quadratic constraints disagree with the recomputation"));
        }
    }
    let canonical_json = serde_json::to_string(&mapping).map_err(|error| {
        BridgeError::new("CodecFailure", &format!("mapping could not be encoded: {error}"))
    })?;
    Ok(LinearMappingReport {
        objective_id: mapping.objective_id.clone(),
        variable_count: count(mapping.variables_by_column.len()),
        constraint_count: count(mapping.constraints_by_row.len()),
        quadratic_constraint_count: count(mapping.quadratic_constraints_by_row.len()),
        variables_by_column: mapping.variables_by_column.iter().map(|id| id.0.clone()).collect(),
        constraints_by_row: mapping.constraints_by_row.clone(),
        quadratic_constraints_by_row: mapping.quadratic_constraints_by_row.clone(),
        canonical_json,
    })
}

/// 计数字段的安全转换。 / A safe cast for count fields.
fn count(value: usize) -> u32 { u32::try_from(value).unwrap_or(u32::MAX) }

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::model::intermediate::LinearTriadModel;
    use crate::encode_model;
    use ospf_rust_core::model::{ConstraintRelation, Linear, LinearInequality, LinearMonomial, MetaModel};
    use ospf_rust_core::variable::{ContinuousVariableItem, VariableRange};

    /// 两个变量一约束的模型，走真实的 `MetaModel` 构造链（不用手写内部结构）。
    /// A two-variable, one-constraint model built through the real `MetaModel` chain (no hand-written internals).
    fn model(name: &str) -> LinearTriadModel {
        let mut source = MetaModel::<f64>::new(name);
        let left = source.add_variable(ContinuousVariableItem::auto_with_range(
            "x", VariableRange::bounded(0.0, 1.0))).unwrap();
        let right = source.add_variable(ContinuousVariableItem::auto_with_range(
            "y", VariableRange::bounded(0.0, 1.0))).unwrap();
        source.add_linear_constraint(&[(left, 1.0), (right, 1.0)], ConstraintRelation::LessEqual, 1.0, "budget").unwrap();
        source.try_to_linear_triad_model().unwrap()
    }

    fn request(claimed: Option<String>) -> LinearMappingRequest {
        LinearMappingRequest { claimed_mapping: claimed }
    }

    fn artifact(name: &str) -> ModelArtifact { encode_model(&model(name)).unwrap() }

    #[test]
    fn mapping_is_deterministic_and_bijective() {
        let report = verify_linear_mapping(&artifact("probe"), &request(None)).unwrap();
        assert_eq!(report.variable_count, 2);
        assert_eq!(report.constraint_count, 1);
        assert_eq!(report.quadratic_constraint_count, 0);
        // 稳定 ID 用的是长度前缀分帧，实测形如 `linear-variable:5#probe:1#0:1#x`。
        // Stable IDs use length-prefixed framing; the measured shape is `linear-variable:5#probe:1#0:1#x`.
        assert!(report.variables_by_column[0].starts_with("linear-variable:"),
            "{}", report.variables_by_column[0]);
        assert!(report.variables_by_column[0].contains("#probe"), "{}", report.variables_by_column[0]);
        assert!(report.variables_by_column[0].ends_with("#x"), "{}", report.variables_by_column[0]);
        assert!(report.variables_by_column[1].ends_with("#y"), "{}", report.variables_by_column[1]);
        assert!(report.constraints_by_row[0].starts_with("linear-constraint:"),
            "{}", report.constraints_by_row[0]);
        assert!(report.objective_id.starts_with("linear-objective:"), "{}", report.objective_id);
        assert_ne!(report.variables_by_column[0], report.variables_by_column[1]);

        // 规范文本再提交：一致。 / Re-submitting the canonical text: consistent.
        let again = verify_linear_mapping(&artifact("probe"), &request(Some(report.canonical_json.clone()))).unwrap();
        assert_eq!(again.canonical_json, report.canonical_json);
        // 同一模型重建：ID 完全一致（确定性）。 / Rebuilding the same model: byte-identical IDs.
        assert_eq!(again.variables_by_column, report.variables_by_column);

        // 换模型名：每个 ID 都变，声称的旧映射不再成立。
        // Changing the model name changes every ID, so the claimed old mapping no longer holds.
        // 实测：目标指纹的措辞是 `objective disagrees`，变量/约束是 `variables/constraints disagree`。
        // Measured: the objective fingerprint says `objective disagrees` while variables and constraints say `disagree`.
        let renamed = verify_linear_mapping(&artifact("other"), &request(Some(report.canonical_json.clone()))).unwrap_err();
        assert!(renamed.message.contains("objective disagrees with the recomputation"), "{}", renamed.message);
    }

    #[test]
    fn illegal_models_and_claims_are_refused() {
        // 指纹不符：`decode_model` 先拒绝，不会给出"合法"映射。
        // A fingerprint mismatch is refused by `decode_model` before any mapping is produced.
        let mut mismatch = artifact("probe");
        mismatch.fingerprint = "foreign".to_owned();
        let error = verify_linear_mapping(&mismatch, &request(None)).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("could not be decoded"), "{}", error.message);

        // 模型文本不是 JSON：同样在解码阶段拒绝。
        // Model text that is not JSON is refused at the decoding stage.
        let mut garbage = artifact("probe");
        garbage.model_json = "not json".to_owned();
        assert!(verify_linear_mapping(&garbage, &request(None)).unwrap_err().message.contains("could not be decoded"));

        // 声称的映射不是 JSON / 不是合法映射。 / A claimed mapping that is not JSON or not a legal mapping.
        assert_eq!(verify_linear_mapping(&artifact("probe"), &request(Some("not json".into()))).unwrap_err().code,
            "InvalidInput");
        assert_eq!(verify_linear_mapping(&artifact("probe"), &request(Some("{}".into()))).unwrap_err().code,
            "InvalidInput");

        let real = verify_linear_mapping(&artifact("probe"), &request(None)).unwrap();

        // 交换两个变量：正向列表变了但反向映射没变 → 领域自己先报"非双射"。
        // Swapping two variables changes the forward list but not the reverse map, so the domain reports a non-bijection.
        let mut swapped: serde_json::Value = serde_json::from_str(&real.canonical_json).unwrap();
        let first = swapped["variablesByColumn"][0].clone();
        swapped["variablesByColumn"][0] = swapped["variablesByColumn"][1].clone();
        swapped["variablesByColumn"][1] = first;
        let error = verify_linear_mapping(&artifact("probe"), &request(Some(swapped.to_string()))).unwrap_err();
        assert!(error.message.contains("not bijective"), "{}", error.message);

        // 目标 ID 为空：维度一致但目标为空，领域先拒绝。
        // An empty objective ID: dimensions agree but the objective is blank, so the domain refuses it first.
        let mut blank: serde_json::Value = serde_json::from_str(&real.canonical_json).unwrap();
        blank["objectiveId"] = serde_json::json!("");
        let error = verify_linear_mapping(&artifact("probe"), &request(Some(blank.to_string()))).unwrap_err();
        assert!(error.message.contains("dimensions are inconsistent"), "{}", error.message);

        // 只有反向映射丢了变量：维度不一致。 / A lost reverse entry: inconsistent dimensions.
        let mut missing_reverse: serde_json::Value = serde_json::from_str(&real.canonical_json).unwrap();
        missing_reverse["columnsByVariable"] = serde_json::json!({});
        let error = verify_linear_mapping(&artifact("probe"), &request(Some(missing_reverse.to_string()))).unwrap_err();
        assert!(error.message.contains("dimensions are inconsistent"), "{}", error.message);
    }
}
