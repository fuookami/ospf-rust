//! 批次 D 第 3 单：Constraint Programming 检查点的结构校验与类型化状态投影。
//! Batch D, delivery 3: the Constraint Programming checkpoint structure and its typed-state projection.
//!
//! CP 检查点的线格式早已能通过 `remote_codec` 往返，但 TS 侧只拿到"外层信封的身份"（run / attempt / schema），
//! **类型化 state 一直是黑箱**。这一单把 `ConstraintProgrammingCheckpointArtifact::from_json` 解出来的东西投影出来：
//! 引擎标识、恢复级别、迭代号、assumption 数、solution hint 条目数、primitive state 的键。
//!
//! 顺带把这些规则变成可验证的公开结论（全部来自领域 `validate`，桥接不重写）：
//! * state 的 `schemaVersion` 与 `kind` 必须精确匹配当前版本；`restoreMode` 只允许 `RebuildFromSnapshot`；
//! * `engine` 不能空白；snapshot artifact 自身必须自洽；
//! * assumptions 必须按稳定 ID **严格递增**（既不许乱序，也不许重复）；
//! * **完整 solution hint 必须作为整体赋值复验**——覆盖全部变量时，领域会额外跑值域 + interval + 逐约束的
//!   `validate_assignment`。这是 TS 侧最不可能自己实现的一条，也是这一单最有价值的部分；
//! * `primitiveState` 的键不能空白；
//! * 外层 `checkpoint.state` 的规范字节必须与类型化 state 完全一致；
//! * 输入必须是**规范 JSON**（重新编码后逐字节相同），否则拒绝。
//!
//! CP checkpoints already round-trip through `remote_codec`, but TypeScript only ever saw the **envelope identity**
//! (run / attempt / schema) while the **typed state stayed opaque**. This delivery projects what
//! `ConstraintProgrammingCheckpointArtifact::from_json` decodes: the engine, the restore level, the iteration, the
//! assumption count, the solution-hint size and the primitive-state keys. It also turns the domain rules into verifiable
//! public conclusions (the bridge re-implements none of them): exact `schemaVersion` and `kind`, a `restoreMode` limited to
//! `RebuildFromSnapshot`, a non-blank engine, a self-consistent snapshot artifact, assumptions in **strictly increasing**
//! stable-ID order, a **complete solution hint revalidated as a whole assignment** (domains + intervals + every
//! constraint), non-blank primitive-state keys, typed state byte-identical to the envelope's canonical bytes, and
//! **canonical JSON** input (re-encoding must be byte-identical) or it is refused.

use ospf_rust_core::solver::constraint_programming::checkpoint::ConstraintProgrammingCheckpointArtifact;
use ospf_rust_core::solver::constraint_programming::checkpoint::ConstraintProgrammingRestoreMode;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// CP 检查点结构校验请求。 / The CP checkpoint structure request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpCheckpointRequest {
    /// portable CP 检查点的规范 JSON 文本。 / The canonical JSON text of the portable CP checkpoint.
    pub payload: String,
}

/// CP 检查点的类型化状态投影。 / The typed-state projection of a CP checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpCheckpointReport {
    /// 检查点信封的 schema 版本。 / The envelope schema version.
    pub schema_version: String,
    /// 运行 ID。 / The run ID.
    pub run_id: String,
    /// attempt ID。 / The attempt ID.
    pub attempt_id: String,
    /// 父 attempt ID。 / The parent attempt ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parent_attempt_id: Option<String>,
    /// 类型化 state 的 schema 版本。 / The typed-state schema version.
    pub state_schema_version: String,
    /// 类型化 state 的种类。 / The typed-state kind.
    pub state_kind: String,
    /// 恢复级别（目前只有 `RebuildFromSnapshot`）。 / The restore level (only `RebuildFromSnapshot` today).
    pub restore_mode: String,
    /// 产生该 state 的 CP 引擎。 / The CP engine that produced the state.
    pub engine: String,
    /// 算法迭代号。 / The algorithm iteration.
    pub iteration: u64,
    /// 重新应用的 assumption 条数。 / How many assumptions are reapplied.
    pub assumption_count: u32,
    /// solution hint 的条目数（0 表示没有 hint）。 / The solution-hint size (0 means no hint).
    pub solution_hint_count: u32,
    /// primitive state 的键数。 / The number of primitive-state keys.
    pub primitive_state_count: u32,
    /// primitive state 的键（按键排序，值不外泄）。 / The primitive-state keys (sorted; values are not exposed).
    pub primitive_state_keys: Vec<String>,
    /// CP snapshot 的模型指纹值。 / The CP snapshot's model fingerprint value.
    pub snapshot_fingerprint: String,
    /// CP snapshot 的变量数。 / The CP snapshot's variable count.
    pub snapshot_variable_count: u32,
    /// CP snapshot 的约束数。 / The CP snapshot's constraint count.
    pub snapshot_constraint_count: u32,
    /// 输入是否已是规范 JSON（实测恒为真：非规范输入在 `from_json` 就被拒）。
    /// Whether the input was already canonical JSON (always true in practice: non-canonical input is refused).
    pub canonical_input: bool,
    /// 规范化后的载荷文本（可再提交，必须逐字一致）。 / The normalized payload text (re-submittable, must match).
    pub canonical_payload: String,
}

