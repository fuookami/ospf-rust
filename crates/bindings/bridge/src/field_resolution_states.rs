//! 续挖第 30 单：字段解析的四态词汇与**有损**的兼容桥
//! （`persistence/expression/persistence_field_resolver.rs`）
//! Continuation, delivery 30: the four-state resolution vocabulary and its **lossy** compatibility bridge.
//!
//! 框架在这里给字段解析定义了**四种**结果，并且明确要求"实现者只实现详细接口"：
//!
//! * `Resolved(字段)`——解析成功；
//! * `Missing { path }`——**字段不存在**；
//! * `Ambiguous { path, candidates }`——**字段存在多个候选**；
//! * `InvalidConfiguration { reason }`——**解析器自身配置非法**。
//!
//! 而兼容用的 `DiagnosticPersistenceFieldResolver::resolve_field`（返回 `Option<C>`）把后**三种全部压成 `None`**。
//! 于是本单的结论是两条：
//!
//! 1. **四态 → 二态是有损的**：`Missing` / `Ambiguous` / `InvalidConfiguration` 在可空接口上**完全无法区分**。
//!    调用方若用 `None` 表示"这个字段没有值"，就会把"配置写错了"当成"字段为空"——这正是第 22 单
//!    `index_of` 与 `resolve` 那种"同一个对象两套答案"的另一种表现。
//! 2. **闭包式 resolver 天生只有两态**：`impl<C, F> PersistenceFieldResolver<C> for F where F: Fn(&str) -> Option<C>`
//!    这个 blanket impl 让任何 `Fn(&str) -> Option<C>` 直接成为 resolver，而它**永远不可能**报告 `Ambiguous`
//!    或 `InvalidConfiguration`——最方便的那条路也是信息最少的那条。
//!
//! 另外闭包收到的是 **`path.value()`**（去前缀后的路径本体），不是整条原始字符串。
//!
//! The framework defines **four** resolution outcomes here and asks implementers to provide the detailed one:
//! `Resolved(field)`, `Missing { path }`, `Ambiguous { path, candidates }` and `InvalidConfiguration { reason }`. The
//! nullable compatibility method `DiagnosticPersistenceFieldResolver::resolve_field` collapses the last **three into
//! `None`**, which yields two findings. **Four states collapse lossily into two**: `Missing` / `Ambiguous` /
//! `InvalidConfiguration` are indistinguishable through the nullable interface, so a caller that reads `None` as "this field
//! has no value" mistakes a broken configuration for an empty field — the same kind of two-answers-one-object problem as
//! `index_of` versus `resolve` in delivery 22. And **a closure resolver is two-state by construction**: the blanket impl
//! `impl<C, F> PersistenceFieldResolver<C> for F where F: Fn(&str) -> Option<C>` turns any `Fn(&str) -> Option<C>` into a
//! resolver, yet such a resolver can **never** report `Ambiguous` or `InvalidConfiguration` — the most convenient path is the
//! least informative one. Note that closures receive **`path.value()`** (the path body without its prefix), not the raw
//! string.

use ospf_rust_framework::persistence::expression::persistence_field_resolver::{
    DiagnosticPersistenceFieldResolver, PersistenceFieldResolution, resolve_field_name,
};
use ospf_rust_math::symbol::PropertyPath;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 字段解析请求。 / The field resolution request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldResolutionRequest {
    /// 要解析的路径。 / The path to resolve.
    pub path: String,
    /// 命中的后端字段名；给空表示不命中。 / The backend field name; blank means no match.
    #[serde(default)]
    pub backend: String,
    /// 歧义候选；两个以上才构成歧义。 / Ambiguity candidates; two or more are ambiguous.
    #[serde(default)]
    pub candidates: Vec<String>,
    /// 解析器配置错误原因；非空即视为配置非法。 / The configuration error; non-blank means invalid configuration.
    #[serde(default)]
    pub invalid_reason: String,
}

