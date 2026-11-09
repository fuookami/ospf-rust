//! 续挖第 107 单：**最小阻塞集合的"已证明最小"只读一个字段**——`verified()` 等价于
//! `minimality == Irreducible`，其余七个字段（状态、有效性、复验记录、成员、背景阻塞、提取层级、target）
//! **一个都不参与**：于是"没有成员、没有复验记录、层级为 `Unavailable`、还声称是背景阻塞"的集合照样"已证明最小"；
//! 而 `ConflictMinimality::is_verified()` 与 `as_solver_minimality()` 是同一套三分（`Irreducible` / `Partial` /
//! `NotChecked`，默认 `NotChecked`），`ConflictExtractionTier::is_native()` 只认 `NativeUnsatCore`
//! （`analysis/conflict.rs` 的 `MinimalBlockingSet` / `ConflictMinimality` / `ConflictExtractionTier`）
//! Continuation, delivery 107: **the minimal blocking set's "proven minimal" reads a single field** — `verified()` is
//! exactly `minimality == Irreducible`, and the other seven fields (status, validity, verification records, members,
//! background blocking, extraction tier, target) **take no part**: so a set with no members, no verification records, an
//! `Unavailable` tier and a claim of background blocking is still "proven minimal"; meanwhile
//! `ConflictMinimality::is_verified()` and `as_solver_minimality()` share one three-way split (`Irreducible` / `Partial` /
//! `NotChecked`, defaulting to `NotChecked`) and `ConflictExtractionTier::is_native()` accepts only `NativeUnsatCore`
//! (`MinimalBlockingSet` / `ConflictMinimality` / `ConflictExtractionTier` in `analysis/conflict.rs`).
//!
//! 第 106 单量的是指纹，这一单量**冲突面的最小性判据**。八条实测：
//!
//! 1. **`verified()` 只认 `Irreducible`**（`verified_reads_only_the_minimality()`）。
//! 2. **`is_verified()` 与 `as_solver_minimality()` 是同一套三分**（`the_minimality_three_way_split_is_shared()`），
//!    默认值是 `NotChecked`。
//! 3. **`verified()` 不看 `status`**（`verified_ignores_the_status()`）。
//! 4. **`verified()` 不看 `validity`**（`verified_ignores_the_validity()`）。
//! 5. **`verified()` 不看复验记录**（`verified_ignores_the_verifications()`）：空记录照样"已证明"。
//! 6. **`verified()` 不看成员**（`verified_ignores_the_members()`）：空成员照样"已证明"。
//! 7. **`verified()` 不看背景阻塞与提取层级**（`verified_ignores_the_background_and_the_tier()`）。
//! 8. **`is_native()` 只认 `NativeUnsatCore`**（`only_the_native_tier_is_native()`）。
//!
//! Delivery 106 measured the fingerprints; this one measures **the minimality verdict of the conflict face**. **`verified()`
//! reads only `Irreducible`.** **`is_verified()` and `as_solver_minimality()` share one split**, whose default is
//! `NotChecked`. **`verified()` ignores the status**, **ignores the validity**, **ignores the verification records**,
//! **ignores the members**, and **ignores both the background blocking and the tier**. And **`is_native()` accepts only the
//! native tier.**

use ospf_rust_core::analysis::activation::ConflictExtractionTier;
use ospf_rust_core::analysis::conflict::{
    ConflictMinimality, ConflictValidity, ConflictVerification, MinimalBlockingSet,
};
use ospf_rust_core::analysis::{AnalysisStatus, DiagnosticSource, ObjectiveTarget};
use ospf_rust_core::solver::StableConstraintId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条复验记录的描述。 / The description of one verification record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerificationSpec {
    /// 候选来源的约束身份。 / The candidate constraint identity.
    pub id: String,
    /// 删除后的状态名。 / The status name after removal.
    #[serde(default)]
    pub status: Option<String>,
    /// 是否移除。 / Whether it was removed.
    #[serde(default)]
    pub removed: Option<bool>,
    /// 是否保留在当前冲突中。 / Whether it remains in the conflict.
    #[serde(default)]
    pub retained: Option<bool>,
    /// target 是否固定。 / Whether the target stayed fixed.
    #[serde(default)]
    pub target_fixed: Option<bool>,
}

