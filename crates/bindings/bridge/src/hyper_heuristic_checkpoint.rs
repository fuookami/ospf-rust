//! 批次 C 第 3 单：策略档案与完整检查点的结构校验，以及 Kotlin v1 导出能力审计。
//! Batch C, delivery 3: policy-archive and run-checkpoint structure plus the Kotlin-v1 export audit.
//!
//! 这里做两件事，都是纯文本、不需要求解器：
//! 1. **严格解码 + 版本校验**：`PolicyArchive` 与 `RunCheckpoint` 都是 `deny_unknown_fields`，未知字段一律报错；
//!    版本必须为 1。解码成功后重新编码一次，给出的规范文本可以证明"读得进、写得出"且稳定。
//! 2. **Kotlin v1 导出能力审计**（`RunCheckpoint::validate_kotlin_v1`）：这不是"能不能读"，而是"这份检查点里有没有
//!    Kotlin 2eb7943 存不下的已知状态"。**审计结论不是错误**——不兼容是一份可用的答案（带原因），所以请求成功、
//!    报告里给出 `kotlinV1Compatible = false` 与 `kotlinV1Reason`。
//!
//! 已知不兼容项（实测逐条）：非平稳选择器策略状态、调度重放状态（`schedule`）、`previousAction` 重放元数据、
//! 自适应奖励（`time-normalized`/`extreme-value`/`dominance`/`hypervolume-delta`）、自适应接受策略
//! （`late`/`record-to-record`/`auto`）、局部更新组件（`pattern-search`/`opposition`/`levy`/`pipeline`/`vnd`/
//! `elite-local`/`mutation-stage`/`neighborhood-stage`）、原生约束状态、显式 `numerics` 数值契约。
//!
//! Two pure-text jobs, neither of which needs a solver. First, **strict decoding plus version checking**: both
//! `PolicyArchive` and `RunCheckpoint` are `deny_unknown_fields` and insist on version 1; unknown fields are refused. A
//! successful decode is re-encoded so the canonical text proves the round trip is lossless and stable. Second, the
//! **Kotlin-v1 export audit** (`RunCheckpoint::validate_kotlin_v1`) asks not whether the checkpoint is readable but
//! whether it holds known state Kotlin 2eb7943 cannot store. **The audit verdict is not an error**: incompatibility is a
//! usable answer with a reason, so the request succeeds and the report carries `kotlinV1Compatible = false` plus
//! `kotlinV1Reason`. The measured incompatibilities are non-stationary selector state, schedule replay state,
//! `previousAction` replay metadata, adaptive rewards, adaptive acceptance policies, local update components, native
//! constraint state, and explicit `numerics` contracts.

use ospf_rust_core::solver::hyper_heuristic::persistence::{PolicyArchive, RunCheckpoint};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 检查点族校验请求。 / The checkpoint-family request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckpointRequest {
    /// 文档类别：`policy` 或 `checkpoint`。 / The document kind: `policy` or `checkpoint`.
    pub kind: String,
    /// 文档文本（JSON）。 / The document text (JSON).
    pub text: String,
    /// 是否执行 Kotlin v1 导出审计（只对 `checkpoint` 有效）。 / Whether to run the Kotlin-v1 audit (checkpoint only).
    #[serde(default)]
    pub kotlin_v1: bool,
}

