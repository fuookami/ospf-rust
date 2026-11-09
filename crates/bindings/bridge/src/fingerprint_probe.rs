//! 续挖第 106 单：**指纹与 provenance 是纯容器，谁都不校验**——空白的 solver id、空白的摘要算法、任意的指纹
//! schema 版本、甚至"声明确定性却带着随机种子"都能通过 `validate()`；唯一真正被读的地方是**证书闸门里的模型指纹
//! 比较**，而那个比较是**三个字段的结构相等**（同一个 `value` 配不同的 `algorithm` 就不相等），并且**报告没带
//! 模型指纹时闸门直接拒**（`solver/report/mod.rs` 的 `SolverProvenance` / `SolveFingerprints` / `AuditFingerprint`）
//! Continuation, delivery 106: **the fingerprints and the provenance are pure containers that nobody validates** — a
//! blank solver id, a blank digest algorithm, an arbitrary fingerprint schema version and even "declares determinism while
//! carrying a random seed" all pass `validate()`; the only place they are truly read is **the model-fingerprint comparison
//! inside the certificate gates**, and that comparison is **structural equality over three fields** (the same `value` with
//! a different `algorithm` is not equal), while **a report carrying no model fingerprint is refused outright**
//! (`SolverProvenance` / `SolveFingerprints` / `AuditFingerprint` in `solver/report/mod.rs`).
//!
//! 第 105 单量的是诊断面，这一单量**指纹与 provenance 的边界**。八条实测：
//!
//! 1. **`SolverProvenance::default()` 把 solver 与 backend 都写成 `unknown`**（不是空白），其余字段为空或未知
//!    （`the_provenance_default_names_an_unknown_solver()`）。
//! 2. **`SolveFingerprints::default()` 三个都是 `None`**（`the_fingerprints_default_to_nothing()`）。
//! 3. **指纹相等是结构相等**（`the_fingerprint_equality_is_structural()`）：两个空白指纹互相相等。
//! 4. **比较会读每一个字段**（`the_comparison_reads_every_field()`）：同一个 `value` 配不同 `algorithm` **不相等**。
//! 5. **指纹的 schema 版本从不被检查**（`the_fingerprint_schema_version_is_never_checked()`）。
//! 6. **报告没带模型指纹时闸门直接拒**（`a_missing_model_fingerprint_fails_the_gate()`）。
//! 7. **provenance 从不被校验**（`the_provenance_is_never_validated()`）：空白 id、"确定性 + 有种子"都通过。
//! 8. **报告 schema 版本从不与常量比对**（`the_report_schema_version_is_never_compared()`）。
//!
//! Delivery 105 measured the diagnostic face; this one measures **the fingerprints and the provenance**. **The provenance
//! default names both the solver and the backend `unknown`.** **The fingerprints default to nothing.** **Fingerprint equality is structural**, and **the
//! comparison reads every field**, so the same value with a different algorithm is not equal. **The fingerprint schema
//! version is never checked.** **A missing model fingerprint fails the gate.** **The provenance is never validated.** And
//! **the report's schema version is never compared with the constant.**

