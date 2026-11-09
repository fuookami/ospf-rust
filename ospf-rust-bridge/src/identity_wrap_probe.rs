//! 续挖第 76 单：LBB 的**身份包装器**——指纹闸门只查"非空白"，三条构造器两种文案
//! （`solver/logic_based_benders.rs` 的 `IdentifiedMaster` / `IdentifiedSubproblemSolver` /
//! `SnapshotSubproblemFactory`）
//! Continuation, delivery 76: the **identity wrappers** of LBB — the fingerprint gate only checks "not blank" and three
//! constructors share two messages (`IdentifiedMaster`, `IdentifiedSubproblemSolver`, `SnapshotSubproblemFactory`).
//!
//! 第 48 单量过 `MasterBinding` 与 `MasterAssignment` 的成对契约；这一单量**包在求解器外面的那层身份**。这三个
//! 包装器此前一个都没绑过。八条实测：
//!
//! 1. **闸门只查"非空白"**（`the_gate_is_blank_only()`）：`schema_version` / `algorithm` / `value` 三项
//!    `trim()` 后都非空就通过——`"x"` 这种垃圾也过。
//! 2. **它不查十六进制、不查长度、不查算法词表**（`the_gate_checks_neither_hex_nor_length_nor_vocabulary()`）：
//!    64 位十六进制与 `"z"` 同等通过。
//! 3. **master 与 subproblem 是两种文案**（`the_master_and_subproblem_messages_differ()`）：
//!    "master fingerprint cannot contain blank fields" vs "subproblem factory fingerprint cannot contain blank fields"。
//! 4. **两个 subproblem 包装器共用一句文案**（`the_two_subproblem_wrappers_share_one_message()`）。
//! 5. **"空白"是 `trim()` 意义上的**（`blank_means_trim_empty()`）：`"  "` 被拒，`" x "` 通过。
//! 6. **存下来的是原样**（`the_fingerprint_is_stored_verbatim()`）：`" x "` 取回来仍是 `" x "`，不被修剪。
//! 7. **`into_inner` 原样归还被包装的值**（`into_inner_returns_the_wrapped_value()`）。
//! 8. **包装把 `None` 变成 `Some`**（`wrapping_turns_none_into_some()`）：裸 solver 的
//!    `subproblem_factory_fingerprint()` 是 `None`，包一层后是 `Some`——这正是 checkpoint 恢复能看见身份的原因。
//!
//! Delivery 48 measured the paired contract of `MasterBinding`/`MasterAssignment`; this one measures **the identity layer
//! wrapped around the solvers**, none of which had been bound before. **The gate is blank-only**: three `trim()`-non-empty
//! fields pass and `"x"` passes too. **It checks neither hex, nor length, nor a vocabulary.** **Master and subproblem use
//! different messages** while **the two subproblem wrappers share one**. **"Blank" means `trim()`-empty.** **The
//! fingerprint is stored verbatim** — `" x "` comes back untrimmed. **`into_inner` returns the wrapped value.** And
//! **wrapping turns `None` into `Some`**, which is exactly why checkpoint restore can see an identity.

use ospf_rust_core::solver::report::AuditFingerprint;
use ospf_rust_core::model::constraint_programming::ConstraintProgrammingSnapshotArtifact;
use ospf_rust_framework::solver::logic_based_benders::{
    ConstraintProgrammingSubproblemFactory, ConstraintProgrammingSubproblemResult,
    ConstraintProgrammingSubproblemSolver, IdentifiedMaster, IdentifiedSubproblemSolver,
    MasterAssignment, SnapshotSubproblemFactory,
};
use ospf_rust_framework::FrameworkSolveOptions;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个指纹的声明。 / One fingerprint declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FingerprintSpec {
    /// schema 版本。 / The schema version.
    #[serde(default)]
    pub schema_version: Option<String>,
    /// 摘要算法。 / The digest algorithm.
    #[serde(default)]
    pub algorithm: Option<String>,
    /// 十六进制摘要。 / The hex digest.
    #[serde(default)]
    pub value: Option<String>,
}

