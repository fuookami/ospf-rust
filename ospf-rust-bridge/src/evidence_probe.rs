//! 续挖第 102 单：**不可行证据的"权威性"闸门只认来源，不认质量**——`NativeIis` 与 `Farkas` 无论标成
//! `HEURISTIC` + `UNAVAILABLE` 都**照样权威**；而 `REBUILD_VERIFICATION` 必须**同时**是 `EXACT` 与 `COMPLETE`
//! （缺任一条都不权威，`RELIABLE` 也不行）；`DELETION_FILTER` / `ELASTIC_FILTER` **永远不会**权威，
//! 哪怕质量拉满；`minimality` 默认是 `NOT_CHECKED`（从不主动声明最小）；
//! `constraint_ids` 与 `members` 是**两套互不推导的集合**；`unavailable_reason` 与任何状态**都不联动**
//! （`solver/report/mod.rs` 的 `InfeasibilityEvidence` / `ProofReliability` / `ProofCompleteness`）
//! Continuation, delivery 102: **the authority gate for infeasibility evidence only reads the source, never the
//! quality** — `NATIVE_IIS` and `FARKAS` stay **authoritative even when labelled `HEURISTIC` plus `UNAVAILABLE`**, while
//! `REBUILD_VERIFICATION` must be **both `EXACT` and `COMPLETE`** (`RELIABLE` is not enough, and neither quality alone
//! suffices); `DELETION_FILTER` / `ELASTIC_FILTER` are **never** authoritative even at full quality; `minimality`
//! defaults to `NOT_CHECKED`, so minimality is never claimed on its own; `constraint_ids` and `members` are **two sets
//! that never derive from each other**; and `unavailable_reason` **is tied to no state at all**
//! (`InfeasibilityEvidence` / `ProofReliability` / `ProofCompleteness` in `solver/report/mod.rs`).
//!
//! 第 101 单量的是活动性分档，这一单量**证据面的权威性闸门**。八条实测：
//!
//! 1. **原生 IIS 与 Farkas 无条件权威**（`native_and_farkas_are_authoritative_whatever_their_quality()`）：
//!    连 `HEURISTIC` + `UNAVAILABLE` 都算。
//! 2. **CP 重建必须同时 `EXACT` 与 `COMPLETE`**（`rebuild_needs_both_exact_and_complete()`）：
//!    `RELIABLE` + `COMPLETE` 不权威，`EXACT` + `PARTIAL` 也不权威。
//! 3. **删除过滤与弹性过滤永不权威**（`the_filters_are_never_authoritative()`）：质量拉满也一样。
//! 4. **`UNAVAILABLE` 永不权威**（`the_unavailable_source_is_never_authoritative()`）。
//! 5. **`minimality` 默认 `NOT_CHECKED`**（`minimality_defaults_to_not_checked()`）。
//! 6. **两套集合互不推导**（`the_two_member_sets_never_derive_from_each_other()`）。
//! 7. **`unavailable_reason` 与状态不联动**（`the_unavailable_reason_is_tied_to_no_state()`）。
//! 8. **线上的枚举名是 SCREAMING_SNAKE_CASE，证据本体是 camelCase**（`the_wire_names_are_screaming_snake_case()`）。
//!
//! Delivery 101 measured the activity banding; this one measures **the evidence authority gate**. **Native IIS and Farkas
//! are authoritative whatever their quality.** **A CP rebuild needs both `EXACT` and `COMPLETE`.** **The two filters are
//! never authoritative.** **The unavailable source never is either.** **`minimality` defaults to `NOT_CHECKED`.** **The
//! two member sets never derive from each other.** **The unavailable reason is tied to no state.** And **the wire enum
//! names are SCREAMING_SNAKE_CASE while the evidence itself is camelCase.**

