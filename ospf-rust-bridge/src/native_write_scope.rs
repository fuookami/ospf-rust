//! 续挖第 53 单：原生写入记录与 lowering 报告——**两个谓词成对互补、两个计数不与结果交叉校验**
//! （`model/intermediate/native_writer.rs` + `deferred_function.rs::verify_native_write`）
//! Continuation, delivery 53: native write records and the lowering report — **two predicates are exact complements while two
//! counters are never cross-checked against the outcomes** (`native_writer.rs` + `verify_native_write`).
//!
//! 这一单接着 §11.14 点名的 `model/intermediate`，量的是"原生写入 vs 通用 fallback"这条恢复路径。七条实测：
//!
//! 1. **`is_native()` 与 `requires_fallback()` 精确互补**（`the_two_predicates_are_complementary()`）：两种变体各答一个。
//! 2. **空报告两边都是 `false`**（`an_empty_report_says_neither()`）：`outcomes` 为空时 `is_fully_native()` 是 `false`
//!    （因为它要求 `!outcomes.is_empty()`），而 `has_fallbacks()` 也是 `false`——"没事可做"既不算全原生也不算有回退。
//! 3. **`is_fully_native()` 只看 `materialized_fallbacks == 0`**（`fully_native_ignores_the_outcomes()`）：
//!    `outcomes` 里全是 `Fallback`、但 `materialized_fallbacks` 写成 `0` 的报告**照样回答 `true`**——
//!    两个计数字段是调用方填的，本层**不校验它们与 `outcomes` 一致**。
//! 4. **`has_fallbacks()` 同样只看计数**（`has_fallbacks_ignores_the_outcomes()`）：`outcomes` 全 fallback、
//!    计数为 0 时回答 `false`。
//! 5. **`NativeWriteRecord::new` 造出来的记录永远匹配不上**（`a_fresh_record_never_matches()`）：它把 `fingerprint`
//!    设为 `None`，而匹配要求**两侧都存在且相等**——必须先 `with_fingerprint(Some(...))`。
//! 6. **指纹两侧缺一不可**（`both_fingerprints_must_be_present()`）：记录没指纹、或当前结构没指纹，`matches_structure`
//!    都是 `false`，所以"没有指纹"与"指纹不同"在结论上**无法区分**。
//! 7. **函数名不参与匹配**（`the_function_name_is_not_part_of_the_match()`）：`with_function("other")` 之后仍然匹配，
//!    匹配只看指纹；`FallbackReason` 的三种变体（`NoWriter` / `Rejected(..)` / `WriterFailed(..)`）也因此互不相等。
//!
//! This one continues the `model/intermediate` surface the audit names, measuring the "native write versus generic fallback"
//! recovery path. **`is_native()` and `requires_fallback()` are exact complements** (`the_two_predicates_are_complementary()`).
//! **An empty report answers neither** (`an_empty_report_says_neither()`): with no outcomes `is_fully_native()` is `false`
//! (it demands `!outcomes.is_empty()`) and `has_fallbacks()` is `false` too. **`is_fully_native()` reads only
//! `materialized_fallbacks == 0`** (`fully_native_ignores_the_outcomes()`): a report whose outcomes are all `Fallback` while the
//! counter says `0` **still answers `true`**, because both counters are caller-supplied and **never cross-checked against the
//! outcomes**. **`has_fallbacks()` likewise reads only the counter** (`has_fallbacks_ignores_the_outcomes()`). **A record made
//! by `NativeWriteRecord::new` never matches** (`a_fresh_record_never_matches()`): it leaves `fingerprint` as `None` while
//! matching demands **both sides present and equal**. **Both fingerprints are required** (`both_fingerprints_must_be_present()`),
//! so "no fingerprint" and "different fingerprint" are **indistinguishable** in the answer. And **the function name is not part
//! of the match** (`the_function_name_is_not_part_of_the_match()`), which is why the three `FallbackReason` variants never
//! compare equal to each other.

