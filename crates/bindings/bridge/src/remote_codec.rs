//! 远端求解协议的可移植编解码面 / Portable codec surface of the remote-solver protocol.
//!
//! 批次 A：`solver/remote` 里真正跨进程的部分是**数据**（DTO + JSON 往返 + 校验），transport 不是。
//! 本模块只做「给定线格式字节 → 解码 + 框架校验 → 规范化重编码 + 身份回显」，因此不需要任何后端特性。
//! Batch A: the genuinely cross-process part of `solver/remote` is **data** (DTOs, JSON round-trip, validation), not the
//! transport. This module only does "wire bytes in → decode + framework validation → canonical re-encode + identity
//! echo", so it needs no backend feature.

use ospf_rust_framework::solver::remote::ospf_serializer::{
    checkpoint_artifact_from_json, checkpoint_artifact_to_json, constraint_programming_checkpoint_from_json,
    constraint_programming_checkpoint_to_json, constraint_programming_result_from_json,
    logic_based_benders_checkpoint_from_json, logic_based_benders_checkpoint_to_json,
    validate_checkpoint_artifact_for_resume, CheckpointResumeExpectation,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 可编解码的远端工件种类。 / A decodable remote artifact kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RemoteArtifactKind {
    /// 通用可移植检查点 artifact。 / The generic portable checkpoint artifact.
    CheckpointArtifact,
    /// CP 结果物化 artifact。 / The CP result-materialization artifact.
    ConstraintProgrammingResult,
    /// direct CP typed 检查点。 / The typed direct-CP checkpoint.
    ConstraintProgrammingCheckpoint,
    /// Logic-Based Benders typed 检查点。 / The typed Logic-Based Benders checkpoint.
    LogicBasedBendersCheckpoint,
}

/// 编解码操作。 / The codec operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RemoteCodecOperation {
    /// 解码 + 完整性校验 + 规范化重编码 + 身份回显。 / Decode, validate integrity, re-encode canonically, echo identity.
    Decode,
    /// 在解码之上再校验能否按给定身份恢复；恢复被拒是**错误**（带领域原因），不是布尔值。
    /// On top of decoding, validate that the artifact can be resumed under the given identity. A refusal is an **error**
    /// carrying the domain reason, not a boolean.
    Resume,
}

/// 一个期望指纹：算法 + 摘要值。`schemaVersion` 由 artifact 侧提供——恢复比对要求整条指纹相等，
/// 所以版本不同会自然体现为不匹配，而不是被这里掩盖。
/// One expected fingerprint: algorithm plus digest value. `schemaVersion` comes from the artifact side — resume compares
/// whole fingerprints for equality, so a version difference surfaces naturally as a mismatch instead of being masked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteFingerprintSpec {
    /// 摘要算法。 / The digest algorithm.
    pub algorithm: String,
    /// 摘要值。 / The digest value.
    pub value: String,
}

/// 恢复身份期望（不含源 attempt，与 `CheckpointResumeExpectation` 对应）。
/// The resume identity expectation (no source attempt; mirrors `CheckpointResumeExpectation`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteResumeExpectation {
    /// 求解运行 ID。 / The solve run identity.
    pub run_id: String,
    /// 模型指纹。 / The model fingerprint.
    pub model_fingerprint: RemoteFingerprintSpec,
    /// 生效配置指纹。 / The effective configuration fingerprint.
    pub configuration_fingerprint: RemoteFingerprintSpec,
    /// solver/runtime 指纹。 / The solver-runtime fingerprint.
    pub solver_fingerprint: RemoteFingerprintSpec,
}

/// 一次远端工件编解码请求。 / One remote-artifact codec request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteArtifactRequest {
    /// 工件种类。 / The artifact kind.
    pub kind: RemoteArtifactKind,
    /// 线格式载荷（JSON 文本）。 / The wire payload (JSON text).
    pub payload: String,
    /// 操作；省略等价于 `decode`。 / The operation; omission means `decode`.
    #[serde(default = "default_operation")]
    pub operation: RemoteCodecOperation,
    /// 仅 `resume` 需要；缺失时该操作被拒。 / Required by `resume` only; the operation is refused without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expected: Option<RemoteResumeExpectation>,
}

/// 默认操作。 / The default operation.
fn default_operation() -> RemoteCodecOperation { RemoteCodecOperation::Decode }