use ospf_rust_core::solver::report::{
    InfeasibilityEvidence, InfeasibilityEvidenceMember, InfeasibilityEvidenceSource,
    InfeasibilityMinimality, ProofCompleteness, ProofReliability,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::Duration;
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 按名字取来源。 / Resolve a source by name.
fn source_named(name: &str) -> Option<InfeasibilityEvidenceSource> {
    match name {
        "NativeIis" => Some(InfeasibilityEvidenceSource::NativeIis),
        "Farkas" => Some(InfeasibilityEvidenceSource::Farkas),
        "DeletionFilter" => Some(InfeasibilityEvidenceSource::DeletionFilter),
        "ElasticFilter" => Some(InfeasibilityEvidenceSource::ElasticFilter),
        "RebuildVerification" => Some(InfeasibilityEvidenceSource::RebuildVerification),
        "Unavailable" => Some(InfeasibilityEvidenceSource::Unavailable),
        _ => None,
    }
}

/// 按名字取可靠性。 / Resolve a reliability by name.
fn reliability_named(name: &str) -> Option<ProofReliability> {
    match name {
        "Exact" => Some(ProofReliability::Exact),
        "Reliable" => Some(ProofReliability::Reliable),
        "Heuristic" => Some(ProofReliability::Heuristic),
        "Unknown" => Some(ProofReliability::Unknown),
        _ => None,
    }
}

/// 按名字取完整度。 / Resolve a completeness by name.
fn completeness_named(name: &str) -> Option<ProofCompleteness> {
    match name {
        "Complete" => Some(ProofCompleteness::Complete),
        "Partial" => Some(ProofCompleteness::Partial),
        "Unavailable" => Some(ProofCompleteness::Unavailable),
        _ => None,
    }
}

/// 按名字取最小性。 / Resolve a minimality by name.
fn minimality_named(name: &str) -> Option<InfeasibilityMinimality> {
    match name {
        "Irreducible" => Some(InfeasibilityMinimality::Irreducible),
        "Partial" => Some(InfeasibilityMinimality::Partial),
        "NotChecked" => Some(InfeasibilityMinimality::NotChecked),
        _ => None,
    }
}

/// 造一个证据成员。 / Build one evidence member.
fn member_of(kind: &str, id: &str) -> Result<InfeasibilityEvidenceMember> {
    let id = id.to_owned();
    match kind {
        "Constraint" => Ok(InfeasibilityEvidenceMember::Constraint(id)),
        "LowerBound" => Ok(InfeasibilityEvidenceMember::LowerBound(id)),
        "UpperBound" => Ok(InfeasibilityEvidenceMember::UpperBound(id)),
        "Domain" => Ok(InfeasibilityEvidenceMember::Domain(id)),
        "Interval" => Ok(InfeasibilityEvidenceMember::Interval(id)),
        "Assumption" => Ok(InfeasibilityEvidenceMember::Assumption(id)),
        _ => Err(BridgeError::new("InvalidInput", "evidence probe failed validation: \
            unknown member kind")),
    }
}

/// 一份证据的描述。 / The description of one piece of evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InfeasibilityEvidenceSpec {
    /// 来源名。 / The source name.
    pub source: String,
    /// 可靠性名（默认 `Unknown`）。 / The reliability name.
    #[serde(default)]
    pub reliability: Option<String>,
    /// 完整度名（默认 `Unavailable`）。 / The completeness name.
    #[serde(default)]
    pub completeness: Option<String>,
    /// 最小性名（缺省走默认值）。 / The minimality name; absent uses the default.
    #[serde(default)]
    pub minimality: Option<String>,
    /// 稳定约束 ID 列表。 / The stable constraint IDs.
    #[serde(default)]
    pub constraint_ids: Vec<String>,
    /// 带类型的成员（`"Kind:id"` 形式）。 / The typed members, as `"Kind:id"`.
    #[serde(default)]
    pub members: Vec<String>,
    /// 不可用原因。 / The unavailable reason.
    #[serde(default)]
    pub unavailable_reason: Option<String>,
}

/// 证据探针请求。 / The evidence probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceProbeRequest {
    /// 要判定权威性的证据。 / The evidence whose authority is judged.
    pub evidence: InfeasibilityEvidenceSpec,
}