/// 解码、校验并投影一份 portable CP 检查点。非法输入是**错误**，原因原样带出。
/// Decode, validate and project a portable CP checkpoint. An illegal input is an **error** carrying the reason verbatim.
pub fn verify_cp_checkpoint(request: &CpCheckpointRequest) -> Result<CpCheckpointReport> {
    if request.payload.len() > 32 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "CP checkpoint payload exceeds 32 MiB"));
    }
    let bytes = request.payload.as_bytes();
    // 领域入口：解码 → 校验（含 assumption 顺序、完整 hint 整体复验、state 双表示一致）→ 规范字节必须与输入相同。
    // The domain entry: decode, validate (assumption order, whole-assignment hint revalidation, both state
    // representations agreeing), and require the canonical bytes to equal the input.
    let artifact = ConstraintProgrammingCheckpointArtifact::from_json(bytes).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("CP checkpoint failed validation: {error}"))
    })?;
    let canonical = artifact.to_json().map_err(|error| {
        BridgeError::new("CodecFailure", &format!("CP checkpoint could not be re-encoded: {error}"))
    })?;
    let canonical_payload = String::from_utf8(canonical).map_err(|error| {
        BridgeError::new("CodecFailure", &format!("CP checkpoint canonical bytes are not UTF-8: {error}"))
    })?;
    let state = &artifact.state;
    Ok(CpCheckpointReport {
        schema_version: artifact.checkpoint.checkpoint.schema_version.clone(),
        run_id: artifact.checkpoint.checkpoint.run_id.clone(),
        attempt_id: artifact.checkpoint.checkpoint.attempt_id.clone(),
        parent_attempt_id: artifact.checkpoint.checkpoint.parent_attempt_id.clone(),
        state_schema_version: state.schema_version.clone(),
        state_kind: state.kind.clone(),
        restore_mode: match state.restore_mode {
            ConstraintProgrammingRestoreMode::RebuildFromSnapshot => "RebuildFromSnapshot".to_owned(),
        },
        engine: state.engine.clone(),
        iteration: u64::try_from(state.iteration).unwrap_or(u64::MAX),
        assumption_count: count(state.assumptions.len()),
        solution_hint_count: count(state.solution_hint.len()),
        primitive_state_count: count(state.primitive_state.len()),
        primitive_state_keys: state.primitive_state.keys().cloned().collect(),
        snapshot_fingerprint: state.snapshot.snapshot.fingerprint.value.clone(),
        snapshot_variable_count: count(state.snapshot.snapshot.variables.len()),
        snapshot_constraint_count: count(state.snapshot.snapshot.constraints.len()),
        canonical_input: canonical_payload.as_bytes() == bytes,
        canonical_payload,
    })
}

