//! 续挖第 54 单：原生写入器注册表的**调度契约**——先注册先决定、跳过不等于回退、条数不符立即失败
//! （`model/intermediate/native_writer.rs` 的 `NativeFunctionWriterRegistry`）
//! Continuation, delivery 54: the native writer registry's **scheduling contract** — first registered decides first, a skip is
//! not a fallback, and a length mismatch fails immediately (`NativeFunctionWriterRegistry`).
//!
//! 第 53 单量的是"写完之后的记录与报告"，这一单量**调度过程本身**。注册表文档里写了六条契约，逐条实测：
//!
//! 1. **先注册的 writer 先决定**（`registration_order_decides()`）：两个 writer 都声明支持同一结构时，
//!    结果属于**先注册**那个——调度是**顺序优先**，不是"最合适优先"。
//! 2. **被前面决定的结构不再进入后续批次**（`decided_structures_leave_the_batch()`）：后注册的 writer
//!    **一次都没被调用**（`write_batch` 计数为 0）。
//! 3. **没有可处理结构的 writer 被整体跳过**（`an_empty_batch_skips_the_writer()`）：不调用它的 `write_batch`。
//! 4. **返回条目数与批次大小不符立即失败**（`a_length_mismatch_fails_immediately()`）：错误消息同时带
//!    writer 名与两个数字，避免静默丢结构。
//! 5. **任一 writer 返回错误就整体失败**（`a_writer_error_propagates()`）：不会把已写入的部分留下。
//! 6. **`Ok(None)` 不是回退**（`a_skip_is_not_a_fallback()`）：主动跳过整批会把结构**交给后续 writer**；
//!    只有遍历结束仍没人认领，才按 `NoWriter` 走进通用 fallback。
//! 7. **`writer_names()` 按注册序且不去重**（`names_are_in_order_and_not_deduped()`）：同名 writer 注册两次
//!    就有两个名字；空注册表则每个结构都得到 `NoWriter`。
//!
//! Delivery 53 measured the records and the report *after* writing; this one measures **the scheduling itself**. The registry
//! documents six rules, each measured here. **The first registered writer decides first** (`registration_order_decides()`): when
//! two writers both claim a structure the result belongs to the **earlier** one — scheduling is **order-first**, not
//! best-match-first. **Structures already decided leave the batch** (`decided_structures_leave_the_batch()`): the later writer is
//! **never called** (its `write_batch` count is 0). **A writer with nothing to handle is skipped entirely**
//! (`an_empty_batch_skips_the_writer()`). **An outcome count that differs from the batch size fails immediately**
//! (`a_length_mismatch_fails_immediately()`), with the writer name and both numbers in the message so no structure is dropped
//! silently. **Any writer error fails the whole call** (`a_writer_error_propagates()`), leaving no half-written state.
//! **`Ok(None)` is not a fallback** (`a_skip_is_not_a_fallback()`): skipping a batch hands it to **later writers**, and only a
//! structure nobody claims ends as `NoWriter`. And **`writer_names()` is registration order without dedupe**
//! (`names_are_in_order_and_not_deduped()`).

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use ospf_rust_core::error::Result as CoreResult;
use ospf_rust_core::model::LinearConstraint;
use ospf_rust_core::model::intermediate::deferred_function::DeferredFunctionStructure;
use ospf_rust_core::model::intermediate::function_usage::FunctionUsageSummary;
use ospf_rust_core::model::intermediate::native_writer::{
    FallbackReason, NativeFunctionWriter, NativeFunctionWriterRegistry, NativeWriteOutcome,
    NativeWriteRecord, NativeWriteRequest,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个 writer 的字面量。 / One writer literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriterSpec {
    pub name: String,
    /// 它声明支持的函数名。 / The function names it claims to support.
    #[serde(default)]
    pub claims: Vec<String>,
    /// `native` / `skip` / `fail` / `short` / `long`。 / The write behaviour.
    #[serde(default)]
    pub mode: Option<String>,
}

/// 注册表请求。 / The registry request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriterRegistryRequest {
    #[serde(default)]
    pub writers: Vec<WriterSpec>,
    /// 待展开批次的函数名。 / The function names of the pending batch.
    #[serde(default)]
    pub structures: Vec<String>,
}

