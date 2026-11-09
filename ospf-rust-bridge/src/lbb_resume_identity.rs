//! 批次 D 第 2 单：Logic-Based Benders 恢复身份的预检 / Batch D, delivery 2: the Logic-Based Benders resume identity.
//!
//! 恢复一次 LBB 运行之前，调用方必须能证明"我要恢复的这次运行，就是我手上这份检查点所记录的那次"。这份证明就是
//! `LogicBasedBendersResumeIdentity`：源 attempt、父链、模型 / 配置 / solver 三个指纹、solver provenance、
//! CP snapshot / master / subproblem 工厂指纹，外加取消链前缀。`validate()` 是**纯校验**，不需要重建引擎：
//!
//! * `runId` 与 `sourceAttemptId` 不能空白；
//! * 父 attempt **不能等于**源 attempt；给了父 attempt 也不能是空白；
//! * provenance 的 `solverId` 与 `backendName` 不能空白；
//! * 六个指纹（model / configuration / solver / cpSnapshot / master / subproblemFactory）的
//!   `schemaVersion`、`algorithm`、`value` 三段都不能空白——每个指纹**各自点名**。
//!
//! **一条必须说清的实测事实**：`validate()` **不检查取消链内容**。所以这一层能证明的是"身份字段自洽"，不是"取消链
//! 与检查点一致"——后者属于检查点本体（`validate_resume_identity`）的职责。报告里用 `cancellationCount` 与
//! `cancellationOrigins` 如实回显，不做任何"已验证"的暗示。取消来源的线格式代码是无损的：无法识别的代码落到
//! `Other` 并**原样保留**，所以对端私有取值也能往返。
//!
//! Before resuming a run the caller must be able to prove that the run being resumed is the one the checkpoint records. That
//! proof is `LogicBasedBendersResumeIdentity`: the source attempt, the parent chain, the model / configuration / solver
//! fingerprints, the solver provenance, the CP snapshot / master / subproblem-factory fingerprints, and a cancellation-chain
//! prefix. `validate()` is **pure** and needs no engine rebuild: run and source attempt IDs non-blank, the parent attempt
//! **not equal** to the source attempt (and non-blank when present), provenance `solverId` / `backendName` non-blank, and all
//! three parts of all six fingerprints non-blank, each named separately. **One measured fact must be stated plainly**:
//! `validate()` **does not inspect the cancellation chain**, so this layer proves the identity fields are self-consistent,
//! not that the chain matches the checkpoint — that belongs to the checkpoint body (`validate_resume_identity`). The report
//! echoes `cancellationCount` and `cancellationOrigins` honestly and implies nothing more. Cancellation origin wire codes are
//! lossless: an unrecognized code lands in `Other` with its text **preserved verbatim**, so a peer's private value
//! round-trips.

use ospf_rust_core::solver::cancellation::{CancellationOrigin, CancellationRecord};
use ospf_rust_core::solver::report::{AuditFingerprint, SolverProvenance};
use ospf_rust_framework::solver::logic_based_benders_checkpoint::LogicBasedBendersResumeIdentity;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个审计指纹。 / One audit fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FingerprintInput {
    /// 指纹 schema 版本；省略为 `1.0`。 / The fingerprint schema version; `1.0` when omitted.
    #[serde(default = "default_schema")]
    pub schema_version: String,
    /// 摘要算法；省略为 `sha256`。 / The digest algorithm; `sha256` when omitted.
    #[serde(default = "default_algorithm")]
    pub algorithm: String,
    /// 十六进制摘要。 / The hex digest.
    pub value: String,
}

/// 指纹 schema 版本默认值。 / The default fingerprint schema version.
fn default_schema() -> String { "1.0".to_owned() }
/// 摘要算法默认值。 / The default digest algorithm.
fn default_algorithm() -> String { "sha256".to_owned() }

/// solver provenance。 / Solver provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProvenanceInput {
    /// 稳定 solver ID。 / The stable solver ID.
    pub solver_id: String,
    /// backend 名称。 / The backend name.
    pub backend_name: String,
    /// backend 版本。 / The backend version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub backend_version: Option<String>,
    /// plugin 版本。 / The plugin version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub plugin_version: Option<String>,
}