/// 检查点形态 artifact 的专有身份字段。 / Identity fields specific to checkpoint-shaped artifacts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteCheckpointIdentity {
    /// 父 attempt ID；根 attempt 为空。 / The parent attempt identity; absent for a root attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parent_attempt_id: Option<String>,
    /// 检查点迭代号。 / The checkpoint iteration.
    pub iteration: u32,
    /// 可移植状态字节数。 / The portable state byte count.
    pub state_bytes: u32,
    /// 状态摘要，形如 `algorithm:value`。 / The state digest, shaped as `algorithm:value`.
    pub state_digest: String,
}

/// 一次解码的结论与身份回显。 / The verdict of one decode plus the identity echo.
///
/// 不同种类携带的身份不一样：检查点形态有 `checkpoint`（迭代号/状态摘要），结果形态只有整封包 `digest`。
/// 所以这些字段都是可选的，**缺省即"该种类没有这个字段"**，而不是被填成零值。
/// Different kinds carry different identities: checkpoint-shaped ones have `checkpoint` (iteration, state digest),
/// result-shaped ones only a whole-envelope `digest`. The fields are therefore optional, and an absent field means
/// "this kind has no such field" instead of being filled with a zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteArtifactReport {
    /// 工件种类。 / The artifact kind.
    pub kind: RemoteArtifactKind,
    /// 实际执行的操作。 / The operation that was performed.
    pub operation: RemoteCodecOperation,
    /// artifact 的 schema 版本。 / The artifact schema version.
    pub schema_version: String,
    /// 求解运行 ID；该种类不携带时为空。 / The solve run identity; absent when the kind does not carry one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub run_id: Option<String>,
    /// attempt ID；该种类不携带时为空。 / The attempt identity; absent when the kind does not carry one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub attempt_id: Option<String>,
    /// 检查点专有身份；结果形态为空。 / Checkpoint-specific identity; absent for result-shaped artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub checkpoint: Option<RemoteCheckpointIdentity>,
    /// 整封包摘要（`algorithm:value`）；检查点形态为空。 / The whole-envelope digest; absent for checkpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub digest: Option<String>,
    /// 规范化载荷是否与输入一致。框架强制规范的形式（CP 结果与 CP 检查点）成功解码即意味着 `canonical = true`。
    /// Whether the canonical payload equals the input. For forms the framework forces to be canonical (the CP result
    /// and the CP checkpoint), a successful decode implies `canonical = true`.
    pub canonical: bool,
    /// 规范化后的线格式文本。 / The canonical wire text.
    pub canonical_payload: String,
    /// 输入字节数。 / The input byte count.
    pub byte_length: u32,
}

