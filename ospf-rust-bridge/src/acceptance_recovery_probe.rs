//! 续挖第 121 单：**三种接受准则的可恢复状态各有自己的"定义锁"和"历史一致性"两把锁，而构造参数的合法区间是硬写的**——
//! `LateAcceptance` 的 `new` 只拒 `length == 0` 与 `length > i32::MAX`，`load` 再拒**定义被改**、**`cursor >= length`**、
//! **非空历史长度不等于 `length`**、**空历史却 `cursor != 0`**；`RecordToRecordAcceptance` 的 `new` 只拒
//! **非有限或出 `[0,1]` 的 ratio** 与 **`horizon == 0`**，`load` 拒**定义被改**、**`|best| > scale`**、
//! **`clock == 0` 与 `best.is_none()` 不同步**；`AutoAcceptance` 的 `new` 只拒 **`horizon > i64::MAX`**，
//! **`capacity = sqrt(horizon).clamp(8, 256)`**、**`span = (capacity/2).max(2)`** 都是推导出来的，
//! `load` 再拒 **`span` 出 `[2, capacity]`**、**`improvements > decisions % capacity`**、**历史长度不等于
//! `min(decisions, capacity)`**（`memory/adaptive.rs`）
//! Continuation, delivery 121: **each of the three acceptance policies keeps a definition lock and a history-consistency lock, and the
//! legal ranges of their constructors are hardcoded** — `LateAcceptance::new` refuses only `length == 0` and `length > i32::MAX`,
//! while `load` refuses **a changed definition**, **`cursor >= length`**, **a non-empty history whose length is not `length`** and
//! **an empty history with `cursor != 0`**; `RecordToRecordAcceptance::new` refuses only **a non-finite or out-of-`[0,1]` ratio**
//! and **`horizon == 0`**, while `load` refuses **a changed definition**, **`|best| > scale`** and **`clock == 0` out of step with
//! `best.is_none()`**; `AutoAcceptance::new` refuses only **`horizon > i64::MAX`**, with
//! **`capacity = sqrt(horizon).clamp(8, 256)`** and **`span = (capacity/2).max(2)`** both derived, while `load` refuses **`span`
//! outside `[2, capacity]`**, **`improvements > decisions % capacity`** and **a history length other than
//! `min(decisions, capacity)`** (`memory/adaptive.rs`).
//!
//! 第 120 单量的是训练切分，这一单量**三个接受准则的定义锁与历史锁**。八条实测：
//!
//! 1. **`late` 的 `length` 只在 `(0, i32::MAX]` 内**（`the_late_length_range_is_hardcoded()`）。
//! 2. **`late` 的 `load` 拒改过的定义**（`the_late_definition_is_locked()`）。
//! 3. **`late` 的 `load` 拒越界游标与不自洽历史**（`the_late_history_stays_consistent()`）。
//! 4. **`record-to-record` 的 ratio 只在 `[0,1]` 内且必须有限**（`the_record_ratio_range_is_hardcoded()`）。
//! 5. **`record-to-record` 的 `|best| <= scale` 与时钟同步**（`the_record_best_is_bounded_by_its_scale()`）。
//! 6. **`auto` 的 capacity 与 span 是推导出来的**（`the_auto_span_is_derived()`）。
//! 7. **`auto` 拒 `span` 越界与 `improvements` 超额**（`the_auto_span_and_improvements_are_bounded()`）。
//! 8. **`auto` 的历史长度由 `min(decisions, capacity)` 定死**（`the_auto_history_matches_its_decisions()`）。
//!
//! Delivery 120 measured the training split; this one measures **the three acceptance policies' definition and history locks**.
//! **The late length range is hardcoded**, **the late definition is locked**, and **the late history stays consistent**.
//! **The record ratio range is hardcoded**, and **the record best is bounded by its own scale**. **The auto span is derived**,
//! **the auto span and improvements are bounded**, and **the auto history matches its decisions**.

