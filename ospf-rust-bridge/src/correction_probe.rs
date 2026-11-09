//! 续挖第 108 单：**修正建议是"按成本升序截断"的确定性排序，且从不自称完整 MCS**——`weighted_correction_set`
//! 先按 `weight × relaxation` **升序**排（并列按稳定 ID），再**截断到 `max_candidates`（留下最便宜的）**，
//! `total_cost` 是**乘积之和**且必须与成员自洽（1e-9 内），`minimal` **恒为假**；`minimal_correction_set` 则
//! **必须显式声明已复验**，而且**一旦发生截断就退回 `minimal = false`**；`is_complete_mcs()` 恒假、
//! `requires_revalidation()` 恒真；策略的零上限被拒，权重 ≤ 0 **只在策略要求正权重时才被拒**
//! （`analysis/correction.rs` 的 `weighted_correction_set` / `minimal_correction_set` / `CorrectionSet`）
//! Continuation, delivery 108: **the correction recommendation is a deterministic ascending-cost truncation that never
//! claims a complete MCS** — `weighted_correction_set` sorts by `weight * relaxation` **ascending** (ties by stable id) and
//! then **truncates to `max_candidates`, keeping the cheapest**; `total_cost` is the **sum of the products** and must agree
//! with the members within 1e-9; `minimal` is **always false**; `minimal_correction_set` **demands an explicit
//! verification claim** and **drops back to `minimal = false` as soon as truncation happens**; `is_complete_mcs()` is always
//! false and `requires_revalidation()` always true; a zero policy limit is refused, while a weight of zero or less **is
//! refused only when the policy demands a positive weight**
//! (`weighted_correction_set` / `minimal_correction_set` / `CorrectionSet` in `analysis/correction.rs`).
//!
//! 第 107 单量的是最小性判据，这一单量**修正建议的排序与截断口径**。八条实测：
//!
//! 1. **按 `weight × relaxation` 升序排**（`the_recommendation_sorts_by_cost_ascending()`），并列按稳定 ID。
//! 2. **截断留下最便宜的**（`the_truncation_keeps_the_cheapest()`）。
//! 3. **`total_cost` 是乘积之和且必须自洽**（`the_total_cost_is_the_sum_of_the_products()`）。
//! 4. **`minimal` 恒为假，且协议从不自称完整 MCS**（`the_weighted_set_is_never_minimal()`）：
//!    `is_complete_mcs()` 假、`requires_revalidation()` 真。
//! 5. **`minimal_correction_set` 必须显式声明已复验**（`the_minimal_set_requires_the_verification_claim()`）。
//! 6. **一旦截断就退回 `minimal = false`**（`the_truncation_drops_the_minimal_flag()`）。
//! 7. **策略的零上限被拒**（`the_policy_refuses_zero_limits()`）。
//! 8. **权重 ≤ 0 只在策略要求正权重时才被拒**（`a_non_positive_weight_is_policy_dependent()`）。
//!
//! Delivery 107 measured the minimality verdict; this one measures **the ordering and truncation of the correction
//! recommendation**. **The recommendation sorts by cost ascending** with ties by stable id. **The truncation keeps the
//! cheapest.** **`total_cost` is the sum of the products.** **The weighted set is never minimal**, and the protocol never
//! claims a complete MCS. **The minimal set requires the verification claim**, and **truncation drops the minimal flag**.
//! **The policy refuses zero limits.** And **a non-positive weight is policy dependent.**

use ospf_rust_core::analysis::correction::{
    minimal_correction_set, weighted_correction_set, CorrectionCandidate, CorrectionSet,
    RelaxabilityPolicy,
};
use ospf_rust_core::analysis::DiagnosticSource;
use ospf_rust_core::solver::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个候选的描述。 / The description of one candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorrectionCandidateSpec {
    /// 稳定约束身份。 / The stable constraint identity.
    pub id: String,
    /// 业务成本权重。 / The business cost weight.
    pub weight: f64,
    /// 建议松弛幅度。 / The suggested relaxation amount.
    pub relaxation: f64,
}