/// 解码（可选恢复校验）一份远端工件。 / Decode (optionally resume-validating) one remote artifact.
pub fn decode_remote_artifact(
    kind: RemoteArtifactKind,
    payload: &str,
    operation: RemoteCodecOperation,
    expected: Option<&RemoteResumeExpectation>,
) -> Result<RemoteArtifactReport> {
    if payload.len() > 32 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "the remote artifact payload exceeds 32 MiB"));
    }
    let bytes = payload.as_bytes();
    match kind {
        RemoteArtifactKind::CheckpointArtifact => {
            let artifact = checkpoint_artifact_from_json(bytes).map_err(|error| {
                BridgeError::new("InvalidInput",
                    &format!("remote checkpoint artifact is not acceptable: {error}"))
            })?;
            let canonical_bytes = checkpoint_artifact_to_json(&artifact).map_err(|error| {
                BridgeError::new("CodecFailure",
                    &format!("remote checkpoint artifact cannot be re-encoded: {error}"))
            })?;
            let canonical_payload = utf8(canonical_bytes.clone())?;
            if operation == RemoteCodecOperation::Resume {
                let expected = expected.ok_or_else(|| BridgeError::new("InvalidInput",
                    "the resume operation requires the expected identity"))?;
                // 恢复校验由框架完成：本函数只把 DTO 转成领域的期望结构，不重复实现比对规则。
                // The framework performs the resume validation; this function only maps the DTO onto the domain
                // expectation and does not re-implement the comparison rules.
                let checkpoint = &artifact.checkpoint;
                let expectation = CheckpointResumeExpectation {
                    run_id: expected.run_id.clone(),
                    model_fingerprint: with_artifact_schema(&expected.model_fingerprint,
                        &checkpoint.model_fingerprint),
                    configuration_fingerprint: with_artifact_schema(&expected.configuration_fingerprint,
                        &checkpoint.configuration_fingerprint),
                    solver_fingerprint: with_artifact_schema(&expected.solver_fingerprint,
                        &checkpoint.solver_fingerprint),
                };
                validate_checkpoint_artifact_for_resume(&artifact, &expectation).map_err(|error| {
                    BridgeError::new("InvalidInput", &format!("checkpoint cannot be resumed: {error}"))
                })?;
            }
            let checkpoint = &artifact.checkpoint;
            Ok(RemoteArtifactReport {
                kind,
                operation,
                schema_version: checkpoint.schema_version.clone(),
                run_id: Some(checkpoint.run_id.clone()),
                attempt_id: Some(checkpoint.attempt_id.clone()),
                checkpoint: Some(checkpoint_identity(checkpoint, artifact.state.len())),
                digest: None,
                canonical: canonical_bytes == bytes,
                canonical_payload,
                byte_length: byte_length(bytes),
            })
        }
        RemoteArtifactKind::ConstraintProgrammingResult => {
            reject_resume(kind, operation)?;
            // 框架的 `*_from_json` 已做 DTO 校验与规范形式强制（重编码必须与输入逐字节相同），这里不重复实现。
            // The framework's `*_from_json` already validates the DTO and enforces canonicality (the re-encode must be
            // byte-identical), so that is not re-implemented here.
            let dto = constraint_programming_result_from_json(bytes).map_err(|error| {
                BridgeError::new("InvalidInput", &format!("CP result artifact is not acceptable: {error}"))
            })?;
            Ok(RemoteArtifactReport {
                kind,
                operation,
                schema_version: dto.schema_version.clone(),
                run_id: dto.run_id.clone(),
                attempt_id: dto.attempt_id.clone(),
                checkpoint: None,
                digest: Some(fingerprint_text(&dto.artifact_digest)),
                canonical: true,
                canonical_payload: payload.to_owned(),
                byte_length: byte_length(bytes),
            })
        }
        RemoteArtifactKind::LogicBasedBendersCheckpoint => {
            reject_resume(kind, operation)?;
            let artifact = logic_based_benders_checkpoint_from_json(bytes).map_err(|error| {
                BridgeError::new("InvalidInput",
                    &format!("LBB checkpoint artifact is not acceptable: {error}"))
            })?;
            let canonical_bytes = logic_based_benders_checkpoint_to_json(&artifact).map_err(|error| {
                BridgeError::new("CodecFailure",
                    &format!("LBB checkpoint artifact cannot be re-encoded: {error}"))
            })?;
            let canonical_payload = utf8(canonical_bytes.clone())?;
            // LBB 的分层结构与 CP 一致：通用检查点在外层，类型化 state 在内层。
            // The LBB layering matches CP: the generic checkpoint is the outer envelope and the typed state is inner.
            let checkpoint = &artifact.checkpoint.checkpoint;
            Ok(RemoteArtifactReport {
                kind,
                operation,
                schema_version: checkpoint.schema_version.clone(),
                run_id: Some(checkpoint.run_id.clone()),
                attempt_id: Some(checkpoint.attempt_id.clone()),
                checkpoint: Some(checkpoint_identity(checkpoint, artifact.checkpoint.state.len())),
                digest: None,
                canonical: canonical_bytes == bytes,
                canonical_payload,
                byte_length: byte_length(bytes),
            })
        }
        RemoteArtifactKind::ConstraintProgrammingCheckpoint => {
            reject_resume(kind, operation)?;
            let artifact = constraint_programming_checkpoint_from_json(bytes).map_err(|error| {
                BridgeError::new("InvalidInput",
                    &format!("CP checkpoint artifact is not acceptable: {error}"))
            })?;
            let canonical_bytes = constraint_programming_checkpoint_to_json(&artifact).map_err(|error| {
                BridgeError::new("CodecFailure",
                    &format!("CP checkpoint artifact cannot be re-encoded: {error}"))
            })?;
            let canonical_payload = utf8(canonical_bytes.clone())?;
            let checkpoint = &artifact.checkpoint.checkpoint;
            Ok(RemoteArtifactReport {
                kind,
                operation,
                schema_version: checkpoint.schema_version.clone(),
                run_id: Some(checkpoint.run_id.clone()),
                attempt_id: Some(checkpoint.attempt_id.clone()),
                checkpoint: Some(checkpoint_identity(checkpoint, artifact.checkpoint.state.len())),
                digest: None,
                canonical: canonical_bytes == bytes,
                canonical_payload,
                byte_length: byte_length(bytes),
            })
        }
    }
}