use std::collections::HashMap;
use std::fmt::Debug;

use ospf_rust_core::error::Result as CoreResult;
use ospf_rust_core::model::LinearConstraint;
use ospf_rust_core::model::intermediate::deferred_function::{
    verify_native_write, DeferredFunctionStructure,
};
use ospf_rust_core::model::intermediate::native_writer::{
    FallbackReason, NativeLoweringReport, NativeWriteOutcome, NativeWriteRecord,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条写入记录的字面量。 / One write-record literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteRecordSpec {
    pub writer: String,
    pub schema: String,
    /// 指纹；缺省表示记录没写指纹。 / The fingerprint; absent means the record carries none.
    #[serde(default)]
    pub fingerprint: Option<String>,
    /// 函数名；缺省是空串。 / The function name; absent means the empty string.
    #[serde(default)]
    pub function: Option<String>,
}

/// 当前结构的字面量。 / The current structure's literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeStructureSpec {
    /// 当前结构的指纹；缺省表示没有。 / The current structure's fingerprint; absent means none.
    #[serde(default)]
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub function_name: Option<String>,
}

/// 一个结果的字面量。 / One outcome literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutcomeSpec {
    /// `native` 或 `fallback`。 / `native` or `fallback`.
    pub kind: String,
    /// fallback 的原因：`no-writer` / `rejected` / `writer-failed`。 / The fallback reason.
    #[serde(default)]
    pub reason: Option<String>,
    /// 原因消息（`rejected` / `writer-failed` 用）。 / The reason message.
    #[serde(default)]
    pub message: Option<String>,
}

/// 原生写入面请求。 / The native-write surface request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeWriteScopeRequest {
    #[serde(default)]
    pub records: Vec<WriteRecordSpec>,
    #[serde(default)]
    pub structure: Option<NativeStructureSpec>,
    #[serde(default)]
    pub outcomes: Vec<OutcomeSpec>,
    /// 报告里的 `native_writes`。 / The report's `native_writes`.
    #[serde(default)]
    pub native_writes: Option<u64>,
    /// 报告里的 `materialized_fallbacks`。 / The report's `materialized_fallbacks`.
    #[serde(default)]
    pub materialized_fallbacks: Option<u64>,
}

/// 原生写入面结论。 / The native-write surface verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeWriteScopeReport {
    /// 每条记录是否匹配当前结构。 / Whether each record matches the current structure.
    pub matches: Vec<bool>,
    /// 与 `matches` 对齐的 fallback 原因名（结果不是 fallback 时为 `null`）。
    /// The fallback reason names aligned with `outcomes` (`null` when the outcome is native).
    pub reasons: Vec<Option<String>>,
    /// 报告里的 `is_fully_native()`。 / The report's `is_fully_native()`.
    pub fully_native: bool,
    /// 报告里的 `has_fallbacks()`。 / The report's `has_fallbacks()`.
    pub has_fallbacks: bool,
    /// 每种结果的两个谓词。 / Both predicates for each outcome.
    pub predicates: Vec<OutcomePredicates>,
    /// 两个谓词精确互补（**是**）。 / Whether the two predicates are exact complements (**yes**).
    pub the_two_predicates_are_complementary: bool,
    /// 空报告两边都是 false（**是**）。 / Whether an empty report says neither (**yes**).
    pub an_empty_report_says_neither: bool,
    /// `is_fully_native()` 只看计数（**是**）。 / Whether `is_fully_native()` reads only the counter (**yes**).
    pub fully_native_ignores_the_outcomes: bool,
    /// `has_fallbacks()` 只看计数（**是**）。 / Whether `has_fallbacks()` reads only the counter (**yes**).
    pub has_fallbacks_ignores_the_outcomes: bool,
    /// `new` 的记录永远匹配不上（**是**）。 / Whether a fresh record never matches (**yes**).
    pub a_fresh_record_never_matches: bool,
    /// 指纹两侧缺一不可（**是**）。 / Whether both fingerprints are required (**yes**).
    pub both_fingerprints_must_be_present: bool,
    /// 函数名不参与匹配（**是**）。 / Whether the function name is out of the match (**yes**).
    pub the_function_name_is_not_part_of_the_match: bool,
}