/// 一条取消记录。 / One cancellation record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancellationInput {
    /// 线格式规范代码：`user` / `external` / `callback` / `frameworkLoser` / `remoteStop` / `taskAbort` / `backend`；
    /// 其他文本原样保留。 / The canonical wire code; any other text is preserved verbatim.
    pub origin: String,
    /// 首次取消时间（epoch 毫秒）。 / The first cancellation time in epoch milliseconds.
    pub requested_at_epoch_ms: u64,
    /// 调用方给出的取消原因。 / The caller-supplied cancellation reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reason: Option<String>,
}

/// 恢复身份预检请求。 / The resume-identity preflight request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LbbResumeIdentityRequest {
    /// 稳定运行 ID。 / The stable run ID.
    pub run_id: String,
    /// 生成检查点的源 attempt。 / The source attempt that produced the checkpoint.
    pub source_attempt_id: String,
    /// 源 attempt 的父 attempt。 / The parent attempt of the source attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_parent_attempt_id: Option<String>,
    /// 期望模型指纹。 / The expected model fingerprint.
    pub model: FingerprintInput,
    /// 期望配置指纹。 / The expected configuration fingerprint.
    pub configuration: FingerprintInput,
    /// 期望 solver 指纹。 / The expected solver fingerprint.
    pub solver: FingerprintInput,
    /// 期望 CP snapshot 指纹。 / The expected CP snapshot fingerprint.
    pub cp_snapshot: FingerprintInput,
    /// 期望 master 指纹。 / The expected master fingerprint.
    pub master: FingerprintInput,
    /// 期望 subproblem 工厂指纹。 / The expected subproblem-factory fingerprint.
    pub subproblem_factory: FingerprintInput,
    /// 期望 solver provenance。 / The expected solver provenance.
    pub provenance: ProvenanceInput,
    /// 调用方期望的取消链前缀。 / The cancellation-chain prefix the caller expects.
    #[serde(default)]
    pub cancellation_chain: Vec<CancellationInput>,
}

/// 恢复身份预检结论。 / The resume-identity verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LbbResumeIdentityReport {
    /// 已校验的运行 ID。 / The validated run ID.
    pub run_id: String,
    /// 已校验的源 attempt。 / The validated source attempt.
    pub source_attempt_id: String,
    /// 已校验的父 attempt。 / The validated parent attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_parent_attempt_id: Option<String>,
    /// 校验过的指纹个数（恒为六）。 / How many fingerprints were validated (always six).
    pub fingerprint_count: u32,
    /// 取消链条数（**未参与校验**，只回显）。 / The chain length (**not validated**, only echoed).
    pub cancellation_count: u32,
    /// 取消链的规范来源代码（原样回显，`Other` 兜底也保留）。
    /// The canonical origin codes (**echoed verbatim**, `Other` fallbacks preserved).
    pub cancellation_origins: Vec<String>,
    /// provenance 的 solver ID。 / The provenance solver ID.
    pub provenance_solver_id: String,
    /// provenance 的 backend 名称。 / The provenance backend name.
    pub provenance_backend_name: String,
    /// 是否给了 backend 版本。 / Whether a backend version is present.
    pub has_backend_version: bool,
    /// 是否给了 plugin 版本。 / Whether a plugin version is present.
    pub has_plugin_version: bool,
    /// 请求的规范文本（可再提交，必须逐字一致）。 / The canonical request text (re-submittable, must match verbatim).
    pub canonical_json: String,
}

