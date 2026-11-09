//! 续挖第 15 单：原生写入指纹校验的两侧语义与"只可收紧"的域证明
//! （`model/intermediate/deferred_function.rs` 的 `verify_native_write` / `StructureUsageBinding` / 两个错误码）
//! Continuation, delivery 15: the two-sided semantics of the native-write fingerprint check and the "tighten only" domain proof.
//!
//! 这一单钉住三件互相咬合的东西：
//!
//! **1. `verify_native_write` 是**成对**判定，不是单侧判定。**
//! 只有"记录侧有指纹且结构侧也有指纹且两者相等"才算通过；四种组合里另外三种全**失败**——
//! 其中包括**只缺一侧**的两种。把"没指纹"当成通过会让指纹彻底失去意义，所以缺一侧就是不一致。
//!
//! **2. 两个默认方法都返回 `None`。**
//! 一个只实现必需方法的结构既不给使用语境绑定、也不给指纹；于是任何带指纹的原生写入记录对它都会
//! 校验失败。这是**保守的默认值**：新增结构不会"默认通过"校验。
//!
//! **3. 域证明只可收紧、不可扩大、不可丢失。**
//! `DOMAIN_PROOF_WIDENED` 与 `DOMAIN_PROOF_LOST` 两个常量就是这两条拒绝的文案前缀，而方向由
//! `is_contained_in` 表达：收紧的方向为真、扩大的方向为假。丢掉证明既不能声称 Big-M 来自域推断，
//! 也不能靠"没有证明"混过校验（回到第 1 条）。
//!
//! Three interlocking things. **The fingerprint check is pairwise, not one-sided**: only "recorded present, current
//! present, equal" passes, and the other three of four combinations fail — including the two that lack just one side,
//! because treating "no fingerprint" as a pass would make fingerprints meaningless. **Both default methods return `None`**:
//! a structure implementing only the required methods offers neither a usage binding nor a fingerprint, so any record with a
//! fingerprint fails against it — a **conservative default**, since a new structure never passes validation by accident.
//! And **the domain proof may be tightened but never widened or dropped**: the two constants are the message prefixes for
//! those refusals, while the direction is expressed by `is_contained_in` — the tightening direction holds and the widening
//! direction does not. Dropping the proof may neither claim a Big-M came from domain inference nor slip past validation.

use ospf_rust_core::model::intermediate::{
    verify_native_write, DeferredFunctionStructure, InputDomainProof, StructureUsageBinding,
    DOMAIN_PROOF_LOST, DOMAIN_PROOF_WIDENED,
};
use ospf_rust_core::model::LinearConstraint;
use ospf_rust_core::variable::VariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个只实现必需方法的结构：两个默认方法都返回 `None`。
/// A structure implementing only the required methods: both defaults return `None`.
#[derive(Debug)]
struct BareStructure {
    name: String,
    fingerprint: Option<String>,
    binding: Option<StructureUsageBinding>,
}

impl DeferredFunctionStructure<f64> for BareStructure {
    fn function_name(&self) -> &str { &self.name }

    fn materialize(
        &self,
        _symbol_to_index: &std::collections::HashMap<usize, usize>,
    ) -> ospf_rust_core::Result<Vec<LinearConstraint<f64>>> {
        // 这一单不看物化结果。 / Materialization is irrelevant here.
        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn usage_binding(&self) -> Option<StructureUsageBinding> { self.binding.clone() }

    fn fingerprint(&self) -> Option<String> { self.fingerprint.clone() }
}

/// 一次指纹校验。 / One fingerprint check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FingerprintCase {
    /// 原生写入记录里带的指纹（`None` 表示没带）。 / The fingerprint in the record (`None` = absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub recorded: Option<String>,
    /// 当前结构给出的指纹（`None` 表示不给）。 / The structure's fingerprint (`None` = none offered).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub current: Option<String>,
}

/// 一次绑定构造。 / One binding construction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BindingSpec {
    /// 来源符号 ID。 / The source symbol ID.
    pub source_symbol_id: u64,
    /// 结果列的组号。 / The result column's group id.
    pub result_group: usize,
    /// 结果列的组内序号。 / The result column's index within its group.
    pub result_index: usize,
    /// 辅助列（`[组号, 组内序号]`）。 / The helper columns as `[group, index]` pairs.
    #[serde(default)]
    pub helpers: Vec<Vec<usize>>,
}