/// 一条结果的两个谓词。 / Both predicates for one outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutcomePredicates {
    pub is_native: bool,
    pub requires_fallback: bool,
}

/// 记录与结果条数上限（信任边界）。 / The record and outcome cap (a trust boundary).
const MAX_ENTRIES: usize = 64;

/// 测试用的结构描述：只有指纹与函数名参与本单的度量。 / A probe structure: only the fingerprint and name matter here.
#[derive(Debug)]
struct ProbeStructure {
    fingerprint: Option<String>,
    function_name: String,
}

impl DeferredFunctionStructure<f64> for ProbeStructure {
    fn function_name(&self) -> &str {
        &self.function_name
    }

    fn materialize(
        &self,
        _symbol_to_index: &HashMap<usize, usize>,
    ) -> CoreResult<Vec<LinearConstraint<f64>>> {
        // 本单不物化任何行。 / This delivery materializes no rows.
        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fingerprint(&self) -> Option<String> {
        self.fingerprint.clone()
    }
}

/// 造一条记录。 / Build one record.
fn record_of(spec: &WriteRecordSpec) -> NativeWriteRecord {
    NativeWriteRecord::new(spec.writer.clone(), spec.schema.clone())
        .with_fingerprint(spec.fingerprint.clone())
        .with_function(spec.function.clone().unwrap_or_default())
}

/// 造一条结果。 / Build one outcome.
fn outcome_of(spec: &OutcomeSpec) -> Result<NativeWriteOutcome> {
    Ok(match spec.kind.as_str() {
        "native" => NativeWriteOutcome::Native(NativeWriteRecord::new("probe", "probe-1")
            .with_fingerprint(Some("probe-fingerprint".to_owned()))
            .with_function("probe")),
        "fallback" => NativeWriteOutcome::Fallback(
            match spec.reason.as_deref().unwrap_or("no-writer") {
                "no-writer" => FallbackReason::NoWriter,
                "rejected" => FallbackReason::Rejected(
                    spec.message.clone().unwrap_or_else(|| "rejected".to_owned())),
                "writer-failed" => FallbackReason::WriterFailed(
                    spec.message.clone().unwrap_or_else(|| "failed".to_owned())),
                other => return Err(BridgeError::new("InvalidInput", &format!(
                    "native write scope failed validation: unknown fallback reason `{other}`"))),
            }),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "native write scope failed validation: unknown outcome kind `{other}`"))),
    })
}

/// 原因名。 / The reason name.
fn reason_name(reason: &FallbackReason) -> &'static str {
    match reason {
        FallbackReason::NoWriter => "no-writer",
        FallbackReason::Rejected(_) => "rejected",
        FallbackReason::WriterFailed(_) => "writer-failed",
    }
}