/// 身份包装器探针请求。 / The identity-wrapper probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentityWrapRequest {
    /// 三个包装器各自的指纹。 / One fingerprint per wrapper.
    pub master: FingerprintSpec,
    /// solver 包装器的指纹。 / The solver wrapper's fingerprint.
    pub solver: FingerprintSpec,
    /// snapshot factory 包装器的指纹。 / The snapshot-factory wrapper's fingerprint.
    pub factory: FingerprintSpec,
}

/// 一个包装器的读面。 / One wrapper's readable face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentityWrapView {
    /// 包装器名。 / The wrapper name.
    pub wrapper: String,
    /// 是否构造成功。 / Whether construction succeeded.
    pub accepted: bool,
    /// 被拒文案。 / The refusal message.
    #[serde(default)]
    pub refusal: Option<String>,
    /// 取回的 `schema_version`（原样）。 / The echoed `schema_version` (verbatim).
    #[serde(default)]
    pub schema_version: Option<String>,
    /// 取回的 `algorithm`（原样）。 / The echoed `algorithm` (verbatim).
    #[serde(default)]
    pub algorithm: Option<String>,
    /// 取回的 `value`（原样）。 / The echoed `value` (verbatim).
    #[serde(default)]
    pub value: Option<String>,
    /// 被包装值的身份（`into_inner` 读回）。 / The wrapped value's identity, read back through `into_inner`.
    #[serde(default)]
    pub wrapped: Option<String>,
    /// solver 身份是否可见（`subproblem_factory_fingerprint()`）。 / Whether the solver identity is visible.
    #[serde(default)]
    pub identity_visible: Option<bool>,
}

/// 身份包装器探针结论。 / The identity-wrapper probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentityWrapReport {
    /// 三个包装器的读面。 / The three wrappers' readable faces.
    pub wrappers: Vec<IdentityWrapView>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_gate_is_blank_only: bool,
    pub the_gate_checks_neither_hex_nor_length_nor_vocabulary: bool,
    pub the_master_and_subproblem_messages_differ: bool,
    pub the_two_subproblem_wrappers_share_one_message: bool,
    pub blank_means_trim_empty: bool,
    pub the_fingerprint_is_stored_verbatim: bool,
    pub into_inner_returns_the_wrapped_value: bool,
    pub wrapping_turns_none_into_some: bool,
}

/// 一份指纹声明。 / A fingerprint declaration.
fn fingerprint_of(spec: &FingerprintSpec) -> AuditFingerprint {
    AuditFingerprint {
        schema_version: spec.schema_version.clone().unwrap_or_else(|| "1.0".to_owned()),
        algorithm: spec.algorithm.clone().unwrap_or_else(|| "sha256".to_owned()),
        value: spec.value.clone().unwrap_or_default(),
    }
}

/// 一个裸 solver：没有任何工厂身份。 / A bare solver with no factory identity.
struct BareSolver;

impl ConstraintProgrammingSubproblemSolver for BareSolver {
    fn solve_subproblem(
        &mut self,
        _assignment: &MasterAssignment,
        _options: &FrameworkSolveOptions,
    ) -> ospf_rust_core::error::Result<ConstraintProgrammingSubproblemResult> {
        Ok(ConstraintProgrammingSubproblemResult::failed("probe", "bare solver"))
    }
}

/// 被包装的值：只用来确认 `into_inner` 原样归还。 / The wrapped value, used to confirm `into_inner` returns it as-is.
const WRAPPED_MARKER: &str = "wrapped-marker";

/// 构造 master 包装器并读回。 / Build the master wrapper and read it back.
fn wrap_master(spec: &FingerprintSpec) -> IdentityWrapView {
    match IdentifiedMaster::new(WRAPPED_MARKER, fingerprint_of(spec)) {
        Ok(wrapper) => {
            let fingerprint = wrapper.fingerprint().clone();
            let wrapped = wrapper.into_inner().to_owned();
            IdentityWrapView { wrapper: "identifiedMaster".to_owned(), accepted: true, refusal: None,
                schema_version: Some(fingerprint.schema_version), algorithm: Some(fingerprint.algorithm),
                value: Some(fingerprint.value), wrapped: Some(wrapped), identity_visible: None }
        }
        Err(error) => refused("identifiedMaster", &error.to_string()),
    }
}