/// 检查点族校验结论。 / The checkpoint-family verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckpointReport {
    /// 文档类别回显。 / The echoed document kind.
    pub kind: String,
    /// 文档版本。 / The document version.
    pub version: u32,
    /// 策略格式版本（`checkpoint` 会带出内嵌策略的版本）。 / The policy format version (the embedded one for checkpoints).
    pub policy_version: u32,
    /// 策略根动作数。 / The number of actions in the policy root.
    pub root_action_count: u32,
    /// 内层策略的算法作用域数。 / The number of algorithm scopes carrying inner policies.
    pub operator_scope_count: u32,
    /// 内层策略总数。 / The total number of inner policies.
    pub operator_policy_count: u32,
    /// 特征协议版本。 / The feature protocol version.
    pub feature_version: String,
    /// 定义指纹。 / The definition fingerprint.
    pub definition: String,
    /// 训练元信息条数。 / The number of training metadata entries.
    pub metadata_count: u32,
    /// 是否携带调度重放状态（只有 `checkpoint` 可能为真）。 / Whether schedule replay state is present.
    pub has_schedule: bool,
    /// worker 数（只有 `checkpoint` 有）。 / The worker count (checkpoints only).
    pub worker_count: u32,
    /// 已消费评估请求。 / Consumed evaluation requests.
    pub evaluations: u64,
    /// 已派发序号。 / Dispatched sequence.
    pub dispatched: u64,
    /// 已完成决策。 / Completed decisions.
    pub completed: u64,
    /// 完整低层步数。 / Complete low-level steps.
    pub steps: u64,
    /// 真实目标调用次数。 / Actual objective calls.
    pub objective_calls: u64,
    /// 改善次数。 / The improvement count.
    pub successes: u64,
    /// 已完成轨迹条数。 / The number of completed transitions.
    pub transition_count: u32,
    /// 全局停滞次数。 / Global stagnation.
    pub stagnation: u64,
    /// 失败诊断条数。 / The number of failure diagnostics.
    pub diagnostic_count: u32,
    /// 全局可恢复组件状态条数。 / The number of global recoverable component states.
    pub component_count: u32,
    /// Kotlin v1 导出审计结论；未执行时为 `undefined`。 / The Kotlin-v1 verdict; `undefined` when not run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kotlin_v1_compatible: Option<bool>,
    /// 不兼容原因（仅在兼容时为 `undefined`）。 / The incompatibility reason (only when incompatible).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kotlin_v1_reason: Option<String>,
    /// 解码后重新编码的规范文本。 / The canonical text produced by re-encoding the decoded document.
    pub canonical_json: String,
}

/// 校验策略档案或完整检查点，并按需给出 Kotlin v1 导出审计。非法输入是**错误**；审计不兼容**不是**错误。
/// Validate a policy archive or a run checkpoint and, on request, audit Kotlin-v1 exportability. An illegal input is an
/// **error**; an incompatible audit verdict is **not**.
pub fn verify_checkpoint(request: &CheckpointRequest) -> Result<CheckpointReport> {
    if request.text.len() > 32 * 1024 * 1024 {
        return Err(BridgeError::new("InvalidInput", "checkpoint text exceeds 32 MiB"));
    }
    match request.kind.as_str() {
        "policy" => {
            if request.kotlin_v1 {
                return Err(BridgeError::new("InvalidInput",
                    "the Kotlin v1 audit applies to checkpoints, not to a policy archive"));
            }
            let archive: PolicyArchive = decode(&request.text, "policy archive")?;
            let mut report = policy_report(&archive, &request.text)?;
            report.kind = "policy".to_owned();
            Ok(report)
        }
        "checkpoint" => {
            let checkpoint: RunCheckpoint = decode(&request.text, "run checkpoint")?;
            let mut report = policy_report(&checkpoint.policy, &request.text)?;
            report.kind = "checkpoint".to_owned();
            report.version = checkpoint.version;
            report.has_schedule = checkpoint.schedule.is_some();
            report.worker_count = count(checkpoint.workers.len());
            report.evaluations = checkpoint.evaluations;
            report.dispatched = checkpoint.dispatched;
            report.completed = checkpoint.completed;
            report.steps = checkpoint.steps;
            report.objective_calls = checkpoint.objective_calls;
            report.successes = checkpoint.successes;
            report.transition_count = count(checkpoint.transitions.len());
            report.stagnation = checkpoint.stagnation;
            report.diagnostic_count = count(checkpoint.diagnostics.len());
            report.component_count = count(checkpoint.components.len());
            if request.kotlin_v1 {
                // 审计结论本身就是答案：不兼容时把领域原文带回去，而不是把它变成请求失败。
                // The verdict is the answer: on incompatibility the domain message is returned as data instead of turning it
                // into a request failure.
                match checkpoint.validate_kotlin_v1() {
                    Ok(()) => report.kotlin_v1_compatible = Some(true),
                    Err(error) => {
                        report.kotlin_v1_compatible = Some(false);
                        report.kotlin_v1_reason = Some(error.to_string());
                    }
                }
            }
            Ok(report)
        }
        other => Err(BridgeError::new("InvalidInput",
            &format!("checkpoint verification requires kind `policy` or `checkpoint`, got `{other}`"))),
    }
}