/// 计数字段的安全转换。 / A safe cast for count fields.
fn count(value: usize) -> u32 { u32::try_from(value).unwrap_or(u32::MAX) }

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::model::constraint_programming::{
        ConstraintDefinition, ConstraintProgrammingConstraint, ConstraintProgrammingModel,
        ConstraintProgrammingSnapshot, IntegerDomain, IntegerExpression, IntegerObjective, IntegerRelation,
        IntegerVariable,
    };
    use ospf_rust_core::solver::checkpoint::SolveCheckpoint;
    use ospf_rust_core::solver::fingerprint::sha256_fingerprint;
    use ospf_rust_core::solver::report::{AuditFingerprint, SolverProvenance,
        CURRENT_SOLVE_REPORT_SCHEMA_VERSION};

    fn fingerprint(value: &str) -> AuditFingerprint {
        AuditFingerprint {
            schema_version: CURRENT_SOLVE_REPORT_SCHEMA_VERSION.to_owned(),
            algorithm: "sha256".to_owned(),
            value: value.to_owned(),
        }
    }

    fn snapshot() -> ConstraintProgrammingSnapshot {
        let x = IntegerVariable::new("x");
        let mut model = ConstraintProgrammingModel::new("cp-checkpoint-probe");
        model.register_variable(x.clone(), IntegerDomain::range(0, 5).expect("domain")).expect("variable");
        model.add_constraint(ConstraintDefinition::new("capacity",
            ConstraintProgrammingConstraint::integer(
                IntegerExpression::variable(x.clone()), IntegerRelation::LessOrEqual, 5)))
            .expect("constraint");
        model.set_objective(IntegerObjective::maximize(IntegerExpression::variable(x)));
        model.freeze().expect("snapshot")
    }

    /// 一份真实构造出来的 portable CP 检查点。 / A genuinely constructed portable CP checkpoint.
    fn payload(engine: &str, primitive: &[(&str, &str)]) -> String {
        let snapshot = snapshot();
        let state = br#"{"iteration":4}"#.to_vec();
        let checkpoint = SolveCheckpoint::new(
            "run-cp", "attempt-cp", None,
            snapshot.fingerprint.clone(), fingerprint("config-cp"), fingerprint("solver-cp"),
            SolverProvenance { solver_id: "cp".to_owned(), backend_name: "cp".to_owned(),
                backend_version: None, thread_count: Some(1), ..Default::default() },
            4, Some(2.0), Some(3.0), Some(0.5),
            sha256_fingerprint("ospf.solve.checkpoint.state", &state),
        ).expect("checkpoint envelope");
        let artifact = ConstraintProgrammingCheckpointArtifact::new_direct(
            checkpoint, &snapshot, Vec::new(), Default::default(),
            primitive.iter().map(|(key, value)| ((*key).to_owned(), (*value).to_owned())).collect(),
        ).expect("CP checkpoint");
        let _ = engine;
        String::from_utf8(artifact.to_json().expect("encode")).expect("utf8")
    }

    fn request(payload: &str) -> CpCheckpointRequest { CpCheckpointRequest { payload: payload.to_owned() } }

    #[test]
    fn typed_state_is_projected_and_round_trips() {
        let text = payload("direct-cp", &[("backend", "fake"), ("bound", "5")]);
        let report = verify_cp_checkpoint(&request(&text)).unwrap();
        assert_eq!(report.schema_version, "1.0");
        assert_eq!(report.run_id, "run-cp");
        assert_eq!(report.attempt_id, "attempt-cp");
        assert_eq!(report.parent_attempt_id, None);
        // 类型化 state 的固定取值（都来自领域校验）。 / The typed-state constants, all enforced by the domain.
        assert_eq!(report.state_schema_version, "1.0");
        assert_eq!(report.restore_mode, "RebuildFromSnapshot");
        assert_eq!(report.engine, "direct-cp");
        assert_eq!(report.iteration, 4);
        assert_eq!(report.assumption_count, 0);
        assert_eq!(report.solution_hint_count, 0);
        // primitive state 的键按序投影，值不外泄。 / Keys are projected in order; values are not exposed.
        assert_eq!(report.primitive_state_count, 2);
        assert_eq!(report.primitive_state_keys, vec!["backend".to_owned(), "bound".to_owned()]);
        // snapshot 的投影。 / The snapshot projection.
        assert_eq!(report.snapshot_variable_count, 1);
        assert_eq!(report.snapshot_constraint_count, 1);
        assert_eq!(report.snapshot_fingerprint.len(), 64, "sha256 hex");
        assert!(report.canonical_input);
        // 规范载荷再提交：逐字一致。 / Re-submitting the canonical payload: byte-identical.
        let again = verify_cp_checkpoint(&request(&report.canonical_payload)).unwrap();
        assert_eq!(again.canonical_payload, report.canonical_payload);
        assert_eq!(again.snapshot_fingerprint, report.snapshot_fingerprint);
    }

    #[test]
    fn non_canonical_and_broken_payloads_are_refused() {
        let text = payload("direct-cp", &[]);

        // 非规范 JSON（多了一个空格）：领域要求规范字节，拒绝。
        // Non-canonical JSON (one extra space): the domain requires canonical bytes and refuses it.
        let pretty = text.replacen("{", "{ ", 1);
        let error = verify_cp_checkpoint(&request(&pretty)).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("failed validation"), "{}", error.message);

        // 不是 JSON。 / Not JSON at all.
        assert!(verify_cp_checkpoint(&request("not json")).unwrap_err().message.contains("failed validation"));

        // 篡改类型化 state 的 engine 为空：领域拒绝。
        // Blank the typed-state engine: the domain refuses it.
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["state"]["engine"] = serde_json::json!("   ");
        let error = verify_cp_checkpoint(&request(&value.to_string())).unwrap_err();
        assert!(error.message.contains("engine cannot be blank") || error.message.contains("failed validation"),
            "{}", error.message);

        // 篡改 state 的 schema 版本：领域拒绝。
        // Tamper with the state schema version: the domain refuses it.
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["state"]["schemaVersion"] = serde_json::json!("9.9");
        let error = verify_cp_checkpoint(&request(&value.to_string())).unwrap_err();
        assert!(error.message.contains("failed validation"), "{}", error.message);

        // 空白 primitive-state 键：**构造器**就先拒绝（实测），所以只能靠篡改载荷来测这一条。
        // A blank primitive-state key is refused by the **constructor** first (measured), so this rule is exercised by
        // tampering with the payload.
        let mut blank_key: serde_json::Value = serde_json::from_str(&(payload("direct-cp", &[("backend", "x")]))).unwrap();
        let keys = blank_key["state"]["primitiveState"].as_object().unwrap()
            .keys().cloned().collect::<Vec<_>>();
        let value = blank_key["state"]["primitiveState"][keys[0].as_str()].clone();
        blank_key["state"]["primitiveState"] = serde_json::json!({ "  ": value });
        let error = verify_cp_checkpoint(&request(&blank_key.to_string())).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("failed validation"), "{}", error.message);

        // 超限。 / Over the limit.
        let huge = "x".repeat(33 * 1024 * 1024);
        assert_eq!(verify_cp_checkpoint(&request(&huge)).unwrap_err().code, "InvalidInput");
    }
}