/// 修正建议探针请求。 / The correction probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorrectionProbeRequest {
    /// 单个集合允许的最大来源数（默认 32）。 / The per-set candidate cap.
    #[serde(default)]
    pub max_candidates: Option<u32>,
    /// 允许返回的最大方案数（默认 8）。 / The plan cap.
    #[serde(default)]
    pub max_plans: Option<u32>,
    /// 是否要求权重严格为正（默认真）。 / Whether a strictly positive weight is required.
    #[serde(default)]
    pub require_positive_weight: Option<bool>,
    /// 候选列表。 / The candidate list.
    #[serde(default)]
    pub candidates: Vec<CorrectionCandidateSpec>,
    /// 走 `minimal_correction_set` 并给出"已复验"声明。 / Use `minimal_correction_set` with a verification claim.
    #[serde(default)]
    pub verified: Option<bool>,
    /// 是否走 `minimal_correction_set`（否则走 `weighted_correction_set`）。 / Whether to take the minimal path.
    #[serde(default)]
    pub minimal: Option<bool>,
}

/// 一条成员读数。 / One member reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorrectionMemberRow {
    /// 稳定身份。 / The stable identity.
    pub id: String,
    /// 该成员的乘积成本。 / The member's product cost.
    pub cost: f64,
}

/// 修正建议探针结论。 / The correction probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorrectionProbeReport {
    /// 是否成功构造（拒绝时为假）。 / Whether construction succeeded.
    pub built: bool,
    /// 拒绝或校验的错误文本（成功时为空）。 / The refusal or validation error text.
    pub error: String,
    /// 成员的顺序读数。 / The members in order.
    pub members: Vec<CorrectionMemberRow>,
    /// 带权总成本。 / The weighted total cost.
    pub total_cost: f64,
    /// `minimal` 的读数。 / What `minimal` says.
    pub minimal: bool,
    /// `is_complete_mcs()` 的读数。 / What `is_complete_mcs()` says.
    pub complete_mcs: bool,
    /// `requires_revalidation()` 的读数。 / What `requires_revalidation()` says.
    pub requires_revalidation: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_recommendation_sorts_by_cost_ascending: bool,
    pub the_truncation_keeps_the_cheapest: bool,
    pub the_total_cost_is_the_sum_of_the_products: bool,
    pub the_weighted_set_is_never_minimal: bool,
    pub the_minimal_set_requires_the_verification_claim: bool,
    pub the_truncation_drops_the_minimal_flag: bool,
    pub the_policy_refuses_zero_limits: bool,
    pub a_non_positive_weight_is_policy_dependent: bool,
}

/// 造一个候选。 / Build one candidate.
fn candidate_of(spec: &CorrectionCandidateSpec) -> CorrectionCandidate {
    CorrectionCandidate {
        source: DiagnosticSource::Constraint {
            id: StableConstraintId(spec.id.clone()) },
        weight: spec.weight,
        relaxation: spec.relaxation,
    }
}

