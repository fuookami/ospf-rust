//! 续挖第 124 单：**调度日志的校验和是先决条件，而它之后的九条顺序规则构成一台真正的状态机**——
//! 校验和不过就只看得到 `invalid schedule identity, version or checksum`，所以这台状态机要么永远摸不到、
//! 要么得先按框架自己的规范形式算出摘要。这一单**在桥里重算规范 JSON 的 SHA-256**（框架的 `digest()` 是私有的、
//! `persistence::fingerprint` 是 `pub(crate)`），让框架自己的 `validate()` 当裁判：一旦我的规范形式与它一致，
//! 顺序规则就全部可测；不一致的话连第一条都过不去——两种结局都是实测数据
//! Continuation, delivery 124: **the schedule log's checksum is a precondition, and the ordering rules behind it form a real
//! state machine** — until the checksum holds, the only thing visible is `invalid schedule identity, version or checksum`, so
//! that state machine is either unreachable or needs the digest computed in the framework's own canonical form. This delivery
//! **recomputes the canonical-JSON SHA-256 inside the bridge** (the framework's `digest()` is private and
//! `persistence::fingerprint` is `pub(crate)`) and lets the framework's own `validate()` judge: once the canonical form agrees,
//! every ordering rule is measurable, and if it disagrees not even the first gate passes — both outcomes are measurements.
//!
//! 第 123 单量的是算子能力，这一单量**调度日志的身份门与九条顺序规则**。八条实测：
//!
//! 1. **身份门**：`version != 1`、空 `configuration`、空 `problem`、坏 `checksum` 四者都拒
//!    （`the_identity_gate_comes_first()`）。
//! 2. **`begin` 只在未推进时成立**（`the_begin_is_not_repeatable()`）。
//! 3. **`dispatch` 要 `advancing`、`decision` 顺序、正预算、动作在 `available` 里、且同动作不在册**
//!    （`the_dispatch_preconditions_are_strict()`）。
//! 4. **`complete` 必须匹配在册的 `(decision, 预留评估, 预留步数)` 且只准 `<=` 预留**
//!    （`the_completion_only_shrinks_its_allownce()`）。
//! 5. **`drain` 要 `active` 非空、`yield` 要 `advancing`**（`the_drain_and_yield_need_state()`）。
//! 6. **`end` 要静止且两个总数与逐事件累计一致**（`the_end_totals_must_reconcile()`）。
//! 7. **`end` 之后不许再有事件，未 `end` 的完整日志被拒**（`the_log_must_be_sealed()`）。
//! 8. **事件默认值不上线材**（`decision = 0`、空动作、零预算、`random = 0`、空 `available`、两个 false
//!    全部消失），这也是校验和稳定的原因（`the_event_defaults_stay_off_the_wire()`）。
//!
//! Delivery 123 measured the operator capabilities; this one measures **the schedule log's identity gate and nine ordering
//! rules**. **The identity gate comes first**, **the begin is not repeatable**, and **the dispatch preconditions are strict**.
//! **The completion only shrinks its allowance**, while **the drain and yield need state**. **The end totals must reconcile**,
//! **the log must be sealed**, and **the event defaults stay off the wire** — which is also why the checksum stays stable.

use std::collections::BTreeMap;

use ospf_rust_core::solver::hyper_heuristic::{ScheduleEvent, ScheduleLog};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条事件的可选读数。 / One event's optional reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleEventSpec {
    /// `begin` / `dispatch` / `complete` / `drain` / `yield` / `end`。
    pub kind: String,
    #[serde(default)]
    pub decision: Option<f64>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub evaluations: Option<f64>,
    #[serde(default)]
    pub steps: Option<f64>,
    #[serde(default)]
    pub random: Option<f64>,
    #[serde(default)]
    pub available: Option<Vec<String>>,
    #[serde(default)]
    pub stopped: Option<bool>,
    #[serde(default)]
    pub failed: Option<bool>,
}