/// 字段解析结论。 / The field resolution verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldResolutionReport {
    /// 详细结果的状态名。 / The detailed outcome's state name.
    pub state: String,
    /// 详细结果里的字段名。 / The field name from the detailed outcome.
    #[serde(default)]
    pub resolved: Option<String>,
    /// 详细结果里的歧义候选。 / The ambiguous candidates from the detailed outcome.
    pub candidates: Vec<String>,
    /// 详细结果里的错误原因。 / The reason from the detailed outcome.
    #[serde(default)]
    pub reason: Option<String>,
    /// 可空接口的答案。 / What the nullable interface says.
    #[serde(default)]
    pub nullable: Option<String>,
    /// 闭包 resolver 的答案。 / What the closure resolver says.
    #[serde(default)]
    pub closure: Option<String>,
    /// 闭包看到的路径文本。 / The path text the closure receives.
    pub closure_sees: String,
    /// 四态齐全（**是**）。 / Whether all four states exist (**yes**).
    pub four_states_exist: bool,
    /// `Missing` 被压成 `None`（**是**）。 / Whether `Missing` collapses to `None` (**yes**).
    pub missing_collapses: bool,
    /// `Ambiguous` 被压成 `None`（**是**）。 / Whether `Ambiguous` collapses to `None` (**yes**).
    pub ambiguous_collapses: bool,
    /// `InvalidConfiguration` 也被压成 `None`（**是**）。 / Whether `InvalidConfiguration` collapses too (**yes**).
    pub invalid_configuration_collapses: bool,
    /// 三种失败在可空接口上**不可区分**（**是**）。 / Whether the three failures are indistinguishable (**yes**).
    pub failures_are_indistinguishable: bool,
    /// 闭包 resolver 只有两态（**是**）。 / Whether the closure resolver is two-state only (**yes**).
    pub closure_is_two_state: bool,
    /// 闭包收到的是 `path.value()`（**是**）。 / Whether closures receive `path.value()` (**yes**).
    pub closure_sees_path_value: bool,
}

/// 条目上限（信任边界）。 / The candidate cap (a trust boundary).
const MAX_CANDIDATES: usize = 64;

/// 由请求驱动的详细 resolver。 / A detailed resolver driven by the request.
struct RequestResolver {
    backend: String,
    candidates: Vec<String>,
    invalid_reason: String,
}

impl DiagnosticPersistenceFieldResolver<String> for RequestResolver {
    fn resolve_detailed(&self, path: &PropertyPath) -> PersistenceFieldResolution<String> {
        if !self.invalid_reason.trim().is_empty() {
            return PersistenceFieldResolution::InvalidConfiguration {
                reason: self.invalid_reason.clone(),
            };
        }
        if self.candidates.len() > 1 {
            return PersistenceFieldResolution::Ambiguous { path: path.value().to_owned(),
                candidates: self.candidates.clone() };
        }
        if !self.backend.trim().is_empty() {
            return PersistenceFieldResolution::Resolved(self.backend.clone());
        }
        PersistenceFieldResolution::Missing { path: path.value().to_owned() }
    }
}

