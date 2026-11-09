//! 续挖第 28 单：master cut 的两级不变量与表达式身份（`solver/logic_based_benders.rs` 的 `MasterCut`）
//! Continuation, delivery 28: the two-level invariants of a master cut and its expression identity.
//!
//! `MasterCut` 有**两级**校验，而且它们拒绝的东西**不一样**——这是本单的主结论：
//!
//! 1. **`new`（构造）只做合并，几乎不拒绝**：同一变量出现两次会被`+=`累加；**零系数照样留下**；
//!    空白 id、非有限 `rhs` 都能构造出来。`verified_global` 只是 `new` 的预填（`Global` + `Verified` + `Some(证明引用)`）。
//! 2. **`normalize`（规范化）才真正把关**：空白 id、非有限 `rhs`、空白变量 id、非有限系数、系数聚合溢出、
//!    `Verified` 却**没有**证明引用、重复的辅助变量声明——全部在这里拒绝；并且**丢掉零系数**、把证明引用
//!    trim 后把空串变回 `None`。所以"构造成功"**不等于**"能用"。
//! 3. **`expression_key()` 是表达式身份，不是 cut 身份**：它只拼 `sense` + `rhs` 的位模式 + 每一项的
//!    `名字长度:名字=系数位模式`，**完全忽略** `id` / `origin` / `reason` / `validity` / `proof_status`。
//!    于是两个"来源不同、理由不同"但数学上相同的 cut 得到**同一个 key**——去重按语义、不按出处。
//!
//! `MasterCut` has **two levels** of validation that reject **different** things. **`new` only merges and barely
//! refuses**: a variable appearing twice is summed with `+=`, **zero coefficients survive**, and a blank id or a
//! non-finite `rhs` all construct fine — `verified_global` is merely a prefilled `new` (`Global` + `Verified` + a proof
//! reference). **`normalize` is where the gate is**: blank id, non-finite `rhs`, blank variable ids, non-finite
//! coefficients, aggregation overflow, `Verified` **without** a proof reference, and duplicate auxiliary declarations are
//! all refused there, while zero coefficients are dropped and a blanked proof reference becomes `None` again — so
//! "constructed" does **not** mean "usable". And **`expression_key()` is the expression's identity, not the cut's**: it
//! concatenates only `sense`, the `rhs` bit pattern and each term's `name-length:name=coefficient-bits`, **ignoring** `id` /
//! `origin` / `reason` / `validity` / `proof_status`, so two mathematically equal cuts from different origins and reasons
//! share one key — dedupe by semantics, not by provenance.

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_framework::solver::logic_based_benders::{
    MasterCut, MasterCutProofStatus, MasterCutValidity,
};
use ospf_rust_framework::solver::linear_benders_decomposition_solver::CutSense;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个 master cut 项。 / One master cut term.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterCutTerm {
    /// 稳定变量 ID。 / The stable variable ID.
    pub variable: String,
    /// 系数。 / The coefficient.
    pub coefficient: f64,
}

/// master cut 请求。 / The master cut request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterCutRequest {
    /// cut ID。 / The cut ID.
    pub id: String,
    /// 稀疏项（**允许同一变量出现多次**）。 / The sparse terms (**duplicates allowed**).
    #[serde(default)]
    pub terms: Vec<MasterCutTerm>,
    /// `le` / `ge` / `eq`。 / `le` / `ge` / `eq`.
    pub sense: String,
    /// 右端项。 / The right-hand side.
    pub rhs: f64,
    /// 证明引用。 / The proof reference.
    #[serde(default)]
    pub proof_reference: Option<String>,
    /// 来源。 / The origin.
    #[serde(default)]
    pub origin: String,
    /// 理由。 / The reason.
    #[serde(default)]
    pub reason: String,
    /// 用 `verified_global` 构造（Global + Verified + 必须带证明引用）。 / Build through `verified_global`.
    #[serde(default)]
    pub verified_global: bool,
    /// 规范化一次。 / Normalize once.
    #[serde(default)]
    pub normalize: bool,
    /// 规范化两次（验幂等）。 / Normalize twice (to check idempotence).
    #[serde(default)]
    pub normalize_twice: bool,
}

/// 一个项的回读。 / One term read back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterCutTermRow {
    /// 稳定变量 ID。 / The stable variable ID.
    pub variable: String,
    /// 系数。 / The coefficient.
    pub coefficient: f64,
}