use ospf_rust_core::solver::heuristic::session::{NumberVectorCodec, SearchCodec};
use ospf_rust_core::solver::hyper_heuristic::memory::{
    AutoAcceptance, LateAcceptance, RecordToRecordAcceptance,
};
use ospf_rust_core::solver::hyper_heuristic::RecoverableComponent;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 接受准则的定义。 / The acceptance policy definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptanceDefinitionSpec {
    /// 准则名（`late` / `record-to-record` / `auto`）。 / The policy name.
    pub policy: String,
    /// `late` 的历史长度。 / The late history length.
    #[serde(default)]
    pub length: Option<u64>,
    /// `record-to-record` 的初始容差比例。 / The initial tolerance ratio.
    #[serde(default)]
    pub ratio: Option<f64>,
    /// `record-to-record` 与 `auto` 的预算规模。 / The budget-scale hint.
    #[serde(default)]
    pub horizon: Option<u64>,
}

/// 对保存态的一处改写。 / One override on the saved state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptanceOverrideSpec {
    /// 被改写的键名。 / The key being overridden.
    pub key: String,
    /// 改写成 null。 / Override to null.
    #[serde(default)]
    pub null: Option<bool>,
    /// 改写成数值。 / Override to a number.
    #[serde(default)]
    pub number: Option<f64>,
    /// 改写成数值数组。 / Override to a numeric array.
    #[serde(default)]
    pub numbers: Option<Vec<f64>>,
    /// 改写成文本。 / Override to text.
    #[serde(default)]
    pub text: Option<String>,
}

/// 接受准则探针请求。 / The acceptance policy probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptanceRecoveryRequest {
    /// 定义。 / The definition.
    pub definition: AcceptanceDefinitionSpec,
    /// 依次施加的改写（每一项都在**全新实例**上试）。 / The overrides, each tried on a fresh instance.
    #[serde(default)]
    pub overrides: Vec<AcceptanceOverrideSpec>,
}

/// 一处改写的读数。 / One override's reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptanceLoadRow {
    /// 键名。 / The key.
    pub key: String,
    /// 是否被 `load` 接受。 / Whether `load` accepted it.
    pub accepted: bool,
    /// 拒收时的错误文本。 / The refusal text.
    pub error: String,
}

/// 接受准则探针结论。 / The acceptance policy probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptanceRecoveryReport {
    /// 准则名回读。 / The policy name, read back.
    pub policy: String,
    /// 框架自己 `save()` 出来的键名（按字典序）。 / The keys the framework's own `save()` produced.
    pub saved_keys: Vec<String>,
    /// 未改写的保存态能否 `load` 回来。 / Whether the untouched saved state loads back.
    pub baseline_accepted: bool,
    /// `auto` 的当前跨度。 / The current span for `auto`.
    pub history_span: u32,
    /// 各处改写的读数。 / Each override's reading.
    pub rows: Vec<AcceptanceLoadRow>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_late_length_range_is_hardcoded: bool,
    pub the_late_definition_is_locked: bool,
    pub the_late_history_stays_consistent: bool,
    pub the_record_ratio_range_is_hardcoded: bool,
    pub the_record_best_is_bounded_by_its_scale: bool,
    pub the_auto_span_is_derived: bool,
    pub the_auto_span_and_improvements_are_bounded: bool,
    pub the_auto_history_matches_its_decisions: bool,
}

/// 三个准则的统一句柄。 / One handle over the three policies.
enum AnyAcceptance {
    Late(LateAcceptance<Vec<f64>>),
    Record(RecordToRecordAcceptance),
    Auto(AutoAcceptance<Vec<f64>>),
}

impl AnyAcceptance {
    /// 造一个全新实例。 / Build a fresh instance.
    fn new(spec: &AcceptanceDefinitionSpec) -> Result<Self> {
        let invalid = |what: &str| BridgeError::new("InvalidInput",
            format!("acceptance probe failed validation: {what}"));
        let codec = || Box::new(NumberVectorCodec) as Box<dyn SearchCodec<Vec<f64>>>;
        match spec.policy.as_str() {
            "late" => Ok(Self::Late(LateAcceptance::new(codec(),
                usize::try_from(spec.length.unwrap_or(50)).map_err(|_| invalid("late length"))?)
                .map_err(|error| BridgeError::new("InvalidInput", error))?)),
            "record-to-record" => Ok(Self::Record(RecordToRecordAcceptance::new(
                spec.ratio.ok_or_else(|| invalid("the record policy needs a ratio"))?,
                spec.horizon.unwrap_or(1000))
                .map_err(|error| BridgeError::new("InvalidInput", error))?)),
            "auto" => Ok(Self::Auto(AutoAcceptance::new(codec(),
                spec.horizon.unwrap_or(1000))
                .map_err(|error| BridgeError::new("InvalidInput", error))?)),
            _ => Err(invalid("policy reads late/record-to-record/auto")),
        }
    }

