//! 续挖第 116 单：**原生 lowering 报告的三个判定各看各的字段，"全原生"与"有退化"既不互补也不自检**——
//! `is_fully_native()` 要求 `outcomes` **非空**且 `materialized_fallbacks == 0`，于是**空报告既不是全原生也不是
//! 有退化**；两者都**只读计数、不数 `outcomes`**，所以"每个结构都退化了但计数写着 0"的报告仍然自称全原生；
//! 报告**不校验** `native_writes + materialized_fallbacks == outcomes.len()`，自相矛盾的计数照收；而每个结果的
//! `is_native()`/`requires_fallback()` 对两个变体**严格互补**；`FallbackReason` 的相等是结构相等，
//! `Rejected("x")` 与 `WriterFailed("x")` **不相等**
//! （`model/intermediate/native_writer.rs` 的 `NativeLoweringReport` / `NativeWriteOutcome` / `FallbackReason`）
//! Continuation, delivery 116: **the native lowering report's three predicates each read their own field, and "fully native"
//! and "has fallbacks" are neither complementary nor self-checking** — `is_fully_native()` requires `outcomes` to be
//! **non-empty** and `materialized_fallbacks == 0`, so **an empty report is neither fully native nor falling back**; both read
//! **only the counters, never the `outcomes`**, so a report claiming zero materialized fallbacks while every outcome fell back
//! still calls itself fully native; the report **does not check** that `native_writes + materialized_fallbacks` equals
//! `outcomes.len()`, so contradictory counters are accepted; each outcome's `is_native()`/`requires_fallback()` are strictly
//! complementary across the two variants; and `FallbackReason` equality is structural, so `Rejected("x")` is **not**
//! `WriterFailed("x")` (`NativeLoweringReport` / `NativeWriteOutcome` / `FallbackReason` in
//! `model/intermediate/native_writer.rs`).
//!
//! 第 115 单量的是 native 终态证明门，这一单量**原生 lowering 报告的自检缺口**。八条实测：
//!
//! 1. **空报告不是全原生**（`an_empty_report_is_not_fully_native()`）。
//! 2. **空报告也没有退化**（`an_empty_report_has_no_fallbacks()`）。
//! 3. **两个判定不互补**（`the_two_predicates_are_not_complementary()`）。
//! 4. **"全原生"只读计数、不数 `outcomes`**（`fully_native_reads_only_the_counter()`）。
//! 5. **"有退化"同样只读计数**（`has_fallbacks_reads_only_the_counter()`）。
//! 6. **计数与 `outcomes` 不自检**（`the_counts_are_never_cross_checked()`）。
//! 7. **每个结果的两个判定严格互补**（`the_two_outcome_predicates_are_complementary()`）。
//! 8. **`FallbackReason` 的相等是结构相等**（`the_fallback_reason_equals_structurally()`）。
//!
//! Delivery 115 measured the native terminal proof gate; this one measures **the self-check gaps of the native lowering
//! report**. **An empty report is neither fully native nor falling back**, and **the two predicates are not complementary**.
//! **"Fully native" reads only the counter** and **so does "has fallbacks"**, because **the counts are never cross-checked**.
//! **Each outcome's two predicates are complementary**, and **`FallbackReason` equals structurally**.

use ospf_rust_core::model::intermediate::native_writer::{
    FallbackReason, NativeLoweringReport, NativeWriteOutcome, NativeWriteRecord,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个结构去向的描述。 / The description of one structure's destination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeOutcomeSpec {
    /// 变体名（`Native` / `NoWriter` / `Rejected` / `WriterFailed`）。 / The variant name.
    pub kind: String,
    /// 附带文本（`Rejected` / `WriterFailed` 的消息，或 `Native` 的 writer 名）。
    /// The attached text.
    #[serde(default)]
    pub text: Option<String>,
}

/// 原生 lowering 报告探针请求。 / The native lowering report probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeLoweringProbeRequest {
    /// 每个待展开结构的去向。 / Every pending structure's destination.
    #[serde(default)]
    pub outcomes: Vec<NativeOutcomeSpec>,
    /// 报告自带的"已物化 fallback"计数（默认等于各结果的 fallback 数）。
    /// The report's own materialized-fallback counter.
    #[serde(default)]
    pub materialized_fallbacks: Option<u32>,
    /// 报告自带的"原生写入"计数（默认等于各结果的 native 数）。
    /// The report's own native-write counter.
    #[serde(default)]
    pub native_writes: Option<u32>,
}