/// master cut 结论。 / The master cut verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MasterCutReport {
    /// 是否走到了 `normalize`。 / Whether `normalize` ran.
    pub normalized: bool,
    /// 构造/规范化失败的原因（成功时为空）。 / The failure reason (absent on success).
    #[serde(default)]
    pub error: Option<String>,
    /// cut ID。 / The cut ID.
    pub id: String,
    /// `le` / `ge` / `eq`。 / `le` / `ge` / `eq`.
    pub sense: String,
    /// 右端项。 / The right-hand side.
    pub rhs: f64,
    /// 项数。 / The number of terms.
    pub term_count: u64,
    /// 项的回读（按变量 ID 排序）。 / The terms read back (sorted by variable ID).
    pub terms: Vec<MasterCutTermRow>,
    /// 证明状态。 / The proof status.
    pub proof_status: String,
    /// 证明引用（trim 后，空串变 `None`）。 / The proof reference (trimmed, blank becomes `None`).
    #[serde(default)]
    pub proof_reference: Option<String>,
    /// 有效性范围。 / The validity scope.
    pub validity: String,
    /// 编码类型（构造出来的都是 `linear`）。 / The encoding kind (always `linear` here).
    pub encoding: String,
    /// 表达式身份。 / The expression identity.
    pub expression_key: String,
    /// `new` 会合并同变量系数（**是**）。 / Whether `new` merges duplicate coefficients (**yes**).
    pub new_merges_duplicates: bool,
    /// `new` 保留零系数（**是**）。 / Whether `new` keeps zero coefficients (**yes**).
    pub new_keeps_zeros: bool,
    /// `normalize` 丢掉零系数（**是**）。 / Whether `normalize` drops zero coefficients (**yes**).
    pub normalize_drops_zeros: bool,
    /// `normalize` 拒空白 ID（**是**）。 / Whether `normalize` refuses a blank ID (**yes**).
    pub normalize_refuses_blank_id: bool,
    /// `normalize` 拒非有限 `rhs`（**是**）。 / Whether `normalize` refuses a non-finite `rhs` (**yes**).
    pub normalize_refuses_non_finite_rhs: bool,
    /// `Verified` 必须带证明引用（**是**）。 / Whether `Verified` needs a proof reference (**yes**).
    pub verified_needs_reference: bool,
    /// 空白证明引用被丢成 `None`（**是**）。 / Whether a blank proof reference becomes `None` (**yes**).
    pub blank_reference_is_dropped: bool,
    /// `expression_key` 忽略出处与身份（**是**）。 / Whether `expression_key` ignores provenance (**yes**).
    pub expression_key_ignores_identity: bool,
    /// `normalize` 幂等（**是**）。 / Whether `normalize` is idempotent (**yes**).
    pub normalize_is_idempotent: bool,
}

/// 项数上限（信任边界）。 / The term cap (a trust boundary).
const MAX_TERMS: usize = 4_096;

/// 走一遍 master cut。非法输入是**错误**。 / Walk the master cut.
pub fn verify_master_cut(request: &MasterCutRequest) -> Result<MasterCutReport> {
    if request.terms.len() > MAX_TERMS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "master cut failed validation: at most {MAX_TERMS} terms are accepted")));
    }
    for term in &request.terms {
        if term.variable.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "master cut failed validation: every term needs a variable"));
        }
    }
    let sense = sense(&request.sense)?;
    let coefficients: Vec<(StableVariableId, f64)> = request.terms.iter()
        .map(|term| (StableVariableId(term.variable.clone()), term.coefficient)).collect();
    let cut = build(request, sense, coefficients.clone());
    // 先看"构造成功但规范化失败"的分裂。 / Look at the construct-vs-normalize split first.
    let (value, normalized, error) = if request.normalize || request.normalize_twice {
        let first = cut.clone().normalize();
        let value = match (first, request.normalize_twice) {
            (Ok(cut), true) => match cut.normalize() {
                Ok(again) => Ok(again),
                Err(error) => Err(error),
            },
            (Ok(cut), false) => Ok(cut),
            (Err(error), _) => Err(error),
        };
        match value {
            Ok(cut) => (Some(cut), true, None),
            Err(error) => (None, true, Some(error.to_string())),
        }
    } else {
        (Some(cut), false, None)
    };
    let findings = canonical_findings(sense)?;
    match value {
        Some(cut) => {
            let terms = cut.coefficients.iter()
                .map(|(id, coefficient)| MasterCutTermRow { variable: id.0.clone(),
                    coefficient: *coefficient })
                .collect::<Vec<_>>();
            Ok(MasterCutReport {
                normalized,
                error,
                id: cut.id.clone(),
                sense: sense_name(cut.sense).to_owned(),
                rhs: cut.rhs,
                term_count: terms.len() as u64,
                terms,
                proof_status: format!("{:?}", cut.proof_status),
                proof_reference: cut.proof_reference.clone(),
                validity: format!("{:?}", cut.validity),
                encoding: format!("{:?}", cut.encoding).split(' ').next()
                    .unwrap_or("Linear").to_owned(),
                expression_key: cut.expression_key(),
                new_merges_duplicates: findings.new_merges_duplicates,
                new_keeps_zeros: findings.new_keeps_zeros,
                normalize_drops_zeros: findings.normalize_drops_zeros,
                normalize_refuses_blank_id: findings.normalize_refuses_blank_id,
                normalize_refuses_non_finite_rhs: findings.normalize_refuses_non_finite_rhs,
                verified_needs_reference: findings.verified_needs_reference,
                blank_reference_is_dropped: findings.blank_reference_is_dropped,
                expression_key_ignores_identity: findings.expression_key_ignores_identity,
                normalize_is_idempotent: findings.normalize_is_idempotent,
            })
        }
        None => Err(BridgeError::new("InvalidInput", &format!(
            "master cut failed validation: {}", error.unwrap_or_default()))),
    }
}