/// 指纹与域证明请求。 / The fingerprint-and-proof request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeWriteRequest {
    /// 要校验的指纹组合。 / The fingerprint pairs to check.
    #[serde(default)]
    pub cases: Vec<FingerprintCase>,
    /// 要构造的绑定。 / The bindings to construct.
    #[serde(default)]
    pub bindings: Vec<BindingSpec>,
    /// 要判断"可否替换"的两对区间（第一个是否被第二个包含）。
    /// The interval pairs to test for containment (is the first inside the second).
    #[serde(default)]
    pub proofs: Vec<ProofPair>,
}

/// 一对区间。 / A pair of intervals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofPair {
    /// 提议的（更紧或更宽的）区间。 / The proposed interval (tighter or wider).
    pub proposed_lower: f64,
    /// 提议区间的上界。 / The proposed upper bound.
    pub proposed_upper: f64,
    /// 已在记录里的区间。 / The interval already recorded.
    pub recorded_lower: f64,
    /// 已记录区间的上界。 / The recorded upper bound.
    pub recorded_upper: f64,
}

/// 一次指纹校验的结论。 / One fingerprint check's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteFingerprintRow {
    /// 记录侧是否有指纹。 / Whether the record carries a fingerprint.
    pub has_recorded: bool,
    /// 结构侧是否给指纹。 / Whether the structure offers one.
    pub has_current: bool,
    /// 校验是否通过（**只有两侧都有且相等**）。 / Whether the check passes (**only when both exist and match**).
    pub accepted: bool,
    /// 不通过的原因（桥接定义的分类）。 / Why it failed (a bridge-defined classification).
    pub reason: String,
}

/// 一次绑定构造的结论。 / One binding construction's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BindingRow {
    /// 来源符号 ID。 / The source symbol ID.
    pub source_symbol_id: u64,
    /// 结果列是否出现在辅助列里（**构造器不拦**）。 / Whether the result column also appears in the helpers.
    pub result_also_helper: bool,
    /// 辅助列条数。 / The number of helper columns.
    pub helper_count: u32,
    /// 辅助列里的重复列数。 / The number of duplicated helper columns.
    pub duplicate_helpers: u32,
}

/// 一次域证明替换的结论。 / One proof replacement's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofRow {
    /// 提议是否被记录包含（收紧或持平 → 允许）。 / Whether the proposal is contained (tighten or equal → allowed).
    pub allowed: bool,
    /// 记录是否被提议包含（**扩大** → 这正是被拒的方向）。 / Whether the record is inside the proposal (**widening**).
    pub widened: bool,
    /// 两个区间是否相等。 / Whether the two intervals are equal.
    pub unchanged: bool,
    /// 拒绝时用的错误码前缀（`widened` 时为 `DOMAIN_PROOF_WIDENED`）。 / The refusal prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub refusal: Option<String>,
}

/// 指纹与域证明结论。 / The fingerprint-and-proof verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeWriteReport {
    /// 指纹校验结论。 / The fingerprint verdicts.
    pub cases: Vec<WriteFingerprintRow>,
    /// 绑定构造结论。 / The binding verdicts.
    pub bindings: Vec<BindingRow>,
    /// 域证明替换结论。 / The proof verdicts.
    pub proofs: Vec<ProofRow>,
    /// 两侧都缺指纹时**是否通过**（实测：不通过）。 / Whether both-missing passes (**no**).
    pub both_absent_passes: bool,
    /// 只缺一侧时**是否通过**（实测：不通过）。 / Whether one-missing passes (**no**).
    pub one_absent_passes: bool,
    /// 只实现必需方法的结构是否会给指纹（**否**）。 / Whether a bare structure offers a fingerprint (**no**).
    pub bare_structure_has_fingerprint: bool,
    /// 只实现必需方法的结构是否会给使用语境绑定（**否**）。 / Whether it offers a usage binding (**no**).
    pub bare_structure_has_binding: bool,
    /// `widened` 拒绝的文案前缀。 / The refusal prefix for a widened proof.
    pub widened_prefix: String,
    /// `lost` 拒绝的文案前缀。 / The refusal prefix for a lost proof.
    pub lost_prefix: String,
}

