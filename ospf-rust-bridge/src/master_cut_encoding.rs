//! 继续挖掘第 1 单：master cut 的编码分类与验证冲突子集（`logic_based_benders`）/ Continuation, delivery 1: the master-cut
//! encoding classification and the verified conflict subset from `logic_based_benders`.
//!
//! `logic_based_benders.rs` 里 17 个未引用类型中大部分是**证明器接口**（`MasterObjectiveEvaluator`、`*Factory`、
//! `LogicBasedBendersCutOracle` 等）——那些是闭包与工厂，属于真边界。剩下真正可数据化的只有两处，这一单全绑：
//!
//! * `MasterCutEncoding`：一条 master cut 的**表达式编码类型**。三型：
//!   `linear`（普通线性 cut）、`binaryNoGood`（完整或子集 binary no-good，带被排除的赋值）、
//!   `boundedIntegerNoGood`（**有界整数多行** no-good——同一族里每行一种 `BoundedIntegerNoGoodRow`：`lowerSide` /
//!   `upperSide` / `atMostOne` / `requireDifference`）。这一层最容易误解的是：多行 no-good **不是一条 cut**，而是
//!   一个 `familyId` 下的一组行，所以报告把 family 与行分开给；
//! * `VerifiedMasterConflict::new`：验证过的 master 冲突子集。**构造器自带校验**——成员集合为空、或证明引用空白，
//!   都直接拒绝。这是"我真的验证过这个冲突"与"我只是声称"的分界，值得让 TS 侧也能触发这条拒绝。
//!
//! 桥接只做两件事：把 DTO 映射成**领域值**（映射成立由编译器证明），再按**字段文档语义**做结构复核；领域规则一条不重写。
//! 报告的 `canonicalJson` 是**领域值自己序列化出来的**，所以 TS 拿到的标签是领域的标签，不是桥接猜的。
//!
//! Of the 17 unreferenced types there, most are **prover interfaces** (`MasterObjectiveEvaluator`, the `*Factory` types,
//! `LogicBasedBendersCutOracle`) — closures and factories, a true boundary. Only two things are genuinely datafiable and this
//! delivery binds both: `MasterCutEncoding`, the expression encoding of a master cut in three forms (`linear`, `binaryNoGood`
//! with its excluded assignment, and `boundedIntegerNoGood` — a **multi-row** no-good family whose each row is one
//! `BoundedIntegerNoGoodRow`, so the report keeps the family and the row apart since a multi-row no-good is **not one cut**),
//! and `VerifiedMasterConflict::new`, whose **constructor validates** and refuses an empty member set or a blank proof
//! reference — the line between "I verified this conflict" and "I merely claim it", worth letting TypeScript trigger. The
//! bridge only maps the DTO into the **domain value** (the mapping is proven by the compiler) and re-checks it against the
//! documented field semantics; no domain rule is re-implemented, and `canonicalJson` is the **domain value's own**
//! serialization, so the tags TypeScript sees are the domain's, not ones the bridge guessed.

use std::collections::{BTreeMap, BTreeSet};

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_framework::solver::logic_based_benders::{
    BoundedIntegerNoGoodRow, MasterCutEncoding, VerifiedMasterConflict,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个稳定 ID → 整数值的赋值。 / One stable-ID to integer-value assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterAssignmentEntry {
    /// 稳定变量 ID（不能空白）。 / The stable variable ID (must not be blank).
    pub id: String,
    /// 被排除的取值。 / The excluded value.
    pub value: i64,
}

/// bounded-integer no-good 的行。 / A bounded-integer no-good row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterNoGoodRowSpec {
    /// 行类型：`lowerSide` / `upperSide` / `atMostOne` / `requireDifference`。 / The row kind.
    pub kind: String,
    /// 行引用的稳定变量 ID（`requireDifference` 不需要）。 / The row's stable variable ID (`requireDifference` needs none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<String>,
}

/// 一条 master cut 的编码与（可选）验证冲突的请求。 / A master-cut encoding plus an optional verified conflict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterCutEncodingRequest {
    /// 编码类型：`linear` / `binaryNoGood` / `boundedIntegerNoGood`。 / The encoding kind.
    pub kind: String,
    /// 被排除的赋值。 / The excluded assignment.
    #[serde(default)]
    pub assignment: Vec<MasterAssignmentEntry>,
    /// 多行族的稳定身份（仅 `boundedIntegerNoGood` 需要）。 / The family identity (multi-row only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub family_id: Option<String>,
    /// 当前行（仅 `boundedIntegerNoGood` 需要）。 / The row (multi-row only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub row: Option<MasterNoGoodRowSpec>,
    /// 验证冲突：成员稳定 ID 列表。 / The verified conflict's member IDs.
    #[serde(default)]
    pub conflict_members: Vec<String>,
    /// 验证冲突的证明引用。 / The verified conflict's proof reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub conflict_proof_reference: Option<String>,
}

