//! 续挖第 50 单：`MasterCutValidity` 与 `MasterCutProofStatus`——**声明性字段**：`Local` 从不被拒、
//! `Verified` 只是自称、两者都不进 `expression_key`
//! （`solver/logic_based_benders.rs`）
//! Continuation, delivery 50: `MasterCutValidity` and `MasterCutProofStatus` are **declarative fields** — `Local` is never
//! refused, `Verified` is only a self-declaration, and neither enters the `expression_key`
//! (`solver/logic_based_benders.rs`).
//!
//! 第 43 单量过"`normalize` 是真闸门、`expression_key` 看不见编码"；这一单量**有效性与证明状态**这一对声明字段。六条实测：
//!
//! 1. **`Local` 在可达层从不被拒**（`local_is_never_refused_at_construction()`）：
//!    `MasterCut::new(..., Local, Claimed, None, ...)` + `normalize()` 都成功。拒 `Local` 的三处闸门都是**私有**的
//!    求解器内部函数，所以"局部有效"这个状态**能造出来、能留在集合里**，只是求解器不会用它。
//! 2. **`normalize()` 原样保留有效性与证明状态**（`normalize_keeps_them()`）：它只清系数、trim `proof_reference`。
//! 3. **`expression_key()` 完全看不见这两者**（`the_key_ignores_them()`）：`Global`/`Local`、
//!    `Verified`/`Claimed`/`None` 给出**同一个** key——key 只由 `sense`、`rhs` 位模式与系数决定。
//! 4. **`PartialEq` 看得见它们**（`equality_sees_them()`）：id/origin/reason 全钉住时，改一项有效性或证明状态就不等。
//! 5. **`verified_global` 钉住三样**（`verified_global_pins_three_fields()`）：`Global` + `Verified` +
//!    `Some(reference.trim())`，其余参数由调用方给。
//! 6. **`Verified` 必须挂一个非空证明引用**（`verified_requires_a_reference()`）：`Verified` + 无引用会被
//!    `normalize()` 拒绝（"must carry a proof reference"）；但**引用的内容不被校验**——`"x"` 与
//!    `"proof-of-everything"` 一样过。所以"已验证"不是纯自称，只是**自称加一根非空字符串**。
//!
//! Delivery 43 measured that `normalize` is the real gate and that `expression_key` cannot see the encoding; this one measures
//! the **declarative pair** validity and proof status. **`Local` is never refused in the reachable layer**
//! (`local_is_never_refused_at_construction()`): `MasterCut::new(..., Local, Claimed, None, ...)` plus `normalize()` both
//! succeed, because the three gates that reject `Local` are **private** solver functions — so "locally valid" **can be built and
//! kept in a set**, it is just never used by the solver. **`normalize()` keeps both verbatim** (`normalize_keeps_them()`), only
//! clearing coefficients and trimming `proof_reference`. **`expression_key()` cannot see either** (`the_key_ignores_them()`):
//! `Global`/`Local` and `Verified`/`Claimed`/`None` give the **same** key, which depends only on `sense`, the `rhs` bit pattern
//! and the coefficients. **`PartialEq` does see them** (`equality_sees_them()`): with id/origin/reason pinned, flipping either
//! field breaks equality. **`verified_global` pins three fields** (`verified_global_pins_three_fields()`): `Global` +
//! `Verified` + `Some(reference.trim())`. And **`Verified` must carry a non-blank proof reference**
//! (`verified_requires_a_reference()`): `Verified` with no reference is refused by `normalize()` ("must carry a proof
//! reference") — but **the reference's content is never checked**, so `"x"` passes as readily as `"proof-of-everything"`;
//! "verified" is a self-declaration plus one non-empty string.

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_framework::CutSense;
use ospf_rust_framework::solver::logic_based_benders::{
    MasterCut, MasterCutProofStatus, MasterCutValidity,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条 cut 的声明字面量。 / One cut's declaration literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CutValiditySpec {
    /// cut ID。 / The cut ID.
    pub id: String,
    /// `global` 或 `local`。 / `global` or `local`.
    #[serde(default)]
    pub validity: Option<String>,
    /// `verified` / `claimed` / `none`。 / `verified`, `claimed` or `none`.
    #[serde(default)]
    pub proof_status: Option<String>,
    /// 证明引用。 / The proof reference.
    #[serde(default)]
    pub proof_reference: Option<String>,
    /// cut 的右端项。 / The right-hand side.
    #[serde(default)]
    pub rhs: Option<f64>,
}