/// 走一遍原生写入面。非法输入是**错误**。 / Walk the native-write surface.
pub fn verify_native_write_scope(request: &NativeWriteScopeRequest)
    -> Result<NativeWriteScopeReport>
{
    if request.records.len() > MAX_ENTRIES || request.outcomes.len() > MAX_ENTRIES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "native write scope failed validation: at most {MAX_ENTRIES} entries are accepted")));
    }
    let outcomes = request.outcomes.iter().map(outcome_of).collect::<Result<Vec<_>>>()?;
    let report = NativeLoweringReport {
        materialized_fallbacks: request.materialized_fallbacks
            .map(|count| count as usize).unwrap_or_else(|| outcomes.iter()
            .filter(|outcome| !outcome.is_native()).count()),
        native_writes: request.native_writes
            .map(|count| count as usize).unwrap_or_else(|| outcomes.iter()
            .filter(|outcome| outcome.is_native()).count()),
        outcomes: outcomes.clone(),
    };
    let structure = request.structure.as_ref().map(|spec| ProbeStructure {
        fingerprint: spec.fingerprint.clone(),
        function_name: spec.function_name.clone().unwrap_or_default() });
    let matches = request.records.iter().map(|spec| match &structure {
        Some(structure) => record_of(spec).matches_structure::<f64>(structure),
        None => verify_native_write::<f64>(spec.fingerprint.as_deref(),
            &ProbeStructure { fingerprint: None, function_name: String::new() }),
    }).collect::<Vec<_>>();
    let reasons = outcomes.iter().map(|outcome| match outcome {
        NativeWriteOutcome::Native(_) => None,
        NativeWriteOutcome::Fallback(reason) => Some(reason_name(reason).to_owned()),
    }).collect::<Vec<_>>();
    let predicates = outcomes.iter().map(|outcome| OutcomePredicates {
        is_native: outcome.is_native(),
        requires_fallback: outcome.requires_fallback() }).collect::<Vec<_>>();
    let findings = canonical_findings();
    Ok(NativeWriteScopeReport { matches, reasons, fully_native: report.is_fully_native(),
        has_fallbacks: report.has_fallbacks(), predicates,
        the_two_predicates_are_complementary: findings.the_two_predicates_are_complementary,
        an_empty_report_says_neither: findings.an_empty_report_says_neither,
        fully_native_ignores_the_outcomes: findings.fully_native_ignores_the_outcomes,
        has_fallbacks_ignores_the_outcomes: findings.has_fallbacks_ignores_the_outcomes,
        a_fresh_record_never_matches: findings.a_fresh_record_never_matches,
        both_fingerprints_must_be_present: findings.both_fingerprints_must_be_present,
        the_function_name_is_not_part_of_the_match:
            findings.the_function_name_is_not_part_of_the_match })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    the_two_predicates_are_complementary: bool,
    an_empty_report_says_neither: bool,
    fully_native_ignores_the_outcomes: bool,
    has_fallbacks_ignores_the_outcomes: bool,
    a_fresh_record_never_matches: bool,
    both_fingerprints_must_be_present: bool,
    the_function_name_is_not_part_of_the_match: bool,
}