/// 编码分类的结论。 / The encoding-classification verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterCutEncodingReport {
    /// 编码类型（线格式代码）。 / The encoding kind (wire code).
    pub kind: String,
    /// 被排除赋值的条数。 / The number of excluded assignments.
    pub assignment_count: u32,
    /// 被排除的赋值，按稳定 ID 升序。 / The excluded assignments, sorted by stable ID.
    pub assignment: Vec<MasterAssignmentEntry>,
    /// 多行族的稳定身份。 / The multi-row family identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub family_id: Option<String>,
    /// 行类型（线格式代码）。 / The row kind (wire code).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub row_kind: Option<String>,
    /// 行引用的稳定变量 ID。 / The row's stable variable ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub row_id: Option<String>,
    /// 是否是多行族里的一行（`kind == "boundedIntegerNoGood"`）。
    /// Whether this is one row of a multi-row family.
    pub multi_row: bool,
    /// 行引用的变量是否确实在被排除的赋值里（多行族才检查）。
    /// Whether the row's variable is actually among the excluded assignments (multi-row only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub row_variable_is_assigned: Option<bool>,
    /// 验证冲突的成员数（未给冲突时为 `undefined`）。 / The verified conflict's member count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub conflict_member_count: Option<u32>,
    /// 验证冲突的证明引用。 / The verified conflict's proof reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub conflict_proof_reference: Option<String>,
    /// 编码词表（桥接定义）。 / The encoding vocabulary (bridge-defined).
    pub kind_vocabulary: Vec<String>,
    /// 行类型词表（桥接定义）。 / The row vocabulary (bridge-defined).
    pub row_vocabulary: Vec<String>,
    /// **领域值自己序列化**出来的编码（标签是领域标签）。 / The encoding **serialized by the domain value itself**.
    pub canonical_json: String,
}