/// 一条 cut 的结论。 / One cut's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CutValidityOutcome {
    /// 是否通过 `normalize()`。 / Whether `normalize()` passed.
    pub accepted: bool,
    /// 失败消息。 / The failure message.
    #[serde(default)]
    pub error: Option<String>,
    /// 规范化后的有效性名。 / The normalized validity name.
    #[serde(default)]
    pub validity: Option<String>,
    /// 规范化后的证明状态名。 / The normalized proof-status name.
    #[serde(default)]
    pub proof_status: Option<String>,
    /// 规范化后的证明引用。 / The normalized proof reference.
    #[serde(default)]
    pub proof_reference: Option<String>,
    /// `expression_key()`。 / The `expression_key()`.
    #[serde(default)]
    pub expression_key: Option<String>,
}

/// 有效性与证明状态请求。 / The validity request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CutValidityRequest {
    /// 要逐条试的 cut（第一条与第二条会被互相比较）。 / The cuts to try (the first two are compared).
    #[serde(default)]
    pub cuts: Vec<CutValiditySpec>,
}

/// 有效性与证明状态结论。 / The validity verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CutValidityReport {
    /// 逐条结论。 / One verdict per cut.
    pub cuts: Vec<CutValidityOutcome>,
    /// 前两条的 `expression_key` 是否相同。 / Whether the first two share an `expression_key`.
    #[serde(default)]
    pub same_expression_key: Option<bool>,
    /// 前两条是否 `==`。 / Whether the first two are `==`.
    #[serde(default)]
    pub equal: Option<bool>,
    /// `Local` 在可达层从不被拒（**是**）。 / Whether `Local` is never refused here (**yes**).
    pub local_is_never_refused: bool,
    /// `normalize()` 原样保留两者（**是**）。 / Whether `normalize()` keeps both verbatim (**yes**).
    pub normalize_keeps_them: bool,
    /// key 看不见两者（**是**）。 / Whether the key ignores both (**yes**).
    pub the_key_ignores_them: bool,
    /// `PartialEq` 看得见两者（**是**）。 / Whether `PartialEq` sees both (**yes**).
    pub equality_sees_them: bool,
    /// `verified_global` 钉住三样（**是**）。 / Whether `verified_global` pins three fields (**yes**).
    pub verified_global_pins_three_fields: bool,
    /// `Verified` 必须有非空引用（**是**）。 / Whether `Verified` requires a non-blank reference (**yes**).
    pub verified_requires_a_reference: bool,
}

/// cut 条数上限（信任边界）。 / The cut cap (a trust boundary).
const MAX_CUTS: usize = 16;

/// 有效性名。 / The validity name.
fn validity_name(validity: MasterCutValidity) -> &'static str {
    match validity {
        MasterCutValidity::Global => "global",
        MasterCutValidity::Local => "local",
    }
}

/// 证明状态名。 / The proof-status name.
fn status_name(status: MasterCutProofStatus) -> &'static str {
    match status {
        MasterCutProofStatus::Verified => "verified",
        MasterCutProofStatus::Claimed => "claimed",
        MasterCutProofStatus::None => "none",
    }
}

/// 造一条 cut。 / Build one cut.
fn cut(spec: &CutValiditySpec) -> Result<MasterCut> {
    let validity = match spec.validity.as_deref().unwrap_or("global") {
        "global" => MasterCutValidity::Global,
        "local" => MasterCutValidity::Local,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "cut validity failed validation: unknown validity `{other}`"))),
    };
    let status = match spec.proof_status.as_deref().unwrap_or("none") {
        "verified" => MasterCutProofStatus::Verified,
        "claimed" => MasterCutProofStatus::Claimed,
        "none" => MasterCutProofStatus::None,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "cut validity failed validation: unknown proof status `{other}`"))),
    };
    Ok(MasterCut::new(spec.id.clone(),
        vec![(StableVariableId("x".to_owned()), 1.0)], CutSense::LessOrEqual,
        spec.rhs.unwrap_or(1.0), validity, status, spec.proof_reference.clone(),
        "probe-origin", "probe-reason"))
}