    /// 保存。 / Save.
    fn save(&self) -> Result<Value> {
        match self {
            Self::Late(policy) => policy.save(), Self::Record(policy) => policy.save(),
            Self::Auto(policy) => policy.save(),
        }.map_err(|error| BridgeError::new("InvalidInput", error))
    }

    /// 载入。 / Load.
    fn load(&mut self, state: &Value) -> std::result::Result<(), String> {
        match self {
            Self::Late(policy) => policy.load(state), Self::Record(policy) => policy.load(state),
            Self::Auto(policy) => policy.load(state),
        }.map_err(|error| error.to_string())
    }

    /// 回读准则名。 / The policy name.
    fn name(&self) -> String {
        match self {
            Self::Late(_) => "late", Self::Record(_) => "record-to-record", Self::Auto(_) => "auto",
        }.to_owned()
    }

    /// `auto` 的跨度，其它准则给 0。 / The span for `auto`, zero otherwise.
    fn span(&self) -> u32 {
        match self {
            Self::Auto(policy) => u32::try_from(policy.history_span()).unwrap_or(u32::MAX),
            _ => 0,
        }
    }
}

/// 施加一处改写。 / Apply one override.
fn override_value(spec: &AcceptanceOverrideSpec) -> Value {
    if spec.null.unwrap_or(false) {
        return Value::Null;
    }
    if let Some(number) = spec.number {
        // 整值浮点写成 JSON 整数：`cursor`/`decisions` 这类字段是 `usize`/`u64`，浮点会被序列化器拒掉。
        // Integral floats become JSON integers: fields like `cursor`/`decisions` are `usize`/`u64` and refuse a float.
        let truncated = number.trunc();
        if truncated == number && truncated.abs() <= 9_007_199_254_740_992.0 {
            return json!(truncated as i64);
        }
        return json!(number);
    }
    if let Some(numbers) = &spec.numbers {
        return json!(numbers);
    }
    json!(spec.text.clone().unwrap_or_default())
}