/// 校验一条 master cut 编码与（可选）验证冲突。非法输入是**错误**，原因原样带出。
/// Validate a master-cut encoding and an optional verified conflict.
pub fn verify_master_cut_encoding(request: &MasterCutEncodingRequest) -> Result<MasterCutEncodingReport> {
    let mut assignment = BTreeMap::new();
    for entry in &request.assignment {
        if entry.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "master cut encoding failed validation: assignment IDs must not be blank"));
        }
        // 同一 ID 出现两次说明稀疏映射写错了，拒绝而不是后者覆盖前者。
        // A repeated ID means the sparse map is malformed; it is refused rather than letting the later value win.
        if assignment.insert(StableVariableId(entry.id.clone()), entry.value).is_some() {
            return Err(BridgeError::new("InvalidInput",
                "master cut encoding failed validation: the assignment repeats a stable ID"));
        }
    }
    // 先把投影取出来，再让 `assignment` 被移动进编码值（顺序颠倒会构成"借用中移动"）。
    // Take the projection first so `assignment` can then be moved into the encoding value (the other order moves while
    // borrowed).
    let assignment_pairs = assignment.iter()
        .map(|(id, value)| MasterAssignmentEntry { id: id.0.clone(), value: *value })
        .collect::<Vec<_>>();
    let encoding = match request.kind.as_str() {
        "linear" => {
            // 线性 cut 没有赋值，也不该带族或行；带了就是调用方搞混了两种编码。
            // A linear cut carries no assignment, family or row; carrying one means the caller mixed up two encodings.
            if !assignment.is_empty() {
                return Err(BridgeError::new("InvalidInput",
                    "master cut encoding failed validation: a linear cut carries no excluded assignment"));
            }
            if request.family_id.is_some() || request.row.is_some() {
                return Err(BridgeError::new("InvalidInput",
                    "master cut encoding failed validation: a linear cut carries no no-good family"));
            }
            MasterCutEncoding::Linear
        }
        "binaryNoGood" => {
            if assignment.is_empty() {
                return Err(BridgeError::new("InvalidInput",
                    "master cut encoding failed validation: a binary no-good needs an excluded assignment"));
            }
            if request.family_id.is_some() || request.row.is_some() {
                return Err(BridgeError::new("InvalidInput",
                    "master cut encoding failed validation: a binary no-good carries no multi-row family"));
            }
            MasterCutEncoding::BinaryNoGood { assignment }
        }
        "boundedIntegerNoGood" => {
            if assignment.is_empty() {
                return Err(BridgeError::new("InvalidInput",
                    "master cut encoding failed validation: a multi-row no-good needs an excluded assignment"));
            }
            let family_id = request.family_id.as_deref().ok_or_else(|| BridgeError::new(
                "InvalidInput", "master cut encoding failed validation: a multi-row no-good needs a family ID"))?;
            if family_id.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput",
                    "master cut encoding failed validation: the family ID must not be blank"));
            }
            let row_spec = request.row.as_ref().ok_or_else(|| BridgeError::new(
                "InvalidInput", "master cut encoding failed validation: a multi-row no-good needs a row"))?;
            let row = match row_spec.kind.as_str() {
                "requireDifference" => BoundedIntegerNoGoodRow::RequireDifference,
                other => {
                    let id = row_spec.id.as_deref().ok_or_else(|| BridgeError::new("InvalidInput",
                        &format!("master cut encoding failed validation: row `{other}` needs a stable ID")))?;
                    if id.trim().is_empty() {
                        return Err(BridgeError::new("InvalidInput",
                            "master cut encoding failed validation: the row ID must not be blank"));
                    }
                    let id = StableVariableId(id.to_owned());
                    match other {
                        "lowerSide" => BoundedIntegerNoGoodRow::LowerSide(id),
                        "upperSide" => BoundedIntegerNoGoodRow::UpperSide(id),
                        "atMostOne" => BoundedIntegerNoGoodRow::AtMostOne(id),
                        unknown => return Err(BridgeError::new("InvalidInput", &format!(
                            "master cut encoding failed validation: unknown row kind `{unknown}`"))),
                    }
                }
            };
            MasterCutEncoding::BoundedIntegerNoGood {
                assignment, family_id: family_id.to_owned(), row }
        }
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "master cut encoding failed validation: unknown encoding kind `{other}`"))),
    };
    // 冲突：只要给了任一字段就走领域构造器，让**领域**决定合法与否。
    // The conflict goes through the domain constructor as soon as either field appears, so the **domain** decides.
    let conflict = if request.conflict_members.is_empty() && request.conflict_proof_reference.is_none() {
        None
    } else {
        let members = request.conflict_members.iter()
            .map(|id| StableVariableId(id.clone())).collect::<BTreeSet<_>>();
        Some(VerifiedMasterConflict::new(members,
            request.conflict_proof_reference.clone().unwrap_or_default())
            .map_err(|error| BridgeError::new("InvalidInput", &format!(
                "master cut encoding failed validation: {error}")))?)
    };
    let (row_kind, row_id, row_variable_is_assigned) = match &encoding {
        MasterCutEncoding::BoundedIntegerNoGood { assignment, row, .. } => {
            let (kind, id) = match row {
                BoundedIntegerNoGoodRow::LowerSide(id) => ("lowerSide", Some(id.0.clone())),
                BoundedIntegerNoGoodRow::UpperSide(id) => ("upperSide", Some(id.0.clone())),
                BoundedIntegerNoGoodRow::AtMostOne(id) => ("atMostOne", Some(id.0.clone())),
                BoundedIntegerNoGoodRow::RequireDifference => ("requireDifference", None),
            };
            let assigned = id.as_ref().map(|id| assignment.contains_key(&StableVariableId(id.clone())));
            (Some(kind.to_owned()), id, assigned)
        }
        _ => (None, None, None),
    };
    let (family_id, multi_row) = match &encoding {
        MasterCutEncoding::BoundedIntegerNoGood { family_id, .. } => (Some(family_id.clone()), true),
        _ => (None, false),
    };
    let canonical_json = serde_json::to_string(&encoding).map_err(|error| BridgeError::new(
        "CodecFailure", &format!("master cut encoding could not be encoded: {error}")))?;
    Ok(MasterCutEncodingReport {
        kind: request.kind.clone(),
        assignment_count: u32::try_from(assignment_pairs.len()).unwrap_or(u32::MAX),
        assignment: assignment_pairs,
        family_id,
        row_kind,
        row_id,
        multi_row,
        row_variable_is_assigned,
        conflict_member_count: conflict.as_ref()
            .map(|conflict| u32::try_from(conflict.variable_ids.len()).unwrap_or(u32::MAX)),
        conflict_proof_reference: conflict.as_ref().map(|conflict| conflict.proof_reference.clone()),
        kind_vocabulary: vec!["linear".to_owned(), "binaryNoGood".to_owned(),
            "boundedIntegerNoGood".to_owned()],
        row_vocabulary: vec!["lowerSide".to_owned(), "upperSide".to_owned(), "atMostOne".to_owned(),
            "requireDifference".to_owned()],
        canonical_json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(kind: &str) -> MasterCutEncodingRequest {
        MasterCutEncodingRequest { kind: kind.to_owned(), assignment: Vec::new(), family_id: None,
            row: None, conflict_members: Vec::new(), conflict_proof_reference: None }
    }

    fn assignment(entries: &[(&str, i64)]) -> Vec<MasterAssignmentEntry> {
        entries.iter().map(|(id, value)| MasterAssignmentEntry {
            id: (*id).to_owned(), value: *value }).collect()
    }

    #[test]
    fn every_encoding_kind_is_measured() {
        // 线性：无赋值、无族。 / Linear: no assignment and no family.
        let report = verify_master_cut_encoding(&request("linear")).unwrap();
        assert_eq!(report.kind, "linear");
        assert_eq!(report.assignment_count, 0);
        assert!(!report.multi_row);
        assert_eq!(report.family_id, None);
        assert_eq!(report.row_kind, None);
        // 领域的序列化标签原样交出。 / The domain's own serialization tag is handed back verbatim.
        assert_eq!(report.canonical_json, "\"Linear\"");
        assert_eq!(report.kind_vocabulary.len(), 3);
        assert_eq!(report.row_vocabulary, vec!["lowerSide".to_owned(), "upperSide".to_owned(),
            "atMostOne".to_owned(), "requireDifference".to_owned()]);

        // binary no-good：赋值按稳定 ID 升序投影。 / A binary no-good: the assignment is projected sorted.
        let mut document = request("binaryNoGood");
        document.assignment = assignment(&[("z", 1), ("a", 0)]);
        let report = verify_master_cut_encoding(&document).unwrap();
        assert_eq!(report.assignment_count, 2);
        assert_eq!(report.assignment[0].id, "a");
        assert_eq!(report.assignment[1].id, "z");
        assert!(report.canonical_json.starts_with("{\"BinaryNoGood\""), "{}", report.canonical_json);
        assert!(report.canonical_json.contains("\"a\":0"), "{}", report.canonical_json);

        // 多行 no-good：族与行分开给出，行变量必须在赋值里。 / Multi-row: family and row split; the row ID is assigned.
        let mut multi = request("boundedIntegerNoGood");
        multi.assignment = assignment(&[("side:x", 1), ("other", 0)]);
        multi.family_id = Some("family-1".to_owned());
        multi.row = Some(MasterNoGoodRowSpec { kind: "lowerSide".to_owned(), id: Some("side:x".to_owned()) });
        let report = verify_master_cut_encoding(&multi).unwrap();
        assert!(report.multi_row);
        assert_eq!(report.family_id.as_deref(), Some("family-1"));
        assert_eq!(report.row_kind.as_deref(), Some("lowerSide"));
        assert_eq!(report.row_id.as_deref(), Some("side:x"));
        assert_eq!(report.row_variable_is_assigned, Some(true));
        assert!(report.canonical_json.starts_with("{\"BoundedIntegerNoGood\""), "{}", report.canonical_json);
        // 行变量不在赋值里时如实报 false（不是拒绝——那是调用方的语义选择）。
        // A row variable outside the assignment reports false rather than refusing: that is the caller's semantics.
        multi.row = Some(MasterNoGoodRowSpec { kind: "upperSide".to_owned(), id: Some("absent".to_owned()) });
        assert_eq!(verify_master_cut_encoding(&multi).unwrap().row_variable_is_assigned, Some(false));
        // `requireDifference` 不带 ID。 / `requireDifference` carries no ID.
        multi.row = Some(MasterNoGoodRowSpec { kind: "requireDifference".to_owned(), id: None });
        let report = verify_master_cut_encoding(&multi).unwrap();
        assert_eq!(report.row_kind.as_deref(), Some("requireDifference"));
        assert_eq!(report.row_id, None);
        assert_eq!(report.row_variable_is_assigned, None);
        // 四种行类型都能命名。 / All four row kinds name themselves.
        for kind in ["lowerSide", "upperSide", "atMostOne"] {
            let mut row = request("boundedIntegerNoGood");
            row.assignment = assignment(&[("v", 1)]);
            row.family_id = Some("f".to_owned());
            row.row = Some(MasterNoGoodRowSpec { kind: kind.to_owned(), id: Some("v".to_owned()) });
            assert_eq!(verify_master_cut_encoding(&row).unwrap().row_kind.as_deref(), Some(kind));
        }
    }

    #[test]
    fn verified_conflict_is_rejected_by_the_domain_constructor() {
        // 合法：成员非空 + 证明引用非空白。 / Legal: non-empty members and a non-blank proof reference.
        let mut document = request("binaryNoGood");
        document.assignment = assignment(&[("a", 1)]);
        document.conflict_members = vec!["b".to_owned(), "a".to_owned()];
        document.conflict_proof_reference = Some("proof-1".to_owned());
        let report = verify_master_cut_encoding(&document).unwrap();
        assert_eq!(report.conflict_member_count, Some(2));
        assert_eq!(report.conflict_proof_reference.as_deref(), Some("proof-1"));
        // 没给冲突时不报冲突字段。 / No conflict means no conflict fields.
        let report = verify_master_cut_encoding(&{ let mut bare = request("linear"); bare }).unwrap();
        assert_eq!(report.conflict_member_count, None);
        // 成员为空：领域构造器拒绝。 / Empty members: the domain constructor refuses.
        let mut empty = document.clone();
        empty.conflict_members = Vec::new();
        assert!(verify_master_cut_encoding(&empty).unwrap_err().message
            .contains("requires members"), "{}", verify_master_cut_encoding(&empty).unwrap_err().message);
        // 证明引用空白 / 缺失：同样拒绝。 / A blank or missing proof reference is refused too.
        let mut blank = document.clone();
        blank.conflict_proof_reference = Some("  ".to_owned());
        assert!(verify_master_cut_encoding(&blank).is_err());
        let mut absent = document;
        absent.conflict_proof_reference = None;
        assert!(verify_master_cut_encoding(&absent).is_err());
    }

    #[test]
    fn mixed_up_kinds_are_refused() {
        // 线性带赋值 / 带族 / 带行：全是把两种编码搞混。 / A linear cut carrying an assignment, family or row mixes encodings.
        let mut linear = request("linear");
        linear.assignment = assignment(&[("a", 1)]);
        assert!(verify_master_cut_encoding(&linear).unwrap_err().message.contains("carries no excluded"));
        let mut with_family = request("linear");
        with_family.family_id = Some("f".to_owned());
        assert!(verify_master_cut_encoding(&with_family).unwrap_err().message.contains("no no-good family"));
        // binary no-good 没有赋值、带族：拒绝。 / A binary no-good without assignment or with a family: refused.
        assert!(verify_master_cut_encoding(&request("binaryNoGood")).unwrap_err().message
            .contains("needs an excluded assignment"));
        let mut binary_family = request("binaryNoGood");
        binary_family.assignment = assignment(&[("a", 1)]);
        binary_family.family_id = Some("f".to_owned());
        assert!(verify_master_cut_encoding(&binary_family).unwrap_err().message.contains("no multi-row family"));
        // 多行缺族 / 缺行 / 族空白 / 行 ID 缺失。 / Multi-row without family, without row, blank family, missing row ID.
        let mut multi = request("boundedIntegerNoGood");
        multi.assignment = assignment(&[("a", 1)]);
        assert!(verify_master_cut_encoding(&multi).unwrap_err().message.contains("needs a family ID"));
        multi.family_id = Some("f".to_owned());
        assert!(verify_master_cut_encoding(&multi).unwrap_err().message.contains("needs a row"));
        multi.row = Some(MasterNoGoodRowSpec { kind: "lowerSide".to_owned(), id: None });
        assert!(verify_master_cut_encoding(&multi).unwrap_err().message.contains("needs a stable ID"));
        multi.row = Some(MasterNoGoodRowSpec { kind: "lowerSide".to_owned(), id: Some(" ".to_owned()) });
        assert!(verify_master_cut_encoding(&multi).unwrap_err().message.contains("row ID must not be blank"));
        multi.row = Some(MasterNoGoodRowSpec { kind: "middleSide".to_owned(), id: Some("a".to_owned()) });
        assert!(verify_master_cut_encoding(&multi).unwrap_err().message.contains("unknown row kind"));
        // 空白赋值 ID / 重复赋值 ID / 未知编码。 / A blank ID, a repeated ID and an unknown encoding.
        let mut blank_id = request("binaryNoGood");
        blank_id.assignment = assignment(&[(" ", 1)]);
        assert!(verify_master_cut_encoding(&blank_id).unwrap_err().message.contains("must not be blank"));
        let mut repeated = request("binaryNoGood");
        repeated.assignment = assignment(&[("a", 1), ("a", 0)]);
        assert!(verify_master_cut_encoding(&repeated).unwrap_err().message.contains("repeats a stable ID"));
        assert!(verify_master_cut_encoding(&request("quadratic")).unwrap_err().message
            .contains("unknown encoding kind"));
    }
}
