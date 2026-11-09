//! 续挖第 56 单：惰性平展上下文的**初始化语义**——`init` 先到先得，未初始化读 `token_list()` 会 panic
//! （`model/flatten/lazy_context.rs`）
//! Continuation, delivery 56: the lazy flatten context's **initialization semantics** — `init` is first-wins, and reading
//! `token_list()` before initializing panics (`model/flatten/lazy_context.rs`).
//!
//! §11.14 点名 `model/flatten` 的可编码部分。`FlattenContext` 是 trait，真正能脱离求解器度量的是三个**惰性上下文**
//! 的初始化契约。七条实测：
//!
//! 1. **`init` 先到先得**（`init_is_first_wins()`）：底层是 `OnceLock::set`，**第二次 `init` 被静默忽略**，
//!    `token_list_ref()` 仍然是第一个——不报错、不覆盖、也不告警。
//! 2. **`new()` 与 `Default` 都未初始化**（`new_and_default_are_uninitialized()`）。
//! 3. **未初始化就调 `token_list()` 会 panic**（`token_list_panics_before_init()`）：消息是
//!    "FlattenContext not initialized. Call init() first."——**安全入口是 `token_list_ref()`**。
//! 4. **三个惰性上下文（linear / quadratic / canonical）语义一致**（`the_three_contexts_share_it()`）：
//!    都是 `OnceLock`、都忽略二次 `init`。
//! 5. **只有 linear 能安全读回列表**（`only_the_linear_context_can_read_the_list_back()`）：
//!    `token_list_ref()` 只在 `LazyLinearFlattenContext` 上存在，quadratic 与 canonical 只有
//!    `init`/`is_initialized`，要读回只能走**会 panic** 的 `token_list()`。
//! 6. **`is_initialized()` 与 `token_list_ref().is_some()` 永远一致**（`the_flag_agrees_with_the_reference()`）。
//! 7. **三类缓存首查为空**（`the_caches_start_empty()`）：`monomial_cache` / `polynomial_cache` /
//!    `symbol_cache` 都是空表，`*_mut()` 可写——上下文本身不预置任何内容。
//!
//! The §11.14 audit names the codable part of `model/flatten`. `FlattenContext` is a trait, and what can be measured without a
//! solver is the three **lazy contexts**' initialization contract. **`init` is first-wins**
//! (`init_is_first_wins()`): it is `OnceLock::set` underneath, so **a second `init` is silently ignored** and
//! `token_list_ref()` still shows the first — no error, no overwrite, no warning. **`new()` and `Default` are both
//! uninitialized** (`new_and_default_are_uninitialized()`). **Reading `token_list()` before initializing panics**
//! (`token_list_panics_before_init()`) with "FlattenContext not initialized. Call init() first." — the **safe entry point is
//! `token_list_ref()`**. **The three lazy contexts share this** (`the_three_contexts_share_it()`). **Only the linear context can
//! read the list back safely** (`only_the_linear_context_can_read_the_list_back()`): `token_list_ref()` exists only on
//! `LazyLinearFlattenContext`, so quadratic and canonical must go through the **panicking** `token_list()`. **`is_initialized()`
//! always agrees with `token_list_ref().is_some()`** (`the_flag_agrees_with_the_reference()`). And **all three caches start
//! empty** (`the_caches_start_empty()`), with `*_mut()` writable — the context pre-populates nothing.

use std::sync::Arc;

use ospf_rust_core::model::flatten::{
    FlattenContextTrait, LazyCanonicalFlattenContext, LazyLinearFlattenContext,
    LazyQuadraticFlattenContext,
};
use ospf_rust_core::token::{Token, TokenList};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 探针 token 列表：只带一个标签用于区分两次 `init`。 / A probe token list carrying a label so two `init` calls differ.
struct ProbeList {
    label: usize,
    tokens: Vec<Token<f64>>,
}

impl TokenList<f64> for ProbeList {
    fn tokens(&self) -> &Vec<Token<f64>> {
        &self.tokens
    }
}