/// 构造一个 cut（`verified_global` 或 `new`）。 / Build a cut through `verified_global` or `new`.
fn build(request: &MasterCutRequest, sense: CutSense,
    coefficients: Vec<(StableVariableId, f64)>) -> MasterCut {
    if request.verified_global {
        MasterCut::verified_global(request.id.clone(), coefficients, sense, request.rhs,
            request.proof_reference.clone().unwrap_or_default(), request.origin.clone(),
            request.reason.clone())
    } else {
        MasterCut::new(request.id.clone(), coefficients, sense, request.rhs,
            MasterCutValidity::Global, MasterCutProofStatus::Claimed,
            request.proof_reference.clone(), request.origin.clone(), request.reason.clone())
    }
}

/// 九条规则属性的内部固定探针。 / The nine rule properties measured by fixed internal probes.
struct Findings {
    new_merges_duplicates: bool,
    new_keeps_zeros: bool,
    normalize_drops_zeros: bool,
    normalize_refuses_blank_id: bool,
    normalize_refuses_non_finite_rhs: bool,
    verified_needs_reference: bool,
    blank_reference_is_dropped: bool,
    expression_key_ignores_identity: bool,
    normalize_is_idempotent: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。 
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings(sense: CutSense) -> Result<Findings> {
    let id = |name: &str| StableVariableId(name.to_owned());
    let probe = |name: &str, terms: Vec<(StableVariableId, f64)>, rhs: f64| {
        MasterCut::new(name, terms, sense, rhs, MasterCutValidity::Global,
            MasterCutProofStatus::Claimed, None, "probe", "probe")
    };
    // `new` 合并同变量：x + 2x = 3x。 / `new` merges: x + 2x = 3x.
    let merged = probe("probe-merge", vec![(id("x"), 1.0), (id("x"), 2.0)], 0.0);
    let new_merges_duplicates = merged.coefficients.len() == 1
        && (merged.coefficients[&id("x")] - 3.0).abs() < f64::EPSILON;
    // `new` 保留零系数；`normalize` 丢掉它。 / `new` keeps a zero; `normalize` drops it.
    let zero = probe("probe-zero", vec![(id("x"), 0.0), (id("y"), 1.0)], 0.0);
    let new_keeps_zeros = zero.coefficients.len() == 2;
    let normalize_drops_zeros = zero.clone().normalize()
        .map(|cut| cut.coefficients.len() == 1 && !cut.coefficients.contains_key(&id("x")))
        .unwrap_or(false);
    // 空白 ID / 非有限 rhs：构造成功、规范化拒绝。 / Blank id / non-finite rhs: constructs, then refuses.
    let blank = probe("  ", vec![(id("x"), 1.0)], 0.0);
    let normalize_refuses_blank_id = blank.normalize().is_err();
    let infinite = probe("probe-inf", vec![(id("x"), 1.0)], f64::INFINITY);
    let normalize_refuses_non_finite_rhs = infinite.normalize().is_err();
    // `Verified` 但无证明引用 → 拒绝；空白引用被丢成 `None` 后同样触发上面那条。
    // `Verified` without a reference is refused; a blank reference becomes `None` and then trips the same rule.
    let verified = MasterCut::verified_global("probe-verified", vec![(id("x"), 1.0)], sense, 0.0,
        "", "probe", "probe");
    let verified_needs_reference = verified.clone().normalize().is_err();
    let blank_reference_is_dropped = verified.proof_reference.is_some()
        && MasterCut::new("probe-blank", vec![(id("x"), 1.0)], sense, 0.0,
            MasterCutValidity::Global, MasterCutProofStatus::Claimed, Some("   ".to_owned()),
            "probe", "probe")
            .normalize()
            .map(|cut| cut.proof_reference.is_none())
            .unwrap_or(false);
    // 表达式身份忽略出处：只有 id/origin/reason 不同 → 同一个 key。
    // The expression identity ignores provenance: differing only in id/origin/reason gives one key.
    let first = MasterCut::new("a", vec![(id("x"), 1.5), (id("y"), -2.0)], sense, 7.0,
        MasterCutValidity::Global, MasterCutProofStatus::Claimed, None, "origin-a", "reason-a");
    let second = MasterCut::new("b", vec![(id("y"), -2.0), (id("x"), 1.5)], sense, 7.0,
        MasterCutValidity::Local, MasterCutProofStatus::None, Some("proof".to_owned()),
        "origin-b", "reason-b");
    let expression_key_ignores_identity = first.expression_key() == second.expression_key();
    // 幂等：规范化两次等于一次。 / Idempotent: normalizing twice equals once.
    let once = probe("probe-idem", vec![(id("x"), 1.0), (id("y"), 0.0)], 3.5).normalize();
    let normalize_is_idempotent = match once {
        Ok(cut) => match cut.clone().normalize() {
            Ok(again) => again.expression_key() == cut.expression_key()
                && again.coefficients == cut.coefficients,
            Err(_) => false,
        },
        Err(_) => false,
    };
    Ok(Findings { new_merges_duplicates, new_keeps_zeros, normalize_drops_zeros,
        normalize_refuses_blank_id, normalize_refuses_non_finite_rhs, verified_needs_reference,
        blank_reference_is_dropped, expression_key_ignores_identity, normalize_is_idempotent })
}

/// 把字面量转成方向。 / Turn a literal into a sense.
fn sense(name: &str) -> Result<CutSense> {
    match name {
        "le" => Ok(CutSense::LessOrEqual),
        "ge" => Ok(CutSense::GreaterOrEqual),
        "eq" => Ok(CutSense::Equal),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "master cut failed validation: unknown sense `{other}`"))),
    }
}