/// 证据探针结论。 / The evidence probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceProbeReport {
    /// 是否权威。 / Whether the evidence is authoritative.
    pub authoritative: bool,
    /// 约束 ID 个数。 / The number of constraint IDs.
    pub constraint_id_count: u32,
    /// 成员个数。 / The number of members.
    pub member_count: u32,
    /// 最小性名（回读线上的默认值）。 / The minimality name, reading back the default.
    pub minimality: String,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub native_and_farkas_are_authoritative_whatever_their_quality: bool,
    pub rebuild_needs_both_exact_and_complete: bool,
    pub the_filters_are_never_authoritative: bool,
    pub the_unavailable_source_is_never_authoritative: bool,
    pub minimality_defaults_to_not_checked: bool,
    pub the_two_member_sets_never_derive_from_each_other: bool,
    pub the_unavailable_reason_is_tied_to_no_state: bool,
    pub the_wire_names_are_screaming_snake_case: bool,
}

/// 造一份证据。 / Build one piece of evidence.
fn evidence_of(spec: &InfeasibilityEvidenceSpec) -> Result<InfeasibilityEvidence> {
    let source = source_named(&spec.source).ok_or_else(|| BridgeError::new("InvalidInput",
        "evidence probe failed validation: unknown evidence source"))?;
    let reliability = match spec.reliability.as_deref() {
        None => ProofReliability::Unknown,
        Some(name) => reliability_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
            "evidence probe failed validation: unknown reliability"))?,
    };
    let completeness = match spec.completeness.as_deref() {
        None => ProofCompleteness::Unavailable,
        Some(name) => completeness_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
            "evidence probe failed validation: unknown completeness"))?,
    };
    let minimality = match spec.minimality.as_deref() {
        // 缺省时走类型的默认值，这一条正是规则 5 的读数来源。 
        // Absent means the type's default, which is where rule five is read from.
        None => InfeasibilityMinimality::default(),
        Some(name) => minimality_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
            "evidence probe failed validation: unknown minimality"))?,
    };
    let mut members = BTreeSet::new();
    for entry in &spec.members {
        let (kind, id) = entry.split_once(':').ok_or_else(|| BridgeError::new("InvalidInput",
            "evidence probe failed validation: members read \"Kind:id\""))?;
        members.insert(member_of(kind, id)?);
    }
    Ok(InfeasibilityEvidence {
        source,
        reliability,
        completeness,
        constraint_ids: spec.constraint_ids.iter().cloned().collect(),
        members,
        minimality,
        computation_time: Duration::from_millis(0),
        unavailable_reason: spec.unavailable_reason.clone(),
    })
}

/// 最小性名（回读）。 / The minimality name, read back.
fn minimality_name(minimality: InfeasibilityMinimality) -> String {
    match minimality {
        InfeasibilityMinimality::Irreducible => "Irreducible",
        InfeasibilityMinimality::Partial => "Partial",
        InfeasibilityMinimality::NotChecked => "NotChecked",
    }.to_owned()
}