/// 一份待校验的日志。 / One log to validate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleLogSpec {
    /// 协议版本（默认 1）。 / The protocol version.
    #[serde(default)]
    pub version: Option<u32>,
    /// 搜索配置指纹（默认非空占位）。 / The configuration fingerprint.
    #[serde(default)]
    pub configuration: Option<String>,
    /// 问题标识与数据指纹。 / The problem fingerprint.
    #[serde(default)]
    pub problem: Option<String>,
    /// 显式校验和；缺省则按框架的规范形式重算。 / An explicit checksum; empty recomputes it.
    #[serde(default)]
    pub checksum: Option<String>,
    #[serde(default)]
    pub events: Vec<ScheduleEventSpec>,
}

/// 调度日志探针请求。 / The schedule log probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleLogRequest {
    #[serde(default)]
    pub logs: Vec<ScheduleLogSpec>,
}

/// 一份日志的读数。 / One log's reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleLogRow {
    /// 是否通过校验。 / Whether it validated.
    pub valid: bool,
    /// 拒收时的错误文本。 / The refusal text.
    pub error: String,
    /// 探针算出的校验和。 / The checksum the probe computed.
    pub computed_checksum: String,
    /// 该日志里被框架当作"在册"的事件数。 / How many events the framework had to track as active.
    pub dispatched: u32,
    /// 事件数。 / The event count.
    pub events: u32,
}

/// 调度日志探针结论。 / The schedule log probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleLogReport {
    pub rows: Vec<ScheduleLogRow>,
    pub the_identity_gate_comes_first: bool,
    pub the_begin_is_not_repeatable: bool,
    pub the_dispatch_preconditions_are_strict: bool,
    pub the_completion_only_shrinks_its_allownce: bool,
    pub the_drain_and_yield_need_state: bool,
    pub the_end_totals_must_reconcile: bool,
    pub the_log_must_be_sealed: bool,
    pub the_event_defaults_stay_off_the_wire: bool,
}

/// 规范 JSON 的规范形式（键按 UTF-16 排序、逗号后一个空格）——与 `persistence::fingerprint` 逐字节相同。
/// The canonical form used by the framework's own fingerprint, reproduced byte for byte.
fn canonical(value: &Value) -> String {
    match value {
        Value::Object(entries) => {
            let mut keys: Vec<_> = entries.keys().collect();
            keys.sort_by(|lhs, rhs| lhs.encode_utf16().cmp(rhs.encode_utf16()));
            format!("{{{}}}", keys.iter()
                .map(|key| format!("{}:{}", Value::String((*key).clone()), canonical(&entries[*key])))
                .collect::<Vec<_>>().join(", "))
        }
        Value::Array(values) => {
            format!("[{}]", values.iter().map(canonical).collect::<Vec<_>>().join(", "))
        }
        _ => value.to_string(),
    }
}

/// 除 `checksum` 之外的规范 JSON SHA-256。 / The canonical-JSON SHA-256 of everything but `checksum`.
fn digest(log: &ScheduleLog) -> Result<String> {
    let mut value = serde_json::to_value(log)
        .map_err(|error| BridgeError::new("InvalidInput", error.to_string()))?;
    value.as_object_mut().map(|object| object.remove("checksum"))
        .ok_or_else(|| BridgeError::new("InvalidInput", "schedule log is not an object"))?;
    Ok(Sha256::digest(canonical(&value).as_bytes()).iter()
        .map(|byte| format!("{byte:02x}")).collect())
}

/// 造一条事件。 / Build one event.
fn event_of(spec: &ScheduleEventSpec) -> Result<ScheduleEvent> {
    let integer = |value: Option<f64>, what: &str| -> Result<u64> {
        let value = value.unwrap_or(0.0);
        if !value.is_finite() || value < 0.0 || value.trunc() != value {
            return Err(BridgeError::new("InvalidInput", format!(
                "schedule log probe failed validation: {what} reads a non-negative integer")));
        }
        Ok(value as u64)
    };
    Ok(ScheduleEvent {
        kind: spec.kind.clone(),
        decision: integer(spec.decision, "decision")?,
        action: spec.action.clone().unwrap_or_default(),
        evaluations: integer(spec.evaluations, "evaluations")?,
        steps: integer(spec.steps, "steps")?,
        random: spec.random.unwrap_or(0.0) as i64,
        available: spec.available.clone().unwrap_or_default(),
        stopped: spec.stopped.unwrap_or(false),
        failed: spec.failed.unwrap_or(false),
    })
}