/// 用领域自己的 `validate()` 预检一份恢复身份。非法输入是**错误**，原因原样带出。
/// Preflight a resume identity through the domain's own `validate()`. An illegal input is an **error** carrying the reason.
pub fn verify_lbb_resume_identity(request: &LbbResumeIdentityRequest) -> Result<LbbResumeIdentityReport> {
    let identity = LogicBasedBendersResumeIdentity {
        run_id: request.run_id.clone(),
        source_attempt_id: request.source_attempt_id.clone(),
        source_parent_attempt_id: request.source_parent_attempt_id.clone(),
        model_fingerprint: fingerprint(&request.model),
        configuration_fingerprint: fingerprint(&request.configuration),
        solver_fingerprint: fingerprint(&request.solver),
        provenance: SolverProvenance {
            solver_id: request.provenance.solver_id.clone(),
            backend_name: request.provenance.backend_name.clone(),
            backend_version: request.provenance.backend_version.clone(),
            plugin_version: request.provenance.plugin_version.clone(),
            ..SolverProvenance::default()
        },
        cancellation_chain: request.cancellation_chain.iter().map(cancellation).collect(),
        cp_snapshot_fingerprint: fingerprint(&request.cp_snapshot),
        master_fingerprint: fingerprint(&request.master),
        subproblem_factory_fingerprint: fingerprint(&request.subproblem_factory),
    };
    // 领域入口：六组规则一次跑完，错误信息各自点名是哪一项。
    // The domain entry runs all six rule groups at once, and each message names which item failed.
    identity.validate().map_err(|error| {
        BridgeError::new("InvalidInput", &format!("resume identity failed validation: {error}"))
    })?;
    let canonical_json = serde_json::to_string(request).map_err(|error| {
        BridgeError::new("CodecFailure", &format!("resume identity could not be encoded: {error}"))
    })?;
    Ok(LbbResumeIdentityReport {
        run_id: identity.run_id,
        source_attempt_id: identity.source_attempt_id,
        source_parent_attempt_id: identity.source_parent_attempt_id,
        fingerprint_count: 6,
        cancellation_count: u32::try_from(identity.cancellation_chain.len()).unwrap_or(u32::MAX),
        cancellation_origins: identity.cancellation_chain.iter()
            .map(|record| record.origin.to_wire_code().to_owned()).collect(),
        provenance_solver_id: identity.provenance.solver_id,
        provenance_backend_name: identity.provenance.backend_name,
        has_backend_version: identity.provenance.backend_version.is_some(),
        has_plugin_version: identity.provenance.plugin_version.is_some(),
        canonical_json,
    })
}

/// 把输入转成领域指纹。 / Convert the input into a domain fingerprint.
fn fingerprint(input: &FingerprintInput) -> AuditFingerprint {
    AuditFingerprint {
        schema_version: input.schema_version.clone(),
        algorithm: input.algorithm.clone(),
        value: input.value.clone(),
    }
}