/// 走一遍证据探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_evidence_probe(request: &EvidenceProbeRequest) -> Result<EvidenceProbeReport> {
    if request.evidence.constraint_ids.len() > 16 || request.evidence.members.len() > 16 {
        return Err(BridgeError::new("InvalidInput", "evidence probe failed validation: \
            at most sixteen identities per set"));
    }
    let findings = canonical_findings();
    let evidence = evidence_of(&request.evidence)?;
    Ok(EvidenceProbeReport {
        authoritative: evidence.is_authoritative(),
        constraint_id_count: u32::try_from(evidence.constraint_ids.len()).unwrap_or(u32::MAX),
        member_count: u32::try_from(evidence.members.len()).unwrap_or(u32::MAX),
        minimality: minimality_name(evidence.minimality),
        native_and_farkas_are_authoritative_whatever_their_quality:
            findings.native_and_farkas_are_authoritative_whatever_their_quality,
        rebuild_needs_both_exact_and_complete: findings.rebuild_needs_both_exact_and_complete,
        the_filters_are_never_authoritative: findings.the_filters_are_never_authoritative,
        the_unavailable_source_is_never_authoritative:
            findings.the_unavailable_source_is_never_authoritative,
        minimality_defaults_to_not_checked: findings.minimality_defaults_to_not_checked,
        the_two_member_sets_never_derive_from_each_other:
            findings.the_two_member_sets_never_derive_from_each_other,
        the_unavailable_reason_is_tied_to_no_state:
            findings.the_unavailable_reason_is_tied_to_no_state,
        the_wire_names_are_screaming_snake_case: findings.the_wire_names_are_screaming_snake_case,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    native_and_farkas_are_authoritative_whatever_their_quality: bool,
    rebuild_needs_both_exact_and_complete: bool,
    the_filters_are_never_authoritative: bool,
    the_unavailable_source_is_never_authoritative: bool,
    minimality_defaults_to_not_checked: bool,
    the_two_member_sets_never_derive_from_each_other: bool,
    the_unavailable_reason_is_tied_to_no_state: bool,
    the_wire_names_are_screaming_snake_case: bool,
}

/// 造一份最小证据。 / Build a minimal piece of evidence.
fn bare(source: InfeasibilityEvidenceSource, reliability: ProofReliability,
    completeness: ProofCompleteness) -> InfeasibilityEvidence
{
    InfeasibilityEvidence {
        source,
        reliability,
        completeness,
        constraint_ids: BTreeSet::new(),
        members: BTreeSet::new(),
        minimality: InfeasibilityMinimality::default(),
        computation_time: Duration::from_millis(0),
        unavailable_reason: None,
    }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 原生 IIS 与 Farkas 无条件权威。 / Native IIS and Farkas are unconditionally authoritative.
        let native_and_farkas_are_authoritative_whatever_their_quality = {
            // 连最差的组合都算权威。 / Even the worst combination counts.
            [InfeasibilityEvidenceSource::NativeIis, InfeasibilityEvidenceSource::Farkas]
                .iter().all(|source| bare(*source, ProofReliability::Heuristic,
                    ProofCompleteness::Unavailable).is_authoritative())
        };
        // 2) CP 重建必须同时 EXACT 与 COMPLETE。 / A CP rebuild needs both exact and complete.
        let rebuild_needs_both_exact_and_complete = {
            let rebuild = |reliability, completeness| bare(
                InfeasibilityEvidenceSource::RebuildVerification, reliability, completeness)
                .is_authoritative();
            rebuild(ProofReliability::Exact, ProofCompleteness::Complete)
                && !rebuild(ProofReliability::Reliable, ProofCompleteness::Complete)
                && !rebuild(ProofReliability::Exact, ProofCompleteness::Partial)
                && !rebuild(ProofReliability::Heuristic, ProofCompleteness::Complete)
        };
        // 3) 两种过滤永不权威。 / The two filters are never authoritative.
        let the_filters_are_never_authoritative = {
            [InfeasibilityEvidenceSource::DeletionFilter, InfeasibilityEvidenceSource::ElasticFilter]
                .iter().all(|source| !bare(*source, ProofReliability::Exact,
                    ProofCompleteness::Complete).is_authoritative())
        };
        // 4) 不可用来源永不权威。 / The unavailable source is never authoritative.
        let the_unavailable_source_is_never_authoritative = {
            !bare(InfeasibilityEvidenceSource::Unavailable, ProofReliability::Exact,
                ProofCompleteness::Complete).is_authoritative()
        };
        // 5) minimality 默认 NOT_CHECKED。 / Minimality defaults to not checked.
        let minimality_defaults_to_not_checked = {
            InfeasibilityMinimality::default() == InfeasibilityMinimality::NotChecked
                && bare(InfeasibilityEvidenceSource::NativeIis, ProofReliability::Exact,
                    ProofCompleteness::Complete).minimality
                    == InfeasibilityMinimality::NotChecked
        };
        // 6) 两套集合互不推导。 / The two member sets never derive from each other.
        let the_two_member_sets_never_derive_from_each_other = {
            let mut only_ids = bare(InfeasibilityEvidenceSource::NativeIis,
                ProofReliability::Exact, ProofCompleteness::Complete);
            only_ids.constraint_ids.insert("c1".to_owned());
            let mut only_members = bare(InfeasibilityEvidenceSource::NativeIis,
                ProofReliability::Exact, ProofCompleteness::Complete);
            only_members.members.insert(InfeasibilityEvidenceMember::Constraint("c1".to_owned()));
            // 只填 ID 的证据成员集为空，只填成员的证据 ID 集为空。 
            // An evidence carrying only IDs has no members, and the other way round.
            only_ids.members.is_empty() && only_members.constraint_ids.is_empty()
                // 同名同值的两种写法是不同的键。 / The same identity in two shapes is two distinct keys.
                && only_ids.constraint_ids.len() == 1 && only_members.members.len() == 1
        };
        // 7) 不可用原因与状态不联动。 / The unavailable reason is tied to no state.
        let the_unavailable_reason_is_tied_to_no_state = {
            let mut authoritative = bare(InfeasibilityEvidenceSource::Farkas,
                ProofReliability::Exact, ProofCompleteness::Complete);
            authoritative.unavailable_reason = Some("nothing is missing".to_owned());
            // 权威证据可以带原因；不可用来源也可以不带原因。 
            // An authoritative evidence may carry a reason, and an unavailable one may carry none.
            authoritative.is_authoritative()
                && bare(InfeasibilityEvidenceSource::Unavailable, ProofReliability::Unknown,
                    ProofCompleteness::Unavailable).unavailable_reason.is_none()
        };
        // 8) 线上的枚举名是 SCREAMING_SNAKE_CASE。 / The wire enum names are screaming snake case.
        let the_wire_names_are_screaming_snake_case = {
            let encoded = serde_json::to_value(bare(InfeasibilityEvidenceSource::RebuildVerification,
                ProofReliability::Exact, ProofCompleteness::Complete))
                .expect("evidence serialises");
            // 证据本体是 camelCase，枚举值是 SCREAMING_SNAKE_CASE。 
            // The evidence body is camelCase while the enum values are screaming snake case.
            encoded.get("source").and_then(|value| value.as_str())
                == Some("REBUILD_VERIFICATION")
                && encoded.get("reliability").and_then(|value| value.as_str()) == Some("EXACT")
                && encoded.get("completeness").and_then(|value| value.as_str())
                    == Some("COMPLETE")
                && encoded.get("minimality").and_then(|value| value.as_str())
                    == Some("NOT_CHECKED")
                && encoded.get("constraintIds").is_some()
                && encoded.get("constraint_ids").is_none()
        };
        Findings {
            native_and_farkas_are_authoritative_whatever_their_quality,
            rebuild_needs_both_exact_and_complete,
            the_filters_are_never_authoritative,
            the_unavailable_source_is_never_authoritative,
            minimality_defaults_to_not_checked,
            the_two_member_sets_never_derive_from_each_other,
            the_unavailable_reason_is_tied_to_no_state,
            the_wire_names_are_screaming_snake_case,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(source: &str, reliability: Option<&str>, completeness: Option<&str>)
        -> InfeasibilityEvidenceSpec
    {
        InfeasibilityEvidenceSpec { source: source.to_owned(),
            reliability: reliability.map(str::to_owned),
            completeness: completeness.map(str::to_owned), minimality: None,
            constraint_ids: Vec::new(), members: Vec::new(), unavailable_reason: None }
    }

    fn run(spec: InfeasibilityEvidenceSpec) -> EvidenceProbeReport {
        verify_evidence_probe(&EvidenceProbeRequest { evidence: spec }).unwrap()
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = run(InfeasibilityEvidenceSpec {
            constraint_ids: vec!["c1".to_owned()],
            members: vec!["Constraint:c1".to_owned(), "LowerBound:x".to_owned()],
            ..spec("Farkas", Some("Exact"), Some("Complete")) });
        assert!(report.native_and_farkas_are_authoritative_whatever_their_quality);
        assert!(report.rebuild_needs_both_exact_and_complete);
        assert!(report.the_filters_are_never_authoritative);
        assert!(report.the_unavailable_source_is_never_authoritative);
        assert!(report.minimality_defaults_to_not_checked);
        assert!(report.the_two_member_sets_never_derive_from_each_other);
        assert!(report.the_unavailable_reason_is_tied_to_no_state);
        assert!(report.the_wire_names_are_screaming_snake_case);
        // Farkas 无条件权威，默认最小性是 NotChecked。 
        // Farkas is unconditionally authoritative and the default minimality is not checked.
        assert!(report.authoritative);
        assert_eq!(report.minimality, "NotChecked");
        assert_eq!(report.constraint_id_count, 1);
        assert_eq!(report.member_count, 2);
    }

    #[test]
    fn native_and_farkas_ignore_the_quality_labels() {
        for source in ["NativeIis", "Farkas"] {
            assert!(run(spec(source, Some("Heuristic"), Some("Unavailable"))).authoritative,
                "{source}");
            assert!(run(spec(source, None, None)).authoritative, "{source}");
        }
    }

    #[test]
    fn a_rebuild_needs_exact_and_complete() {
        assert!(run(spec("RebuildVerification", Some("Exact"), Some("Complete"))).authoritative);
        assert!(!run(spec("RebuildVerification", Some("Reliable"), Some("Complete")))
            .authoritative);
        assert!(!run(spec("RebuildVerification", Some("Exact"), Some("Partial"))).authoritative);
        assert!(!run(spec("RebuildVerification", None, None)).authoritative);
    }

    #[test]
    fn the_filters_and_the_unavailable_source_are_never_authoritative() {
        for source in ["DeletionFilter", "ElasticFilter", "Unavailable"] {
            assert!(!run(spec(source, Some("Exact"), Some("Complete"))).authoritative, "{source}");
        }
    }

    #[test]
    fn the_two_identity_sets_stay_separate() {
        let report = run(InfeasibilityEvidenceSpec { constraint_ids: vec!["c1".to_owned()],
            members: Vec::new(), ..spec("NativeIis", Some("Exact"), Some("Complete")) });
        assert_eq!(report.constraint_id_count, 1);
        assert_eq!(report.member_count, 0);
        let mirrored = run(InfeasibilityEvidenceSpec { constraint_ids: Vec::new(),
            members: vec!["Constraint:c1".to_owned()],
            ..spec("NativeIis", Some("Exact"), Some("Complete")) });
        assert_eq!(mirrored.constraint_id_count, 0);
        assert_eq!(mirrored.member_count, 1);
    }

    #[test]
    fn an_explicit_minimality_is_echoed() {
        assert_eq!(run(InfeasibilityEvidenceSpec { minimality: Some("Irreducible".to_owned()),
            ..spec("NativeIis", Some("Exact"), Some("Complete")) }).minimality, "Irreducible");
        assert_eq!(run(InfeasibilityEvidenceSpec { minimality: Some("Partial".to_owned()),
            ..spec("NativeIis", Some("Exact"), Some("Complete")) }).minimality, "Partial");
    }

    #[test]
    fn bad_input_is_refused() {
        let bad = |spec: InfeasibilityEvidenceSpec| verify_evidence_probe(
            &EvidenceProbeRequest { evidence: spec }).unwrap_err().message;
        assert!(bad(spec("Nearly", None, None)).contains("unknown evidence source"));
        assert!(bad(spec("Farkas", Some("Nearly"), None)).contains("unknown reliability"));
        assert!(bad(spec("Farkas", None, Some("Nearly"))).contains("unknown completeness"));
        assert!(bad(InfeasibilityEvidenceSpec { minimality: Some("Nearly".to_owned()),
            ..spec("Farkas", None, None) }).contains("unknown minimality"));
        assert!(bad(InfeasibilityEvidenceSpec { members: vec!["Constraint".to_owned()],
            ..spec("Farkas", None, None) }).contains("Kind:id"));
        assert!(bad(InfeasibilityEvidenceSpec { members: vec!["Nearly:c1".to_owned()],
            ..spec("Farkas", None, None) }).contains("unknown member kind"));
        assert!(bad(InfeasibilityEvidenceSpec {
            constraint_ids: (0..17).map(|index| format!("c{index}")).collect(),
            ..spec("Farkas", None, None) }).contains("at most sixteen"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = run(spec("Farkas", Some("Exact"), Some("Complete")));
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"constraintIdCount\""), "{json}");
        assert!(json.contains("\"rebuildNeedsBothExactAndComplete\""), "{json}");
        assert!(!json.contains("constraint_id_count"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<EvidenceProbeReport>(&copy.to_string()).is_err());
    }
}