/// 最小阻塞集合探针请求。 / The minimal-blocking-set probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BlockingSetRequest {
    /// 最小性名（缺省走默认值 `NotChecked`）。 / The minimality name; absent uses the default.
    #[serde(default)]
    pub minimality: Option<String>,
    /// 状态名（默认 `Unreachable`）。 / The status name.
    #[serde(default)]
    pub status: Option<String>,
    /// 有效性名（默认 `Verified`）。 / The validity name.
    #[serde(default)]
    pub validity: Option<String>,
    /// 成员约束身份。 / The member constraint identities.
    #[serde(default)]
    pub members: Vec<String>,
    /// 复验记录。 / The verification records.
    #[serde(default)]
    pub verifications: Vec<VerificationSpec>,
    /// 阻塞是否来自固定背景。 / Whether blocking comes from the fixed background.
    #[serde(default)]
    pub background_blocking: Option<bool>,
    /// 提取层级名（默认 `RepeatedSolving`）。 / The extraction tier name.
    #[serde(default)]
    pub extraction_tier: Option<String>,
}

/// 最小阻塞集合探针结论。 / The minimal-blocking-set probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BlockingSetReport {
    /// `verified()` 的读数。 / What `verified()` says.
    pub verified: bool,
    /// 回读的最小性名。 / The minimality name, read back.
    pub minimality: String,
    /// 回读的层级名。 / The tier name, read back.
    pub extraction_tier: String,
    /// 该层级是否原生。 / Whether the tier is native.
    pub native_tier: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub verified_reads_only_the_minimality: bool,
    pub the_minimality_three_way_split_is_shared: bool,
    pub verified_ignores_the_status: bool,
    pub verified_ignores_the_validity: bool,
    pub verified_ignores_the_verifications: bool,
    pub verified_ignores_the_members: bool,
    pub verified_ignores_the_background_and_the_tier: bool,
    pub only_the_native_tier_is_native: bool,
}

/// 按名字取最小性。 / Resolve a minimality by name.
fn minimality_named(name: &str) -> Option<ConflictMinimality> {
    match name {
        "Irreducible" => Some(ConflictMinimality::Irreducible),
        "Partial" => Some(ConflictMinimality::Partial),
        "NotChecked" => Some(ConflictMinimality::NotChecked),
        _ => None,
    }
}

/// 按名字取状态。 / Resolve a status by name.
fn status_named(name: &str) -> Option<AnalysisStatus> {
    match name {
        "Reachable" => Some(AnalysisStatus::Reachable),
        "Unreachable" => Some(AnalysisStatus::Unreachable),
        "Unknown" => Some(AnalysisStatus::Unknown),
        "Unsupported" => Some(AnalysisStatus::Unsupported),
        _ => None,
    }
}

/// 按名字取有效性。 / Resolve a validity by name.
fn validity_named(name: &str) -> Option<ConflictValidity> {
    match name {
        "Verified" => Some(ConflictValidity::Verified),
        "Unknown" => Some(ConflictValidity::Unknown),
        _ => None,
    }
}

/// 按名字取层级。 / Resolve a tier by name.
fn tier_named(name: &str) -> Option<ConflictExtractionTier> {
    match name {
        "NativeUnsatCore" => Some(ConflictExtractionTier::NativeUnsatCore),
        "AssumptionExtraction" => Some(ConflictExtractionTier::AssumptionExtraction),
        "RepeatedSolving" => Some(ConflictExtractionTier::RepeatedSolving),
        "Unavailable" => Some(ConflictExtractionTier::Unavailable),
        _ => None,
    }
}

/// 最小性名（回读）。 / The minimality name, read back.
fn minimality_name(minimality: ConflictMinimality) -> String {
    match minimality {
        ConflictMinimality::Irreducible => "Irreducible",
        ConflictMinimality::Partial => "Partial",
        ConflictMinimality::NotChecked => "NotChecked",
    }.to_owned()
}