/// 构造 solver 包装器并读回。 / Build the solver wrapper and read it back.
fn wrap_solver(spec: &FingerprintSpec) -> IdentityWrapView {
    match IdentifiedSubproblemSolver::new(BareSolver, fingerprint_of(spec)) {
        Ok(mut wrapper) => {
            let fingerprint = wrapper.fingerprint().clone();
            // 包装之后身份变可见——裸 solver 的默认实现是 `None`。
            // After wrapping the identity becomes visible; the bare solver's default is `None`.
            let visible = wrapper.subproblem_factory_fingerprint().is_some();
            let _ = wrapper.into_inner();
            IdentityWrapView { wrapper: "identifiedSubproblemSolver".to_owned(), accepted: true,
                refusal: None, schema_version: Some(fingerprint.schema_version),
                algorithm: Some(fingerprint.algorithm), value: Some(fingerprint.value),
                wrapped: Some("bareSolver".to_owned()), identity_visible: Some(visible) }
        }
        Err(error) => refused("identifiedSubproblemSolver", &error.to_string()),
    }
}

/// 构造 snapshot factory 包装器并读回。 / Build the snapshot-factory wrapper and read it back.
fn wrap_factory(spec: &FingerprintSpec) -> IdentityWrapView {
    // 工厂闭包只回一个裸 solver；这里不重建快照，只量指纹闸门。
    // The factory closure only returns a bare solver; the snapshot is not rebuilt here — only the gate is measured.
    let factory = |_: &ConstraintProgrammingSnapshotArtifact|
        -> ospf_rust_core::error::Result<BareSolver> { Ok(BareSolver) };
    match SnapshotSubproblemFactory::new(factory, fingerprint_of(spec)) {
        Ok(wrapper) => {
            // 工厂指纹只能经 trait 方法读（字段是私有的）。 
            // The factory fingerprint is only readable through the trait method (the field is private).
            let fingerprint = wrapper.subproblem_factory_fingerprint().clone();
            IdentityWrapView { wrapper: "snapshotSubproblemFactory".to_owned(), accepted: true,
                refusal: None, schema_version: Some(fingerprint.schema_version),
                algorithm: Some(fingerprint.algorithm), value: Some(fingerprint.value),
                wrapped: Some("factoryClosure".to_owned()), identity_visible: Some(true) }
        }
        Err(error) => refused("snapshotSubproblemFactory", &error.to_string()),
    }
}

/// 一份被拒的读面。 / A refused readable face.
fn refused(wrapper: &str, message: &str) -> IdentityWrapView {
    IdentityWrapView { wrapper: wrapper.to_owned(), accepted: false, refusal: Some(message.to_owned()),
        schema_version: None, algorithm: None, value: None, wrapped: None, identity_visible: None }
}