/// 把输入转成领域取消记录（来源走无损线格式代码）。 / Convert the input into a domain record (via the lossless code).
fn cancellation(input: &CancellationInput) -> CancellationRecord {
    CancellationRecord::new(
        CancellationOrigin::from_wire_code(&input.origin),
        input.requested_at_epoch_ms,
        input.reason.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一份全部合法的恢复身份（手写值在这里是**被测对象**）。
    /// A fully legal resume identity (the hand-written values are the **subject under test**).
    fn identity() -> LbbResumeIdentityRequest {
        let print = |value: &str| FingerprintInput {
            schema_version: default_schema(), algorithm: default_algorithm(), value: value.to_owned(),
        };
        LbbResumeIdentityRequest {
            run_id: "run-1".to_owned(),
            source_attempt_id: "attempt-1".to_owned(),
            source_parent_attempt_id: None,
            model: print("model-a"),
            configuration: print("config-a"),
            solver: print("solver-a"),
            cp_snapshot: print("cp-a"),
            master: print("master-a"),
            subproblem_factory: print("factory-a"),
            provenance: ProvenanceInput {
                solver_id: "scip".to_owned(), backend_name: "scip".to_owned(),
                backend_version: Some("8.0".to_owned()), plugin_version: None,
            },
            cancellation_chain: Vec::new(),
        }
    }

    #[test]
    fn legal_identity_is_accepted_and_echoed() {
        let mut request = identity();
        request.cancellation_chain = vec![
            CancellationInput { origin: "user".to_owned(), requested_at_epoch_ms: 1, reason: Some("stop".into()) },
            CancellationInput { origin: "remoteStop".to_owned(), requested_at_epoch_ms: 2, reason: None },
            // 词表之外的私有取值：必须**原样保留**。 / A private value outside the vocabulary: **preserved verbatim**.
            CancellationInput { origin: "tenantShutdown".to_owned(), requested_at_epoch_ms: 3, reason: None },
        ];
        let report = verify_lbb_resume_identity(&request).unwrap();
        assert_eq!(report.run_id, "run-1");
        assert_eq!(report.source_attempt_id, "attempt-1");
        assert_eq!(report.source_parent_attempt_id, None);
        assert_eq!(report.fingerprint_count, 6);
        // 取消链只回显、不校验（实测 `validate()` 不碰它）。
        // The chain is echoed, not validated (measured: `validate()` never touches it).
        assert_eq!(report.cancellation_count, 3);
        assert_eq!(report.cancellation_origins, vec!["user", "remoteStop", "tenantShutdown"]);
        assert_eq!(report.provenance_solver_id, "scip");
        assert!(report.has_backend_version);
        assert!(!report.has_plugin_version);

        // 规范文本再提交：逐字一致。 / Re-submitting the canonical text: byte-identical.
        let again: LbbResumeIdentityRequest = serde_json::from_str(&report.canonical_json).unwrap();
        assert_eq!(again, request);
        assert_eq!(verify_lbb_resume_identity(&again).unwrap().canonical_json, report.canonical_json);
    }

    #[test]
    fn every_identity_rule_names_itself() {
        // 运行 / 源 attempt 空白。 / Blank run and source attempt IDs.
        for (mutate, expected) in [
            (Box::new(|value: &mut LbbResumeIdentityRequest| value.run_id = "  ".to_owned())
                as Box<dyn Fn(&mut LbbResumeIdentityRequest)>, "run and source attempt identities cannot be blank"),
            (Box::new(|value: &mut LbbResumeIdentityRequest| value.source_attempt_id = String::new()), "run and source attempt identities cannot be blank"),
            // 父 attempt 等于源 attempt。 / The parent attempt equal to the source attempt.
            (Box::new(|value: &mut LbbResumeIdentityRequest| value.source_parent_attempt_id = Some("attempt-1".to_owned())),
                "source parent cannot equal the source attempt"),
            // 父 attempt 空白。 / A blank parent attempt.
            (Box::new(|value: &mut LbbResumeIdentityRequest| value.source_parent_attempt_id = Some(" ".to_owned())),
                "source parent cannot be blank"),
            // provenance 空白。 / Blank provenance identity.
            (Box::new(|value: &mut LbbResumeIdentityRequest| value.provenance.solver_id = String::new()),
                "provenance identity cannot be blank"),
            (Box::new(|value: &mut LbbResumeIdentityRequest| value.provenance.backend_name = "  ".to_owned()),
                "provenance identity cannot be blank"),
        ] {
            let mut request = identity();
            mutate(&mut request);
            let error = verify_lbb_resume_identity(&request).unwrap_err();
            assert_eq!(error.code, "InvalidInput");
            assert!(error.message.contains("resume identity failed validation"), "{}", error.message);
            assert!(error.message.contains(expected), "{}", error.message);
        }

        // 六个指纹各自点名：任意一段空白都拒绝，且错误里带出是哪一个指纹。
        // The six fingerprints each name themselves: any blank part is refused and the message names which one.
        let cases = [
            ("model", "resume model"), ("configuration", "resume configuration"), ("solver", "resume solver"),
            ("cpSnapshot", "resume CP snapshot"), ("master", "resume master"),
            ("subproblemFactory", "resume subproblem factory"),
        ];
        for (field, expected) in cases {
            for part in ["schema", "algorithm", "value"] {
                let mut request = identity();
                let target = match field {
                    "model" => &mut request.model,
                    "configuration" => &mut request.configuration,
                    "solver" => &mut request.solver,
                    "cpSnapshot" => &mut request.cp_snapshot,
                    "master" => &mut request.master,
                    _ => &mut request.subproblem_factory,
                };
                match part {
                    "schema" => target.schema_version = " ".to_owned(),
                    "algorithm" => target.algorithm = String::new(),
                    _ => target.value = "  ".to_owned(),
                }
                let error = verify_lbb_resume_identity(&request).unwrap_err();
                assert!(error.message.contains(expected), "{field}/{part}: {}", error.message);
                assert!(error.message.contains("cannot contain blank fields"), "{field}/{part}: {}", error.message);
            }
        }
    }
}