/// 原生 lowering 报告探针结论。 / The native lowering report probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeLoweringProbeReport {
    /// `is_fully_native()` 的读数。 / What `is_fully_native()` says.
    pub fully_native: bool,
    /// `has_fallbacks()` 的读数。 / What `has_fallbacks()` says.
    pub has_fallbacks: bool,
    /// 结果条数。 / The outcome count.
    pub outcome_count: u32,
    /// `is_native()` 为真的条数。 / How many outcomes say `is_native()`.
    pub native_outcomes: u32,
    /// `requires_fallback()` 为真的条数。 / How many outcomes demand the fallback.
    pub fallback_outcomes: u32,
    /// 计数与 `outcomes` 是否一致。 / Whether the counters agree with the outcomes.
    pub counts_agree: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub an_empty_report_is_not_fully_native: bool,
    pub an_empty_report_has_no_fallbacks: bool,
    pub the_two_predicates_are_not_complementary: bool,
    pub fully_native_reads_only_the_counter: bool,
    pub has_fallbacks_reads_only_the_counter: bool,
    pub the_counts_are_never_cross_checked: bool,
    pub the_two_outcome_predicates_are_complementary: bool,
    pub the_fallback_reason_equals_structurally: bool,
}

/// 造一条去向。 / Build one destination.
fn outcome_of(spec: &NativeOutcomeSpec) -> Result<NativeWriteOutcome> {
    let text = spec.text.clone().unwrap_or_default();
    match spec.kind.as_str() {
        "Native" => Ok(NativeWriteOutcome::Native(
            NativeWriteRecord::new(text.as_str(), "functions-probe-1"))),
        "NoWriter" => Ok(NativeWriteOutcome::Fallback(FallbackReason::NoWriter)),
        "Rejected" => Ok(NativeWriteOutcome::Fallback(FallbackReason::Rejected(text))),
        "WriterFailed" => Ok(NativeWriteOutcome::Fallback(FallbackReason::WriterFailed(text))),
        _ => Err(BridgeError::new("InvalidInput", "native lowering probe failed validation: \
            kind reads Native/NoWriter/Rejected/WriterFailed")),
    }
}