/// 走一遍修正建议探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_correction_probe(request: &CorrectionProbeRequest) -> Result<CorrectionProbeReport> {
    if request.candidates.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "correction probe failed validation: \
            at most eight candidates are accepted"));
    }
    let findings = canonical_findings();
    let policy = RelaxabilityPolicy {
        max_candidates: usize::try_from(request.max_candidates.unwrap_or(32))
            .map_err(|_| BridgeError::new("InvalidInput", "candidate cap overflow"))?,
        max_plans: usize::try_from(request.max_plans.unwrap_or(8))
            .map_err(|_| BridgeError::new("InvalidInput", "plan cap overflow"))?,
        require_positive_weight: request.require_positive_weight.unwrap_or(true),
    };
    let candidates: Vec<CorrectionCandidate> = request.candidates.iter().map(candidate_of).collect();
    let built = if request.minimal.unwrap_or(false) {
        minimal_correction_set(candidates, &policy, request.verified.unwrap_or(false))
    } else {
        weighted_correction_set(candidates, &policy)
    };
    let (set, error): (Option<CorrectionSet>, String) = match built {
        Ok(set) => (Some(set), String::new()),
        Err(error) => (None, error.to_string()),
    };
    Ok(CorrectionProbeReport {
        built: set.is_some(),
        error,
        members: set.as_ref().map(|set| set.members.iter().map(|member| CorrectionMemberRow {
            id: member.source.stable_id(),
            cost: member.weight * member.relaxation,
        }).collect()).unwrap_or_default(),
        total_cost: set.as_ref().map(|set| set.total_cost).unwrap_or(0.0),
        minimal: set.as_ref().is_some_and(|set| set.minimal),
        complete_mcs: set.as_ref().is_some_and(|set| set.is_complete_mcs()),
        requires_revalidation: set.as_ref().is_some_and(|set| set.requires_revalidation()),
        the_recommendation_sorts_by_cost_ascending:
            findings.the_recommendation_sorts_by_cost_ascending,
        the_truncation_keeps_the_cheapest: findings.the_truncation_keeps_the_cheapest,
        the_total_cost_is_the_sum_of_the_products:
            findings.the_total_cost_is_the_sum_of_the_products,
        the_weighted_set_is_never_minimal: findings.the_weighted_set_is_never_minimal,
        the_minimal_set_requires_the_verification_claim:
            findings.the_minimal_set_requires_the_verification_claim,
        the_truncation_drops_the_minimal_flag: findings.the_truncation_drops_the_minimal_flag,
        the_policy_refuses_zero_limits: findings.the_policy_refuses_zero_limits,
        a_non_positive_weight_is_policy_dependent:
            findings.a_non_positive_weight_is_policy_dependent,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_recommendation_sorts_by_cost_ascending: bool,
    the_truncation_keeps_the_cheapest: bool,
    the_total_cost_is_the_sum_of_the_products: bool,
    the_weighted_set_is_never_minimal: bool,
    the_minimal_set_requires_the_verification_claim: bool,
    the_truncation_drops_the_minimal_flag: bool,
    the_policy_refuses_zero_limits: bool,
    a_non_positive_weight_is_policy_dependent: bool,
}

/// 造一个候选（内部用）。 / Build one candidate (internal).
fn candidate(id: &str, weight: f64, relaxation: f64) -> CorrectionCandidate {
    CorrectionCandidate {
        source: DiagnosticSource::Constraint { id: StableConstraintId(id.to_owned()) },
        weight,
        relaxation,
    }
}

/// 默认策略。 / The default policy.
fn policy() -> RelaxabilityPolicy {
    RelaxabilityPolicy::default()
}

/// 取成员身份序列（去掉 `constraint:` 前缀，便于与请求里的裸 ID 比较）。
/// Take the member identity sequence with the `constraint:` prefix stripped, so it compares with the bare IDs.
fn ids(set: &CorrectionSet) -> Vec<String> {
    set.members.iter().map(|member| {
        let stable = member.source.stable_id();
        stable.rsplit(':').next().unwrap_or(&stable).to_owned()
    }).collect()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 按乘积成本升序，并列按稳定 ID。 / Ascending product cost, ties by stable id.
        let the_recommendation_sorts_by_cost_ascending = {
            let set = weighted_correction_set(vec![
                candidate("b", 3.0, 1.0), candidate("a", 1.0, 1.0), candidate("c", 2.0, 1.0),
                // 与 `a` 同成本但身份更大。 / Same cost as `a` but a larger identity.
                candidate("aa", 1.0, 1.0),
            ], &policy()).expect("set builds");
            ids(&set) == vec!["a".to_owned(), "aa".to_owned(), "c".to_owned(), "b".to_owned()]
        };
        // 2) 截断留下最便宜的。 / The truncation keeps the cheapest.
        let the_truncation_keeps_the_cheapest = {
            let capped = RelaxabilityPolicy { max_candidates: 2, ..policy() };
            let set = weighted_correction_set(vec![
                candidate("expensive", 9.0, 1.0), candidate("cheap", 1.0, 1.0),
                candidate("middle", 5.0, 1.0),
            ], &capped).expect("set builds");
            ids(&set) == vec!["cheap".to_owned(), "middle".to_owned()]
        };
        // 3) 总成本是乘积之和。 / The total cost is the sum of the products.
        let the_total_cost_is_the_sum_of_the_products = {
            let set = weighted_correction_set(vec![
                candidate("a", 2.0, 3.0), candidate("b", 0.5, 4.0),
            ], &policy()).expect("set builds");
            // 2×3 + 0.5×4 = 8。 / Two times three plus a half times four is eight.
            (set.total_cost - 8.0).abs() < 1e-12
                // 自洽的集合通过校验，改坏成本就被拒。 
                // A consistent set validates while a broken cost is refused.
                && set.validate(&policy()).is_ok()
                && CorrectionSet { total_cost: 9.0, ..set.clone() }.validate(&policy()).is_err()
        };
        // 4) 带权集合从不 minimal，也从不自称完整 MCS。 
        // The weighted set is never minimal and never a complete MCS.
        let the_weighted_set_is_never_minimal = {
            let set = weighted_correction_set(vec![candidate("a", 1.0, 1.0)], &policy())
                .expect("set builds");
            !set.minimal && !set.deletion_minimality_verified() && !set.is_complete_mcs()
                && set.requires_revalidation()
        };
        // 5) minimal 入口必须声明已复验。 / The minimal entry point demands the claim.
        let the_minimal_set_requires_the_verification_claim = {
            let candidates = vec![candidate("a", 1.0, 1.0)];
            minimal_correction_set(candidates.clone(), &policy(), false).is_err()
                && minimal_correction_set(candidates, &policy(), true)
                    .is_ok_and(|set| set.minimal)
        };
        // 6) 截断会丢掉 minimal。 / Truncation drops the minimal flag.
        let the_truncation_drops_the_minimal_flag = {
            let capped = RelaxabilityPolicy { max_candidates: 2, ..policy() };
            let three = vec![candidate("a", 1.0, 1.0), candidate("b", 2.0, 1.0),
                candidate("c", 3.0, 1.0)];
            let truncated = minimal_correction_set(three, &capped, true).expect("set builds");
            // 没截断时保留 minimal。 / Without truncation the flag survives.
            let whole = minimal_correction_set(vec![candidate("a", 1.0, 1.0)], &capped, true)
                .expect("set builds");
            !truncated.minimal && truncated.members.len() == 2 && whole.minimal
        };
        // 7) 零上限被拒。 / Zero limits are refused.
        let the_policy_refuses_zero_limits = {
            RelaxabilityPolicy { max_candidates: 0, ..policy() }.validate().is_err()
                && RelaxabilityPolicy { max_plans: 0, ..policy() }.validate().is_err()
                && policy().validate().is_ok()
        };
        // 8) 权重 ≤ 0 取决于策略。 / A non-positive weight is policy dependent.
        let a_non_positive_weight_is_policy_dependent = {
            let strict = policy();
            let lenient = RelaxabilityPolicy { require_positive_weight: false, ..policy() };
            let negative = vec![candidate("a", -1.0, 1.0)];
            weighted_correction_set(negative.clone(), &strict).is_err()
                && weighted_correction_set(negative, &lenient).is_ok()
                // 松弛幅度必须为正，这一条与策略无关。 
                // The relaxation amount must be positive regardless of the policy.
                && weighted_correction_set(vec![candidate("a", 1.0, 0.0)], &lenient).is_err()
        };
        Findings {
            the_recommendation_sorts_by_cost_ascending,
            the_truncation_keeps_the_cheapest,
            the_total_cost_is_the_sum_of_the_products,
            the_weighted_set_is_never_minimal,
            the_minimal_set_requires_the_verification_claim,
            the_truncation_drops_the_minimal_flag,
            the_policy_refuses_zero_limits,
            a_non_positive_weight_is_policy_dependent,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: CorrectionProbeRequest) -> CorrectionProbeReport {
        verify_correction_probe(&request).unwrap()
    }

    fn spec(id: &str, weight: f64, relaxation: f64) -> CorrectionCandidateSpec {
        CorrectionCandidateSpec { id: id.to_owned(), weight, relaxation }
    }

    fn request() -> CorrectionProbeRequest {
        CorrectionProbeRequest { max_candidates: None, max_plans: None,
            require_positive_weight: None,
            candidates: vec![spec("b", 3.0, 1.0), spec("a", 1.0, 2.0)], verified: None,
            minimal: None }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request());
        assert!(report.built, "{}", report.error);
        assert!(report.the_recommendation_sorts_by_cost_ascending);
        assert!(report.the_truncation_keeps_the_cheapest);
        assert!(report.the_total_cost_is_the_sum_of_the_products);
        assert!(report.the_weighted_set_is_never_minimal);
        assert!(report.the_minimal_set_requires_the_verification_claim);
        assert!(report.the_truncation_drops_the_minimal_flag);
        assert!(report.the_policy_refuses_zero_limits);
        assert!(report.a_non_positive_weight_is_policy_dependent);
        // 成本 2 与 3 → a 在前。 / Costs two and three put `a` first.
        // 线上身份带 `constraint:` 前缀。 / The wire identity carries the `constraint:` prefix.
        assert_eq!(report.members.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            vec!["constraint:a", "constraint:b"]);
        assert_eq!(report.total_cost, 5.0);
        assert!(!report.minimal);
        assert!(!report.complete_mcs);
        assert!(report.requires_revalidation);
    }

    #[test]
    fn the_cheapest_survive_the_truncation() {
        let report = ask(CorrectionProbeRequest { max_candidates: Some(2),
            candidates: vec![spec("expensive", 9.0, 1.0), spec("cheap", 1.0, 1.0),
                spec("middle", 5.0, 1.0)], ..request() });
        assert_eq!(report.members.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            vec!["constraint:cheap", "constraint:middle"]);
    }

    #[test]
    fn the_minimal_path_demands_the_claim() {
        let unverified = ask(CorrectionProbeRequest { minimal: Some(true),
            verified: Some(false), candidates: vec![spec("a", 1.0, 1.0)], ..request() });
        assert!(!unverified.built);
        assert!(unverified.error.contains("deletion verification"), "{}", unverified.error);
        let verified = ask(CorrectionProbeRequest { minimal: Some(true), verified: Some(true),
            candidates: vec![spec("a", 1.0, 1.0)], ..request() });
        assert!(verified.built, "{}", verified.error);
        assert!(verified.minimal);
        assert!(!verified.complete_mcs);
        assert!(verified.requires_revalidation);
    }

    #[test]
    fn truncation_drops_the_minimal_flag() {
        let report = ask(CorrectionProbeRequest { minimal: Some(true), verified: Some(true),
            max_candidates: Some(2), candidates: vec![spec("a", 1.0, 1.0), spec("b", 2.0, 1.0),
                spec("c", 3.0, 1.0)], ..request() });
        assert!(report.built, "{}", report.error);
        assert_eq!(report.members.len(), 2);
        assert!(!report.minimal);
    }

    #[test]
    fn the_policy_and_the_weights_are_checked() {
        assert!(!ask(CorrectionProbeRequest { max_candidates: Some(0), ..request() }).built);
        assert!(!ask(CorrectionProbeRequest { max_plans: Some(0), ..request() }).built);
        // 负权重在严格策略下被拒，在宽松策略下放行。 
        // A negative weight is refused under the strict policy and accepted under the lenient one.
        assert!(!ask(CorrectionProbeRequest { candidates: vec![spec("a", -1.0, 1.0)],
            ..request() }).built);
        assert!(ask(CorrectionProbeRequest { require_positive_weight: Some(false),
            candidates: vec![spec("a", -1.0, 1.0)], ..request() }).built);
        // 松弛幅度为 0 与策略无关地被拒。 / A zero relaxation is refused regardless of the policy.
        assert!(!ask(CorrectionProbeRequest { require_positive_weight: Some(false),
            candidates: vec![spec("a", 1.0, 0.0)], ..request() }).built);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_correction_probe(&CorrectionProbeRequest {
            candidates: (0..9).map(|index| spec(&format!("c{index}"), 1.0, 1.0)).collect(),
            ..request() }).unwrap_err().message.contains("at most eight candidates"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"totalCost\""), "{json}");
        assert!(json.contains("\"completeMcs\""), "{json}");
        assert!(json.contains("\"theTruncationKeepsTheCheapest\""), "{json}");
        assert!(!json.contains("total_cost"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<CorrectionProbeReport>(&copy.to_string()).is_err());
    }
}