/// 把方向转回字面量。 / Turn a sense back into a literal.
fn sense_name(sense: CutSense) -> &'static str {
    match sense {
        CutSense::LessOrEqual => "le",
        CutSense::GreaterOrEqual => "ge",
        CutSense::Equal => "eq",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(variable: &str, coefficient: f64) -> MasterCutTerm {
        MasterCutTerm { variable: variable.to_owned(), coefficient }
    }

    fn request(id: &str, terms: Vec<MasterCutTerm>) -> MasterCutRequest {
        MasterCutRequest { id: id.to_owned(), terms, sense: "le".to_owned(), rhs: 4.0,
            proof_reference: Some("proof-1".to_owned()), origin: "capacity".to_owned(),
            reason: "cutoff".to_owned(), verified_global: false, normalize: true,
            normalize_twice: false }
    }

    #[test]
    fn construction_merges_but_keeps_zeros_while_normalize_drops_them() {
        // 同一变量出现两次 + 一个零系数。 / One variable twice plus a zero coefficient.
        let report = verify_master_cut(&request("c1", vec![term("x", 1.0), term("x", 2.5),
            term("y", 0.0)])).unwrap();
        assert!(report.normalized);
        // 规范化后只剩合并后的 x。 / After normalize only the merged x remains.
        assert_eq!(report.term_count, 1);
        assert_eq!(report.terms, vec![MasterCutTermRow { variable: "x".to_owned(),
            coefficient: 3.5 }]);
        assert!(report.new_merges_duplicates);
        assert!(report.new_keeps_zeros);
        assert!(report.normalize_drops_zeros);
    }

    #[test]
    fn without_normalize_the_zero_coefficient_survives() {
        let mut document = request("c1", vec![term("x", 1.0), term("y", 0.0)]);
        document.normalize = false;
        let report = verify_master_cut(&document).unwrap();
        assert!(!report.normalized);
        assert_eq!(report.term_count, 2);
        assert!(report.terms.iter().any(|row| row.variable == "y" && row.coefficient == 0.0));
    }

    #[test]
    fn normalize_refuses_what_construction_accepted() {
        // 空白 ID。 / A blank id.
        assert!(verify_master_cut(&request("  ", vec![term("x", 1.0)]))
            .unwrap_err().message.contains("must not be empty"));
        // 非有限 rhs。 / A non-finite rhs.
        let mut document = request("c2", vec![term("x", 1.0)]);
        document.rhs = f64::NAN;
        assert!(verify_master_cut(&document).unwrap_err().message.contains("non-finite rhs"));
        // `Verified` 却没有证明引用。 / `Verified` without a reference.
        let mut document = request("c3", vec![term("x", 1.0)]);
        document.verified_global = true;
        document.proof_reference = Some("   ".to_owned());
        assert!(verify_master_cut(&document).unwrap_err().message
            .contains("must carry a proof reference"));
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        // 换成完全不同的请求，九条规则属性仍为真。 / A completely different request keeps all nine properties true.
        let report = verify_master_cut(&request("other", vec![term("z", -1.0)])).unwrap();
        assert!(report.new_merges_duplicates);
        assert!(report.new_keeps_zeros);
        assert!(report.normalize_drops_zeros);
        assert!(report.normalize_refuses_blank_id);
        assert!(report.normalize_refuses_non_finite_rhs);
        assert!(report.verified_needs_reference);
        assert!(report.blank_reference_is_dropped);
        assert!(report.expression_key_ignores_identity);
        assert!(report.normalize_is_idempotent);
    }

    #[test]
    fn verified_global_prefills_global_verified_and_the_reference() {
        let mut document = request("c4", vec![term("x", 1.0)]);
        document.verified_global = true;
        document.proof_reference = Some("  proof-9  ".to_owned());
        let report = verify_master_cut(&document).unwrap();
        assert_eq!(report.proof_status, "Verified");
        assert_eq!(report.validity, "Global");
        // 规范化会 trim 证明引用。 / Normalize trims the reference.
        assert_eq!(report.proof_reference.as_deref(), Some("proof-9"));
        assert_eq!(report.encoding, "Linear");
        assert_eq!(report.sense, "le");
    }

    #[test]
    fn the_expression_key_ignores_provenance() {
        let mut first = request("a", vec![term("x", 1.5), term("y", -2.0)]);
        first.origin = "origin-a".to_owned();
        first.reason = "reason-a".to_owned();
        let mut second = request("b", vec![term("x", 1.5), term("y", -2.0)]);
        second.origin = "origin-b".to_owned();
        second.reason = "reason-b".to_owned();
        let first = verify_master_cut(&first).unwrap();
        let second = verify_master_cut(&second).unwrap();
        // 出处与 ID 不同，表达式身份相同。 / Different provenance and ID, same expression identity.
        assert_ne!(first.id, second.id);
        assert_eq!(first.expression_key, second.expression_key);
        // 系数变了，身份就变。 / Change a coefficient and the identity changes.
        let changed = verify_master_cut(&request("a", vec![term("x", 1.6), term("y", -2.0)])).unwrap();
        assert_ne!(first.expression_key, changed.expression_key);
    }

    #[test]
    fn normalizing_twice_is_the_same_as_once() {
        let mut document = request("c5", vec![term("x", 1.0), term("y", 0.0)]);
        document.normalize_twice = true;
        let report = verify_master_cut(&document).unwrap();
        assert!(report.normalized);
        assert_eq!(report.term_count, 1);
        assert!(report.normalize_is_idempotent);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut blank_term = request("c6", vec![term(" ", 1.0)]);
        blank_term.normalize = false;
        assert!(verify_master_cut(&blank_term).unwrap_err().message
            .contains("every term needs a variable"));
        let mut wrong_sense = request("c7", vec![term("x", 1.0)]);
        wrong_sense.sense = "lt".to_owned();
        assert!(verify_master_cut(&wrong_sense).unwrap_err().message.contains("unknown sense"));
        let many = request("c8", vec![term("x", 1.0); MAX_TERMS + 1]);
        assert!(verify_master_cut(&many).unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_master_cut(&request("c9", vec![term("x", 1.0)])).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"expressionKey\""), "{json}");
        assert!(json.contains("\"normalizeDropsZeros\""), "{json}");
        assert!(json.contains("\"proofReference\""), "{json}");
        assert!(!json.contains("expression_key"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<MasterCutReport>(&copy.to_string()).is_err());
    }
}