/// 一个结构的最终去向。 / One structure's final destination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DestinationRow {
    /// 函数名。 / The function name.
    pub function: String,
    /// `native` 或 `fallback`。 / `native` or `fallback`.
    pub kind: String,
    /// 原生写入时的 writer 名。 / The writer name for a native write.
    #[serde(default)]
    pub writer: Option<String>,
    /// fallback 原因名。 / The fallback reason name.
    #[serde(default)]
    pub reason: Option<String>,
}

/// 注册表结论。 / The registry verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriterRegistryReport {
    /// `writer_names()`。 / `writer_names()`.
    pub names: Vec<String>,
    /// 每个 writer 的 `write_batch` 调用次数。 / Each writer's `write_batch` call count.
    pub calls: Vec<u64>,
    /// 每个结构的最终去向。 / Each structure's destination.
    pub destinations: Vec<DestinationRow>,
    /// 是否失败。 / Whether the call failed.
    pub failed: bool,
    /// 失败消息。 / The failure message.
    #[serde(default)]
    pub error: Option<String>,
    /// 注册表是否为空。 / Whether the registry is empty.
    pub empty: bool,
    /// 先注册先决定（**是**）。 / Whether registration order decides (**yes**).
    pub registration_order_decides: bool,
    /// 已决定的结构离开批次（**是**）。 / Whether decided structures leave the batch (**yes**).
    pub decided_structures_leave_the_batch: bool,
    /// 空批次跳过 writer（**是**）。 / Whether an empty batch skips the writer (**yes**).
    pub an_empty_batch_skips_the_writer: bool,
    /// 条数不符立即失败（**是**）。 / Whether a length mismatch fails immediately (**yes**).
    pub a_length_mismatch_fails_immediately: bool,
    /// writer 错误向外传播（**是**）。 / Whether a writer error propagates (**yes**).
    pub a_writer_error_propagates: bool,
    /// 跳过不是回退（**是**）。 / Whether a skip is not a fallback (**yes**).
    pub a_skip_is_not_a_fallback: bool,
    /// 名字按注册序且不去重（**是**）。 / Whether names keep registration order without dedupe (**yes**).
    pub names_are_in_order_and_not_deduped: bool,
}

/// 探针结构：只有函数名参与调度。 / A probe structure: only the function name matters for scheduling.
#[derive(Debug)]
struct ProbeStructure {
    name: String,
}

impl DeferredFunctionStructure<f64> for ProbeStructure {
    fn function_name(&self) -> &str {
        &self.name
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
        Some(format!("fingerprint:{}", self.name))
    }
}

/// 一个可控的探针 writer。 / One controllable probe writer.
struct ProbeWriter {
    name: String,
    claims: Vec<String>,
    mode: String,
    calls: Arc<Mutex<u64>>,
}

impl Debug for ProbeWriter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ProbeWriter").field("name", &self.name).finish()
    }
}

impl NativeFunctionWriter<(), f64> for ProbeWriter {
    fn name(&self) -> &str {
        &self.name
    }

    fn supports(&self, structure: &dyn DeferredFunctionStructure<f64>) -> bool {
        self.claims.iter().any(|claim| claim == structure.function_name())
    }

    fn write_batch(
        &self,
        _container: &mut (),
        requests: &[NativeWriteRequest<'_, f64>],
    ) -> CoreResult<Option<Vec<NativeWriteOutcome>>> {
        if let Ok(mut calls) = self.calls.lock() {
            *calls += 1;
        }
        let native = |request: &NativeWriteRequest<'_, f64>| NativeWriteOutcome::Native(
            NativeWriteRecord::new(self.name.clone(), "probe-1")
                .with_fingerprint(Some(format!("fingerprint:{}", request.structure.function_name())))
                .with_function(request.structure.function_name().to_owned()));
        Ok(match self.mode.as_str() {
            "skip" => None,
            "fail" => return Err(ospf_rust_core::error::ModelError::InvalidConstraint(
                format!("writer `{}` failed", self.name)).into()),
            "short" => Some(requests.iter().take(requests.len().saturating_sub(1))
                .map(native).collect()),
            "long" => Some(requests.iter().map(native)
                .chain(std::iter::once(native(&requests[0]))).collect()),
            _ => Some(requests.iter().map(native).collect()),
        })
    }
}