/// 走一遍有效性与证明状态。非法输入是**错误**。 / Walk the validity and proof status.
pub fn verify_cut_validity(request: &CutValidityRequest) -> Result<CutValidityReport> {
    if request.cuts.len() > MAX_CUTS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "cut validity failed validation: at most {MAX_CUTS} cuts are accepted")));
    }
    let declared = request.cuts.iter().map(cut).collect::<Result<Vec<_>>>()?;
    let normalized = declared.iter().cloned().map(|cut| cut.normalize())
        .collect::<Vec<_>>();
    let outcomes = normalized.iter().map(|outcome| match outcome {
        Ok(cut) => CutValidityOutcome { accepted: true, error: None,
            validity: Some(validity_name(cut.validity).to_owned()),
            proof_status: Some(status_name(cut.proof_status).to_owned()),
            proof_reference: cut.proof_reference.clone(),
            expression_key: Some(cut.expression_key()) },
        Err(failure) => CutValidityOutcome { accepted: false,
            error: Some(failure.to_string()), validity: None, proof_status: None,
            proof_reference: None, expression_key: None },
    }).collect::<Vec<_>>();
    let (same_expression_key, equal) = match (normalized.first(), normalized.get(1)) {
        (Some(Ok(first)), Some(Ok(second))) => (Some(first.expression_key() == second.expression_key()),
            Some(first == second)),
        _ => (None, None),
    };
    let findings = canonical_findings();
    Ok(CutValidityReport { cuts: outcomes, same_expression_key, equal,
        local_is_never_refused: findings.local_is_never_refused,
        normalize_keeps_them: findings.normalize_keeps_them,
        the_key_ignores_them: findings.the_key_ignores_them,
        equality_sees_them: findings.equality_sees_them,
        verified_global_pins_three_fields: findings.verified_global_pins_three_fields,
        verified_requires_a_reference: findings.verified_requires_a_reference })
}

/// 六条规则属性的内部固定探针。 / The six rule properties measured by fixed internal probes.
struct Findings {
    local_is_never_refused: bool,
    normalize_keeps_them: bool,
    the_key_ignores_them: bool,
    equality_sees_them: bool,
    verified_global_pins_three_fields: bool,
    verified_requires_a_reference: bool,
}