/// 走一遍字段解析。非法输入是**错误**。 / Walk the field resolution.
pub fn verify_field_resolution(request: &FieldResolutionRequest) -> Result<FieldResolutionReport> {
    if request.path.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "field resolution failed validation: the path must not be blank"));
    }
    if request.candidates.len() > MAX_CANDIDATES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "field resolution failed validation: at most {MAX_CANDIDATES} candidates are accepted")));
    }
    if request.candidates.iter().any(|candidate| candidate.trim().is_empty()) {
        return Err(BridgeError::new("InvalidInput",
            "field resolution failed validation: a candidate must not be blank"));
    }
    let resolver = RequestResolver { backend: request.backend.clone(),
        candidates: request.candidates.clone(), invalid_reason: request.invalid_reason.clone() };
    let path = PropertyPath::parse(&request.path);
    let detailed = resolver.resolve_detailed(&path);
    // 可空兼容接口的答案。 / What the nullable compatibility interface says.
    let nullable = resolver.resolve_field(&path);
    // 闭包式 resolver：收到 `path.value()`，且只有两态。 / The closure resolver: sees `path.value()`, two-state only.
    let seen = std::cell::RefCell::new(String::new());
    let closure = |text: &str| {
        *seen.borrow_mut() = text.to_owned();
        if text == path.value() && !request.backend.trim().is_empty() {
            Some(request.backend.clone())
        } else {
            None
        }
    };
    let closure_answer = resolve_field_name(&closure, request.path.clone());
    let closure_sees = seen.borrow().clone();
    let (state, resolved, candidates, reason) = match &detailed {
        PersistenceFieldResolution::Resolved(field) => {
            ("resolved", Some(field.clone()), Vec::new(), None)
        }
        PersistenceFieldResolution::Missing { .. } => ("missing", None, Vec::new(), None),
        PersistenceFieldResolution::Ambiguous { candidates, .. } => {
            ("ambiguous", None, candidates.clone(), None)
        }
        PersistenceFieldResolution::InvalidConfiguration { reason } => {
            ("invalidConfiguration", None, Vec::new(), Some(reason.clone()))
        }
    };
    let findings = canonical_findings();
    Ok(FieldResolutionReport {
        state: state.to_owned(),
        resolved,
        candidates,
        reason,
        nullable,
        closure: closure_answer,
        closure_sees,
        four_states_exist: findings.four_states_exist,
        missing_collapses: findings.missing_collapses,
        ambiguous_collapses: findings.ambiguous_collapses,
        invalid_configuration_collapses: findings.invalid_configuration_collapses,
        failures_are_indistinguishable: findings.failures_are_indistinguishable,
        closure_is_two_state: findings.closure_is_two_state,
        closure_sees_path_value: findings.closure_sees_path_value,
    })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    four_states_exist: bool,
    missing_collapses: bool,
    ambiguous_collapses: bool,
    invalid_configuration_collapses: bool,
    failures_are_indistinguishable: bool,
    closure_is_two_state: bool,
    closure_sees_path_value: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let path = PropertyPath::parse("probe.field");
    let probe = |backend: &str, candidates: Vec<String>, invalid: &str| RequestResolver {
        backend: backend.to_owned(), candidates, invalid_reason: invalid.to_owned() };
    // 四态齐全：逐个构造，读回枚举名。 / All four states exist: build each and read its name back.
    let states = [
        format!("{:?}", probe("backend", Vec::new(), "").resolve_detailed(&path)),
        format!("{:?}", probe("", Vec::new(), "").resolve_detailed(&path)),
        format!("{:?}", probe("", vec!["a".to_owned(), "b".to_owned()], "").resolve_detailed(&path)),
        format!("{:?}", probe("", Vec::new(), "broken").resolve_detailed(&path)),
    ];
    let four_states_exist = states[0].starts_with("Resolved") && states[1].starts_with("Missing")
        && states[2].starts_with("Ambiguous") && states[3].starts_with("InvalidConfiguration");
    // 三种失败都在可空接口上变 `None`，而且彼此不可区分。
    // All three failures are `None` through the nullable interface and cannot be told apart.
    let missing = probe("", Vec::new(), "").resolve_field(&path);
    let ambiguous = probe("", vec!["a".to_owned(), "b".to_owned()], "").resolve_field(&path);
    let invalid = probe("", Vec::new(), "broken").resolve_field(&path);
    let missing_collapses = missing.is_none();
    let ambiguous_collapses = ambiguous.is_none();
    let invalid_configuration_collapses = invalid.is_none();
    let failures_are_indistinguishable = missing == ambiguous && ambiguous == invalid;
    // 闭包只能回 Some/None：给它一个歧义路径它也只能说 `None`，永远报不出 Ambiguous。
    // A closure only returns Some/None: handed an ambiguous path it can only say `None`, never `Ambiguous`.
    let closure = |_text: &str| None::<String>;
    let closure_is_two_state = resolve_field_name(&closure, "probe.field").is_none()
        && resolve_field_name(&closure, "other").is_none();
    // 闭包收到的是 `path.value()`。 / Closures receive `path.value()`.
    let seen = std::cell::RefCell::new(String::new());
    let recorder = |text: &str| {
        *seen.borrow_mut() = text.to_owned();
        None::<String>
    };
    let _ = resolve_field_name(&recorder, "user.name");
    let closure_sees_path_value = *seen.borrow() == PropertyPath::parse("user.name").value();
    Findings { four_states_exist, missing_collapses, ambiguous_collapses,
        invalid_configuration_collapses, failures_are_indistinguishable, closure_is_two_state,
        closure_sees_path_value }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(path: &str) -> FieldResolutionRequest {
        FieldResolutionRequest { path: path.to_owned(), backend: String::new(),
            candidates: Vec::new(), invalid_reason: String::new() }
    }

    #[test]
    fn the_four_states_are_all_reachable() {
        let report = verify_field_resolution(&request("orders.status")).unwrap();
        assert_eq!(report.state, "missing");
        let mut resolved = request("orders.status");
        resolved.backend = "user_status".to_owned();
        let resolved = verify_field_resolution(&resolved).unwrap();
        assert_eq!(resolved.state, "resolved");
        assert_eq!(resolved.resolved.as_deref(), Some("user_status"));
        assert_eq!(resolved.nullable.as_deref(), Some("user_status"));
        let mut ambiguous = request("orders.status");
        ambiguous.candidates = vec!["a".to_owned(), "b".to_owned()];
        let ambiguous = verify_field_resolution(&ambiguous).unwrap();
        assert_eq!(ambiguous.state, "ambiguous");
        assert_eq!(ambiguous.candidates.len(), 2);
        let mut invalid = request("orders.status");
        invalid.invalid_reason = "unknown table".to_owned();
        let invalid = verify_field_resolution(&invalid).unwrap();
        assert_eq!(invalid.state, "invalidConfiguration");
        assert_eq!(invalid.reason.as_deref(), Some("unknown table"));
        assert!(invalid.four_states_exist);
    }

    #[test]
    fn the_three_failures_all_collapse_to_none() {
        let missing = verify_field_resolution(&request("a.b")).unwrap();
        let mut ambiguous = request("a.b");
        ambiguous.candidates = vec!["x".to_owned(), "y".to_owned()];
        let ambiguous = verify_field_resolution(&ambiguous).unwrap();
        let mut invalid = request("a.b");
        invalid.invalid_reason = "broken".to_owned();
        let invalid = verify_field_resolution(&invalid).unwrap();
        // 三种失败在可空接口上都是 `None`，无法区分。 / All three are `None` through the nullable interface.
        assert_eq!(missing.nullable, None);
        assert_eq!(ambiguous.nullable, None);
        assert_eq!(invalid.nullable, None);
        assert!(missing.missing_collapses);
        assert!(ambiguous.ambiguous_collapses);
        assert!(invalid.invalid_configuration_collapses);
        assert!(missing.failures_are_indistinguishable);
    }

    #[test]
    fn a_closure_resolver_is_two_state_and_sees_the_path_body() {
        let mut resolved = request("user.name");
        resolved.backend = "name".to_owned();
        let report = verify_field_resolution(&resolved).unwrap();
        // 闭包命中：它拿到的是 `path.value()`（去前缀的路径本体）。 
        // The closure hit: it received `path.value()` (the prefix-free path body).
        assert_eq!(report.closure_sees, PropertyPath::parse("user.name").value());
        assert!(report.closure_sees_path_value);
        assert_eq!(report.closure.as_deref(), Some("name"));
        assert!(report.closure_is_two_state);
        // 歧义请求：闭包只会说 `None`（它没有第三态）。 / An ambiguous request: the closure only says `None`.
        let mut ambiguous = request("user.name");
        ambiguous.candidates = vec!["a".to_owned(), "b".to_owned()];
        let ambiguous = verify_field_resolution(&ambiguous).unwrap();
        assert_eq!(ambiguous.state, "ambiguous");
        assert_eq!(ambiguous.closure, None);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let report = verify_field_resolution(&request("anything.at.all")).unwrap();
        assert!(report.four_states_exist);
        assert!(report.missing_collapses);
        assert!(report.ambiguous_collapses);
        assert!(report.invalid_configuration_collapses);
        assert!(report.failures_are_indistinguishable);
        assert!(report.closure_is_two_state);
        assert!(report.closure_sees_path_value);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_field_resolution(&request("  ")).unwrap_err().message
            .contains("path must not be blank"));
        let mut blank_candidate = request("a.b");
        blank_candidate.candidates = vec!["  ".to_owned(), "x".to_owned()];
        assert!(verify_field_resolution(&blank_candidate).unwrap_err().message
            .contains("candidate must not be blank"));
        let mut many = request("a.b");
        many.candidates = vec!["x".to_owned(); MAX_CANDIDATES + 1];
        assert!(verify_field_resolution(&many).unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_field_resolution(&request("a.b")).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"invalidConfigurationCollapses\""), "{json}");
        assert!(json.contains("\"closureSees\""), "{json}");
        assert!(json.contains("\"fourStatesExist\""), "{json}");
        assert!(!json.contains("closure_sees"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<FieldResolutionReport>(&copy.to_string()).is_err());
    }
}