/// 造一份日志并把校验和补上。 / Build one log and fill its checksum in.
fn log_of(spec: &ScheduleLogSpec) -> Result<ScheduleLog> {
    let mut log = ScheduleLog {
        version: spec.version.unwrap_or(1),
        // 空串是被框架明确拒的一种身份，所以缺省给非空占位。 
        // An empty string is an identity the framework refuses, so the default is a non-empty placeholder.
        configuration: match &spec.configuration {
            Some(text) => text.clone(), None => "probe-config".to_owned(),
        },
        problem: match &spec.problem {
            Some(text) => text.clone(), None => "probe-problem".to_owned(),
        },
        events: spec.events.iter().map(event_of).collect::<Result<Vec<_>>>()?,
        checksum: String::new(),
    };
    log.checksum = match spec.checksum.clone().filter(|text| !text.is_empty()) {
        Some(explicit) => explicit,
        None => digest(&log)?,
    };
    Ok(log)
}

/// 走一遍调度日志探针。非法输入是**错误**，校验不过的日志是**结果**。
/// Walk the probe. An illegal input is an error; a log that fails validation is a result.
pub fn verify_schedule_log(request: &ScheduleLogRequest) -> Result<ScheduleLogReport> {
    if request.logs.len() > 16 {
        return Err(BridgeError::new("InvalidInput", "schedule log probe failed validation: at \
            most sixteen logs are accepted"));
    }
    let findings = canonical_findings();
    let mut rows = Vec::with_capacity(request.logs.len());
    for spec in &request.logs {
        let log = log_of(spec)?;
        let computed_checksum = digest(&log)?;
        let events = u32::try_from(log.events.len()).unwrap_or(u32::MAX);
        // 被框架当作"在册"的事件数：`dispatch` 成功一次算一件。探针自己数，不读框架内部。 
        // The events the framework has to track: every accepted dispatch. The probe counts them itself.
        let dispatched = log.events.iter().filter(|event| event.kind == "dispatch")
            .count().try_into().unwrap_or(u32::MAX);
        let (valid, error) = match log.validate() {
            Ok(()) => (true, String::new()),
            Err(error) => (false, error.to_string()),
        };
        rows.push(ScheduleLogRow { valid, error, computed_checksum, dispatched, events });
    }
    Ok(ScheduleLogReport {
        rows,
        the_identity_gate_comes_first: findings.the_identity_gate_comes_first,
        the_begin_is_not_repeatable: findings.the_begin_is_not_repeatable,
        the_dispatch_preconditions_are_strict: findings.the_dispatch_preconditions_are_strict,
        the_completion_only_shrinks_its_allownce: findings.the_completion_only_shrinks_its_allownce,
        the_drain_and_yield_need_state: findings.the_drain_and_yield_need_state,
        the_end_totals_must_reconcile: findings.the_end_totals_must_reconcile,
        the_log_must_be_sealed: findings.the_log_must_be_sealed,
        the_event_defaults_stay_off_the_wire: findings.the_event_defaults_stay_off_the_wire,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_identity_gate_comes_first: bool,
    the_begin_is_not_repeatable: bool,
    the_dispatch_preconditions_are_strict: bool,
    the_completion_only_shrinks_its_allownce: bool,
    the_drain_and_yield_need_state: bool,
    the_end_totals_must_reconcile: bool,
    the_log_must_be_sealed: bool,
    the_event_defaults_stay_off_the_wire: bool,
}

/// 一条事件的简写。 / A terse event.
fn event(kind: &str) -> ScheduleEvent { ScheduleEvent { kind: kind.to_owned(), ..ScheduleEvent::default() } }

/// 一次派发。 / One dispatch.
fn dispatch(decision: u64, action: &str, evaluations: u64, steps: u64) -> ScheduleEvent {
    ScheduleEvent { decision, action: action.to_owned(), evaluations, steps,
        available: vec![action.to_owned()], ..event("dispatch") }
}

/// 一次完成，预算按预留给满。 / One completion with the full reserved budget.
fn complete(decision: u64, action: &str, evaluations: u64, steps: u64) -> ScheduleEvent {
    ScheduleEvent { decision, action: action.to_owned(), evaluations, steps, ..event("complete") }
}

/// 收尾事件。 / The closing event.
fn end(evaluations: u64, steps: u64) -> ScheduleEvent {
    ScheduleEvent { evaluations, steps, ..event("end") }
}

/// 一份封好的日志。 / One sealed log.
fn sealed(events: Vec<ScheduleEvent>) -> ScheduleLog {
    let mut log = ScheduleLog { version: 1, configuration: "probe-config".to_owned(),
        problem: "probe-problem".to_owned(), events, checksum: String::new() };
    log.checksum = digest(&log).expect("the canonical form is computable");
    log
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 一份形状完全合法的两派发日志，用作对照。 
        // A perfectly shaped two-dispatch log, used as the control.
        let sound = || sealed(vec![event("begin"), dispatch(0, "a", 4, 2),
            complete(0, "a", 4, 2), dispatch(1, "b", 3, 1), complete(1, "b", 3, 1),
            event("yield"), end(7, 3)]);
        // 1) 身份门先判：版本、两个指纹、校验和四种坏法都被同一句话拦下。 
        // The identity gate comes first: four broken identities are stopped by one message.
        let the_identity_gate_comes_first = {
            let gate = |log: &ScheduleLog| log.validate().err().map(|error| error.to_string())
                .unwrap_or_default();
            let sound_log = sound();
            let bad_version = ScheduleLog { version: 2, ..sound_log.clone() };
            let bad_configuration = ScheduleLog { configuration: String::new(),
                ..sound_log.clone() };
            let bad_problem = ScheduleLog { problem: String::new(), ..sound_log.clone() };
            let bad_checksum = ScheduleLog { checksum: "0".repeat(64), ..sound_log.clone() };
            sound_log.validate().is_ok()
                && gate(&bad_version).contains("invalid schedule identity, version or checksum")
                && gate(&bad_configuration).contains("invalid schedule identity, version or checksum")
                && gate(&bad_problem).contains("invalid schedule identity, version or checksum")
                && gate(&bad_checksum).contains("invalid schedule identity, version or checksum")
        };
        // 2) begin 不能重复。 / The begin is not repeatable.
        let the_begin_is_not_repeatable = {
            let twice = sealed(vec![event("begin"), event("begin"), end(0, 0)]);
            let without = sealed(vec![dispatch(0, "a", 1, 1), end(0, 0)]);
            twice.validate().is_err() && without.validate().is_err()
                && sealed(vec![event("begin"), event("yield"), end(0, 0)]).validate().is_ok()
        };
        // 3) dispatch 的四个前置：advance、顺序号、正预算、available 与在册。 
        // The dispatch's four preconditions.
        let the_dispatch_preconditions_are_strict = {
            let skip_decision = sealed(vec![event("begin"), dispatch(1, "a", 1, 1), end(0, 0)]);
            let zero_budget = sealed(vec![event("begin"), dispatch(0, "a", 0, 1), end(0, 0)]);
            let not_available = sealed(vec![event("begin"),
                ScheduleEvent { action: "a".to_owned(), decision: 0, evaluations: 1, steps: 1,
                    ..event("dispatch") }, end(0, 0)]);
            let already_active = sealed(vec![event("begin"), dispatch(0, "a", 1, 1),
                dispatch(1, "a", 1, 1), end(0, 0)]);
            skip_decision.validate().is_err() && zero_budget.validate().is_err()
                && not_available.validate().is_err() && already_active.validate().is_err()
        };
        // 4) complete 只准缩不许长，且必须对上在册的决策号。 
        // The completion only shrinks and must match the tracked decision.
        let the_completion_only_shrinks_its_allownce = {
            let exact = sealed(vec![event("begin"), dispatch(0, "a", 4, 2),
                complete(0, "a", 4, 2), event("yield"), end(4, 2)]);
            let shrunk = sealed(vec![event("begin"), dispatch(0, "a", 4, 2),
                complete(0, "a", 3, 1), event("yield"), end(3, 1)]);
            let grown = sealed(vec![event("begin"), dispatch(0, "a", 4, 2),
                complete(0, "a", 5, 2), end(5, 2)]);
            let wrong_decision = sealed(vec![event("begin"), dispatch(0, "a", 4, 2),
                complete(1, "a", 4, 2), end(4, 2)]);
            let unknown = sealed(vec![event("begin"), complete(0, "a", 4, 2), end(4, 2)]);
            exact.validate().is_ok() && shrunk.validate().is_ok() && grown.validate().is_err()
                && wrong_decision.validate().is_err() && unknown.validate().is_err()
        };
        // 5) drain 要 active 非空，yield 要 advancing。 
        // The drain and yield need state.
        let the_drain_and_yield_need_state = {
            let idle_drain = sealed(vec![event("begin"), event("drain"), end(0, 0)]);
            let twice_yield = sealed(vec![event("begin"), event("yield"), event("yield"), end(0, 0)]);
            let sound_drain = sealed(vec![event("begin"), dispatch(0, "a", 1, 1),
                event("drain"), complete(0, "a", 1, 1), event("yield"), end(1, 1)]);
            idle_drain.validate().is_err() && twice_yield.validate().is_err()
                && sound_drain.validate().is_ok()
        };
        // 6) end 要静止且总数对得上。 
        // The end totals must reconcile.
        let the_end_totals_must_reconcile = {
            let open = sealed(vec![event("begin"), dispatch(0, "a", 1, 1), end(1, 1)]);
            let wrong_totals = sealed(vec![event("begin"), dispatch(0, "a", 4, 2),
                complete(0, "a", 4, 2), end(5, 2)]);
            open.validate().is_err() && wrong_totals.validate().is_err()
        };
        // 7) end 之后不许再有事件，未 end 被拒。 
        // Nothing follows the end, and an unsealed log is refused.
        let the_log_must_be_sealed = {
            let trailing = sealed(vec![event("begin"), event("yield"), end(0, 0),
                event("dispatch")]);
            let unfinished = sealed(vec![event("begin"), dispatch(0, "a", 1, 1),
                complete(0, "a", 1, 1)]);
            trailing.validate().is_err() && unfinished.validate().is_err()
        };
        // 8) 事件默认值不上线材。 / The event defaults stay off the wire.
        let the_event_defaults_stay_off_the_wire = {
            let value = serde_json::to_value(event("begin")).expect("an event serializes");
            let object = value.as_object().expect("an event is an object");
            object.len() == 1 && object.contains_key("kind")
                // 一份日志里 `version` 默认 1 也不上线材。 
                // The log's default version one stays off the wire as well.
                && serde_json::to_value(sealed(vec![event("begin"), event("yield"), end(0, 0)]))
                    .ok().and_then(|value| value.as_object().map(|object| !object.contains_key("version")))
                    .unwrap_or(false)
        };
        Findings {
            the_identity_gate_comes_first,
            the_begin_is_not_repeatable,
            the_dispatch_preconditions_are_strict,
            the_completion_only_shrinks_its_allownce,
            the_drain_and_yield_need_state,
            the_end_totals_must_reconcile,
            the_log_must_be_sealed,
            the_event_defaults_stay_off_the_wire,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: ScheduleLogRequest) -> ScheduleLogReport {
        verify_schedule_log(&request).unwrap()
    }

    fn spec(kind: &str) -> ScheduleEventSpec {
        ScheduleEventSpec { kind: kind.to_owned(), decision: None, action: None,
            evaluations: None, steps: None, random: None, available: None, stopped: None,
            failed: None }
    }

    fn dispatch(decision: f64, action: &str, evaluations: f64, steps: f64) -> ScheduleEventSpec {
        ScheduleEventSpec { decision: Some(decision), action: Some(action.to_owned()),
            evaluations: Some(evaluations), steps: Some(steps),
            available: Some(vec![action.to_owned()]), ..spec("dispatch") }
    }

    fn complete(decision: f64, action: &str, evaluations: f64, steps: f64) -> ScheduleEventSpec {
        ScheduleEventSpec { decision: Some(decision), action: Some(action.to_owned()),
            evaluations: Some(evaluations), steps: Some(steps), ..spec("complete") }
    }

    fn end(evaluations: f64, steps: f64) -> ScheduleEventSpec {
        ScheduleEventSpec { evaluations: Some(evaluations), steps: Some(steps), ..spec("end") }
    }

    fn log(events: Vec<ScheduleEventSpec>) -> ScheduleLogRequest {
        ScheduleLogRequest { logs: vec![ScheduleLogSpec { version: None, configuration: None,
            problem: None, checksum: None, events }] }
    }

    fn sound() -> ScheduleLogRequest {
        log(vec![spec("begin"), dispatch(0.0, "a", 4.0, 2.0), complete(0.0, "a", 4.0, 2.0),
            dispatch(1.0, "b", 3.0, 1.0), complete(1.0, "b", 3.0, 1.0), spec("yield"),
            end(7.0, 3.0)])
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(sound());
        assert_eq!(report.rows.len(), 1);
        assert!(report.rows[0].valid, "{}", report.rows[0].error);
        assert_eq!(report.rows[0].events, 7);
        assert_eq!(report.rows[0].dispatched, 2);
        assert_eq!(report.rows[0].computed_checksum.len(), 64);
        assert!(report.the_identity_gate_comes_first);
        assert!(report.the_begin_is_not_repeatable);
        assert!(report.the_dispatch_preconditions_are_strict);
        assert!(report.the_completion_only_shrinks_its_allownce);
        assert!(report.the_drain_and_yield_need_state);
        assert!(report.the_end_totals_must_reconcile);
        assert!(report.the_log_must_be_sealed);
        assert!(report.the_event_defaults_stay_off_the_wire);
    }

    #[test]
    fn the_identity_gate_rejects_four_kinds_of_identity() {
        // 版本、配置、问题、校验和四种坏法：都能从请求侧制造。 
        // Four broken identities are all reachable from the request side.
        let variants = vec![
            ScheduleLogSpec { version: Some(2), ..log(Vec::new()).logs[0].clone() },
            ScheduleLogSpec { configuration: Some(String::new()),
                ..log(Vec::new()).logs[0].clone() },
            ScheduleLogSpec { problem: Some(String::new()), ..log(Vec::new()).logs[0].clone() },
            ScheduleLogSpec { checksum: Some("probe".to_owned()),
                ..log(Vec::new()).logs[0].clone() },
        ];
        let report = ask(ScheduleLogRequest { logs: variants });
        for row in &report.rows {
            assert!(!row.valid);
            assert!(row.error.contains("invalid schedule identity, version or checksum"), "{}",
                row.error);
            assert_eq!(row.computed_checksum.len(), 64);
        }
    }

    #[test]
    fn the_ordering_violations_report_their_own_messages() {
        let cases = vec![
            // begin 重复。 / A repeated begin.
            (vec![spec("begin"), spec("begin"), end(0.0, 0.0)],
                "invalid schedule event ordering"),
            // 跳号的 dispatch。 / A dispatch with a skipped decision.
            (vec![spec("begin"), dispatch(1.0, "a", 1.0, 1.0), end(0.0, 0.0)],
                "invalid schedule event ordering"),
            // 未知完成。 / An unknown completion.
            (vec![spec("begin"), complete(0.0, "a", 4.0, 2.0), end(4.0, 2.0)],
                "unexpected schedule completion"),
            // 超出预留。 / Growing beyond the reservation.
            (vec![spec("begin"), dispatch(0.0, "a", 4.0, 2.0), complete(0.0, "a", 5.0, 2.0),
                end(5.0, 2.0)], "inconsistent schedule completion allowances"),
            // 空转 drain。 / An idle drain.
            (vec![spec("begin"), spec("drain"), end(0.0, 0.0)],
                "invalid schedule event ordering"),
            // end 总数不符。 / The end totals disagreeing.
            (vec![spec("begin"), dispatch(0.0, "a", 4.0, 2.0), complete(0.0, "a", 4.0, 2.0),
                spec("yield"), end(5.0, 2.0)], "invalid schedule event ordering"),
            // end 之后还有事件。 / A trailing event after the end.
            (vec![spec("begin"), spec("yield"), end(0.0, 0.0), spec("dispatch")],
                "schedule contains events after end"),
            // 没 end。 / An unsealed log.
            (vec![spec("begin"), dispatch(0.0, "a", 1.0, 1.0), complete(0.0, "a", 1.0, 1.0)],
                "truncated or non-quiescent schedule"),
        ];
        let report = ask(ScheduleLogRequest {
            logs: cases.iter().map(|(events, _)| log(events.clone()).logs[0].clone())
                .collect() });
        assert_eq!(report.rows.len(), cases.len());
        for (row, (_, expected)) in report.rows.iter().zip(&cases) {
            assert!(!row.valid, "{:?}", row);
            assert!(row.error.contains(expected), "expected {expected}, got {}", row.error);
        }
    }

    #[test]
    fn a_shrinking_completion_and_a_sound_drain_both_pass() {
        let report = ask(ScheduleLogRequest { logs: vec![
            // 只报 3 次评估 / 1 步（<= 预留 4/2）。 / A completion reporting less than its reservation.
            log(vec![spec("begin"), dispatch(0.0, "a", 4.0, 2.0), complete(0.0, "a", 3.0, 1.0),
                spec("yield"), end(3.0, 1.0)]).logs[0].clone(),
            // 在册时 drain 合法。 / A drain with something active is legal.
            log(vec![spec("begin"), dispatch(0.0, "a", 1.0, 1.0), spec("drain"),
                complete(0.0, "a", 1.0, 1.0), spec("yield"), end(1.0, 1.0)]).logs[0].clone(),
            // yield 之后再 begin、再 yield、再 end 也合法。 
            // A yield, then a begin, then another yield and an end are legal too.
            log(vec![spec("begin"), spec("yield"), spec("begin"), spec("yield"), end(0.0, 0.0)])
                .logs[0].clone(),
        ] });
        assert!(report.rows.iter().all(|row| row.valid), "{:?}", report.rows);
    }

    #[test]
    fn the_checksum_is_sixteen_byte_hex_and_repeatable() {
        let report = ask(sound());
        let again = ask(sound());
        assert_eq!(report.rows[0].computed_checksum, again.rows[0].computed_checksum);
        assert_eq!(report.rows[0].computed_checksum.len(), 64);
        assert!(report.rows[0].computed_checksum.chars()
            .all(|character| character.is_ascii_hexdigit()));
        // 显式但正确以外的校验和一律拒。 / Any explicit checksum other than the right one is refused.
        let explicit_right = ask(ScheduleLogRequest { logs: vec![ScheduleLogSpec {
            checksum: Some(report.rows[0].computed_checksum.clone()),
            ..sound().logs[0].clone() }] });
        // 把这份日志**自己**算出来的摘要显式钉回去 → 通过。 
        // Pinning the log's own digest back explicitly passes.
        assert!(explicit_right.rows[0].valid, "{}", explicit_right.rows[0].error);
        let mismatched = ask(ScheduleLogRequest { logs: vec![ScheduleLogSpec {
            checksum: Some("0".repeat(64)), ..sound().logs[0].clone() }] });
        assert!(!mismatched.rows[0].valid);
        assert!(mismatched.rows[0].error
            .contains("invalid schedule identity, version or checksum"));
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_schedule_log(&ScheduleLogRequest {
            logs: vec![log(vec![dispatch(-1.0, "a", 1.0, 1.0)]).logs[0].clone()] })
            .unwrap_err().message.contains("reads a non-negative integer"));
        assert!(verify_schedule_log(&ScheduleLogRequest {
            logs: vec![log(vec![dispatch(0.5, "a", 1.0, 1.0)]).logs[0].clone()] })
            .unwrap_err().message.contains("reads a non-negative integer"));
        assert!(verify_schedule_log(&ScheduleLogRequest {
            logs: (0..17).map(|index| ScheduleLogSpec { problem: Some(format!("p{index}")),
                ..log(Vec::new()).logs[0].clone() }).collect() })
            .unwrap_err().message.contains("sixteen logs"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(sound());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"computedChecksum\""), "{json}");
        assert!(json.contains("\"theIdentityGateComesFirst\""), "{json}");
        assert!(!json.contains("computed_checksum"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ScheduleLogReport>(&copy.to_string()).is_err());
    }
}