use ospf_rust_core::solver::report::{
    require_infeasibility_certificate_for_model, require_optimal_lp_certificate, AuditFingerprint,
    ProblemStatus, ProofCompleteness, ProofKind, ProofReliability, ProofStatus, SolveDiagnostics,
    SolveFingerprints, SolveProof, SolveReport, SolveSolution, SolveStatistics, SolveTrace,
    SolutionPresence, SolverProvenance, TerminationReason,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个指纹的描述。 / The description of one fingerprint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuditFingerprintSpec {
    /// 指纹 schema 版本。 / The fingerprint schema version.
    #[serde(default)]
    pub schema_version: Option<String>,
    /// 摘要算法。 / The digest algorithm.
    #[serde(default)]
    pub algorithm: Option<String>,
    /// 十六进制摘要。 / The hex digest.
    #[serde(default)]
    pub value: Option<String>,
}

/// 指纹探针请求。 / The fingerprint probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FingerprintProbeRequest {
    /// 报告自带的模型指纹（缺省表示不带）。 / The report's own model fingerprint; absent means none.
    #[serde(default)]
    pub model: Option<AuditFingerprintSpec>,
    /// 闸门期望的模型指纹（缺省表示不做模型比对）。 / The fingerprint the gate expects; absent skips the comparison.
    #[serde(default)]
    pub expected: Option<AuditFingerprintSpec>,
    /// provenance 的 solver id（默认空白）。 / The provenance solver id, blank by default.
    #[serde(default)]
    pub solver_id: Option<String>,
    /// provenance 的 backend 名（默认空白）。 / The provenance backend name.
    #[serde(default)]
    pub backend_name: Option<String>,
    /// 是否声明确定性。 / Whether determinism is claimed.
    #[serde(default)]
    pub deterministic: Option<bool>,
    /// 随机种子。 / The random seed.
    #[serde(default)]
    pub random_seed: Option<u64>,
    /// 报告的 schema 版本（默认随意）。 / The report schema version.
    #[serde(default)]
    pub schema_version: Option<String>,
    /// 报告是否不可行（决定走哪条闸门）。 / Whether the report is infeasible, which picks the gate.
    #[serde(default)]
    pub infeasible: Option<bool>,
}

/// 指纹探针结论。 / The fingerprint probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FingerprintProbeReport {
    /// 报告是否通过 `validate()`。 / Whether the report validates.
    pub report_valid: bool,
    /// 校验错误文本（通过时为空）。 / The validation error text, empty when it passes.
    pub report_error: String,
    /// 闸门是否放行。 / Whether the gate passes.
    pub gate_ok: bool,
    /// 闸门的错误文本（放行时为空）。 / The gate error text, empty when it passes.
    pub gate_error: String,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_provenance_default_names_an_unknown_solver: bool,
    pub the_fingerprints_default_to_nothing: bool,
    pub the_fingerprint_equality_is_structural: bool,
    pub the_comparison_reads_every_field: bool,
    pub the_fingerprint_schema_version_is_never_checked: bool,
    pub a_missing_model_fingerprint_fails_the_gate: bool,
    pub the_provenance_is_never_validated: bool,
    pub the_report_schema_version_is_never_compared: bool,
}

/// 造一个指纹。 / Build one fingerprint.
fn fingerprint_of(spec: &AuditFingerprintSpec) -> AuditFingerprint {
    AuditFingerprint {
        schema_version: spec.schema_version.clone().unwrap_or_else(|| "1".to_owned()),
        algorithm: spec.algorithm.clone().unwrap_or_else(|| "sha256".to_owned()),
        value: spec.value.clone().unwrap_or_else(|| "00".to_owned()),
    }
}

/// 造一份报告。 / Build one report.
fn report_of(request: &FingerprintProbeRequest) -> SolveReport<f64, u32> {
    let infeasible = request.infeasible.unwrap_or(false);
    let solution = if infeasible {
        None
    } else {
        Some(SolveSolution {
            value: Some(0.0),
            values: vec![0.0],
            stable_values: Default::default(),
            objective: Some(0.0),
            objective_value: Some(0.0),
            dual_solution: Some(vec![1.0]),
            quadratic_dual_solution: None,
            pool: Vec::new(),
        })
    };
    SolveReport {
        schema_version: request.schema_version.clone().unwrap_or_else(|| "whatever".to_owned()),
        problem_status: if infeasible { ProblemStatus::Infeasible } else {
            ProblemStatus::Feasible },
        termination_reason: TerminationReason::Completed,
        solution_presence: if infeasible { SolutionPresence::None } else {
            SolutionPresence::Optimal },
        solution,
        proof: Some(if infeasible {
            SolveProof::infeasibility()
        } else {
            SolveProof::optimality()
        }),
        statistics: SolveStatistics::default(),
        diagnostics: SolveDiagnostics::default(),
        provenance: SolverProvenance {
            solver_id: request.solver_id.clone().unwrap_or_default(),
            backend_name: request.backend_name.clone().unwrap_or_default(),
            backend_version: None,
            plugin_version: None,
            requested_configuration: Default::default(),
            effective_configuration: Default::default(),
            thread_count: None,
            random_seed: request.random_seed,
            deterministic: request.deterministic,
            environment_summary: Default::default(),
        },
        fingerprints: SolveFingerprints {
            model: request.model.as_ref().map(fingerprint_of),
            configuration: None,
            solver: None,
        },
        model_mapping: None,
        warnings: Vec::new(),
        trace: SolveTrace::default(),
    }
}