/// 造一批请求。 / Build one batch of requests.
fn batch<'a>(structures: &'a [ProbeStructure]) -> Vec<NativeWriteRequest<'a, f64>> {
    structures.iter().enumerate().map(|(index, structure)| NativeWriteRequest {
        index, structure, usage: FunctionUsageSummary::default() }).collect()
}

/// 结论名。 / The destination name.
fn destination_of(function: &str, outcome: &NativeWriteOutcome) -> DestinationRow {
    match outcome {
        NativeWriteOutcome::Native(record) => DestinationRow { function: function.to_owned(),
            kind: "native".to_owned(), writer: Some(record.writer.clone()), reason: None },
        NativeWriteOutcome::Fallback(reason) => DestinationRow { function: function.to_owned(),
            kind: "fallback".to_owned(), writer: None,
            reason: Some(match reason {
                FallbackReason::NoWriter => "no-writer".to_owned(),
                FallbackReason::Rejected(_) => "rejected".to_owned(),
                FallbackReason::WriterFailed(_) => "writer-failed".to_owned(),
            }) },
    }
}

/// 走一遍注册表调度。非法输入是**错误**。 / Walk the registry scheduling.
pub fn verify_writer_registry(request: &WriterRegistryRequest) -> Result<WriterRegistryReport> {
    const MAX_WRITERS: usize = 16;
    const MAX_STRUCTURES: usize = 32;
    if request.writers.len() > MAX_WRITERS || request.structures.len() > MAX_STRUCTURES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "writer registry failed validation: at most {MAX_WRITERS} writers and \
             {MAX_STRUCTURES} structures are accepted")));
    }
    let structures = request.structures.iter()
        .map(|name| ProbeStructure { name: name.clone() }).collect::<Vec<_>>();
    let requests = batch(&structures);
    let mut registry = NativeFunctionWriterRegistry::<(), f64>::new();
    let mut counters = Vec::new();
    for spec in &request.writers {
        let mode = spec.mode.clone().unwrap_or_else(|| "native".to_owned());
        if !matches!(mode.as_str(), "native" | "skip" | "fail" | "short" | "long") {
            return Err(BridgeError::new("InvalidInput", &format!(
                "writer registry failed validation: unknown writer mode `{mode}`")));
        }
        let counter = Arc::new(Mutex::new(0u64));
        counters.push(Arc::clone(&counter));
        registry.register(Box::new(ProbeWriter { name: spec.name.clone(),
            claims: spec.claims.clone(), mode, calls: counter }));
    }
    let names = registry.writer_names().iter().map(|name| (*name).to_owned()).collect();
    let empty = registry.is_empty();
    let mut container = ();
    let (failed, error, destinations) = match registry.write_batch(&mut container, &requests) {
        Ok(outcomes) => (false, None, structures.iter().zip(outcomes.iter())
            .map(|(structure, outcome)| destination_of(&structure.name, outcome))
            .collect::<Vec<_>>()),
        Err(failure) => (true, Some(failure.to_string()), Vec::new()),
    };
    let calls = counters.iter().map(|counter| counter.lock().map(|value| *value)
        .unwrap_or(u64::MAX)).collect::<Vec<_>>();
    let findings = canonical_findings();
    Ok(WriterRegistryReport { names, calls, destinations, failed, error, empty,
        registration_order_decides: findings.registration_order_decides,
        decided_structures_leave_the_batch: findings.decided_structures_leave_the_batch,
        an_empty_batch_skips_the_writer: findings.an_empty_batch_skips_the_writer,
        a_length_mismatch_fails_immediately: findings.a_length_mismatch_fails_immediately,
        a_writer_error_propagates: findings.a_writer_error_propagates,
        a_skip_is_not_a_fallback: findings.a_skip_is_not_a_fallback,
        names_are_in_order_and_not_deduped: findings.names_are_in_order_and_not_deduped })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    registration_order_decides: bool,
    decided_structures_leave_the_batch: bool,
    an_empty_batch_skips_the_writer: bool,
    a_length_mismatch_fails_immediately: bool,
    a_writer_error_propagates: bool,
    a_skip_is_not_a_fallback: bool,
    names_are_in_order_and_not_deduped: bool,
}