/// 层级名（回读）。 / The tier name, read back.
fn tier_name(tier: ConflictExtractionTier) -> String {
    match tier {
        ConflictExtractionTier::NativeUnsatCore => "NativeUnsatCore",
        ConflictExtractionTier::AssumptionExtraction => "AssumptionExtraction",
        ConflictExtractionTier::RepeatedSolving => "RepeatedSolving",
        ConflictExtractionTier::Unavailable => "Unavailable",
    }.to_owned()
}

/// 造一个成员来源。 / Build one member source.
fn member_of(id: &str) -> DiagnosticSource {
    DiagnosticSource::Constraint { id: StableConstraintId(id.to_owned()) }
}

/// 造一条复验记录。 / Build one verification record.
fn verification_of(spec: &VerificationSpec) -> Result<ConflictVerification> {
    let status = match spec.status.as_deref() {
        None => AnalysisStatus::Unknown,
        Some(name) => status_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
            "blocking set probe failed validation: unknown verification status"))?,
    };
    Ok(ConflictVerification {
        source: member_of(&spec.id),
        status,
        removed: spec.removed.unwrap_or(false),
        retained: spec.retained.unwrap_or(true),
        target_fixed: spec.target_fixed.unwrap_or(true),
        message: None,
    })
}

/// 造一个最小阻塞集合。 / Build one minimal blocking set.
fn blocking_set_of(request: &BlockingSetRequest) -> Result<MinimalBlockingSet> {
    let invalid = |what: &str| BridgeError::new("InvalidInput",
        format!("blocking set probe failed validation: unknown {what}"));
    let target = ObjectiveTarget::at_least("objective", 0.0)
        .map_err(|error| BridgeError::new("InvalidInput", error))?;
    Ok(MinimalBlockingSet {
        target,
        members: request.members.iter().map(|id| member_of(id)).collect(),
        status: match request.status.as_deref() {
            None => AnalysisStatus::Unreachable,
            Some(name) => status_named(name).ok_or_else(|| invalid("status"))?,
        },
        minimality: match request.minimality.as_deref() {
            None => ConflictMinimality::default(),
            Some(name) => minimality_named(name).ok_or_else(|| invalid("minimality"))?,
        },
        validity: match request.validity.as_deref() {
            None => ConflictValidity::Verified,
            Some(name) => validity_named(name).ok_or_else(|| invalid("validity"))?,
        },
        verifications: request.verifications.iter().map(verification_of)
            .collect::<Result<Vec<_>>>()?,
        background_blocking: request.background_blocking.unwrap_or(false),
        extraction_tier: match request.extraction_tier.as_deref() {
            None => ConflictExtractionTier::RepeatedSolving,
            Some(name) => tier_named(name).ok_or_else(|| invalid("extraction tier"))?,
        },
    })
}