/// 只有通用检查点形态支持恢复校验；其余种类的恢复身份校验尚未绑定时**明确报错**，不做静默无操作。
/// Only the generic checkpoint shape supports resume validation; when the resume identity check is not bound yet for
/// another kind this reports it explicitly instead of silently doing nothing.
fn reject_resume(kind: RemoteArtifactKind, operation: RemoteCodecOperation) -> Result<()> {
    if operation == RemoteCodecOperation::Resume {
        return Err(BridgeError::new("UnsupportedDialect",
            &format!("resume validation is not bound yet for artifact kind {kind:?}")));
    }
    Ok(())
}

/// 检查点身份回显。 / The checkpoint identity echo.
fn checkpoint_identity(
    checkpoint: &ospf_rust_core::solver::checkpoint::SolveCheckpoint,
    state_bytes: usize,
) -> RemoteCheckpointIdentity {
    RemoteCheckpointIdentity {
        parent_attempt_id: checkpoint.parent_attempt_id.clone(),
        // 迭代号在领域里是 `usize`；超出 u32 只可能来自被篡改的输入，截到上限而不是静默回绕。
        // The iteration is a `usize` in the domain; an overflow past u32 can only come from a tampered input, so it is
        // clamped instead of silently wrapping.
        iteration: u32::try_from(checkpoint.iteration).unwrap_or(u32::MAX),
        state_bytes: u32::try_from(state_bytes).unwrap_or(u32::MAX),
        state_digest: fingerprint_text(&checkpoint.state_digest),
    }
}

/// 把指纹渲染成 `algorithm:value`。 / Render a fingerprint as `algorithm:value`.
fn fingerprint_text(fingerprint: &ospf_rust_core::solver::report::AuditFingerprint) -> String {
    format!("{}:{}", fingerprint.algorithm, fingerprint.value)
}

/// 把期望指纹与 artifact 的 schema 版本合成为一条完整指纹。
/// Combine an expected fingerprint with the artifact's schema version into a whole fingerprint.
fn with_artifact_schema(
    expected: &RemoteFingerprintSpec,
    artifact: &ospf_rust_core::solver::report::AuditFingerprint,
) -> ospf_rust_core::solver::report::AuditFingerprint {
    ospf_rust_core::solver::report::AuditFingerprint {
        schema_version: artifact.schema_version.clone(),
        algorithm: expected.algorithm.clone(),
        value: expected.value.clone(),
    }
}

/// 字节数安全转换。 / Safe byte-count conversion.
fn byte_length(bytes: &[u8]) -> u32 { u32::try_from(bytes.len()).unwrap_or(u32::MAX) }