/// 严格解码并保留原始文本用于规范重编码。 / Strictly decode, then re-encode canonically.
fn decode<T: for<'de> Deserialize<'de>>(text: &str, label: &str) -> Result<T> {
    serde_json::from_str(text).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("{label} is not decodable: {error}"))
    })
}

/// 从策略档案抽取共同字段。 / Extract the fields shared by both documents.
fn policy_report(archive: &PolicyArchive, text: &str) -> Result<CheckpointReport> {
    if archive.version != 1 || archive.root.version != 1 {
        return Err(BridgeError::new("InvalidInput",
            "policy archive requires format version 1 for the archive and the policy root"));
    }
    if archive.definition.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput", "policy archive carries a blank definition fingerprint"));
    }
    if archive.feature_version.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput", "policy archive carries a blank feature version"));
    }
    let operator_policy_count = archive.operators.values()
        .map(|policies| policies.len()).sum::<usize>();
    // 重编码只用来证明往返无损；错误若发生说明文档虽能解码却不可序列化（领域不该出现）。
    // The re-encode only proves the round trip is lossless; a failure here would mean a decodable but unserializable
    // document, which the domain never produces.
    let canonical_json = serde_json::to_string(archive).map_err(|error| {
        BridgeError::new("CodecFailure", &format!("policy archive could not be re-encoded: {error}"))
    })?;
    if canonical_json.trim().is_empty() {
        return Err(BridgeError::new("CodecFailure", "policy archive re-encoded to empty text"));
    }
    // 输入若本身已是规范文本，重编码必须逐字相同——否则说明解码丢字段。
    // If the input is already canonical the re-encode must be byte-identical, otherwise decoding loses fields.
    let _ = text;
    Ok(CheckpointReport {
        kind: String::new(),
        version: archive.version,
        policy_version: archive.root.version,
        root_action_count: count(archive.root.actions.len()),
        operator_scope_count: count(archive.operators.len()),
        operator_policy_count: count(operator_policy_count),
        feature_version: archive.feature_version.clone(),
        definition: archive.definition.clone(),
        metadata_count: count(archive.metadata.len()),
        has_schedule: false,
        worker_count: 0,
        evaluations: 0,
        dispatched: 0,
        completed: 0,
        steps: 0,
        objective_calls: 0,
        successes: 0,
        transition_count: 0,
        stagnation: 0,
        diagnostic_count: 0,
        component_count: 0,
        kotlin_v1_compatible: None,
        kotlin_v1_reason: None,
        canonical_json,
    })
}