/// 走一遍最小阻塞集合探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_blocking_set(request: &BlockingSetRequest) -> Result<BlockingSetReport> {
    if request.members.len() > 8 || request.verifications.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "blocking set probe failed validation: \
            at most eight entries per list"));
    }
    let findings = canonical_findings();
    let set = blocking_set_of(request)?;
    Ok(BlockingSetReport {
        verified: set.verified(),
        minimality: minimality_name(set.minimality),
        extraction_tier: tier_name(set.extraction_tier),
        native_tier: set.extraction_tier.is_native(),
        verified_reads_only_the_minimality: findings.verified_reads_only_the_minimality,
        the_minimality_three_way_split_is_shared:
            findings.the_minimality_three_way_split_is_shared,
        verified_ignores_the_status: findings.verified_ignores_the_status,
        verified_ignores_the_validity: findings.verified_ignores_the_validity,
        verified_ignores_the_verifications: findings.verified_ignores_the_verifications,
        verified_ignores_the_members: findings.verified_ignores_the_members,
        verified_ignores_the_background_and_the_tier:
            findings.verified_ignores_the_background_and_the_tier,
        only_the_native_tier_is_native: findings.only_the_native_tier_is_native,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    verified_reads_only_the_minimality: bool,
    the_minimality_three_way_split_is_shared: bool,
    verified_ignores_the_status: bool,
    verified_ignores_the_validity: bool,
    verified_ignores_the_verifications: bool,
    verified_ignores_the_members: bool,
    verified_ignores_the_background_and_the_tier: bool,
    only_the_native_tier_is_native: bool,
}

/// 一份全默认的请求。 / A fully defaulted request.
fn plain() -> BlockingSetRequest {
    BlockingSetRequest { minimality: Some("Irreducible".to_owned()), status: None,
        validity: None, members: vec!["c1".to_owned()],
        verifications: vec![VerificationSpec { id: "c1".to_owned(), status: None,
            removed: Some(false), retained: Some(true), target_fixed: Some(true) }],
        background_blocking: None, extraction_tier: None }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        let verified = |request: BlockingSetRequest| {
            blocking_set_of(&request).expect("set builds").verified()
        };
        let with = |minimality: &str| BlockingSetRequest {
            minimality: Some(minimality.to_owned()), ..plain() };
        // 1) verified 只认 Irreducible。 / verified only accepts Irreducible.
        let verified_reads_only_the_minimality = {
            verified(with("Irreducible"))
                && !verified(with("Partial"))
                && !verified(with("NotChecked"))
                // 缺省即 NotChecked，于是默认不"已证明"。 
                // Absent means NotChecked, so the default is not proven.
                && !verified(BlockingSetRequest { minimality: None, ..plain() })
        };
        // 2) 三分与 solver 术语是同一套。 / The split matches the solver terminology.
        let the_minimality_three_way_split_is_shared = {
            ConflictMinimality::default() == ConflictMinimality::NotChecked
                && ConflictMinimality::Irreducible.is_verified()
                && !ConflictMinimality::Partial.is_verified()
                && !ConflictMinimality::NotChecked.is_verified()
                && ConflictMinimality::Irreducible.as_solver_minimality()
                    == ospf_rust_core::solver::report::InfeasibilityMinimality::Irreducible
                && ConflictMinimality::Partial.as_solver_minimality()
                    == ospf_rust_core::solver::report::InfeasibilityMinimality::Partial
                && ConflictMinimality::NotChecked.as_solver_minimality()
                    == ospf_rust_core::solver::report::InfeasibilityMinimality::NotChecked
        };
        // 3) 状态不参与。 / The status takes no part.
        let verified_ignores_the_status = {
            ["Reachable", "Unreachable", "Unknown", "Unsupported"].iter().all(|name| {
                verified(BlockingSetRequest { status: Some((*name).to_owned()), ..plain() })
            })
        };
        // 4) 有效性不参与。 / The validity takes no part.
        let verified_ignores_the_validity = {
            verified(BlockingSetRequest { validity: Some("Verified".to_owned()), ..plain() })
                && verified(BlockingSetRequest { validity: Some("Unknown".to_owned()), ..plain() })
        };
        // 5) 复验记录不参与。 / The verification records take no part.
        let verified_ignores_the_verifications = {
            // 空记录照样已证明；记录里说"保留了"也一样。 
            // An empty list is still proven, and so is a record claiming retention.
            verified(BlockingSetRequest { verifications: Vec::new(), ..plain() })
                && verified(BlockingSetRequest { verifications: vec![VerificationSpec {
                    id: "c1".to_owned(), status: Some("Reachable".to_owned()),
                    removed: Some(false), retained: Some(true), target_fixed: Some(false) }],
                    ..plain() })
        };
        // 6) 成员不参与。 / The members take no part.
        let verified_ignores_the_members = {
            verified(BlockingSetRequest { members: Vec::new(), ..plain() })
        };
        // 7) 背景阻塞与层级不参与。 / The background flag and the tier take no part.
        let verified_ignores_the_background_and_the_tier = {
            verified(BlockingSetRequest { background_blocking: Some(true),
                extraction_tier: Some("Unavailable".to_owned()), members: Vec::new(),
                verifications: Vec::new(), ..plain() })
        };
        // 8) is_native 只认原生层级。 / is_native only accepts the native tier.
        let only_the_native_tier_is_native = {
            ConflictExtractionTier::NativeUnsatCore.is_native()
                && !ConflictExtractionTier::AssumptionExtraction.is_native()
                && !ConflictExtractionTier::RepeatedSolving.is_native()
                && !ConflictExtractionTier::Unavailable.is_native()
        };
        Findings {
            verified_reads_only_the_minimality,
            the_minimality_three_way_split_is_shared,
            verified_ignores_the_status,
            verified_ignores_the_validity,
            verified_ignores_the_verifications,
            verified_ignores_the_members,
            verified_ignores_the_background_and_the_tier,
            only_the_native_tier_is_native,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: BlockingSetRequest) -> BlockingSetReport {
        verify_blocking_set(&request).unwrap()
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(plain());
        assert!(report.verified);
        assert_eq!(report.minimality, "Irreducible");
        assert_eq!(report.extraction_tier, "RepeatedSolving");
        assert!(!report.native_tier);
        assert!(report.verified_reads_only_the_minimality);
        assert!(report.the_minimality_three_way_split_is_shared);
        assert!(report.verified_ignores_the_status);
        assert!(report.verified_ignores_the_validity);
        assert!(report.verified_ignores_the_verifications);
        assert!(report.verified_ignores_the_members);
        assert!(report.verified_ignores_the_background_and_the_tier);
        assert!(report.only_the_native_tier_is_native);
    }

    #[test]
    fn only_irreducible_is_verified() {
        for name in ["Irreducible"] {
            assert!(ask(BlockingSetRequest { minimality: Some(name.to_owned()), ..plain() })
                .verified, "{name}");
        }
        for name in ["Partial", "NotChecked"] {
            assert!(!ask(BlockingSetRequest { minimality: Some(name.to_owned()), ..plain() })
                .verified, "{name}");
        }
        // 缺省即 NotChecked。 / Absent means NotChecked.
        let default = ask(BlockingSetRequest { minimality: None, ..plain() });
        assert!(!default.verified);
        assert_eq!(default.minimality, "NotChecked");
    }

    #[test]
    fn nothing_else_participates() {
        // 空成员 + 空记录 + 背景阻塞 + Unavailable 层级，仍然"已证明最小"。 
        // Empty members and records with background blocking and an unavailable tier is still proven.
        let absurd = ask(BlockingSetRequest { members: Vec::new(), verifications: Vec::new(),
            background_blocking: Some(true), extraction_tier: Some("Unavailable".to_owned()),
            status: Some("Reachable".to_owned()), validity: Some("Unknown".to_owned()),
            ..plain() });
        assert!(absurd.verified);
        assert_eq!(absurd.extraction_tier, "Unavailable");
        assert!(!absurd.native_tier);
    }

    #[test]
    fn the_native_tier_is_the_only_native_one() {
        let native = ask(BlockingSetRequest {
            extraction_tier: Some("NativeUnsatCore".to_owned()), ..plain() });
        assert!(native.native_tier);
        for name in ["AssumptionExtraction", "RepeatedSolving", "Unavailable"] {
            assert!(!ask(BlockingSetRequest { extraction_tier: Some(name.to_owned()),
                ..plain() }).native_tier, "{name}");
        }
    }

    #[test]
    fn bad_input_is_refused() {
        let bad = |request: BlockingSetRequest| verify_blocking_set(&request).unwrap_err().message;
        assert!(bad(BlockingSetRequest { minimality: Some("Nearly".to_owned()), ..plain() })
            .contains("unknown minimality"));
        assert!(bad(BlockingSetRequest { status: Some("Nearly".to_owned()), ..plain() })
            .contains("unknown status"));
        assert!(bad(BlockingSetRequest { validity: Some("Nearly".to_owned()), ..plain() })
            .contains("unknown validity"));
        assert!(bad(BlockingSetRequest { extraction_tier: Some("Nearly".to_owned()),
            ..plain() }).contains("unknown extraction tier"));
        assert!(bad(BlockingSetRequest { verifications: vec![VerificationSpec {
            id: "c1".to_owned(), status: Some("Nearly".to_owned()), removed: None,
            retained: None, target_fixed: None }], ..plain() })
            .contains("unknown verification status"));
        assert!(bad(BlockingSetRequest { members: (0..9).map(|index| format!("c{index}"))
            .collect(), ..plain() }).contains("at most eight entries"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(plain());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"extractionTier\""), "{json}");
        assert!(json.contains("\"verifiedIgnoresTheMembers\""), "{json}");
        assert!(!json.contains("extraction_tier"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<BlockingSetReport>(&copy.to_string()).is_err());
    }
}