/// 造一个探针结构。 / Build a probe structure.
fn probe(fingerprint: Option<&str>, name: &str) -> ProbeStructure {
    ProbeStructure { fingerprint: fingerprint.map(|value| value.to_owned()),
        function_name: name.to_owned() }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 两个谓词精确互补。 / The two predicates are exact complements.
    let the_two_predicates_are_complementary = {
        let native = NativeWriteOutcome::Native(NativeWriteRecord::new("w", "s-1"));
        let fallback = NativeWriteOutcome::Fallback(FallbackReason::NoWriter);
        (native.is_native() && !native.requires_fallback())
            && (!fallback.is_native() && fallback.requires_fallback())
    };
    // 2) 空报告两边都是 false。 / An empty report answers neither.
    let an_empty_report_says_neither = {
        let empty = NativeLoweringReport { outcomes: Vec::new(), materialized_fallbacks: 0,
            native_writes: 0 };
        !empty.is_fully_native() && !empty.has_fallbacks()
    };
    // 3) `is_fully_native()` 只看计数：全是 fallback 但计数为 0 仍为 true。 
    // 3) `is_fully_native()` reads only the counter: all-fallback outcomes with a zero counter still answer true.
    let fully_native_ignores_the_outcomes = {
        let lying = NativeLoweringReport {
            outcomes: vec![NativeWriteOutcome::Fallback(FallbackReason::NoWriter)],
            materialized_fallbacks: 0, native_writes: 0 };
        lying.is_fully_native()
    };
    // 4) `has_fallbacks()` 同样只看计数。 / `has_fallbacks()` reads only the counter too.
    let has_fallbacks_ignores_the_outcomes = {
        let lying = NativeLoweringReport {
            outcomes: vec![NativeWriteOutcome::Fallback(FallbackReason::NoWriter)],
            materialized_fallbacks: 0, native_writes: 1 };
        !lying.has_fallbacks()
            && NativeLoweringReport { outcomes: vec![
                NativeWriteOutcome::Native(NativeWriteRecord::new("w", "s-1"))],
                materialized_fallbacks: 3, native_writes: 1 }.has_fallbacks()
    };
    // 5) `new` 的记录永远匹配不上。 / A record from `new` never matches.
    let a_fresh_record_never_matches = {
        let fresh = NativeWriteRecord::new("writer", "functions-abs-1");
        let structure = probe(Some("fingerprint-1"), "abs");
        !fresh.matches_structure::<f64>(&structure)
            && fresh.with_fingerprint(Some("fingerprint-1".to_owned()))
                .matches_structure::<f64>(&structure)
    };
    // 6) 指纹两侧缺一不可。 / Both fingerprints are required.
    let both_fingerprints_must_be_present = {
        let recorded = NativeWriteRecord::new("writer", "s-1")
            .with_fingerprint(Some("fingerprint-1".to_owned()));
        let without = NativeWriteRecord::new("writer", "s-1");
        !recorded.matches_structure::<f64>(&probe(None, "abs"))
            && !without.matches_structure::<f64>(&probe(Some("fingerprint-1"), "abs"))
            && !without.matches_structure::<f64>(&probe(None, "abs"))
    };
    // 7) 函数名不参与匹配；三种 fallback 原因互不相等。 
    // 7) The function name is out of the match, and the three fallback reasons differ.
    let the_function_name_is_not_part_of_the_match = {
        let recorded = NativeWriteRecord::new("writer", "s-1")
            .with_fingerprint(Some("fingerprint-1".to_owned())).with_function("abs");
        let renamed = recorded.clone().with_function("other");
        recorded.matches_structure::<f64>(&probe(Some("fingerprint-1"), "abs"))
            && renamed.matches_structure::<f64>(&probe(Some("fingerprint-1"), "something-else"))
            && FallbackReason::NoWriter != FallbackReason::Rejected("x".to_owned())
            && FallbackReason::Rejected("x".to_owned())
                != FallbackReason::WriterFailed("x".to_owned())
    };
    Findings { the_two_predicates_are_complementary, an_empty_report_says_neither,
        fully_native_ignores_the_outcomes, has_fallbacks_ignores_the_outcomes,
        a_fresh_record_never_matches, both_fingerprints_must_be_present,
        the_function_name_is_not_part_of_the_match }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> NativeWriteScopeRequest {
        NativeWriteScopeRequest {
            records: vec![
                WriteRecordSpec { writer: "abs-writer".to_owned(),
                    schema: "functions-abs-1".to_owned(), fingerprint: Some("fp-1".to_owned()),
                    function: Some("abs".to_owned()) },
                WriteRecordSpec { writer: "abs-writer".to_owned(),
                    schema: "functions-abs-1".to_owned(), fingerprint: None,
                    function: Some("abs".to_owned()) },
            ],
            structure: Some(NativeStructureSpec { fingerprint: Some("fp-1".to_owned()),
                function_name: Some("abs".to_owned()) }),
            outcomes: vec![OutcomeSpec { kind: "native".to_owned(), reason: None,
                message: None }, OutcomeSpec { kind: "fallback".to_owned(),
                reason: Some("rejected".to_owned()), message: Some("range".to_owned()) }],
            native_writes: None, materialized_fallbacks: None,
        }
    }

    #[test]
    fn records_match_only_when_both_fingerprints_agree() {
        let report = verify_native_write_scope(&request()).unwrap();
        assert_eq!(report.matches, vec![true, false]);
        assert!(report.a_fresh_record_never_matches);
        assert!(report.both_fingerprints_must_be_present);
        assert!(report.the_function_name_is_not_part_of_the_match);
    }

    #[test]
    fn the_outcome_predicates_are_complementary() {
        let report = verify_native_write_scope(&request()).unwrap();
        assert_eq!(report.predicates[0],
            OutcomePredicates { is_native: true, requires_fallback: false });
        assert_eq!(report.predicates[1],
            OutcomePredicates { is_native: false, requires_fallback: true });
        assert_eq!(report.reasons, vec![None, Some("rejected".to_owned())]);
        assert!(report.the_two_predicates_are_complementary);
        // 计数由 `outcomes` 推出时：一个 fallback → 不是全原生、有回退。 
        // With the counters derived from the outcomes: one fallback means not fully native and a fallback exists.
        assert!(!report.fully_native);
        assert!(report.has_fallbacks);
    }

    #[test]
    fn the_counters_are_not_cross_checked() {
        // 全是 fallback，但计数说 0 → `is_fully_native()` 仍为真。 
        // All fallback outcomes while the counter says 0 → `is_fully_native()` is still true.
        let lying = NativeWriteScopeRequest {
            outcomes: vec![OutcomeSpec { kind: "fallback".to_owned(),
                reason: Some("no-writer".to_owned()), message: None }],
            materialized_fallbacks: Some(0), native_writes: Some(1), ..request() };
        let report = verify_native_write_scope(&lying).unwrap();
        assert!(report.fully_native);
        assert!(!report.has_fallbacks);
        assert!(report.fully_native_ignores_the_outcomes);
        assert!(report.has_fallbacks_ignores_the_outcomes);
        // 空报告两边都是 false。 / An empty report answers neither.
        let empty = NativeWriteScopeRequest { outcomes: Vec::new(), ..request() };
        let report = verify_native_write_scope(&empty).unwrap();
        assert!(!report.fully_native);
        assert!(!report.has_fallbacks);
        assert!(report.an_empty_report_says_neither);
    }

    #[test]
    fn a_structure_without_a_fingerprint_never_matches() {
        let report = verify_native_write_scope(&NativeWriteScopeRequest {
            structure: Some(NativeStructureSpec { fingerprint: None,
                function_name: Some("abs".to_owned()) }), ..request() }).unwrap();
        assert!(report.matches.iter().all(|matched| !matched));
        // 不给结构时，记录也匹配不上（当前指纹视为没有）。 
        // With no structure at all, records cannot match either (the current fingerprint counts as absent).
        let report = verify_native_write_scope(&NativeWriteScopeRequest {
            structure: None, ..request() }).unwrap();
        assert!(report.matches.iter().all(|matched| !matched));
    }

    #[test]
    fn bad_input_is_refused() {
        let unknown_kind = NativeWriteScopeRequest {
            outcomes: vec![OutcomeSpec { kind: "cached".to_owned(), reason: None, message: None }],
            ..request() };
        assert!(verify_native_write_scope(&unknown_kind).unwrap_err().message
            .contains("unknown outcome kind"));
        let unknown_reason = NativeWriteScopeRequest {
            outcomes: vec![OutcomeSpec { kind: "fallback".to_owned(),
                reason: Some("stale".to_owned()), message: None }], ..request() };
        assert!(verify_native_write_scope(&unknown_reason).unwrap_err().message
            .contains("unknown fallback reason"));
        let too_many = NativeWriteScopeRequest {
            records: (0..65).map(|index| WriteRecordSpec {
                writer: format!("w{index}"), schema: "s-1".to_owned(), fingerprint: None,
                function: None }).collect(), ..request() };
        assert!(verify_native_write_scope(&too_many).unwrap_err().message
            .contains("at most 64 entries"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_native_write_scope(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"fullyNative\""), "{json}");
        assert!(json.contains("\"requiresFallback\""), "{json}");
        assert!(json.contains("\"aFreshRecordNeverMatches\""), "{json}");
        assert!(!json.contains("fully_native"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<NativeWriteScopeReport>(&copy.to_string()).is_err());
    }
}