/// 走一遍原生 lowering 报告探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_native_lowering_probe(
    request: &NativeLoweringProbeRequest,
) -> Result<NativeLoweringProbeReport> {
    if request.outcomes.len() > 64 {
        return Err(BridgeError::new("InvalidInput", "native lowering probe failed validation: \
            at most sixty-four outcomes are accepted"));
    }
    let findings = canonical_findings();
    let outcomes: Vec<NativeWriteOutcome> = request.outcomes.iter()
        .map(outcome_of).collect::<Result<Vec<_>>>()?;
    let native_outcomes = outcomes.iter().filter(|outcome| outcome.is_native()).count();
    let fallback_outcomes = outcomes.iter().filter(|outcome| outcome.requires_fallback()).count();
    let materialized_fallbacks = request.materialized_fallbacks
        .map(|count| count as usize).unwrap_or(fallback_outcomes);
    let native_writes = request.native_writes
        .map(|count| count as usize).unwrap_or(native_outcomes);
    let report = NativeLoweringReport {
        outcomes,
        materialized_fallbacks,
        native_writes,
    };
    Ok(NativeLoweringProbeReport {
        fully_native: report.is_fully_native(),
        has_fallbacks: report.has_fallbacks(),
        outcome_count: u32::try_from(report.outcomes.len()).unwrap_or(u32::MAX),
        native_outcomes: u32::try_from(native_outcomes).unwrap_or(u32::MAX),
        fallback_outcomes: u32::try_from(fallback_outcomes).unwrap_or(u32::MAX),
        counts_agree: report.native_writes + report.materialized_fallbacks
            == report.outcomes.len(),
        an_empty_report_is_not_fully_native: findings.an_empty_report_is_not_fully_native,
        an_empty_report_has_no_fallbacks: findings.an_empty_report_has_no_fallbacks,
        the_two_predicates_are_not_complementary: findings.the_two_predicates_are_not_complementary,
        fully_native_reads_only_the_counter: findings.fully_native_reads_only_the_counter,
        has_fallbacks_reads_only_the_counter: findings.has_fallbacks_reads_only_the_counter,
        the_counts_are_never_cross_checked: findings.the_counts_are_never_cross_checked,
        the_two_outcome_predicates_are_complementary:
            findings.the_two_outcome_predicates_are_complementary,
        the_fallback_reason_equals_structurally: findings.the_fallback_reason_equals_structurally,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    an_empty_report_is_not_fully_native: bool,
    an_empty_report_has_no_fallbacks: bool,
    the_two_predicates_are_not_complementary: bool,
    fully_native_reads_only_the_counter: bool,
    has_fallbacks_reads_only_the_counter: bool,
    the_counts_are_never_cross_checked: bool,
    the_two_outcome_predicates_are_complementary: bool,
    the_fallback_reason_equals_structurally: bool,
}

/// 造一份报告。 / Build one report.
fn report_of(outcomes: Vec<NativeWriteOutcome>, materialized_fallbacks: usize,
    native_writes: usize) -> NativeLoweringReport
{
    NativeLoweringReport { outcomes, materialized_fallbacks, native_writes }
}

/// 一条原生写入结果。 / One native write outcome.
fn native() -> NativeWriteOutcome {
    NativeWriteOutcome::Native(NativeWriteRecord::new("probe", "functions-probe-1"))
}

/// 一条退化结果。 / One fallback outcome.
fn fallback() -> NativeWriteOutcome {
    NativeWriteOutcome::Fallback(FallbackReason::NoWriter)
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 空报告不是全原生。 / An empty report is not fully native.
        let an_empty_report_is_not_fully_native = {
            let empty = report_of(Vec::new(), 0, 0);
            !empty.is_fully_native()
                // 而真有原生写入、没有计数矛盾时它是全原生。 
                // With a real native write and no conflicting counter it is fully native.
                && report_of(vec![native()], 0, 1).is_fully_native()
        };
        // 2) 空报告也没有退化。 / An empty report has no fallbacks either.
        let an_empty_report_has_no_fallbacks = {
            let empty = report_of(Vec::new(), 0, 0);
            !empty.has_fallbacks() && report_of(vec![fallback()], 1, 0).has_fallbacks()
        };
        // 3) 两个判定不互补。 / The two predicates are not complementary.
        let the_two_predicates_are_not_complementary = {
            let empty = report_of(Vec::new(), 0, 0);
            // 空报告两者皆假 → 不满足 `a == !b`。 
            // On an empty report both are false, so `a == !b` fails.
            !empty.is_fully_native() && !empty.has_fallbacks()
                && empty.is_fully_native() != !empty.has_fallbacks()
        };
        // 4) 全原生只读计数。 / "Fully native" reads only the counter.
        let fully_native_reads_only_the_counter = {
            // 结果全是退化，但计数写着 0 个 fallback、1 个原生写入。 
            // Every outcome fell back while the counters claim zero fallbacks and one native write.
            let lying = report_of(vec![fallback(), fallback()], 0, 1);
            lying.is_fully_native() && !lying.has_fallbacks()
        };
        // 5) 有退化也只读计数。 / "Has fallbacks" reads only the counter too.
        let has_fallbacks_reads_only_the_counter = {
            // 结果全是原生，但计数写着 1 个已物化 fallback。 
            // Every outcome is native while the counter claims one materialized fallback.
            let lying = report_of(vec![native(), native()], 1, 0);
            lying.has_fallbacks() && !lying.is_fully_native()
        };
        // 6) 计数从不交叉核对。 / The counts are never cross-checked.
        let the_counts_are_never_cross_checked = {
            // `native_writes + materialized_fallbacks` 与 `outcomes.len()` 完全不符，构造照样成立。 
            // The counters disagree with `outcomes.len()` and the struct still builds.
            let contradictory = report_of(vec![native()], 9, 9);
            contradictory.native_writes + contradictory.materialized_fallbacks
                != contradictory.outcomes.len()
                && contradictory.is_fully_native() == false
        };
        // 7) 每个结果的两个判定互补。 / Each outcome's two predicates are complementary.
        let the_two_outcome_predicates_are_complementary = {
            let outcomes = [native(), fallback(),
                NativeWriteOutcome::Fallback(FallbackReason::Rejected("x".to_owned())),
                NativeWriteOutcome::Fallback(FallbackReason::WriterFailed("x".to_owned()))];
            outcomes.iter().all(|outcome| outcome.is_native() != outcome.requires_fallback())
                && native().is_native() && fallback().requires_fallback()
        };
        // 8) FallbackReason 的结构相等。 / `FallbackReason` equals structurally.
        let the_fallback_reason_equals_structurally = {
            let rejected = FallbackReason::Rejected("x".to_owned());
            let failed = FallbackReason::WriterFailed("x".to_owned());
            rejected == FallbackReason::Rejected("x".to_owned())
                // 文本相同、变体不同 → 不相等。 / Same text, different variant: not equal.
                && rejected != failed
                && failed != FallbackReason::NoWriter
                && FallbackReason::NoWriter == FallbackReason::NoWriter
        };
        Findings {
            an_empty_report_is_not_fully_native,
            an_empty_report_has_no_fallbacks,
            the_two_predicates_are_not_complementary,
            fully_native_reads_only_the_counter,
            has_fallbacks_reads_only_the_counter,
            the_counts_are_never_cross_checked,
            the_two_outcome_predicates_are_complementary,
            the_fallback_reason_equals_structurally,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: NativeLoweringProbeRequest) -> NativeLoweringProbeReport {
        verify_native_lowering_probe(&request).unwrap()
    }

    fn spec(kind: &str, text: Option<&str>) -> NativeOutcomeSpec {
        NativeOutcomeSpec { kind: kind.to_owned(), text: text.map(str::to_owned) }
    }

    fn request() -> NativeLoweringProbeRequest {
        NativeLoweringProbeRequest {
            outcomes: vec![spec("Native", Some("probe")), spec("NoWriter", None)],
            materialized_fallbacks: None, native_writes: None }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request());
        // 一条原生一条退化：既不全原生也不算自相矛盾。 
        // One native and one fallback: neither fully native nor contradictory.
        assert!(!report.fully_native);
        assert!(report.has_fallbacks);
        assert_eq!(report.outcome_count, 2);
        assert_eq!(report.native_outcomes, 1);
        assert_eq!(report.fallback_outcomes, 1);
        assert!(report.counts_agree);
        assert!(report.an_empty_report_is_not_fully_native);
        assert!(report.an_empty_report_has_no_fallbacks);
        assert!(report.the_two_predicates_are_not_complementary);
        assert!(report.fully_native_reads_only_the_counter);
        assert!(report.has_fallbacks_reads_only_the_counter);
        assert!(report.the_counts_are_never_cross_checked);
        assert!(report.the_two_outcome_predicates_are_complementary);
        assert!(report.the_fallback_reason_equals_structurally);
    }

    #[test]
    fn an_empty_report_says_nothing_either_way() {
        let report = ask(NativeLoweringProbeRequest { outcomes: Vec::new(),
            materialized_fallbacks: None, native_writes: None });
        assert!(!report.fully_native);
        assert!(!report.has_fallbacks);
        assert_eq!(report.outcome_count, 0);
        assert!(report.counts_agree);
    }

    #[test]
    fn the_counters_alone_decide_the_two_predicates() {
        // 结果全退化，但计数说"0 个 fallback、1 个原生"。 
        // Every outcome fell back while the counters claim zero fallbacks and one native write.
        let lying = ask(NativeLoweringProbeRequest {
            outcomes: vec![spec("NoWriter", None), spec("Rejected", Some("x"))],
            materialized_fallbacks: Some(0), native_writes: Some(1) });
        assert!(lying.fully_native);
        assert!(!lying.has_fallbacks);
        // 自相矛盾的计数不会被拒。 / Contradictory counters are not refused.
        assert!(!lying.counts_agree);
        assert_eq!(lying.fallback_outcomes, 2);
    }

    #[test]
    fn everything_native_is_fully_native() {
        let report = ask(NativeLoweringProbeRequest {
            outcomes: vec![spec("Native", Some("a")), spec("Native", Some("b"))],
            materialized_fallbacks: None, native_writes: None });
        assert!(report.fully_native);
        assert!(!report.has_fallbacks);
        assert_eq!(report.native_outcomes, 2);
        assert!(report.counts_agree);
    }

    #[test]
    fn the_two_fallback_texts_are_distinct_variants() {
        // 文本相同、变体不同：两个结果不相等，但都要求 fallback。 
        // Same text, different variants: unequal outcomes that both demand the fallback.
        let rejected = outcome_of(&spec("Rejected", Some("x"))).unwrap();
        let failed = outcome_of(&spec("WriterFailed", Some("x"))).unwrap();
        assert_ne!(rejected, failed);
        assert_eq!(rejected, outcome_of(&spec("Rejected", Some("x"))).unwrap());
        assert!(rejected.requires_fallback() && failed.requires_fallback());
        assert!(!rejected.is_native() && !failed.is_native());
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_native_lowering_probe(&NativeLoweringProbeRequest {
            outcomes: vec![spec("Nearly", None)], materialized_fallbacks: None,
            native_writes: None }).unwrap_err().message.contains("kind reads"));
        assert!(verify_native_lowering_probe(&NativeLoweringProbeRequest {
            outcomes: (0..65).map(|_| spec("NoWriter", None)).collect(),
            materialized_fallbacks: None, native_writes: None }).unwrap_err()
            .message.contains("at most sixty-four"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"fullyNative\""), "{json}");
        assert!(json.contains("\"countsAgree\""), "{json}");
        assert!(json.contains("\"theCountsAreNeverCrossChecked\""), "{json}");
        assert!(!json.contains("fully_native"), "{json}");
        // 请求侧同样是 camelCase。 / The request side is camelCase too.
        let request_json = serde_json::to_string(&request()).unwrap();
        assert!(request_json.contains("\"materializedFallbacks\""), "{request_json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<NativeLoweringProbeReport>(&copy.to_string()).is_err());
    }
}