/// 走一遍指纹探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_fingerprint_probe(request: &FingerprintProbeRequest) -> Result<FingerprintProbeReport> {
    let findings = canonical_findings();
    let report = report_of(request);
    let validation = report.validate();
    let expected = request.expected.as_ref().map(fingerprint_of);
    // 不可行的报告走不可行证书闸门（带模型比对），否则走精确定价闸门。 
    // An infeasible report goes through the infeasibility gate with the model comparison, otherwise through pricing.
    let gate = if request.infeasible.unwrap_or(false) {
        match &expected {
            Some(model) => require_infeasibility_certificate_for_model(&report, model).map(|_| ()),
            None => ospf_rust_core::solver::report::require_infeasibility_certificate(&report)
                .map(|_| ()),
        }
    } else {
        require_optimal_lp_certificate(&report).map(|_| ())
    };
    Ok(FingerprintProbeReport {
        report_valid: validation.is_ok(),
        report_error: validation.err().map(|error| error.to_string()).unwrap_or_default(),
        gate_ok: gate.is_ok(),
        gate_error: gate.err().map(|error| error.to_string()).unwrap_or_default(),
        the_provenance_default_names_an_unknown_solver: findings.the_provenance_default_names_an_unknown_solver,
        the_fingerprints_default_to_nothing: findings.the_fingerprints_default_to_nothing,
        the_fingerprint_equality_is_structural: findings.the_fingerprint_equality_is_structural,
        the_comparison_reads_every_field: findings.the_comparison_reads_every_field,
        the_fingerprint_schema_version_is_never_checked:
            findings.the_fingerprint_schema_version_is_never_checked,
        a_missing_model_fingerprint_fails_the_gate:
            findings.a_missing_model_fingerprint_fails_the_gate,
        the_provenance_is_never_validated: findings.the_provenance_is_never_validated,
        the_report_schema_version_is_never_compared:
            findings.the_report_schema_version_is_never_compared,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_provenance_default_names_an_unknown_solver: bool,
    the_fingerprints_default_to_nothing: bool,
    the_fingerprint_equality_is_structural: bool,
    the_comparison_reads_every_field: bool,
    the_fingerprint_schema_version_is_never_checked: bool,
    a_missing_model_fingerprint_fails_the_gate: bool,
    the_provenance_is_never_validated: bool,
    the_report_schema_version_is_never_compared: bool,
}

/// 一份全默认的请求。 / A fully defaulted request.
fn plain() -> FingerprintProbeRequest {
    FingerprintProbeRequest { model: None, expected: None, solver_id: None, backend_name: None,
        deterministic: None, random_seed: None, schema_version: None, infeasible: None }
}

/// 一个全默认的指纹。 / A fully defaulted fingerprint.
fn fingerprint() -> AuditFingerprintSpec {
    AuditFingerprintSpec { schema_version: None, algorithm: None, value: None }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        let build = |request: FingerprintProbeRequest| report_of(&request);
        // 1) provenance 的默认值把 solver 与 backend 都写成 `unknown`，其余为空或未知。 
        // The provenance default names both the solver and the backend `unknown`, leaving the rest empty or unknown.
        let the_provenance_default_names_an_unknown_solver = {
            let provenance = SolverProvenance::<u32>::default();
            provenance.solver_id == "unknown" && provenance.backend_name == "unknown"
                && provenance.backend_version.is_none() && provenance.plugin_version.is_none()
                && provenance.requested_configuration.is_empty()
                && provenance.effective_configuration.is_empty()
                && provenance.thread_count.is_none() && provenance.random_seed.is_none()
                && provenance.deterministic.is_none()
                && provenance.environment_summary.is_empty()
        };
        // 2) 指纹默认三个 None。 / The fingerprints default to three nones.
        let the_fingerprints_default_to_nothing = {
            let fingerprints = SolveFingerprints::default();
            fingerprints.model.is_none() && fingerprints.configuration.is_none()
                && fingerprints.solver.is_none()
        };
        // 3) 相等是结构相等。 / Equality is structural.
        let the_fingerprint_equality_is_structural = {
            let blank = AuditFingerprint { schema_version: "1".to_owned(),
                algorithm: String::new(), value: String::new() };
            blank == blank.clone()
                // 两个分别造出来的空白指纹也相等。 / Two separately built blanks are equal too.
                && blank == AuditFingerprint { schema_version: "1".to_owned(),
                    algorithm: String::new(), value: String::new() }
        };
        // 4) 比较读每个字段。 / The comparison reads every field.
        let the_comparison_reads_every_field = {
            let base = fingerprint_of(&fingerprint());
            let same_value_other_algorithm = AuditFingerprint {
                algorithm: "blake3".to_owned(), ..base.clone() };
            let same_value_other_schema = AuditFingerprint {
                schema_version: "2".to_owned(), ..base.clone() };
            base != same_value_other_algorithm && base != same_value_other_schema
                // 闸门也按这个相等判定：报告指纹与期望指纹算法不同 → 拒。 
                // The gate uses that same equality: a different algorithm is refused.
                && { let report = build(FingerprintProbeRequest {
                        model: Some(fingerprint()), expected: Some(AuditFingerprintSpec {
                            algorithm: Some("blake3".to_owned()), ..fingerprint() }),
                        infeasible: Some(true), ..plain() });
                    require_infeasibility_certificate_for_model(&report, &same_value_other_algorithm)
                        .is_err() }
        };
        // 5) 指纹 schema 版本不被检查。 / The fingerprint schema version is never checked.
        let the_fingerprint_schema_version_is_never_checked = {
            let odd = build(FingerprintProbeRequest { model: Some(AuditFingerprintSpec {
                schema_version: Some("  ".to_owned()), algorithm: Some(String::new()),
                value: Some(String::new()) }), ..plain() });
            odd.validate().is_ok()
        };
        // 6) 没带模型指纹 → 闸门拒。 / Without a model fingerprint the gate refuses.
        let a_missing_model_fingerprint_fails_the_gate = {
            let bare = build(FingerprintProbeRequest { model: None,
                expected: Some(fingerprint()), infeasible: Some(true), ..plain() });
            let matching = build(FingerprintProbeRequest { model: Some(fingerprint()),
                expected: Some(fingerprint()), infeasible: Some(true), ..plain() });
            bare.validate().is_ok()
                && require_infeasibility_certificate_for_model(&bare, &fingerprint_of(&fingerprint()))
                    .is_err()
                && require_infeasibility_certificate_for_model(&matching,
                    &fingerprint_of(&fingerprint())).is_ok()
        };
        // 7) provenance 不被校验。 / The provenance is never validated.
        let the_provenance_is_never_validated = {
            // 空白 id / 名，外加"声明确定性却带随机种子"。 
            // Blank id and name, plus "claims determinism while carrying a seed".
            let absurd = build(FingerprintProbeRequest { solver_id: Some(String::new()),
                backend_name: Some("   ".to_owned()), deterministic: Some(true),
                random_seed: Some(7), ..plain() });
            absurd.validate().is_ok()
                && absurd.provenance.solver_id.is_empty()
                && absurd.provenance.deterministic == Some(true)
                && absurd.provenance.random_seed == Some(7)
        };
        // 8) 报告 schema 版本从不与常量比对。 / The report schema version is never compared.
        let the_report_schema_version_is_never_compared = {
            build(FingerprintProbeRequest { schema_version: Some(String::new()), ..plain() })
                .validate().is_ok()
                && build(FingerprintProbeRequest {
                    schema_version: Some("9999-not-the-constant".to_owned()), ..plain() })
                    .validate().is_ok()
        };
        Findings {
            the_provenance_default_names_an_unknown_solver,
            the_fingerprints_default_to_nothing,
            the_fingerprint_equality_is_structural,
            the_comparison_reads_every_field,
            the_fingerprint_schema_version_is_never_checked,
            a_missing_model_fingerprint_fails_the_gate,
            the_provenance_is_never_validated,
            the_report_schema_version_is_never_compared,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: FingerprintProbeRequest) -> FingerprintProbeReport {
        verify_fingerprint_probe(&request).unwrap()
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(plain());
        assert!(report.report_valid, "{}", report.report_error);
        assert!(report.gate_ok, "{}", report.gate_error);
        assert!(report.the_provenance_default_names_an_unknown_solver);
        assert!(report.the_fingerprints_default_to_nothing);
        assert!(report.the_fingerprint_equality_is_structural);
        assert!(report.the_comparison_reads_every_field);
        assert!(report.the_fingerprint_schema_version_is_never_checked);
        assert!(report.a_missing_model_fingerprint_fails_the_gate);
        assert!(report.the_provenance_is_never_validated);
        assert!(report.the_report_schema_version_is_never_compared);
    }

    #[test]
    fn the_provenance_and_fingerprints_start_empty() {
        let provenance = SolverProvenance::<u32>::default();
        // 默认值是 `unknown` 而不是空白。 / The default is `unknown` rather than blank.
        assert_eq!(provenance.solver_id, "unknown");
        assert_eq!(provenance.backend_name, "unknown");
        assert!(provenance.deterministic.is_none());
        let fingerprints = SolveFingerprints::default();
        assert!(fingerprints.model.is_none() && fingerprints.configuration.is_none()
            && fingerprints.solver.is_none());
    }

    #[test]
    fn a_matching_fingerprint_passes_the_gate_and_a_missing_one_does_not() {
        let matching = ask(FingerprintProbeRequest { model: Some(fingerprint()),
            expected: Some(fingerprint()), infeasible: Some(true), ..plain() });
        assert!(matching.gate_ok, "{}", matching.gate_error);
        let bare = ask(FingerprintProbeRequest { model: None, expected: Some(fingerprint()),
            infeasible: Some(true), ..plain() });
        assert!(bare.report_valid);
        assert!(!bare.gate_ok);
        assert!(bare.gate_error.contains("fingerprint"), "{}", bare.gate_error);
    }

    #[test]
    fn a_different_algorithm_breaks_the_match() {
        let different = ask(FingerprintProbeRequest { model: Some(fingerprint()),
            expected: Some(AuditFingerprintSpec { algorithm: Some("blake3".to_owned()),
                ..fingerprint() }), infeasible: Some(true), ..plain() });
        assert!(!different.gate_ok);
        // 同一个 value 配同一个 algorithm 就放行。 / The same value with the same algorithm passes.
        let same = ask(FingerprintProbeRequest { model: Some(fingerprint()),
            expected: Some(fingerprint()), infeasible: Some(true), ..plain() });
        assert!(same.gate_ok);
    }

    #[test]
    fn the_provenance_and_schema_versions_are_never_validated() {
        let absurd = ask(FingerprintProbeRequest { solver_id: Some(String::new()),
            backend_name: Some("   ".to_owned()), deterministic: Some(true),
            random_seed: Some(7), schema_version: Some(String::new()), ..plain() });
        assert!(absurd.report_valid, "{}", absurd.report_error);
        let odd = ask(FingerprintProbeRequest { model: Some(AuditFingerprintSpec {
            schema_version: Some("  ".to_owned()), algorithm: Some(String::new()),
            value: Some(String::new()) }), ..plain() });
        assert!(odd.report_valid);
        let wrong_version = ask(FingerprintProbeRequest {
            schema_version: Some("9999-not-the-constant".to_owned()), ..plain() });
        assert!(wrong_version.report_valid);
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(plain());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"reportValid\""), "{json}");
        assert!(json.contains("\"theComparisonReadsEveryField\""), "{json}");
        assert!(!json.contains("report_valid"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<FingerprintProbeReport>(&copy.to_string()).is_err());
    }
}