/// 跑一次探针调度，返回 `(结果名列表, 每个 writer 的调用次数, 错误消息)`。
/// Run one probe schedule, returning `(destination names, per-writer call counts, error message)`.
fn probe_run(writers: &[(&str, &[&str], &str)], structures: &[&str])
    -> (Vec<String>, Vec<u64>, Option<String>)
{
    let probe_structures = structures.iter()
        .map(|name| ProbeStructure { name: (*name).to_owned() }).collect::<Vec<_>>();
    let requests = batch(&probe_structures);
    let mut registry = NativeFunctionWriterRegistry::<(), f64>::new();
    let mut counters = Vec::new();
    for (name, claims, mode) in writers {
        let counter = Arc::new(Mutex::new(0u64));
        counters.push(Arc::clone(&counter));
        registry.register(Box::new(ProbeWriter { name: (*name).to_owned(),
            claims: claims.iter().map(|claim| (*claim).to_owned()).collect(),
            mode: (*mode).to_owned(), calls: counter }));
    }
    let mut container = ();
    let (destinations, error) = match registry.write_batch(&mut container, &requests) {
        Ok(outcomes) => (probe_structures.iter().zip(outcomes.iter())
            .map(|(structure, outcome)| {
                let row = destination_of(&structure.name, outcome);
                match row.writer {
                    Some(writer) => format!("native:{writer}"),
                    None => format!("fallback:{}", row.reason.unwrap_or_default()),
                }
            }).collect::<Vec<_>>(), None),
        Err(failure) => (Vec::new(), Some(failure.to_string())),
    };
    let calls = counters.iter().map(|counter| counter.lock().map(|value| *value)
        .unwrap_or(u64::MAX)).collect::<Vec<_>>();
    (destinations, calls, error)
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 先注册先决定。 / The first registered decides first.
    let registration_order_decides = {
        let (destinations, _, error) = probe_run(
            &[("first", &["abs"], "native"), ("second", &["abs"], "native")], &["abs"]);
        error.is_none() && destinations == vec!["native:first".to_owned()]
    };
    // 2) 已决定的结构离开批次：后注册者一次都没被调用。 
    // 2) Decided structures leave the batch: the later writer is never called.
    let decided_structures_leave_the_batch = {
        let (_, calls, _) = probe_run(
            &[("first", &["abs"], "native"), ("second", &["abs"], "native")], &["abs"]);
        calls == vec![1, 0]
    };
    // 3) 没有可处理结构的 writer 被跳过。 / A writer with nothing to handle is skipped.
    let an_empty_batch_skips_the_writer = {
        let (destinations, calls, _) = probe_run(
            &[("idle", &["exp"], "native"), ("busy", &["abs"], "native")], &["abs"]);
        calls == vec![0, 1] && destinations == vec!["native:busy".to_owned()]
    };
    // 4) 条数不符立即失败，消息带 writer 名与两个数字。 
    // 4) A length mismatch fails immediately with the writer name and both numbers.
    let a_length_mismatch_fails_immediately = {
        let (_, _, short) = probe_run(&[("shorty", &["abs", "exp"], "short")], &["abs", "exp"]);
        let (_, _, long) = probe_run(&[("longy", &["abs"], "long")], &["abs"]);
        short.as_deref().is_some_and(|message| message.contains("shorty")
                && message.contains("batch sizes must match"))
            && long.is_some()
    };
    // 5) writer 错误向外传播。 / A writer error propagates.
    let a_writer_error_propagates = {
        let (destinations, _, error) = probe_run(&[("boom", &["abs"], "fail")], &["abs"]);
        destinations.is_empty() && error.as_deref().is_some_and(|message| message.contains("boom"))
    };
    // 6) 跳过不是回退：交给后续 writer。 / A skip is not a fallback: it goes to a later writer.
    let a_skip_is_not_a_fallback = {
        let (destinations, calls, error) = probe_run(
            &[("skipper", &["abs"], "skip"), ("taker", &["abs"], "native")], &["abs"]);
        error.is_none() && destinations == vec!["native:taker".to_owned()]
            && calls == vec![1, 1]
            // 没人认领时才是 `no-writer`。 / Only an unclaimed structure ends as `no-writer`.
            && probe_run(&[("skipper", &["abs"], "skip")], &["abs"]).0
                == vec!["fallback:no-writer".to_owned()]
    };
    // 7) 名字按注册序且不去重；空注册表全 `no-writer`。 
    // 7) Names keep registration order without dedupe; an empty registry yields `no-writer`.
    let names_are_in_order_and_not_deduped = {
        let mut registry = NativeFunctionWriterRegistry::<(), f64>::new();
        for name in ["alpha", "beta", "alpha"] {
            registry.register(Box::new(ProbeWriter { name: name.to_owned(), claims: Vec::new(),
                mode: "native".to_owned(), calls: Arc::new(Mutex::new(0)) }));
        }
        registry.writer_names() == vec!["alpha", "beta", "alpha"]
            && NativeFunctionWriterRegistry::<(), f64>::new().is_empty()
            && probe_run(&[], &["abs"]).0 == vec!["fallback:no-writer".to_owned()]
    };
    Findings { registration_order_decides, decided_structures_leave_the_batch,
        an_empty_batch_skips_the_writer, a_length_mismatch_fails_immediately,
        a_writer_error_propagates, a_skip_is_not_a_fallback, names_are_in_order_and_not_deduped }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn writer(name: &str, claims: &[&str], mode: &str) -> WriterSpec {
        WriterSpec { name: name.to_owned(),
            claims: claims.iter().map(|claim| (*claim).to_owned()).collect(),
            mode: Some(mode.to_owned()) }
    }

    fn request() -> WriterRegistryRequest {
        WriterRegistryRequest { writers: vec![writer("abs-writer", &["abs"], "native"),
            writer("exp-writer", &["exp"], "native")],
            structures: vec!["abs".to_owned(), "exp".to_owned()] }
    }

    #[test]
    fn each_structure_goes_to_its_writer() {
        let report = verify_writer_registry(&request()).unwrap();
        assert!(!report.failed);
        assert_eq!(report.names, vec!["abs-writer".to_owned(), "exp-writer".to_owned()]);
        assert_eq!(report.calls, vec![1, 1]);
        assert_eq!(report.destinations[0].kind, "native");
        assert_eq!(report.destinations[0].writer.as_deref(), Some("abs-writer"));
        assert_eq!(report.destinations[1].writer.as_deref(), Some("exp-writer"));
        assert!(!report.empty);
        assert!(report.registration_order_decides);
        assert!(report.decided_structures_leave_the_batch);
        assert!(report.an_empty_batch_skips_the_writer);
        assert!(report.a_length_mismatch_fails_immediately);
        assert!(report.a_writer_error_propagates);
        assert!(report.a_skip_is_not_a_fallback);
        assert!(report.names_are_in_order_and_not_deduped);
    }

    #[test]
    fn the_first_writer_wins_and_the_second_is_never_called() {
        let report = verify_writer_registry(&WriterRegistryRequest {
            writers: vec![writer("first", &["abs"], "native"),
                writer("second", &["abs"], "native")],
            structures: vec!["abs".to_owned()] }).unwrap();
        assert_eq!(report.destinations[0].writer.as_deref(), Some("first"));
        assert_eq!(report.calls, vec![1, 0]);
    }

    #[test]
    fn a_skip_hands_the_batch_to_the_next_writer() {
        let report = verify_writer_registry(&WriterRegistryRequest {
            writers: vec![writer("skipper", &["abs"], "skip"),
                writer("taker", &["abs"], "native")],
            structures: vec!["abs".to_owned()] }).unwrap();
        assert_eq!(report.destinations[0].writer.as_deref(), Some("taker"));
        assert_eq!(report.calls, vec![1, 1]);
        // 没人认领时是 `no-writer`。 / An unclaimed structure becomes `no-writer`.
        let report = verify_writer_registry(&WriterRegistryRequest {
            writers: vec![writer("skipper", &["abs"], "skip")],
            structures: vec!["abs".to_owned()] }).unwrap();
        assert_eq!(report.destinations[0].kind, "fallback");
        assert_eq!(report.destinations[0].reason.as_deref(), Some("no-writer"));
    }

    #[test]
    fn a_length_mismatch_and_a_writer_error_fail_the_call() {
        let short = verify_writer_registry(&WriterRegistryRequest {
            writers: vec![writer("shorty", &["abs", "exp"], "short")],
            structures: vec!["abs".to_owned(), "exp".to_owned()] }).unwrap();
        assert!(short.failed);
        let message = short.error.as_deref().unwrap_or_default();
        assert!(message.contains("shorty"), "{message}");
        assert!(message.contains("batch sizes must match"), "{message}");
        assert!(short.destinations.is_empty());
        let boom = verify_writer_registry(&WriterRegistryRequest {
            writers: vec![writer("boom", &["abs"], "fail")],
            structures: vec!["abs".to_owned()] }).unwrap();
        assert!(boom.failed);
        assert!(boom.error.as_deref().unwrap_or_default().contains("boom"));
        assert_eq!(boom.calls, vec![1]);
    }

    #[test]
    fn an_empty_registry_falls_back_everything() {
        let report = verify_writer_registry(&WriterRegistryRequest { writers: Vec::new(),
            structures: vec!["abs".to_owned(), "exp".to_owned()] }).unwrap();
        assert!(report.empty);
        assert!(report.names.is_empty());
        assert_eq!(report.destinations.len(), 2);
        assert!(report.destinations.iter().all(|row| row.kind == "fallback"
            && row.reason.as_deref() == Some("no-writer")));
    }

    #[test]
    fn names_keep_order_and_repeat() {
        let report = verify_writer_registry(&WriterRegistryRequest {
            writers: vec![writer("alpha", &[], "native"), writer("beta", &[], "native"),
                writer("alpha", &[], "native")],
            structures: Vec::new() }).unwrap();
        assert_eq!(report.names, vec!["alpha".to_owned(), "beta".to_owned(),
            "alpha".to_owned()]);
        // 没有结构时没人被调用。 / With no structures nobody is called.
        assert_eq!(report.calls, vec![0, 0, 0]);
        assert!(report.destinations.is_empty());
    }

    #[test]
    fn bad_input_is_refused() {
        let unknown = WriterRegistryRequest { writers: vec![writer("w", &["abs"], "cache")],
            structures: vec!["abs".to_owned()] };
        assert!(verify_writer_registry(&unknown).unwrap_err().message
            .contains("unknown writer mode"));
        let too_many = WriterRegistryRequest {
            writers: (0..17).map(|index| writer(&format!("w{index}"), &[], "native"))
                .collect(), structures: Vec::new() };
        assert!(verify_writer_registry(&too_many).unwrap_err().message
            .contains("at most 16 writers"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_writer_registry(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"destinations\""), "{json}");
        assert!(json.contains("\"registrationOrderDecides\""), "{json}");
        assert!(json.contains("\"aSkipIsNotAFallback\""), "{json}");
        assert!(!json.contains("registration_order"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<WriterRegistryReport>(&copy.to_string()).is_err());
    }
}