/// 走一遍接受准则探针。非法定义是**错误**。 / Walk the probe. An illegal definition is an error.
pub fn verify_acceptance_recovery(
    request: &AcceptanceRecoveryRequest,
) -> Result<AcceptanceRecoveryReport> {
    if request.overrides.len() > 16 {
        return Err(BridgeError::new("InvalidInput", "acceptance probe failed validation: at \
            most sixteen overrides are accepted"));
    }
    let findings = canonical_findings();
    let mut policy = AnyAcceptance::new(&request.definition)?;
    // 起点是**框架自己** `save()` 的输出：不手搓状态形状。 
    // The starting point is the framework's own `save()` output: no hand-authored state shape.
    let baseline = AnyAcceptance::new(&request.definition)?.save()?;
    let mut keys: Vec<String> = baseline.as_object()
        .map(|object| object.keys().cloned().collect()).unwrap_or_default();
    keys.sort();
    let baseline_accepted = AnyAcceptance::new(&request.definition)?.load(&baseline).is_ok();
    let history_span = policy.span();
    let mut rows = Vec::with_capacity(request.overrides.len());
    for override_spec in &request.overrides {
        let mut state = baseline.clone();
        match state.as_object_mut() {
            Some(object) => { object.insert(override_spec.key.clone(), override_value(override_spec)); }
            None => return Err(BridgeError::new("InvalidInput", "acceptance probe failed \
                validation: the saved state is not an object")),
        }
        // 每处改写都在**全新实例**上试，避免借用前一处的结果。 
        // Every override is tried on a **fresh** instance so nothing leaks from the previous one.
        let mut fresh = AnyAcceptance::new(&request.definition)?;
        let (accepted, error) = match fresh.load(&state) {
            Ok(()) => (true, String::new()),
            Err(error) => (false, error),
        };
        rows.push(AcceptanceLoadRow { key: override_spec.key.clone(), accepted, error });
    }
    Ok(AcceptanceRecoveryReport {
        policy: policy.name(),
        saved_keys: keys,
        baseline_accepted,
        history_span,
        rows,
        the_late_length_range_is_hardcoded: findings.the_late_length_range_is_hardcoded,
        the_late_definition_is_locked: findings.the_late_definition_is_locked,
        the_late_history_stays_consistent: findings.the_late_history_stays_consistent,
        the_record_ratio_range_is_hardcoded: findings.the_record_ratio_range_is_hardcoded,
        the_record_best_is_bounded_by_its_scale: findings.the_record_best_is_bounded_by_its_scale,
        the_auto_span_is_derived: findings.the_auto_span_is_derived,
        the_auto_span_and_improvements_are_bounded:
            findings.the_auto_span_and_improvements_are_bounded,
        the_auto_history_matches_its_decisions: findings.the_auto_history_matches_its_decisions,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_late_length_range_is_hardcoded: bool,
    the_late_definition_is_locked: bool,
    the_late_history_stays_consistent: bool,
    the_record_ratio_range_is_hardcoded: bool,
    the_record_best_is_bounded_by_its_scale: bool,
    the_auto_span_is_derived: bool,
    the_auto_span_and_improvements_are_bounded: bool,
    the_auto_history_matches_its_decisions: bool,
}

/// 造一个 `late` 准则。 / Build a late policy.
fn late(length: usize) -> LateAcceptance<Vec<f64>> {
    LateAcceptance::new(Box::new(NumberVectorCodec), length).expect("the fixed length is valid")
}

/// 造一个 `record-to-record` 准则。 / Build a record policy.
fn record(ratio: f64, horizon: u64) -> RecordToRecordAcceptance {
    RecordToRecordAcceptance::new(ratio, horizon).expect("the fixed ratio is valid")
}

/// 造一个 `auto` 准则。 / Build an auto policy.
fn auto(horizon: u64) -> AutoAcceptance<Vec<f64>> {
    AutoAcceptance::new(Box::new(NumberVectorCodec), horizon).expect("the fixed horizon is valid")
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) late 的 length 只在 (0, i32::MAX] 内。
        // The late length range is hardcoded.
        let the_late_length_range_is_hardcoded = {
            LateAcceptance::<Vec<f64>>::new(Box::new(NumberVectorCodec), 0).is_err()
                && LateAcceptance::<Vec<f64>>::new(Box::new(NumberVectorCodec),
                    i32::MAX as usize).is_ok()
                && LateAcceptance::<Vec<f64>>::new(Box::new(NumberVectorCodec),
                    i32::MAX as usize + 1).is_err()
        };
        // 2) late 的定义被锁：把 length 改掉就拒。
        // The late definition is locked: changing the length is refused.
        let the_late_definition_is_locked = {
            let mut policy = late(5);
            let mut state = policy.save().expect("state");
            if let Some(object) = state.as_object_mut() { object.insert("length".to_owned(), json!(6)); }
            let mut fresh = late(5);
            fresh.load(&state).is_err() && state.get("length") == Some(&json!(6))
        };
        // 3) late 的历史自洽：游标越界、非空历史长度不符、空历史却游标非零都拒。
        // The late history stays consistent: an out-of-range cursor, a mismatched non-empty history and an empty history
        // with a non-zero cursor are all refused.
        let the_late_history_stays_consistent = {
            let mut fresh = late(5);
            let mut out_of_range = fresh.save().expect("state");
            if let Some(object) = out_of_range.as_object_mut() { object.insert("cursor".to_owned(), json!(5)); }
            let cursor_refused = fresh.load(&out_of_range).is_err();
            let mut mismatched = fresh.save().expect("state");
            if let Some(object) = mismatched.as_object_mut() {
                object.insert("history".to_owned(), json!([["1.0"], ["2.0"]]));
            }
            let history_refused = fresh.load(&mismatched).is_err();
            // 空历史 + 游标 1 也拒。 / An empty history with cursor one is refused too.
            let mut empty_with_cursor = fresh.save().expect("state");
            if let Some(object) = empty_with_cursor.as_object_mut() {
                object.insert("cursor".to_owned(), json!(1));
            }
            cursor_refused && history_refused && fresh.load(&empty_with_cursor).is_err()
        };
        // 4) record 的 ratio 只在 [0,1] 内且必须有限。
        // The record ratio range is hardcoded.
        let the_record_ratio_range_is_hardcoded = {
            RecordToRecordAcceptance::new(0.0, 10).is_ok()
                && RecordToRecordAcceptance::new(1.0, 10).is_ok()
                && RecordToRecordAcceptance::new(-0.1, 10).is_err()
                && RecordToRecordAcceptance::new(1.1, 10).is_err()
                && RecordToRecordAcceptance::new(f64::NAN, 10).is_err()
                && RecordToRecordAcceptance::new(0.05, 0).is_err()
        };
        // 5) record 的 |best| <= scale，且 clock == 0 与 best.is_none() 同步。
        // The record best is bounded by its own scale, and a zero clock keeps `best` empty.
        let the_record_best_is_bounded_by_its_scale = {
            let mut fresh = record(0.05, 10);
            let mut beyond_scale = fresh.save().expect("state");
            if let Some(object) = beyond_scale.as_object_mut() {
                object.insert("best".to_owned(), json!(2.0));
                object.insert("scale".to_owned(), json!(1.0));
                object.insert("clock".to_owned(), json!(1));
            }
            let bounded_refused = fresh.load(&beyond_scale).is_err();
            // clock 为 0 却带 best 也拒。 / A zero clock carrying a best value is refused.
            let mut zero_clock_with_best = fresh.save().expect("state");
            if let Some(object) = zero_clock_with_best.as_object_mut() {
                object.insert("best".to_owned(), json!(0.5));
                object.insert("scale".to_owned(), json!(1.0));
            }
            let synchronized_refused = fresh.load(&zero_clock_with_best).is_err();
            // 合法的一组照收。 / A consistent set is accepted.
            let mut legal = fresh.save().expect("state");
            if let Some(object) = legal.as_object_mut() {
                object.insert("best".to_owned(), json!(0.5));
                object.insert("scale".to_owned(), json!(1.0));
                object.insert("clock".to_owned(), json!(1));
            }
            bounded_refused && synchronized_refused && fresh.load(&legal).is_ok()
        };
        // 6) auto 的 capacity 与 span 是推导出来的：sqrt(horizon) 夹在 [8,256]，span = (capacity/2).max(2)。
        // The auto span is derived: sqrt(horizon) clamped to [8, 256], span = (capacity/2).max(2).
        let the_auto_span_is_derived = {
            // horizon 1 → capacity 8 → span 4；horizon 1000 → sqrt ≈ 31 → span 15；horizon 1e6 → 256 → span 128。
            // horizon one gives capacity eight and span four; a thousand gives thirty-one and fifteen; a million gives 256/128.
            auto(1).history_span() == 4 && auto(1000).history_span() == 15
                && auto(1_000_000).history_span() == 128
                && AutoAcceptance::<Vec<f64>>::new(Box::new(NumberVectorCodec), u64::MAX).is_err()
        };
        // 7) auto 拒 span 越界与 improvements 超额。
        // The auto span and improvements are bounded.
        let the_auto_span_and_improvements_are_bounded = {
            let mut fresh = auto(1000);
            let mut span_out = fresh.save().expect("state");
            if let Some(object) = span_out.as_object_mut() { object.insert("span".to_owned(), json!(1)); }
            let span_refused = fresh.load(&span_out).is_err();
            let mut improvements_out = fresh.save().expect("state");
            if let Some(object) = improvements_out.as_object_mut() {
                object.insert("improvements".to_owned(), json!(1));
            }
            // decisions 为 0 时 improvements 只能是 0。 / With zero decisions the improvements must be zero.
            let improvements_refused = fresh.load(&improvements_out).is_err();
            span_refused && improvements_refused
        };
        // 8) auto 的历史长度由 min(decisions, capacity) 定死。
        // The auto history matches its decisions.
        let the_auto_history_matches_its_decisions = {
            let mut fresh = auto(1000);
            let mut short = fresh.save().expect("state");
            if let Some(object) = short.as_object_mut() {
                object.insert("decisions".to_owned(), json!(1));
                object.insert("improvements".to_owned(), json!(1));
            }
            // 说做了一次决策却没有历史 → 拒。 / Claiming one decision with no history is refused.
            let refused = fresh.load(&short).is_err();
            let mut whole = fresh.save().expect("state");
            if let Some(object) = whole.as_object_mut() {
                object.insert("decisions".to_owned(), json!(1));
                object.insert("improvements".to_owned(), json!(1));
                object.insert("history".to_owned(), json!([["1.0"]]));
            }
            // 带上一条历史就成立。 / Carrying that one history entry makes it hold.
            let accepted = fresh.load(&whole).is_ok();
            refused && accepted
        };
        Findings {
            the_late_length_range_is_hardcoded,
            the_late_definition_is_locked,
            the_late_history_stays_consistent,
            the_record_ratio_range_is_hardcoded,
            the_record_best_is_bounded_by_its_scale,
            the_auto_span_is_derived,
            the_auto_span_and_improvements_are_bounded,
            the_auto_history_matches_its_decisions,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: AcceptanceRecoveryRequest) -> AcceptanceRecoveryReport {
        verify_acceptance_recovery(&request).unwrap()
    }

    fn definition(policy: &str) -> AcceptanceDefinitionSpec {
        AcceptanceDefinitionSpec { policy: policy.to_owned(), length: None, ratio: None,
            horizon: None }
    }

    fn request(policy: &str) -> AcceptanceRecoveryRequest {
        // `record-to-record` 必须显式给 ratio，所以辅助函数替它补一个。 
        // `record-to-record` demands an explicit ratio, so the helper fills one in.
        let definition = if policy == "record-to-record" {
            AcceptanceDefinitionSpec { ratio: Some(0.05), ..definition(policy) }
        } else { definition(policy) };
        AcceptanceRecoveryRequest { definition, overrides: Vec::new() }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request("late"));
        assert_eq!(report.policy, "late");
        assert!(report.baseline_accepted);
        assert_eq!(report.history_span, 0);
        assert!(report.the_late_length_range_is_hardcoded);
        assert!(report.the_late_definition_is_locked);
        assert!(report.the_late_history_stays_consistent);
        assert!(report.the_record_ratio_range_is_hardcoded);
        assert!(report.the_record_best_is_bounded_by_its_scale);
        assert!(report.the_auto_span_is_derived);
        assert!(report.the_auto_span_and_improvements_are_bounded);
        assert!(report.the_auto_history_matches_its_decisions);
    }

    #[test]
    fn the_saved_keys_come_from_the_framework() {
        // 三个准则各有一组自己的键名。 / Each policy has its own set of keys.
        let late = ask(request("late"));
        assert_eq!(late.saved_keys, vec!["cursor".to_owned(), "history".to_owned(),
            "length".to_owned()]);
        let record = ask(request("record-to-record"));
        assert_eq!(record.saved_keys, vec!["best".to_owned(), "clock".to_owned(),
            "horizon".to_owned(), "ratio".to_owned(), "scale".to_owned()]);
        let auto = ask(request("auto"));
        assert!(auto.saved_keys.contains(&"capacity".to_owned()));
        assert!(auto.saved_keys.contains(&"decisions".to_owned()));
        assert!(auto.saved_keys.contains(&"span".to_owned()));
    }

    #[test]
    fn the_overrides_report_acceptance_per_key() {
        let report = ask(AcceptanceRecoveryRequest { definition: definition("late"),
            overrides: vec![
                // 改定义 → 拒。 / A changed definition is refused.
                AcceptanceOverrideSpec { key: "length".to_owned(), null: None,
                    number: Some(6.0), numbers: None, text: None },
                // 越界游标 → 拒。 / An out-of-range cursor is refused.
                AcceptanceOverrideSpec { key: "cursor".to_owned(), null: None,
                    number: Some(5.0), numbers: None, text: None },
            ] });
        assert_eq!(report.rows.len(), 2);
        assert!(!report.rows[0].accepted);
        assert!(report.rows[0].error.contains("invalid late acceptance definition"),
            "{}", report.rows[0].error);
        assert!(!report.rows[1].accepted);
    }

    #[test]
    fn a_legal_record_state_passes_and_an_illegal_one_fails() {
        let report = ask(AcceptanceRecoveryRequest {
            definition: AcceptanceDefinitionSpec { ratio: Some(0.05), horizon: Some(10),
                ..definition("record-to-record") },
            overrides: vec![AcceptanceOverrideSpec { key: "scale".to_owned(), null: None,
                number: Some(1.0), numbers: None, text: None }] });
        // 只改 scale（best 仍是 None、clock 仍是 0）→ 接受。 
        // Only the scale changes, with `best` still empty and the clock still zero, so it is accepted.
        assert!(report.rows[0].accepted, "{}", report.rows[0].error);
        let illegal = ask(AcceptanceRecoveryRequest {
            definition: AcceptanceDefinitionSpec { ratio: Some(0.05),
                ..definition("record-to-record") },
            overrides: vec![AcceptanceOverrideSpec { key: "best".to_owned(), null: None,
                number: Some(0.5), numbers: None, text: None }] });
        // clock 仍为 0 却带 best → 拒。 / A best value with a zero clock is refused.
        assert!(!illegal.rows[0].accepted);
        assert!(illegal.rows[0].error.contains("invalid record-to-record state"),
            "{}", illegal.rows[0].error);
    }

    #[test]
    fn the_auto_span_is_reported_and_bounded() {
        let report = ask(AcceptanceRecoveryRequest { definition: definition("auto"),
            overrides: vec![AcceptanceOverrideSpec { key: "span".to_owned(), null: None,
                number: Some(1.0), numbers: None, text: None }] });
        // horizon 默认 1000 → span 15；把 span 改成 1 被拒。 
        // The default horizon is a thousand, so the span is fifteen; forcing it to one is refused.
        assert_eq!(report.history_span, 15);
        assert!(!report.rows[0].accepted);
        assert!(report.rows[0].error.contains("incompatible automatic acceptance state"),
            "{}", report.rows[0].error);
    }

    #[test]
    fn bad_definitions_are_refused() {
        assert!(verify_acceptance_recovery(&request("nearly")).unwrap_err()
            .message.contains("policy reads late/record-to-record/auto"));
        assert!(verify_acceptance_recovery(&AcceptanceRecoveryRequest {
            definition: definition("record-to-record"), overrides: Vec::new() }).unwrap_err()
            .message.contains("needs a ratio"));
        assert!(verify_acceptance_recovery(&AcceptanceRecoveryRequest {
            definition: AcceptanceDefinitionSpec { length: Some(0), ..definition("late") },
            overrides: Vec::new() }).unwrap_err().message.contains("invalid late acceptance length"));
        assert!(verify_acceptance_recovery(&AcceptanceRecoveryRequest {
            definition: AcceptanceDefinitionSpec { ratio: Some(1.5),
                ..definition("record-to-record") }, overrides: Vec::new() }).unwrap_err()
            .message.contains("invalid record-to-record definition"));
        assert!(verify_acceptance_recovery(&AcceptanceRecoveryRequest {
            definition: definition("late"),
            overrides: (0..17).map(|index| AcceptanceOverrideSpec {
                key: format!("k{index}"), null: None, number: Some(1.0), numbers: None,
                text: None }).collect() }).unwrap_err().message.contains("sixteen overrides"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request("late"));
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"savedKeys\""), "{json}");
        assert!(json.contains("\"historySpan\""), "{json}");
        assert!(json.contains("\"theLateLengthRangeIsHardcoded\""), "{json}");
        assert!(!json.contains("saved_keys"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<AcceptanceRecoveryReport>(&copy.to_string()).is_err());
    }
}