/// 条目上限（信任边界）。 / The entry cap (a trust boundary).
const MAX_ENTRIES: usize = 512;

/// 走一遍指纹成对判定、绑定构造与域证明方向。非法输入是**错误**。
/// Walk the pairwise fingerprint check, the binding construction and the proof direction.
pub fn verify_native_write_record(request: &NativeWriteRequest) -> Result<NativeWriteReport> {
    if request.cases.len() > MAX_ENTRIES || request.bindings.len() > MAX_ENTRIES
        || request.proofs.len() > MAX_ENTRIES
    {
        return Err(BridgeError::new("InvalidInput", &format!(
            "native write failed validation: at most {MAX_ENTRIES} entries are accepted")));
    }
    let mut cases = Vec::with_capacity(request.cases.len());
    for case in &request.cases {
        let structure = BareStructure { name: "case".to_owned(),
            fingerprint: case.current.clone(), binding: None };
        let accepted = verify_native_write(case.recorded.as_deref(), &structure);
        // 只有"两侧都有且相等"才允许通过；另外三种组合通过就说明领域行为变了。
        // Only both-present-and-equal may pass; any other combination passing means the domain changed.
        let reason = match (case.recorded.as_deref(), case.current.as_deref()) {
            (Some(recorded), Some(current)) if recorded == current => "match",
            (Some(_), Some(_)) => "mismatch",
            (Some(_), None) => "structureOffersNone",
            (None, Some(_)) => "recordLost",
            (None, None) => "bothAbsent",
        };
        if accepted != (reason == "match") {
            return Err(BridgeError::new("InvalidInput", &format!(
                "native write failed validation: `{reason}` disagrees with the domain verdict")));
        }
        cases.push(WriteFingerprintRow { has_recorded: case.recorded.is_some(),
            has_current: case.current.is_some(), accepted, reason: reason.to_owned() });
    }
    let mut bindings = Vec::with_capacity(request.bindings.len());
    for spec in &request.bindings {
        let result = VariableId::new(spec.result_group, spec.result_index);
        let helpers = spec.helpers.iter()
            .map(|pair| match pair.as_slice() {
                [group, index] => Ok(VariableId::new(*group, *index)),
                _ => Err(BridgeError::new("InvalidInput",
                    "native write failed validation: helper columns are [group, index] pairs")),
            })
            .collect::<Result<Vec<_>>>()?;
        // 构造器**不检查**结果列是否混进辅助列、也不去重——如实报告而不是替它拒。
        // The constructor does **not** reject a result column among the helpers nor deduplicate: report, do not refuse.
        let result_also_helper = helpers.iter().any(|helper| *helper == result);
        let mut unique = helpers.clone();
        unique.sort();
        unique.dedup();
        let duplicate_helpers = u32::try_from(helpers.len() - unique.len()).unwrap_or(u32::MAX);
        let binding = StructureUsageBinding::new(spec.source_symbol_id, result, helpers);
        // 读回绑定，确认字段原样保留。 / Read the binding back to confirm the fields survive.
        if binding.source_symbol_id != spec.source_symbol_id || binding.result != result {
            return Err(BridgeError::new("InputFailure",
                "native write binding did not preserve its source symbol or result column"));
        }
        bindings.push(BindingRow { source_symbol_id: binding.source_symbol_id,
            result_also_helper, helper_count: u32::try_from(binding.helpers.len())
                .unwrap_or(u32::MAX), duplicate_helpers });
    }
    let mut proofs = Vec::with_capacity(request.proofs.len());
    for pair in &request.proofs {
        let proposed = InputDomainProof::new(pair.proposed_lower, pair.proposed_upper)
            .map_err(|error| BridgeError::new("InvalidInput",
                &format!("native write failed validation: {error}")))?;
        let recorded = InputDomainProof::new(pair.recorded_lower, pair.recorded_upper)
            .map_err(|error| BridgeError::new("InvalidInput",
                &format!("native write failed validation: {error}")))?;
        let allowed = proposed.is_contained_in(&recorded);
        let widened = recorded.is_contained_in(&proposed) && !allowed;
        proofs.push(ProofRow { allowed, widened,
            unchanged: proposed.lower() == recorded.lower() && proposed.upper() == recorded.upper(),
            refusal: if allowed { None } else { Some(DOMAIN_PROOF_WIDENED.to_owned()) } });
    }
    // 三处实测：两侧都缺不过、只缺一侧不过、裸结构两样都不给。 / Three measured findings.
    let bare = BareStructure { name: "bare".to_owned(), fingerprint: None, binding: None };
    let both_absent_passes = verify_native_write(None, &bare);
    let one_absent_passes = verify_native_write(Some("abc"), &bare);
    Ok(NativeWriteReport {
        cases,
        bindings,
        proofs,
        both_absent_passes,
        one_absent_passes,
        bare_structure_has_fingerprint: bare.fingerprint().is_some(),
        bare_structure_has_binding: DeferredFunctionStructure::<f64>::usage_binding(&bare).is_some(),
        widened_prefix: DOMAIN_PROOF_WIDENED.to_owned(),
        lost_prefix: DOMAIN_PROOF_LOST.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(recorded: Option<&str>, current: Option<&str>) -> FingerprintCase {
        FingerprintCase { recorded: recorded.map(str::to_owned),
            current: current.map(str::to_owned) }
    }

    fn request(cases: Vec<FingerprintCase>) -> NativeWriteRequest {
        NativeWriteRequest { cases, bindings: Vec::new(), proofs: Vec::new() }
    }

    fn proof(pl: f64, pu: f64, rl: f64, ru: f64) -> ProofPair {
        ProofPair { proposed_lower: pl, proposed_upper: pu, recorded_lower: rl,
            recorded_upper: ru }
    }

    #[test]
    fn only_both_sides_present_and_equal_passes() {
        let report = verify_native_write_record(&request(vec![
            case(Some("abc"), Some("abc")),   // 相等 → 通过 / equal → pass
            case(Some("abc"), Some("def")),   // 不等 → 失败 / differ → fail
            case(Some("abc"), None),          // 结构侧没有 → 失败 / structure offers none → fail
            case(None, Some("abc")),          // 记录侧没有 → 失败 / record lost → fail
            case(None, None),                 // 两侧都缺 → 失败 / both absent → fail
        ])).unwrap();
        assert!(report.cases[0].accepted);
        assert_eq!(report.cases[0].reason, "match");
        assert!(!report.cases[1].accepted);
        assert_eq!(report.cases[1].reason, "mismatch");
        assert!(!report.cases[2].accepted);
        assert_eq!(report.cases[2].reason, "structureOffersNone");
        assert!(!report.cases[3].accepted);
        assert_eq!(report.cases[3].reason, "recordLost");
        assert!(!report.cases[4].accepted);
        assert_eq!(report.cases[4].reason, "bothAbsent");
        // "缺一侧"与"两侧都缺"都不是通过。 / Missing a side never passes.
        assert!(!report.both_absent_passes);
        assert!(!report.one_absent_passes);
    }

    #[test]
    fn a_bare_structure_offers_neither_a_fingerprint_nor_a_binding() {
        let report = verify_native_write_record(&request(Vec::new())).unwrap();
        // 两个默认方法都是 `None`：新结构**不会默认通过**校验。 / Both defaults are `None`: no accidental pass.
        assert!(!report.bare_structure_has_fingerprint);
        assert!(!report.bare_structure_has_binding);
        assert!(!report.both_absent_passes);
        // 两个拒绝前缀就是领域常量。 / The two refusal prefixes are the domain constants.
        assert!(report.widened_prefix.contains("may be tightened but not widened"));
        assert!(report.lost_prefix.contains("must not be dropped"));
    }

    #[test]
    fn proof_may_tighten_but_not_widen() {
        let mut document = request(Vec::new());
        document.proofs = vec![
            // 收紧：[1,5] → [2,3]，允许。 / Tightening is allowed.
            proof(2.0, 3.0, 1.0, 5.0),
            // 持平：允许。 / Unchanged passes.
            proof(1.0, 5.0, 1.0, 5.0),
            // 扩大：[0,9] 覆盖 [1,5]，拒绝。 / Widening is refused.
            proof(0.0, 9.0, 1.0, 5.0),
            // 一侧扩大一侧收紧：[0,3] vs [1,5]，拒绝（不是"包含"）。 / Mixed: refused.
            proof(0.0, 3.0, 1.0, 5.0),
            // 单侧收紧：允许。 / One-sided tightening is allowed.
            proof(1.0, 4.0, 1.0, 5.0),
        ];
        let report = verify_native_write_record(&document).unwrap();
        assert!(report.proofs[0].allowed);
        assert!(!report.proofs[0].widened);
        assert!(!report.proofs[0].unchanged);
        assert_eq!(report.proofs[0].refusal, None);
        assert!(report.proofs[1].allowed);
        assert!(report.proofs[1].unchanged);
        // 扩大：拒绝，并用 `widened` 前缀。 / Widening: refused with the `widened` prefix.
        assert!(!report.proofs[2].allowed);
        assert!(report.proofs[2].widened);
        assert!(!report.proofs[2].unchanged);
        assert_eq!(report.proofs[2].refusal.as_deref(), Some(DOMAIN_PROOF_WIDENED));
        // 一侧扩大一侧收紧：既不允许也不是"纯扩大"。 / Mixed: neither allowed nor a pure widening.
        assert!(!report.proofs[3].allowed);
        assert!(!report.proofs[3].widened);
        assert_eq!(report.proofs[3].refusal.as_deref(), Some(DOMAIN_PROOF_WIDENED));
        assert!(report.proofs[4].allowed);
    }

    #[test]
    fn bindings_keep_their_fields_and_do_not_police_the_helpers() {
        let mut document = request(Vec::new());
        document.bindings = vec![
            // 结果列混进辅助列：构造器**不拦**，只报告。 / A result among helpers: reported, not refused.
            BindingSpec { source_symbol_id: 7, result_group: 1, result_index: 2,
                helpers: vec![vec![1, 2], vec![1, 3]] },
            // 重复辅助列：也不去重，只报告。 / Duplicate helpers: reported, not deduplicated.
            BindingSpec { source_symbol_id: 9, result_group: 2, result_index: 0,
                helpers: vec![vec![2, 1], vec![2, 1]] },
            // 空的辅助列合法。 / An empty helper list is legal.
            BindingSpec { source_symbol_id: 11, result_group: 3, result_index: 0,
                helpers: Vec::new() },
        ];
        let report = verify_native_write_record(&document).unwrap();
        assert_eq!(report.bindings[0].source_symbol_id, 7);
        assert!(report.bindings[0].result_also_helper);
        assert_eq!(report.bindings[0].helper_count, 2);
        assert_eq!(report.bindings[0].duplicate_helpers, 0);
        assert!(!report.bindings[1].result_also_helper);
        assert_eq!(report.bindings[1].duplicate_helpers, 1);
        assert_eq!(report.bindings[2].helper_count, 0);
        assert!(!report.bindings[2].result_also_helper);
    }

    #[test]
    fn bad_input_is_refused() {
        // 辅助列不是 `[组号, 组内序号]` 对。 / A helper that is not a `[group, index]` pair.
        let mut document = request(Vec::new());
        document.bindings = vec![BindingSpec { source_symbol_id: 1, result_group: 0,
            result_index: 0, helpers: vec![vec![1, 2, 3]] }];
        assert!(verify_native_write_record(&document).unwrap_err().message.contains("[group, index]"));
        // 域证明里区间非法。 / An illegal interval in the proof pairs.
        let mut document = request(Vec::new());
        document.proofs = vec![proof(5.0, 1.0, 1.0, 5.0)];
        assert!(verify_native_write_record(&document).unwrap_err().message.contains("lower <= upper"));
        // 空请求合法。 / A bare request is legal.
        let report = verify_native_write_record(&request(Vec::new())).unwrap();
        assert!(report.cases.is_empty());
        assert!(report.bindings.is_empty());
        assert!(report.proofs.is_empty());
        assert!(!report.both_absent_passes);
    }
}