/// 规范载荷转文本。 / Convert the canonical payload to text.
fn utf8(bytes: Vec<u8>) -> Result<String> {
    String::from_utf8(bytes)
        .map_err(|error| BridgeError::new("CodecFailure", &format!("canonical JSON is not UTF-8: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::model::constraint_programming::{
        ConstraintDefinition, ConstraintProgrammingConstraint, ConstraintProgrammingModel,
        ConstraintProgrammingSnapshot, IntegerDomain, IntegerExpression, IntegerObjective, IntegerRelation,
        IntegerVariable,
    };
    use ospf_rust_core::solver::checkpoint::{SolveCheckpoint, SolveCheckpointArtifact};
    use ospf_rust_core::ConstraintProgrammingCheckpointArtifact;
    use ospf_rust_core::solver::fingerprint::sha256_fingerprint;
    use ospf_rust_core::solver::report::{
        AuditFingerprint, ProblemStatus, SolveFingerprints, SolveProof, SolveReport, SolverProvenance,
        TerminationReason, CURRENT_SOLVE_REPORT_SCHEMA_VERSION,
    };
    use ospf_rust_framework::solver::remote::ospf_serializer::constraint_programming_result_to_json;

    /// 用核心自己的构造器造一份真实 CP 快照。 / Build a real CP snapshot with the core's own constructors.
    fn cp_snapshot() -> ConstraintProgrammingSnapshot {
        let x = IntegerVariable::new("x");
        let mut model = ConstraintProgrammingModel::new("remote-codec-cp");
        model.register_variable(x.clone(), IntegerDomain::range(0, 5).expect("domain"))
            .expect("variable");
        model.add_constraint(ConstraintDefinition::new("capacity",
            ConstraintProgrammingConstraint::integer(
                IntegerExpression::variable(x.clone()), IntegerRelation::LessOrEqual, 5)))
            .expect("constraint");
        model.set_objective(IntegerObjective::maximize(IntegerExpression::variable(x)));
        model.freeze().expect("snapshot")
    }

    fn fingerprint(value: &str) -> AuditFingerprint {
        AuditFingerprint {
            schema_version: CURRENT_SOLVE_REPORT_SCHEMA_VERSION.to_owned(),
            algorithm: "sha256".to_owned(),
            value: value.to_owned(),
        }
    }

    fn real_artifact() -> SolveCheckpointArtifact {
        let state = br#"{"iteration":7,"best":[1,2,3]}"#.to_vec();
        let checkpoint = SolveCheckpoint::new(
            "run-1", "attempt-1", None,
            fingerprint("model-a"), fingerprint("config-a"), fingerprint("solver-a"),
            SolverProvenance {
                solver_id: "scip".to_owned(), backend_name: "scip".to_owned(),
                backend_version: Some("8.0".to_owned()), thread_count: Some(1),
                ..Default::default()
            },
            7, Some(1.0), Some(2.0), Some(1.0),
            sha256_fingerprint("ospf.solve.checkpoint.state", &state),
        ).expect("checkpoint envelope");
        SolveCheckpointArtifact::new(checkpoint, state).expect("artifact")
    }

    fn real_cp_checkpoint() -> Vec<u8> {
        let snapshot = cp_snapshot();
        let state = br#"{"iteration":4}"#.to_vec();
        let checkpoint = SolveCheckpoint::new(
            "run-cp", "attempt-cp", None,
            snapshot.fingerprint.clone(), fingerprint("config-cp"), fingerprint("solver-cp"),
            SolverProvenance {
                solver_id: "cp".to_owned(), backend_name: "cp".to_owned(), backend_version: None,
                thread_count: Some(1), ..Default::default()
            },
            4, Some(2.0), Some(3.0), Some(0.5),
            sha256_fingerprint("ospf.solve.checkpoint.state", &state),
        ).expect("checkpoint envelope");
        let artifact = ConstraintProgrammingCheckpointArtifact::new_direct(
            checkpoint, &snapshot, Vec::new(), Default::default(), Default::default(),
        ).expect("CP checkpoint");
        constraint_programming_checkpoint_to_json(&artifact).expect("encode")
    }

    fn real_cp_result() -> Vec<u8> {
        let snapshot = cp_snapshot();
        // 最小合法 CP 报告：不可行 + 不可行证明。可行报告必须带已验证的 incumbent，所以这里不用可行状态。
        // The minimal valid CP report: infeasible plus an infeasibility proof. A feasible report must carry a validated
        // incumbent, so a feasible status is not used here.
        // 报告必须携带与快照一致的模型指纹，否则框架拒绝把它绑定成 CP 结果（实测的领域约束）。
        // The report must carry the snapshot's model fingerprint, otherwise the framework refuses to bind it as a CP
        // result (a measured domain constraint).
        let report = SolveReport::<i64>::builder(ProblemStatus::Infeasible, TerminationReason::Completed)
            .proof(SolveProof::infeasibility())
            .fingerprints(SolveFingerprints {
                model: Some(snapshot.fingerprint.clone()),
                ..SolveFingerprints::default()
            })
            .build()
            .expect("CP report");
        constraint_programming_result_to_json(&snapshot, report, Some("run-cp".to_owned()),
            Some("attempt-cp".to_owned())).expect("encode")
    }

    #[test]
    fn checkpoint_artifact_round_trips_and_echoes_identity() {
        let artifact = real_artifact();
        let payload = String::from_utf8(
            checkpoint_artifact_to_json(&artifact).expect("encode"),).expect("utf8");
        let report = decode_remote_artifact(RemoteArtifactKind::CheckpointArtifact, &payload,
            RemoteCodecOperation::Decode, None).unwrap();
        assert_eq!(report.kind, RemoteArtifactKind::CheckpointArtifact);
        assert_eq!(report.operation, RemoteCodecOperation::Decode);
        assert_eq!(report.run_id.as_deref(), Some("run-1"));
        assert_eq!(report.attempt_id.as_deref(), Some("attempt-1"));
        let checkpoint = report.checkpoint.as_ref().expect("checkpoint identity");
        assert_eq!(checkpoint.parent_attempt_id, None);
        assert_eq!(checkpoint.iteration, 7);
        assert!(checkpoint.state_bytes > 0);
        assert_eq!(checkpoint.state_digest,
            format!("sha256:{}", artifact.checkpoint.state_digest.value));
        assert_eq!(report.digest, None);
        assert_eq!(report.byte_length, payload.len() as u32);
        // 框架自己的编码器产物必须是规范的：往返一次字节不变。
        // The framework's own encoder output must be canonical: one round-trip keeps the bytes unchanged.
        assert!(report.canonical);
        assert_eq!(report.canonical_payload, payload);
    }

    #[test]
    fn reformatted_and_tampered_payloads_are_refused() {
        let artifact = real_artifact();
        let payload = String::from_utf8(
            checkpoint_artifact_to_json(&artifact).expect("encode"),).expect("utf8");

        // 实测结论：这份 DTO 自带**整封包完整性摘要**，所以"合法但非规范"根本活不下来——改一个空白字节就会
        // 摘要不匹配。于是 `canonical` 在解码成功时恒为 true，这不是冗余字段，而是"重编码结果与输入一致"的
        // 持续断言。
        // Measured: this DTO carries an **envelope integrity digest**, so "valid but non-canonical" cannot survive —
        // changing a single whitespace byte breaks the digest. `canonical` is therefore always true on a successful
        // decode; that is not a redundant field but a standing assertion that the re-encode matches the input.
        let spaced = payload.replace('{', "{ ").replace('}', " }");
        assert_ne!(spaced, payload);
        assert!(decode_remote_artifact(RemoteArtifactKind::CheckpointArtifact, &spaced,
            RemoteCodecOperation::Decode, None).is_err(),
            "reformatting must break the envelope integrity digest");

        // 实测：顶层没有 `iteration`（它在 `snapshotJson` 与被摘要覆盖的 assumptions 里），所以改 `gap`。
        // Measured: there is no top-level `iteration` (it lives inside `snapshotJson` and the digested assumptions), so
        // mutate `gap` instead.
        let tampered = payload.replace("\"gap\":\"1\"", "\"gap\":\"0.5\"");
        assert_ne!(tampered, payload, "test fixture must actually change the payload");
        assert!(decode_remote_artifact(RemoteArtifactKind::CheckpointArtifact, &tampered,
            RemoteCodecOperation::Decode, None).is_err());

        assert!(decode_remote_artifact(RemoteArtifactKind::CheckpointArtifact, "not json",
            RemoteCodecOperation::Decode, None).is_err());
        assert!(decode_remote_artifact(RemoteArtifactKind::CheckpointArtifact, "",
            RemoteCodecOperation::Decode, None).is_err());
        assert!(decode_remote_artifact(RemoteArtifactKind::CheckpointArtifact,
            &"x".repeat(33 * 1024 * 1024), RemoteCodecOperation::Decode, None).is_err());
    }

    #[test]
    fn resume_accepts_matching_identity_and_refuses_mismatches() {
        let artifact = real_artifact();
        let payload = String::from_utf8(
            checkpoint_artifact_to_json(&artifact).expect("encode"),).expect("utf8");
        // 期望值取自真实 artifact 自己（测试是在验证比对规则，不是在猜指纹格式）。
        // The expectations come from the real artifact itself: the test verifies the comparison rules, it does not guess
        // the fingerprint format.
        let expectation = |run_id: &str, model: &str| RemoteResumeExpectation {
            run_id: run_id.to_owned(),
            model_fingerprint: RemoteFingerprintSpec {
                algorithm: artifact.checkpoint.model_fingerprint.algorithm.clone(), value: model.to_owned() },
            configuration_fingerprint: RemoteFingerprintSpec {
                algorithm: artifact.checkpoint.configuration_fingerprint.algorithm.clone(),
                value: artifact.checkpoint.configuration_fingerprint.value.clone() },
            solver_fingerprint: RemoteFingerprintSpec {
                algorithm: artifact.checkpoint.solver_fingerprint.algorithm.clone(),
                value: artifact.checkpoint.solver_fingerprint.value.clone() },
        };
        let model_value = artifact.checkpoint.model_fingerprint.value.clone();

        let report = decode_remote_artifact(RemoteArtifactKind::CheckpointArtifact, &payload,
            RemoteCodecOperation::Resume, Some(&expectation("run-1", &model_value))).unwrap();
        assert_eq!(report.operation, RemoteCodecOperation::Resume);
        assert_eq!(report.run_id.as_deref(), Some("run-1"));

        let error = decode_remote_artifact(RemoteArtifactKind::CheckpointArtifact, &payload,
            RemoteCodecOperation::Resume, Some(&expectation("run-2", &model_value))).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("resume"), "{}", error.message);

        let error = decode_remote_artifact(RemoteArtifactKind::CheckpointArtifact, &payload,
            RemoteCodecOperation::Resume, Some(&expectation("run-1", "another-model"))).unwrap_err();
        assert_eq!(error.code, "InvalidInput");

        let error = decode_remote_artifact(RemoteArtifactKind::CheckpointArtifact, &payload,
            RemoteCodecOperation::Resume, None).unwrap_err();
        assert_eq!(error.code, "InvalidInput");

        // decode 带上多余的期望身份是无害的：期望只在 resume 时被使用。
        // Passing an expectation to a decode is harmless: expectations are only used for resume.
        let report = decode_remote_artifact(RemoteArtifactKind::CheckpointArtifact, &payload,
            RemoteCodecOperation::Decode, Some(&expectation("run-2", "another-model"))).unwrap();
        assert_eq!(report.operation, RemoteCodecOperation::Decode);
    }

    #[test]
    fn cp_result_round_trips_with_envelope_digest() {
        let payload = String::from_utf8(real_cp_result()).expect("utf8");
        let report = decode_remote_artifact(RemoteArtifactKind::ConstraintProgrammingResult, &payload,
            RemoteCodecOperation::Decode, None).unwrap();
        assert_eq!(report.kind, RemoteArtifactKind::ConstraintProgrammingResult);
        assert_eq!(report.run_id.as_deref(), Some("run-cp"));
        assert_eq!(report.attempt_id.as_deref(), Some("attempt-cp"));
        // 结果形态没有检查点字段，但有整封包摘要——这是"缺省即不存在"，不是零值填充。
        // The result shape has no checkpoint fields but does carry an envelope digest — that is "absent means absent",
        // not zero padding.
        assert_eq!(report.checkpoint, None);
        assert!(report.digest.as_ref().expect("digest").starts_with("sha256:"), "{:?}", report.digest);
        // 框架对该形态强制规范，所以成功解码即规范。 / The framework forces canonicality, so success implies canonical.
        assert!(report.canonical);
        assert_eq!(report.canonical_payload, payload);
        assert_eq!(report.byte_length, payload.len() as u32);

        // 实测：`serde_json::Value` 往返得到**逐字节相同**的文本，所以不能用"重新序列化"来演示规范形式强制。
        // 改用尾部空白——JSON 仍合法，但与重编码结果不再逐字节相同，于是必须被拒。
        // Measured: a `serde_json::Value` round-trip yields **byte-identical** text, so re-serialization cannot
        // demonstrate the canonical-form enforcement. Trailing whitespace is used instead — still valid JSON, but no
        // longer byte-identical to the re-encode, so it must be refused.
        assert!(decode_remote_artifact(RemoteArtifactKind::ConstraintProgrammingResult, &format!("{payload} "),
            RemoteCodecOperation::Decode, None).is_err(),
            "a trailing space must be refused as non-canonical");
        assert!(decode_remote_artifact(RemoteArtifactKind::ConstraintProgrammingResult, "not json",
            RemoteCodecOperation::Decode, None).is_err());
    }

    #[test]
    fn cp_checkpoint_round_trips_and_refuses_unbound_resume() {
        let payload = String::from_utf8(real_cp_checkpoint()).expect("utf8");
        let report = decode_remote_artifact(RemoteArtifactKind::ConstraintProgrammingCheckpoint, &payload,
            RemoteCodecOperation::Decode, None).unwrap();
        assert_eq!(report.run_id.as_deref(), Some("run-cp"));
        let checkpoint = report.checkpoint.as_ref().expect("checkpoint identity");
        assert_eq!(checkpoint.iteration, 4);
        assert!(checkpoint.state_bytes > 0);
        assert!(checkpoint.state_digest.starts_with("sha256:"));
        assert_eq!(report.digest, None);
        assert!(report.canonical);
        assert_eq!(report.canonical_payload, payload);

        // CP 检查点的恢复身份校验还没绑：resume 必须明确报错，而不是静默当成 decode。
        // The CP checkpoint's resume identity check is not bound yet: resume must fail explicitly instead of silently
        // acting as a decode.
        let error = decode_remote_artifact(RemoteArtifactKind::ConstraintProgrammingCheckpoint, &payload,
            RemoteCodecOperation::Resume, None).unwrap_err();
        assert_eq!(error.code, "UnsupportedDialect");
        assert!(error.message.contains("not bound yet"), "{}", error.message);
        assert_eq!(decode_remote_artifact(RemoteArtifactKind::ConstraintProgrammingResult, &payload,
            RemoteCodecOperation::Resume, None).unwrap_err().code, "UnsupportedDialect");
    }

    fn real_lbb_checkpoint() -> Vec<u8> {
        use ospf_rust_core::model::ObjectiveCategory;
        use ospf_rust_framework::solver::logic_based_benders::{
            LogicBasedBendersMode, MasterBinding, MasterVariableBinding,
        };
        use ospf_rust_framework::solver::logic_based_benders_checkpoint::{
            LogicBasedBendersCheckpointArtifact, LogicBasedBendersCheckpointState,
        };
        let binding = MasterBinding::new([MasterVariableBinding::binary("master/use-cp")]).expect("binding");
        let state = LogicBasedBendersCheckpointState::new(
            &binding, LogicBasedBendersMode::Heuristic, 1e-9, ObjectiveCategory::Minimum,
            // 迭代 0 + 空快照：非零迭代要求最后一条 iteration snapshot 与迭代号一致，而构造真实快照
            // 需要更多运行时数据；0 是能自洽的最小值。
            // Iteration 0 with no snapshots: a non-zero iteration requires the final iteration snapshot to match it, and
            // building a real snapshot needs more runtime data; 0 is the smallest self-consistent value.
            Vec::new(), Vec::new(), Vec::new(), 0, false, fingerprint("model-lbb"), None, None,
        ).expect("LBB state");
        // `new` 不填 master 指纹，但 artifact 绑定要求它存在（框架自己的实测约束）。
        // `new` does not fill the master fingerprint, but artifact binding requires it (a measured framework constraint).
        let mut state = state;
        state.master_fingerprint = Some(fingerprint("master-lbb"));
        let raw = br#"{"iteration":3}"#.to_vec();
        let checkpoint = SolveCheckpoint::new(
            "run-lbb", "attempt-lbb", None, fingerprint("model-lbb"), fingerprint("config-lbb"),
            fingerprint("solver-lbb"),
            SolverProvenance {
                solver_id: "scip".to_owned(), backend_name: "scip".to_owned(), backend_version: None,
                thread_count: Some(1), ..Default::default()
            },
            0, Some(1.0), Some(2.0), Some(1.0),
            sha256_fingerprint("ospf.solve.checkpoint.state", &raw),
        ).expect("checkpoint envelope");
        let artifact = LogicBasedBendersCheckpointArtifact::new(checkpoint, state).expect("LBB checkpoint");
        logic_based_benders_checkpoint_to_json(&artifact).expect("encode")
    }

    #[test]
    fn lbb_checkpoint_round_trips_and_refuses_unbound_resume() {
        let payload = String::from_utf8(real_lbb_checkpoint()).expect("utf8");
        let report = decode_remote_artifact(RemoteArtifactKind::LogicBasedBendersCheckpoint, &payload,
            RemoteCodecOperation::Decode, None).unwrap();
        assert_eq!(report.kind, RemoteArtifactKind::LogicBasedBendersCheckpoint);
        assert_eq!(report.run_id.as_deref(), Some("run-lbb"));
        assert_eq!(report.attempt_id.as_deref(), Some("attempt-lbb"));
        let checkpoint = report.checkpoint.as_ref().expect("checkpoint identity");
        assert_eq!(checkpoint.iteration, 0);
        assert!(checkpoint.state_bytes > 0);
        assert!(checkpoint.state_digest.starts_with("sha256:"));
        assert_eq!(report.digest, None);
        assert!(report.canonical);
        assert_eq!(report.canonical_payload, payload);

        // LBB 检查点的恢复身份校验还没绑：resume 必须明确报错。 / LBB resume identity is not bound: explicit error.
        let error = decode_remote_artifact(RemoteArtifactKind::LogicBasedBendersCheckpoint, &payload,
            RemoteCodecOperation::Resume, None).unwrap_err();
        assert_eq!(error.code, "UnsupportedDialect");
        assert!(error.message.contains("not bound yet"), "{}", error.message);
    }
}