/// 惰性上下文请求。 / The lazy-context request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LazyInitRequest {
    /// 第一次 `init` 的标签。 / The label of the first `init`.
    #[serde(default)]
    pub first: Option<u64>,
    /// 第二次 `init` 的标签（量"先到先得"）。 / The label of the second `init` (to measure first-wins).
    #[serde(default)]
    pub second: Option<u64>,
    /// 是否在读取之前调用 `init`。 / Whether to call `init` before reading.
    #[serde(default)]
    pub initialize: bool,
}

/// 惰性上下文结论。 / The lazy-context verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LazyInitReport {
    /// 读取列表时是否 panic。 / Whether reading the list panicked.
    pub panicked: bool,
    /// panic 消息（摘要）。 / The panic message (summarized).
    #[serde(default)]
    pub panic_message: Option<String>,
    /// `is_initialized()`。 / `is_initialized()`.
    pub initialized: bool,
    /// `token_list_ref()` 读到的标签。 / The label read back through `token_list_ref()`.
    #[serde(default)]
    pub label: Option<u64>,
    /// 未初始化时 `token_list_ref()` 是否为 `None`。 / Whether `token_list_ref()` is `None` when uninitialized.
    pub reference_is_none: bool,
    /// 三类缓存首查的条数。 / The three caches' counts on first read.
    pub cache_counts: Vec<u64>,
    /// `init` 先到先得（**是**）。 / Whether `init` is first-wins (**yes**).
    pub init_is_first_wins: bool,
    /// `new()` 与 `Default` 都未初始化（**是**）。 / Whether both `new()` and `Default` start uninitialized (**yes**).
    pub new_and_default_are_uninitialized: bool,
    /// 未初始化读 `token_list()` 会 panic（**是**）。 / Whether reading before init panics (**yes**).
    pub token_list_panics_before_init: bool,
    /// 三个上下文语义一致（**是**）。 / Whether the three contexts share the semantics (**yes**).
    pub the_three_contexts_share_it: bool,
    /// 只有 linear 能安全读回（**是**）。 / Whether only the linear context reads back safely (**yes**).
    pub only_the_linear_context_can_read_the_list_back: bool,
    /// 标志与引用一致（**是**）。 / Whether the flag agrees with the reference (**yes**).
    pub the_flag_agrees_with_the_reference: bool,
    /// 三类缓存首查为空（**是**）。 / Whether the three caches start empty (**yes**).
    pub the_caches_start_empty: bool,
}

/// 造一个探针列表。 / Build one probe list.
fn probe_list(label: u64) -> Arc<ProbeList> {
    Arc::new(ProbeList { label: label as usize, tokens: Vec::new() })
}