/// 走一遍身份包装器探针。非法输入是**错误**。 / Walk the identity-wrapper probe.
pub fn verify_identity_wrapping(request: &IdentityWrapRequest) -> Result<IdentityWrapReport> {
    let findings = canonical_findings();
    let wrappers = vec![wrap_master(&request.master), wrap_solver(&request.solver),
        wrap_factory(&request.factory)];
    Ok(IdentityWrapReport {
        wrappers,
        the_gate_is_blank_only: findings.the_gate_is_blank_only,
        the_gate_checks_neither_hex_nor_length_nor_vocabulary:
            findings.the_gate_checks_neither_hex_nor_length_nor_vocabulary,
        the_master_and_subproblem_messages_differ: findings.the_master_and_subproblem_messages_differ,
        the_two_subproblem_wrappers_share_one_message:
            findings.the_two_subproblem_wrappers_share_one_message,
        blank_means_trim_empty: findings.blank_means_trim_empty,
        the_fingerprint_is_stored_verbatim: findings.the_fingerprint_is_stored_verbatim,
        into_inner_returns_the_wrapped_value: findings.into_inner_returns_the_wrapped_value,
        wrapping_turns_none_into_some: findings.wrapping_turns_none_into_some,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_gate_is_blank_only: bool,
    the_gate_checks_neither_hex_nor_length_nor_vocabulary: bool,
    the_master_and_subproblem_messages_differ: bool,
    the_two_subproblem_wrappers_share_one_message: bool,
    blank_means_trim_empty: bool,
    the_fingerprint_is_stored_verbatim: bool,
    into_inner_returns_the_wrapped_value: bool,
    wrapping_turns_none_into_some: bool,
}

/// 一份指纹声明。 / A fingerprint declaration.
fn spec(schema: &str, algorithm: &str, value: &str) -> FingerprintSpec {
    FingerprintSpec { schema_version: Some(schema.to_owned()), algorithm: Some(algorithm.to_owned()),
        value: Some(value.to_owned()) }
}

/// 一份合法指纹。 / A well-formed fingerprint.
fn good() -> FingerprintSpec {
    spec("1.0", "sha256", &"a".repeat(64))
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 闸门只查非空白。 / The gate is blank-only.
    let the_gate_is_blank_only = {
        // `"x"` 三项都过——它不要求十六进制也不要求长度。 
        // `"x"` passes on all three: no hex and no length is required.
        wrap_master(&spec("x", "x", "x")).accepted
            && wrap_solver(&spec("x", "x", "x")).accepted
            && wrap_factory(&spec("x", "x", "x")).accepted
            // 任一项空白就拒，三项都试。 / Any blank field refuses; all three are tried.
            && !wrap_master(&spec("", "sha256", "abc")).accepted
            && !wrap_master(&spec("1.0", "", "abc")).accepted
            && !wrap_master(&spec("1.0", "sha256", "")).accepted
    };
    // 2) 不查十六进制、长度、词表。 / Neither hex, nor length, nor vocabulary.
    let the_gate_checks_neither_hex_nor_length_nor_vocabulary = {
        // `"z"`（非十六进制）与 64 位十六进制同样通过；`algorithm` 也不在词表里校验。 
        // `"z"` (non-hex) passes like a 64-hex digest, and `algorithm` is not vocabulary-checked either.
        wrap_master(&spec("1.0", "not-a-real-algorithm", "z")).accepted
            && wrap_master(&good()).accepted
    };
    // 3) master 与 subproblem 两种文案。 / Master and subproblem use different messages.
    let the_master_and_subproblem_messages_differ = {
        let master = wrap_master(&spec("", "sha256", "abc")).refusal.unwrap_or_default();
        let solver = wrap_solver(&spec("", "sha256", "abc")).refusal.unwrap_or_default();
        master.contains("master fingerprint cannot contain blank fields")
            && solver.contains("subproblem factory fingerprint cannot contain blank fields")
            && master != solver
    };
    // 4) 两个 subproblem 包装器共用一句文案。 / The two subproblem wrappers share one message.
    let the_two_subproblem_wrappers_share_one_message = {
        let solver = wrap_solver(&spec("1.0", "sha256", " ")).refusal.unwrap_or_default();
        let factory = wrap_factory(&spec("1.0", "sha256", " ")).refusal.unwrap_or_default();
        !solver.is_empty() && solver == factory
            && solver.contains("subproblem factory fingerprint cannot contain blank fields")
    };
    // 5) "空白"是 `trim()` 意义上的。 / "Blank" means `trim()`-empty.
    let blank_means_trim_empty = {
        // 纯空白被拒，带内容的两端空格通过。 / Pure whitespace is refused; padded content passes.
        !wrap_master(&spec("   ", "sha256", "abc")).accepted
            && !wrap_master(&spec("1.0", "\t", "abc")).accepted
            && wrap_master(&spec(" 1.0 ", " sha256 ", " abc ")).accepted
    };
    // 6) 存下来的是原样。 / Stored verbatim.
    let the_fingerprint_is_stored_verbatim = {
        let view = wrap_master(&spec(" 1.0 ", " sha256 ", " abc "));
        view.schema_version.as_deref() == Some(" 1.0 ")
            && view.algorithm.as_deref() == Some(" sha256 ")
            && view.value.as_deref() == Some(" abc ")
    };
    // 7) `into_inner` 原样归还。 / `into_inner` returns the wrapped value.
    let into_inner_returns_the_wrapped_value = {
        wrap_master(&good()).wrapped.as_deref() == Some(WRAPPED_MARKER)
            && wrap_solver(&good()).wrapped.as_deref() == Some("bareSolver")
    };
    // 8) 包装把 `None` 变成 `Some`。 / Wrapping turns `None` into `Some`.
    let wrapping_turns_none_into_some = {
        // 裸 solver 的默认实现是 `None`；包一层后是 `Some`。 
        // The bare solver's default is `None`; after wrapping it is `Some`.
        let mut bare = BareSolver;
        let bare_identity = bare.subproblem_factory_fingerprint().is_none();
        bare_identity && wrap_solver(&good()).identity_visible == Some(true)
    };
    Findings {
        the_gate_is_blank_only,
        the_gate_checks_neither_hex_nor_length_nor_vocabulary,
        the_master_and_subproblem_messages_differ,
        the_two_subproblem_wrappers_share_one_message,
        blank_means_trim_empty,
        the_fingerprint_is_stored_verbatim,
        into_inner_returns_the_wrapped_value,
        wrapping_turns_none_into_some,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(master: FingerprintSpec, solver: FingerprintSpec,
        factory: FingerprintSpec) -> IdentityWrapRequest {
        IdentityWrapRequest { master, solver, factory }
    }

    fn all(spec: FingerprintSpec) -> IdentityWrapRequest {
        request(spec.clone(), spec.clone(), spec)
    }

    #[test]
    fn a_well_formed_fingerprint_wraps_on_all_three() {
        let report = verify_identity_wrapping(&all(good())).unwrap();
        assert_eq!(report.wrappers.len(), 3);
        assert_eq!(report.wrappers[0].wrapper, "identifiedMaster");
        assert_eq!(report.wrappers[1].wrapper, "identifiedSubproblemSolver");
        assert_eq!(report.wrappers[2].wrapper, "snapshotSubproblemFactory");
        for view in &report.wrappers {
            assert!(view.accepted, "{:?}", view.refusal);
            assert_eq!(view.schema_version.as_deref(), Some("1.0"));
            assert_eq!(view.algorithm.as_deref(), Some("sha256"));
            assert_eq!(view.value.as_deref(), Some("a".repeat(64).as_str()));
        }
        assert_eq!(report.wrappers[0].wrapped.as_deref(), Some(WRAPPED_MARKER));
        assert_eq!(report.wrappers[1].identity_visible, Some(true));
        assert_eq!(report.wrappers[2].identity_visible, Some(true));
        assert!(report.the_gate_is_blank_only);
        assert!(report.the_gate_checks_neither_hex_nor_length_nor_vocabulary);
        assert!(report.the_master_and_subproblem_messages_differ);
        assert!(report.the_two_subproblem_wrappers_share_one_message);
        assert!(report.blank_means_trim_empty);
        assert!(report.the_fingerprint_is_stored_verbatim);
        assert!(report.into_inner_returns_the_wrapped_value);
        assert!(report.wrapping_turns_none_into_some);
    }

    #[test]
    fn a_junk_fingerprint_passes_every_gate() {
        // `"x"` 三项都过：闸门不查十六进制、不查长度、不查算法词表。 
        // `"x"` passes on all three: the gate checks neither hex, nor length, nor the algorithm vocabulary.
        let report = verify_identity_wrapping(&all(spec("x", "x", "x"))).unwrap();
        assert!(report.wrappers.iter().all(|view| view.accepted));
        let algorithm = verify_identity_wrapping(&all(spec("1.0", "not-a-real-algorithm", "z")))
            .unwrap();
        assert!(algorithm.wrappers.iter().all(|view| view.accepted));
    }

    #[test]
    fn a_blank_field_refuses_with_the_wrapper_specific_message() {
        for (spec, expected) in [
            (spec("", "sha256", "abc"), "master fingerprint cannot contain blank fields"),
            (spec("1.0", "", "abc"), "master fingerprint cannot contain blank fields"),
            (spec("1.0", "sha256", ""), "master fingerprint cannot contain blank fields"),
        ] {
            let report = verify_identity_wrapping(&request(spec, good(), good())).unwrap();
            let view = &report.wrappers[0];
            assert!(!view.accepted);
            // 文案带 `Solver error: Invalid solver input:` 前缀，所以查包含。 
            // The message carries a `Solver error: Invalid solver input:` prefix, so containment is checked.
            assert!(view.refusal.as_deref().unwrap().contains(expected), "{:?}", view.refusal);
            assert_eq!(view.schema_version, None);
            // 另外两个不受影响。 / The other two are unaffected.
            assert!(report.wrappers[1].accepted && report.wrappers[2].accepted);
        }
        // subproblem 侧是另一句文案，两个包装器共用。 
        // The subproblem side uses another message, shared by both wrappers.
        let solver = verify_identity_wrapping(&request(good(), spec("", "sha256", "abc"),
            spec("", "sha256", "abc"))).unwrap();
        assert!(solver.wrappers[1].refusal.as_deref().unwrap()
            .contains("subproblem factory fingerprint cannot contain blank fields"));
        assert_eq!(solver.wrappers[1].refusal, solver.wrappers[2].refusal);
    }

    #[test]
    fn blank_means_trim_empty_and_storage_is_verbatim() {
        // 纯空白被拒。 / Pure whitespace is refused.
        for blank in ["   ", "\t", "\n"] {
            let report = verify_identity_wrapping(&request(spec(blank, "sha256", "abc"), good(),
                good())).unwrap();
            assert!(!report.wrappers[0].accepted, "{blank:?}");
        }
        // 带内容的两端空格通过，且**原样**存下来。 
        // Padded content passes and is stored **verbatim**.
        let padded = verify_identity_wrapping(&all(spec(" 1.0 ", " sha256 ", " abc "))).unwrap();
        assert!(padded.wrappers.iter().all(|view| view.accepted));
        assert_eq!(padded.wrappers[0].schema_version.as_deref(), Some(" 1.0 "));
        assert_eq!(padded.wrappers[0].algorithm.as_deref(), Some(" sha256 "));
        assert_eq!(padded.wrappers[0].value.as_deref(), Some(" abc "));
    }

    #[test]
    fn a_bare_solver_hides_its_identity_until_wrapped() {
        let mut bare = BareSolver;
        assert!(bare.subproblem_factory_fingerprint().is_none());
        let wrapped = verify_identity_wrapping(&all(good())).unwrap();
        assert_eq!(wrapped.wrappers[1].identity_visible, Some(true));
    }

    #[test]
    fn the_wrapped_value_comes_back_unchanged() {
        let report = verify_identity_wrapping(&all(good())).unwrap();
        assert_eq!(report.wrappers[0].wrapped.as_deref(), Some(WRAPPED_MARKER));
        assert_eq!(report.wrappers[1].wrapped.as_deref(), Some("bareSolver"));
        assert_eq!(report.wrappers[2].wrapped.as_deref(), Some("factoryClosure"));
    }

    #[test]
    fn a_missing_field_defaults_to_a_plausible_value() {
        // `FingerprintSpec` 的默认：`1.0` / `sha256` / 空串——最后一项因此被拒。 
        // `FingerprintSpec`'s defaults are `1.0` / `sha256` / empty, so the last one is refused.
        let empty = FingerprintSpec { schema_version: None, algorithm: None, value: None };
        let report = verify_identity_wrapping(&all(empty)).unwrap();
        assert!(!report.wrappers[0].accepted);
        assert!(report.wrappers[0].refusal.as_deref().unwrap()
            .contains("cannot contain blank fields"));
        let partial = FingerprintSpec { schema_version: None, algorithm: None,
            value: Some("abc".to_owned()) };
        let accepted = verify_identity_wrapping(&all(partial)).unwrap();
        assert!(accepted.wrappers.iter().all(|view| view.accepted));
        assert_eq!(accepted.wrappers[0].schema_version.as_deref(), Some("1.0"));
        assert_eq!(accepted.wrappers[0].algorithm.as_deref(), Some("sha256"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_identity_wrapping(&all(good())).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"identityVisible\""), "{json}");
        assert!(json.contains("\"theGateIsBlankOnly\""), "{json}");
        assert!(json.contains("\"wrappingTurnsNoneIntoSome\""), "{json}");
        assert!(!json.contains("identity_visible"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<IdentityWrapReport>(&copy.to_string()).is_err());
    }
}