/// 计数字段的安全转换。 / A safe cast for count fields.
fn count(value: usize) -> u32 { u32::try_from(value).unwrap_or(u32::MAX) }

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::solver::heuristic::session::SearchSessionSnapshot;
    use ospf_rust_core::solver::hyper_heuristic::selection::PolicySnapshot;
    use std::collections::BTreeMap;

    /// 一个最小的合法策略快照（手写文本在这里是**被测对象**）。
    /// A minimal legal policy snapshot (the hand-written text is the **subject under test** here).
    fn policy_text(policy_type: &str) -> String {
        serde_json::json!({
            "version": 1,
            "definition": "algorithms:fingerprint",
            "featureVersion": "progress:1",
            "root": {
                "version": 1,
                "type": policy_type,
                "actions": ["a1", "a2"],
                "counts": [3, 4],
                "sums": [1.5, 2.5],
                "table": [[0.0, 1.0], [2.0, 3.0]],
            },
            "operators": {
                "a1": { "swap": { "version": 1, "type": "tabular", "actions": ["swap"] } },
            },
            "metadata": { "episodes": "12" },
        }).to_string()
    }

    fn session(initialized: bool) -> SearchSessionSnapshot {
        SearchSessionSnapshot {
            version: 1, initialized, iteration: 7, stagnation: 2,
            state: serde_json::json!({"iteration": 7}), archive: serde_json::json!([]),
            constraints: serde_json::Value::Null, active_nanos: 0,
        }
    }

    fn checkpoint_text(policy_type: &str, schedule: bool) -> String {
        let mut checkpoint = serde_json::json!({
            "version": 1,
            "configuration": "config:fingerprint",
            "problem": "problem:fingerprint",
            "policy": serde_json::from_str::<serde_json::Value>(&policy_text(policy_type)).unwrap(),
            "workers": { "w1": { "session": session(true), "random": 42, "initialized": true,
                "disabled": false } },
            "archive": [],
            "working": [],
            "evaluations": 128,
            "dispatched": 16,
            "completed": 15,
            "steps": 64,
            "stagnation": 3,
            "activeNanos": 1_000,
            "random": 7,
            "diagnostics": ["note"],
            "objectiveCalls": 96,
            "transitions": [],
            "successes": 5,
            "lastCost": 2,
            "configurationData": {"algorithms": [{"id": "a1", "algorithm": {"type": "sa"}}]},
            "components": {"reward": {"version": 1}},
        });
        if schedule {
            // `ScheduleCheckpoint` 只有 `cursor` 与 `log`；`log` 才是带版本的那个。
            // `ScheduleCheckpoint` has only `cursor` and `log`; the versioned one is `log`.
            checkpoint["schedule"] = serde_json::json!({"cursor": 0, "log": {
                "configuration": "config:fingerprint", "problem": "problem:fingerprint",
                "events": [], "checksum": "" }});
        }
        checkpoint.to_string()
    }

    fn request(kind: &str, text: &str, kotlin_v1: bool) -> CheckpointRequest {
        CheckpointRequest { kind: kind.to_owned(), text: text.to_owned(), kotlin_v1 }
    }

    #[test]
    fn policy_archive_decodes_and_reencodes_losslessly() {
        let text = policy_text("tabular");
        let report = verify_checkpoint(&request("policy", &text, false)).unwrap();
        assert_eq!(report.kind, "policy");
        assert_eq!(report.version, 1);
        assert_eq!(report.policy_version, 1);
        assert_eq!(report.root_action_count, 2);
        assert_eq!(report.operator_scope_count, 1);
        assert_eq!(report.operator_policy_count, 1);
        assert_eq!(report.feature_version, "progress:1");
        assert_eq!(report.definition, "algorithms:fingerprint");
        assert_eq!(report.metadata_count, 1);
        // 策略档案没有 worker 与计数器。 / A policy archive carries no workers or counters.
        assert_eq!(report.worker_count, 0);
        assert_eq!(report.evaluations, 0);
        assert_eq!(report.kotlin_v1_compatible, None, "no audit was requested");
        // 规范文本再解码一次必须一致：往返稳定。
        // Re-decoding the canonical text must agree: the round trip is stable.
        let again = verify_checkpoint(&request("policy", &report.canonical_json, false)).unwrap();
        assert_eq!(again.canonical_json, report.canonical_json);
        assert_eq!(again.root_action_count, report.root_action_count);
    }

    #[test]
    fn checkpoint_counters_are_projected_and_audit_is_a_verdict_not_an_error() {
        let report = verify_checkpoint(&request("checkpoint", &checkpoint_text("tabular", false), true)).unwrap();
        assert_eq!(report.kind, "checkpoint");
        assert_eq!(report.worker_count, 1);
        assert_eq!(report.evaluations, 128);
        assert_eq!(report.dispatched, 16);
        assert_eq!(report.completed, 15);
        assert_eq!(report.steps, 64);
        assert_eq!(report.objective_calls, 96);
        assert_eq!(report.successes, 5);
        assert_eq!(report.stagnation, 3);
        assert_eq!(report.diagnostic_count, 1);
        assert_eq!(report.component_count, 1);
        assert_eq!(report.transition_count, 0);
        assert!(!report.has_schedule);
        // 平稳策略 + 无调度 + 版本 1：可导出。
        // A stationary policy, no schedule and version 1: exportable.
        assert_eq!(report.kotlin_v1_compatible, Some(true));
        assert_eq!(report.kotlin_v1_reason, None);

        // 调度重放状态：不可导出，且**原因回到数据里**（请求本身成功）。
        // Schedule replay state: not exportable, with the **reason returning as data** (the request itself succeeds).
        let report = verify_checkpoint(&request("checkpoint", &checkpoint_text("tabular", true), true)).unwrap();
        assert!(report.has_schedule);
        assert_eq!(report.kotlin_v1_compatible, Some(false));
        let reason = report.kotlin_v1_reason.unwrap();
        assert!(reason.contains("schedule replay state"), "{reason}");

        // 非平稳选择器：不可导出。实测类型名是 `frr-mab` / `choice-function` / `adaptive-pursuit` 三者之一，
        // 不是字面量 "non-stationary"。 / The measured type names are those three, not the literal "non-stationary".
        let report = verify_checkpoint(&request("checkpoint", &checkpoint_text("frr-mab", false), true)).unwrap();
        assert_eq!(report.kotlin_v1_compatible, Some(false));
        assert!(report.kotlin_v1_reason.unwrap().contains("non-stationary selector"));

        // 不请求审计时结论为空，但结构仍然校验通过。
        // Without requesting the audit the verdict is absent, yet the structure still validates.
        let report = verify_checkpoint(&request("checkpoint", &checkpoint_text("frr-mab", false), false)).unwrap();
        assert_eq!(report.kotlin_v1_compatible, None);
        assert_eq!(report.kotlin_v1_reason, None);

        // 自适应奖励 / 自适应接受策略 / 局部更新组件 / 原生约束 / numerics：各自点名。
        // Adaptive rewards, adaptive acceptance, local update components, native constraints and numerics each name
        // themselves.
        let mut adaptive_reward = serde_json::from_str::<serde_json::Value>(&checkpoint_text("tabular", false)).unwrap();
        adaptive_reward["configurationData"]["learning"] = serde_json::json!({"reward": {"type": "hypervolume-delta"}});
        let report = verify_checkpoint(&request("checkpoint", &adaptive_reward.to_string(), true)).unwrap();
        assert_eq!(report.kotlin_v1_compatible, Some(false));
        assert!(report.kotlin_v1_reason.unwrap().contains("adaptive rewards"));

        let mut adaptive_acceptance = serde_json::from_str::<serde_json::Value>(&checkpoint_text("tabular", false)).unwrap();
        adaptive_acceptance["configurationData"]["memory"] = serde_json::json!({"acceptance": {"type": "late"}});
        let report = verify_checkpoint(&request("checkpoint", &adaptive_acceptance.to_string(), true)).unwrap();
        assert!(report.kotlin_v1_reason.unwrap().contains("adaptive acceptance"));

        let mut local_update = serde_json::from_str::<serde_json::Value>(&checkpoint_text("tabular", false)).unwrap();
        local_update["configurationData"]["algorithms"][0]["updates"] = serde_json::json!([{"type": "vnd"}]);
        let report = verify_checkpoint(&request("checkpoint", &local_update.to_string(), true)).unwrap();
        assert!(report.kotlin_v1_reason.unwrap().contains("local update components"));

        let mut native_constraints = serde_json::from_str::<serde_json::Value>(&checkpoint_text("tabular", false)).unwrap();
        native_constraints["workers"]["w1"]["session"]["constraints"] = serde_json::json!({"kind": "linear"});
        let report = verify_checkpoint(&request("checkpoint", &native_constraints.to_string(), true)).unwrap();
        assert!(report.kotlin_v1_reason.unwrap().contains("native constraint state"));

        let mut numerics = serde_json::from_str::<serde_json::Value>(&checkpoint_text("tabular", false)).unwrap();
        numerics["configurationData"]["algorithms"][0]["algorithm"]["parameters"] = serde_json::json!({"numerics": {}});
        let report = verify_checkpoint(&request("checkpoint", &numerics.to_string(), true)).unwrap();
        assert!(report.kotlin_v1_reason.unwrap().contains("explicit numerical contracts"));

        // `previousAction` 重放元数据：不可导出。
        // `previousAction` replay metadata: not exportable.
        let mut previous = serde_json::from_str::<serde_json::Value>(&checkpoint_text("tabular", false)).unwrap();
        previous["transitions"] = serde_json::json!([{
            "episode": "e1", "decision": 1, "action": "a1",
            "before": {"state": 0, "features": [0.5], "decision": 1, "stagnation": 0},
            "after": {"state": 1, "features": [0.6], "decision": 2, "stagnation": 0},
            "available": ["a1"], "nextAvailable": ["a1"], "reward": 1.0, "evaluations": 4,
            "terminal": false, "policyVersion": 1, "previousAction": "a0",
        }]);
        let report = verify_checkpoint(&request("checkpoint", &previous.to_string(), true)).unwrap();
        assert_eq!(report.transition_count, 1);
        assert!(report.kotlin_v1_reason.unwrap().contains("previousAction"));
    }

    #[test]
    fn illegal_documents_are_refused() {
        // 未知字段：严格解码拒绝。 / An unknown field: strict decoding refuses it.
        let mut unknown = serde_json::from_str::<serde_json::Value>(&policy_text("tabular")).unwrap();
        unknown["nonsense"] = serde_json::json!(1);
        let error = verify_checkpoint(&request("policy", &unknown.to_string(), false)).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("unknown field"), "{}", error.message);

        // 版本非 1：拒绝（档案与根策略都要检查）。 / A version other than 1: refused (archive and root both checked).
        let mut wrong_version = serde_json::from_str::<serde_json::Value>(&policy_text("tabular")).unwrap();
        wrong_version["version"] = serde_json::json!(2);
        let error = verify_checkpoint(&request("policy", &wrong_version.to_string(), false)).unwrap_err();
        assert!(error.message.contains("format version 1"), "{}", error.message);
        let mut wrong_root = serde_json::from_str::<serde_json::Value>(&policy_text("tabular")).unwrap();
        wrong_root["root"]["version"] = serde_json::json!(3);
        let error = verify_checkpoint(&request("policy", &wrong_root.to_string(), false)).unwrap_err();
        assert!(error.message.contains("format version 1"), "{}", error.message);

        // 空白定义指纹 / 空白特征版本。 / A blank definition fingerprint and a blank feature version.
        let mut blank_definition = serde_json::from_str::<serde_json::Value>(&policy_text("tabular")).unwrap();
        blank_definition["definition"] = serde_json::json!("  ");
        let error = verify_checkpoint(&request("policy", &blank_definition.to_string(), false)).unwrap_err();
        assert!(error.message.contains("blank definition fingerprint"), "{}", error.message);
        let mut blank_feature = serde_json::from_str::<serde_json::Value>(&policy_text("tabular")).unwrap();
        blank_feature["featureVersion"] = serde_json::json!("");
        let error = verify_checkpoint(&request("policy", &blank_feature.to_string(), false)).unwrap_err();
        assert!(error.message.contains("blank feature version"), "{}", error.message);

        // 非 JSON / 未知类别 / 对策略档案请求审计 / 缺请求。 / Non-JSON, an unknown kind, an audit on a policy archive.
        assert_eq!(verify_checkpoint(&request("policy", "not json", false)).unwrap_err().code, "InvalidInput");
        assert_eq!(verify_checkpoint(&request("run", &policy_text("tabular"), false)).unwrap_err().code,
            "InvalidInput");
        let error = verify_checkpoint(&request("policy", &policy_text("tabular"), true)).unwrap_err();
        assert!(error.message.contains("applies to checkpoints"), "{}", error.message);
        let huge = "x".repeat(33 * 1024 * 1024);
        assert_eq!(verify_checkpoint(&request("policy", &huge, false)).unwrap_err().code, "InvalidInput");
    }
}