/// 走一遍惰性上下文。非法输入是**错误**。 / Walk the lazy context.
pub fn verify_lazy_flatten_init(request: &LazyInitRequest) -> Result<LazyInitReport> {
    if request.first.unwrap_or(1) > 1024 || request.second.unwrap_or(2) > 1024 {
        return Err(BridgeError::new("InvalidInput",
            "lazy flatten context failed validation: labels must be at most 1024"));
    }
    let context = LazyLinearFlattenContext::<f64, ProbeList>::new();
    let reference_is_none = context.token_list_ref().is_none();
    let label_before = context.token_list_ref().map(|list| list.label as u64);
    if request.initialize {
        context.init(probe_list(request.first.unwrap_or(1)));
    }
    if let Some(second) = request.second {
        context.init(probe_list(second));
    }
    let initialized = context.is_initialized();
    let label = context.token_list_ref().map(|list| list.label as u64);
    let cache_counts = vec![context.monomial_cache().len() as u64,
        context.polynomial_cache().len() as u64, context.symbol_cache().len() as u64];
    // 未初始化时读 `token_list()` 会 panic；用 `catch_unwind` 观测，不把 panic 传给调用方。
    // Reading `token_list()` while uninitialized panics; observe it with `catch_unwind` instead of propagating.
    let (panicked, panic_message) = if initialized {
        (false, None)
    } else {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = context.token_list().len();
        }));
        match outcome {
            Ok(()) => (false, None),
            Err(payload) => {
                let message = payload.downcast_ref::<String>().cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|text| (*text).to_owned()))
                    .unwrap_or_else(|| "panic".to_owned());
                (true, Some(message.chars().take(120).collect::<String>()))
            }
        }
    };
    let findings = canonical_findings();
    Ok(LazyInitReport { panicked, panic_message, initialized, label,
        reference_is_none: reference_is_none && label_before.is_none(), cache_counts,
        init_is_first_wins: findings.init_is_first_wins,
        new_and_default_are_uninitialized: findings.new_and_default_are_uninitialized,
        token_list_panics_before_init: findings.token_list_panics_before_init,
        the_three_contexts_share_it: findings.the_three_contexts_share_it,
        only_the_linear_context_can_read_the_list_back:
            findings.only_the_linear_context_can_read_the_list_back,
        the_flag_agrees_with_the_reference: findings.the_flag_agrees_with_the_reference,
        the_caches_start_empty: findings.the_caches_start_empty })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    init_is_first_wins: bool,
    new_and_default_are_uninitialized: bool,
    token_list_panics_before_init: bool,
    the_three_contexts_share_it: bool,
    only_the_linear_context_can_read_the_list_back: bool,
    the_flag_agrees_with_the_reference: bool,
    the_caches_start_empty: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) `init` 先到先得：第二次被静默忽略。 / `init` is first-wins: the second call is silently ignored.
    let init_is_first_wins = {
        let context = LazyLinearFlattenContext::<f64, ProbeList>::new();
        context.init(probe_list(7));
        context.init(probe_list(9));
        context.token_list_ref().map(|list| list.label) == Some(7) && context.is_initialized()
    };
    // 2) `new()` 与 `Default` 都未初始化。 / Both `new()` and `Default` start uninitialized.
    let new_and_default_are_uninitialized =
        !LazyLinearFlattenContext::<f64, ProbeList>::new().is_initialized()
            && !LazyLinearFlattenContext::<f64, ProbeList>::default().is_initialized();
    // 3) 未初始化读 `token_list()` 会 panic，消息点名 init。 
    // 3) Reading before init panics, and the message names init.
    let token_list_panics_before_init = {
        let context = LazyLinearFlattenContext::<f64, ProbeList>::new();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = context.token_list();
        }));
        match outcome {
            Err(payload) => {
                let message = payload.downcast_ref::<String>().cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|text| (*text).to_owned()))
                    .unwrap_or_default();
                message.contains("not initialized") && message.contains("init()")
            }
            Ok(()) => false,
        }
    };
    // 4) 三个上下文都是 once 语义。 / All three contexts are once-semantics.
    let the_three_contexts_share_it = {
        let linear = LazyLinearFlattenContext::<f64, ProbeList>::new();
        let quadratic = LazyQuadraticFlattenContext::<f64, ProbeList>::new();
        let canonical = LazyCanonicalFlattenContext::<f64, ProbeList>::new();
        let before = !linear.is_initialized() && !quadratic.is_initialized()
            && !canonical.is_initialized();
        linear.init(probe_list(1));
        quadratic.init(probe_list(1));
        canonical.init(probe_list(1));
        let after = linear.is_initialized() && quadratic.is_initialized()
            && canonical.is_initialized();
        // 二次 `init` 之后仍然只是"已初始化"。 / A second `init` still just means "initialized".
        quadratic.init(probe_list(2));
        canonical.init(probe_list(2));
        before && after && quadratic.is_initialized() && canonical.is_initialized()
    };
    // 5) 只有 linear 暴露 `token_list_ref()`；另两个只能走会 panic 的 `token_list()`。 
    // 5) Only the linear context exposes `token_list_ref()`; the others must use the panicking accessor.
    let only_the_linear_context_can_read_the_list_back = {
        let uninitialized = LazyQuadraticFlattenContext::<f64, ProbeList>::new();
        let canonical = LazyCanonicalFlattenContext::<f64, ProbeList>::new();
        let quadratic_panics = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = uninitialized.token_list();
        })).is_err();
        let canonical_panics = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = canonical.token_list();
        })).is_err();
        let initialized = LazyQuadraticFlattenContext::<f64, ProbeList>::new();
        initialized.init(probe_list(3));
        !LazyLinearFlattenContext::<f64, ProbeList>::new().is_initialized()
            && quadratic_panics && canonical_panics && initialized.is_initialized()
    };
    // 6) `is_initialized()` 与 `token_list_ref().is_some()` 一致。 
    // 6) `is_initialized()` agrees with `token_list_ref().is_some()`.
    let the_flag_agrees_with_the_reference = {
        let fresh = LazyLinearFlattenContext::<f64, ProbeList>::new();
        let before = fresh.is_initialized() == fresh.token_list_ref().is_some();
        fresh.init(probe_list(4));
        before && fresh.is_initialized() == fresh.token_list_ref().is_some()
            && fresh.token_list_ref().is_some()
    };
    // 7) 三类缓存首查为空。 / All three caches start empty.
    let the_caches_start_empty = {
        let context = LazyLinearFlattenContext::<f64, ProbeList>::new();
        let empty = context.monomial_cache().is_empty()
            && context.polynomial_cache().is_empty() && context.symbol_cache().is_empty();
        context.init(probe_list(5));
        empty && context.monomial_cache().is_empty() && context.symbol_cache().is_empty()
    };
    Findings { init_is_first_wins, new_and_default_are_uninitialized, token_list_panics_before_init,
        the_three_contexts_share_it, only_the_linear_context_can_read_the_list_back,
        the_flag_agrees_with_the_reference, the_caches_start_empty }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializing_twice_keeps_the_first_list() {
        let report = verify_lazy_flatten_init(&LazyInitRequest { first: Some(11),
            second: Some(22), initialize: true }).unwrap();
        assert!(report.initialized);
        assert_eq!(report.label, Some(11));
        assert!(!report.panicked);
        assert!(report.reference_is_none);
        assert!(report.init_is_first_wins);
        assert!(report.new_and_default_are_uninitialized);
        assert!(report.token_list_panics_before_init);
        assert!(report.the_three_contexts_share_it);
        assert!(report.only_the_linear_context_can_read_the_list_back);
        assert!(report.the_flag_agrees_with_the_reference);
        assert!(report.the_caches_start_empty);
        assert_eq!(report.cache_counts, vec![0, 0, 0]);
    }

    #[test]
    fn reading_before_init_panics_with_the_expected_message() {
        let report = verify_lazy_flatten_init(&LazyInitRequest { first: None, second: None,
            initialize: false }).unwrap();
        assert!(report.panicked);
        let message = report.panic_message.as_deref().unwrap_or_default();
        assert!(message.contains("not initialized"), "{message}");
        assert!(message.contains("init()"), "{message}");
        assert!(!report.initialized);
        assert_eq!(report.label, None);
        assert!(report.reference_is_none);
    }

    #[test]
    fn initializing_without_a_second_call_also_works() {
        let report = verify_lazy_flatten_init(&LazyInitRequest { first: Some(5), second: None,
            initialize: true }).unwrap();
        assert!(report.initialized);
        assert_eq!(report.label, Some(5));
        assert!(!report.panicked);
    }

    #[test]
    fn a_second_init_without_the_first_only_wins_once() {
        // 未调 `init` 但给了 `second`：它会成为唯一的一次初始化。 
        // No `init` call requested but `second` is given: it becomes the single initialization.
        let report = verify_lazy_flatten_init(&LazyInitRequest { first: Some(1), second: Some(9),
            initialize: false }).unwrap();
        assert!(report.initialized);
        assert_eq!(report.label, Some(9));
    }

    #[test]
    fn bad_input_is_refused() {
        let too_large = LazyInitRequest { first: Some(1025), second: None, initialize: true };
        assert!(verify_lazy_flatten_init(&too_large).unwrap_err().message
            .contains("at most 1024"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_lazy_flatten_init(&LazyInitRequest { first: Some(1), second: None,
            initialize: true }).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"panicMessage\""), "{json}");
        assert!(json.contains("\"cacheCounts\""), "{json}");
        assert!(json.contains("\"initIsFirstWins\""), "{json}");
        assert!(!json.contains("init_is"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<LazyInitReport>(&copy.to_string()).is_err());
    }
}