/// 造一条 pinned cut（id/origin/reason 全钉住）。 / Build a pinned cut (id, origin and reason fixed).
fn pinned(validity: MasterCutValidity, status: MasterCutProofStatus,
    reference: Option<&str>) -> MasterCut {
    MasterCut::new("cut", vec![(StableVariableId("x".to_owned()), 1.0)],
        CutSense::LessOrEqual, 1.0, validity, status,
        reference.map(|reference| reference.to_owned()), "origin", "reason")
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) `Local` 从不被拒：构造与 `normalize()` 都成功。 / `Local` is never refused: both construction and `normalize()` pass.
    let local_is_never_refused =
        pinned(MasterCutValidity::Local, MasterCutProofStatus::Claimed, None).normalize().is_ok();
    // 2) `normalize()` 原样保留。 / `normalize()` keeps both verbatim.
    let normalize_keeps_them = {
        let normalized = pinned(MasterCutValidity::Local, MasterCutProofStatus::Claimed,
            Some("  proof-7  ")).normalize().unwrap();
        // 引用被 trim，但有效性与状态不变。 / The reference is trimmed while validity and status stay.
        normalized.validity == MasterCutValidity::Local
            && normalized.proof_status == MasterCutProofStatus::Claimed
            && normalized.proof_reference.as_deref() == Some("proof-7")
    };
    // 3) key 看不见两者：四个组合给出同一个 key。 / The key ignores both: four combinations share one key.
    let the_key_ignores_them = {
        let keys = [
            (MasterCutValidity::Global, MasterCutProofStatus::Verified),
            (MasterCutValidity::Global, MasterCutProofStatus::None),
            (MasterCutValidity::Local, MasterCutProofStatus::Claimed),
            (MasterCutValidity::Local, MasterCutProofStatus::None),
        ].iter().map(|(validity, status)| pinned(*validity, *status, None).expression_key())
            .collect::<std::collections::BTreeSet<_>>();
        keys.len() == 1
    };
    // 4) `PartialEq` 看得见两者。 / `PartialEq` sees both.
    let equality_sees_them = {
        let global = pinned(MasterCutValidity::Global, MasterCutProofStatus::Verified, None);
        let local = pinned(MasterCutValidity::Local, MasterCutProofStatus::Verified, None);
        let claimed = pinned(MasterCutValidity::Global, MasterCutProofStatus::Claimed, None);
        global != local && global != claimed
            && global == pinned(MasterCutValidity::Global, MasterCutProofStatus::Verified, None)
    };
    // 5) `verified_global` 钉住三样。 / `verified_global` pins three fields.
    let verified_global_pins_three_fields = {
        let cut = MasterCut::verified_global("vg",
            vec![(StableVariableId("x".to_owned()), 2.0)], CutSense::GreaterOrEqual, 3.0,
            "  proof-9  ", "origin", "reason");
        cut.validity == MasterCutValidity::Global
            && cut.proof_status == MasterCutProofStatus::Verified
            && cut.proof_reference.as_deref() == Some("  proof-9  ")
            && cut.normalize().unwrap().proof_reference.as_deref() == Some("proof-9")
    };
    // 6) `Verified` 必须有非空引用，但引用内容不校验。 
    // 6) `Verified` requires a non-blank reference, yet the content is unchecked.
    let verified_requires_a_reference = {
        let missing = pinned(MasterCutValidity::Global, MasterCutProofStatus::Verified, None)
            .normalize();
        let blank = pinned(MasterCutValidity::Global, MasterCutProofStatus::Verified,
            Some("   ")).normalize();
        let trivial = pinned(MasterCutValidity::Global, MasterCutProofStatus::Verified,
            Some("x")).normalize();
        missing.as_ref().err().is_some_and(|failure| failure.to_string()
                .contains("must carry a proof reference"))
            && blank.is_err()
            && trivial.is_ok()
            && pinned(MasterCutValidity::Global, MasterCutProofStatus::Claimed, None)
                .normalize().is_ok()
    };
    Findings { local_is_never_refused, normalize_keeps_them, the_key_ignores_them,
        equality_sees_them, verified_global_pins_three_fields, verified_requires_a_reference }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str, validity: &str, status: &str, reference: Option<&str>) -> CutValiditySpec {
        CutValiditySpec { id: id.to_owned(), validity: Some(validity.to_owned()),
            proof_status: Some(status.to_owned()),
            proof_reference: reference.map(|reference| reference.to_owned()), rhs: Some(1.0) }
    }

    fn request() -> CutValidityRequest {
        CutValidityRequest { cuts: vec![spec("cut", "global", "verified", Some("proof-1"))] }
    }

    #[test]
    fn a_verified_global_cut_keeps_its_declaration() {
        let report = verify_cut_validity(&request()).unwrap();
        assert!(report.cuts[0].accepted);
        assert_eq!(report.cuts[0].validity.as_deref(), Some("global"));
        assert_eq!(report.cuts[0].proof_status.as_deref(), Some("verified"));
        assert_eq!(report.cuts[0].proof_reference.as_deref(), Some("proof-1"));
        assert!(report.cuts[0].expression_key.is_some());
        assert!(report.local_is_never_refused);
        assert!(report.normalize_keeps_them);
        assert!(report.the_key_ignores_them);
        assert!(report.equality_sees_them);
        assert!(report.verified_global_pins_three_fields);
        assert!(report.verified_requires_a_reference);
    }

    #[test]
    fn a_local_cut_is_accepted_and_the_key_cannot_tell() {
        // 两条只有有效性与证明状态不同：key 相同、`==` 不同。 
        // Two cuts differing only in validity and status: same key, different `==`.
        let document = CutValidityRequest { cuts: vec![spec("cut", "global", "verified",
            Some("proof-1")), spec("cut", "local", "claimed", Some("proof-1"))] };
        let report = verify_cut_validity(&document).unwrap();
        assert_eq!(report.cuts[0].validity.as_deref(), Some("global"));
        assert_eq!(report.cuts[1].validity.as_deref(), Some("local"));
        assert_eq!(report.cuts[1].proof_status.as_deref(), Some("claimed"));
        assert_eq!(report.same_expression_key, Some(true));
        assert_eq!(report.equal, Some(false));
    }

    /// **按实测纠正**：`Verified` **必须**挂非空引用（`normalize()` 拒绝），但引用**内容不校验**。
    /// Measured correction: `Verified` **requires** a non-blank reference (refused by `normalize()`) while the content is
    /// **never checked**.
    #[test]
    fn verified_requires_a_reference_but_its_content_is_unchecked() {
        let missing = CutValidityRequest { cuts: vec![spec("cut", "global", "verified", None)] };
        let report = verify_cut_validity(&missing).unwrap();
        assert!(!report.cuts[0].accepted);
        assert!(report.cuts[0].error.as_deref().unwrap_or_default()
            .contains("must carry a proof reference"));
        let blank = CutValidityRequest { cuts: vec![spec("cut", "global", "verified",
            Some("   "))] };
        assert!(!verify_cut_validity(&blank).unwrap().cuts[0].accepted);
        // 内容是 `"x"` 也过。 / The content `"x"` passes too.
        let trivial = CutValidityRequest { cuts: vec![spec("cut", "global", "verified",
            Some("x"))] };
        let report = verify_cut_validity(&trivial).unwrap();
        assert!(report.cuts[0].accepted);
        assert_eq!(report.cuts[0].proof_reference.as_deref(), Some("x"));
        // `Claimed` 与 `None` 不需要引用。 / `Claimed` and `None` need no reference.
        for status in ["claimed", "none"] {
            let document = CutValidityRequest { cuts: vec![spec("cut", "global", status, None)] };
            assert!(verify_cut_validity(&document).unwrap().cuts[0].accepted);
        }
    }

    #[test]
    fn the_reference_is_trimmed_and_blank_becomes_none() {
        let document = CutValidityRequest { cuts: vec![spec("cut", "global", "claimed",
            Some("   "))] };
        let report = verify_cut_validity(&document).unwrap();
        assert_eq!(report.cuts[0].proof_reference, None);
        let document = CutValidityRequest { cuts: vec![spec("cut", "global", "claimed",
            Some("  proof-3  "))] };
        let report = verify_cut_validity(&document).unwrap();
        assert_eq!(report.cuts[0].proof_reference.as_deref(), Some("proof-3"));
    }

    #[test]
    fn bad_input_is_refused() {
        let unknown_validity = CutValidityRequest { cuts: vec![spec("cut", "branch", "none",
            None)] };
        assert!(verify_cut_validity(&unknown_validity).unwrap_err().message
            .contains("unknown validity"));
        let unknown_status = CutValidityRequest { cuts: vec![spec("cut", "global", "maybe",
            None)] };
        assert!(verify_cut_validity(&unknown_status).unwrap_err().message
            .contains("unknown proof status"));
        let too_many = CutValidityRequest { cuts: (0..17)
            .map(|index| spec(&format!("c{index}"), "global", "none", None)).collect() };
        assert!(verify_cut_validity(&too_many).unwrap_err().message
            .contains("at most 16 cuts"));
        // 空白 ID 在 `normalize()` 里被拒，逐条记录。 
        // A blank ID is refused by `normalize()` and recorded per cut.
        let blank = CutValidityRequest { cuts: vec![spec("  ", "global", "none", None)] };
        let report = verify_cut_validity(&blank).unwrap();
        assert!(!report.cuts[0].accepted);
        assert!(report.cuts[0].error.as_deref().unwrap_or_default()
            .contains("ID must not be empty"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_cut_validity(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"expressionKey\""), "{json}");
        assert!(json.contains("\"normalizeKeepsThem\""), "{json}");
        assert!(json.contains("\"theKeyIgnoresThem\""), "{json}");
        assert!(!json.contains("expression_key"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<CutValidityReport>(&copy.to_string()).is_err());
    }
}
