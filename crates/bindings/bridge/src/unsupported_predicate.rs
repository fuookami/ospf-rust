//! 续挖第 11 单：不支持谓词策略的三型与谓词注解的键值面
//! （`persistence/expression/unsupported_predicate_policy.rs` + `predicate_annotations.rs`）
//! Continuation, delivery 11: the three unsupported-predicate policies and the annotation key/value surface.
//!
//! 两个文件加起来 50 行，但策略那个不是可以略过的枚举。四条实测事实：
//!
//! 1. **默认是 `AlwaysFalse`**：不选策略时，无法下推的谓词翻译成**恒假**——也就是"查不到任何行"，而不是报错。
//!    这是个安全默认（宁可少给数据），但它意味着**静默降级**；要靠 `should_fail_fast()` 才发现自己没配。
//! 2. **`IGNORE` / `ERROR` 是常量别名，不是新变体**：`IGNORE == AlwaysFalse`、`ERROR == FailFast`。
//!    所以变体总数**恒为 3**，两个旧名字在解析时归一到已有变体上，别指望它们各自有独立行为。
//! 3. **`ClientFilter` 两边都不做**：`should_fail_fast()` 与 `should_use_always_false()` **都为假**。
//!    它是唯一的"既不报错也不主动改写成恒假"的策略——语义留白，调用方自己决定怎么处理。
//! 4. **谓词注解就是键值对，不去重也不排序**：同一个 `key` 出现两次就留两条。这是占位类型，没有任何约束，
//!    所以报告把重复键**列出来**（`duplicateKeys`）而不是替它去重——去重会偷偷改掉调用方给的东西。
//!
//! These two files total fifty lines, but the policy is not a throwaway enum. Four measured facts: the **default is
//! `AlwaysFalse`**, so an unsupported predicate becomes an always-false condition — "no rows" rather than an error, a safe
//! default that nonetheless **degrades silently**, which `should_fail_fast()` is how you notice you never configured it;
//! **`IGNORE` / `ERROR` are constant aliases, not new variants** (`IGNORE == AlwaysFalse`, `ERROR == FailFast`), so the variant
//! count is fixed at three and the old names normalise onto existing variants rather than getting behaviour of their own;
//! **`ClientFilter` does neither**, with both `should_fail_fast()` and `should_use_always_false()` false — the one policy that
//! neither errors nor rewrites, leaving the semantics to the caller; and **annotations are plain key/value pairs with no
//! deduplication or sorting**, so a repeated `key` simply stays twice, and this layer **lists** those keys rather than
//! deduplicating them behind the caller's back.

use ospf_rust_framework::persistence::expression::{
    PredicateAnnotation, UnsupportedPredicatePolicy,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个注解。 / One annotation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnnotationSpec {
    /// 注解键。 / The annotation key.
    pub key: String,
    /// 注解值。 / The annotation value.
    pub value: String,
}

/// 策略请求。 / The policy request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnsupportedPredicateRequest {
    /// 要解析的策略名：`alwaysFalse` / `failFast` / `clientFilter` / `ignore` / `error` / `default`。
    /// The policy names to resolve: `alwaysFalse` / `failFast` / `clientFilter` / `ignore` / `error` / `default`.
    #[serde(default)]
    pub policies: Vec<String>,
    /// 注解列表（不去重、不排序）。 / The annotations (no deduplication, no sorting).
    #[serde(default)]
    pub annotations: Vec<AnnotationSpec>,
}

/// 一条策略的结论。 / One policy's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyRow {
    /// 输入的策略名。 / The name as given.
    pub name: String,
    /// 解析到的变体名（别名会归一）。 / The resolved variant name (aliases normalise).
    pub resolved: String,
    /// 是否应立即失败。 / Whether it should fail fast.
    pub fail_fast: bool,
    /// 是否应改写成恒假。 / Whether it should become always-false.
    pub always_false: bool,
    /// 是否与默认策略相同。 / Whether it equals the default policy.
    pub is_default: bool,
}

/// 策略结论。 / The policy verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnsupportedPredicateReport {
    /// 默认策略的变体名（`alwaysFalse`）。 / The default policy's variant name (`alwaysFalse`).
    pub default_policy: String,
    /// 变体词表（**恒为三项**，别名不在其中）。 / The variant vocabulary (**always three entries**; aliases excluded).
    pub variant_vocabulary: Vec<String>,
    /// 别名映射（旧名 → 变体名）。 / The alias map (old name to variant name).
    pub aliases: Vec<PolicyAlias>,
    /// 逐条策略结论。 / Each policy's verdict.
    pub policies: Vec<PolicyRow>,
    /// 「既不失败也不改写」的策略名（实测只有 `clientFilter`）。 / The policies that neither fail nor rewrite.
    pub neutral_policies: Vec<String>,
    /// 注解条数。 / The annotation count.
    pub annotation_count: u32,
    /// 注解的键序列（**保持原序、不合并**）。 / The key sequence (**original order, not merged**).
    pub annotation_keys: Vec<String>,
    /// 出现多于一次的注解键（领域不去重，如实列出）。 / Keys appearing more than once.
    pub duplicate_keys: Vec<String>,
}

/// 一个别名映射。 / One alias mapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyAlias {
    /// 旧名（线上的写法）。 / The old name (as spelled on the wire).
    pub name: String,
    /// 归一到哪个变体。 / The variant it normalises to.
    pub resolves_to: String,
}

/// 策略条数上限（信任边界）。 / The policy cap (a trust boundary).
const MAX_ENTRIES: usize = 4_096;

/// 解析策略名并报告三型与别名的真实关系。非法输入是**错误**。
/// Resolve the policy names and report how the three variants and their aliases really relate.
pub fn verify_unsupported_predicate(request: &UnsupportedPredicateRequest)
    -> Result<UnsupportedPredicateReport>
{
    if request.policies.len() > MAX_ENTRIES || request.annotations.len() > MAX_ENTRIES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "unsupported predicate failed validation: at most {MAX_ENTRIES} entries are accepted")));
    }
    let default = UnsupportedPredicatePolicy::default();
    let mut policies = Vec::with_capacity(request.policies.len());
    for name in &request.policies {
        let policy = resolve(name)?;
        policies.push(PolicyRow {
            name: name.clone(),
            resolved: variant_name(policy).to_owned(),
            fail_fast: policy.should_fail_fast(),
            always_false: policy.should_use_always_false(),
            is_default: policy == default,
        });
    }
    // 注解：**原序保留**，只统计重复键。 / Annotations keep their **original order**; only duplicate keys are counted.
    let mut annotation_keys = Vec::with_capacity(request.annotations.len());
    for annotation in &request.annotations {
        if annotation.key.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "unsupported predicate failed validation: annotation keys must not be blank"));
        }
        // 走领域的类型：构造出来只为让这一层不自己发明一个键值对。
        // Go through the domain type so this layer does not invent its own key/value pair.
        let annotation = PredicateAnnotation { key: annotation.key.clone(),
            value: annotation.value.clone() };
        annotation_keys.push(annotation.key);
    }
    let mut counts = std::collections::BTreeMap::<&str, u32>::new();
    for key in &annotation_keys { *counts.entry(key.as_str()).or_default() += 1; }
    let duplicate_keys = counts.into_iter().filter(|(_, count)| *count > 1)
        .map(|(key, _)| key.to_owned()).collect::<Vec<_>>();
    Ok(UnsupportedPredicateReport {
        default_policy: variant_name(default).to_owned(),
        variant_vocabulary: vec!["alwaysFalse".to_owned(), "failFast".to_owned(),
            "clientFilter".to_owned()],
        aliases: vec![
            PolicyAlias { name: "ignore".to_owned(),
                resolves_to: variant_name(UnsupportedPredicatePolicy::IGNORE).to_owned() },
            PolicyAlias { name: "error".to_owned(),
                resolves_to: variant_name(UnsupportedPredicatePolicy::ERROR).to_owned() },
        ],
        policies,
        neutral_policies: vec!["alwaysFalse".to_owned(), "failFast".to_owned(),
            "clientFilter".to_owned()]
            .into_iter()
            .filter(|name| matches!(resolve(name), Ok(policy)
                if !policy.should_fail_fast() && !policy.should_use_always_false()))
            .collect(),
        annotation_count: u32::try_from(annotation_keys.len()).unwrap_or(u32::MAX),
        annotation_keys,
        duplicate_keys,
    })
}

/// 把线格式名字解析成策略（含旧别名）。 / Resolve a wire name into a policy (old aliases included).
fn resolve(name: &str) -> Result<UnsupportedPredicatePolicy> {
    Ok(match name {
        "alwaysFalse" => UnsupportedPredicatePolicy::AlwaysFalse,
        "failFast" => UnsupportedPredicatePolicy::FailFast,
        "clientFilter" => UnsupportedPredicatePolicy::ClientFilter,
        // 两个旧名是**常量别名**，不是新变体。 / The two old names are **constant aliases**, not new variants.
        "ignore" => UnsupportedPredicatePolicy::IGNORE,
        "error" => UnsupportedPredicatePolicy::ERROR,
        "default" => UnsupportedPredicatePolicy::default(),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "unsupported predicate failed validation: unknown policy `{other}`"))),
    })
}

/// 变体名。 / The variant name.
fn variant_name(policy: UnsupportedPredicatePolicy) -> &'static str {
    match policy {
        UnsupportedPredicatePolicy::AlwaysFalse => "alwaysFalse",
        UnsupportedPredicatePolicy::FailFast => "failFast",
        UnsupportedPredicatePolicy::ClientFilter => "clientFilter",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(policies: &[&str]) -> UnsupportedPredicateRequest {
        UnsupportedPredicateRequest { policies: policies.iter().map(|name| (*name).to_owned())
            .collect(), annotations: Vec::new() }
    }

    fn annotation(key: &str, value: &str) -> AnnotationSpec {
        AnnotationSpec { key: key.to_owned(), value: value.to_owned() }
    }

    #[test]
    fn default_is_always_false_not_fail_fast() {
        // 默认是 `AlwaysFalse`：无法下推的谓词变成恒假（查不到行），**不是**报错。
        // The default is `AlwaysFalse`: an unsupported predicate becomes always-false, **not** an error.
        let report = verify_unsupported_predicate(&request(&[])).unwrap();
        assert_eq!(report.default_policy, "alwaysFalse");
        assert_eq!(report.variant_vocabulary.len(), 3);
        let report = verify_unsupported_predicate(&request(&["default"])).unwrap();
        assert_eq!(report.policies[0].resolved, "alwaysFalse");
        assert!(report.policies[0].is_default);
        assert!(report.policies[0].always_false);
        assert!(!report.policies[0].fail_fast);
    }

    #[test]
    fn aliases_are_constants_not_variants() {
        // `IGNORE` / `ERROR` 归一到已有变体，**不增加变体数量**。 / The aliases normalise onto existing variants.
        let report = verify_unsupported_predicate(&request(&["ignore", "error", "alwaysFalse",
            "failFast"])).unwrap();
        assert_eq!(report.aliases.len(), 2);
        assert_eq!(report.aliases[0].name, "ignore");
        assert_eq!(report.aliases[0].resolves_to, "alwaysFalse");
        assert_eq!(report.aliases[1].name, "error");
        assert_eq!(report.aliases[1].resolves_to, "failFast");
        // 别名与它的目标**完全等价**。 / An alias is fully equivalent to its target.
        assert_eq!(report.policies[0].resolved, report.policies[2].resolved);
        assert_eq!(report.policies[1].resolved, report.policies[3].resolved);
        assert_eq!(report.policies[0].fail_fast, report.policies[2].fail_fast);
        assert_eq!(report.policies[1].always_false, report.policies[3].always_false);
        // 变体词表还是三项。 / The variant vocabulary still has three entries.
        assert_eq!(report.variant_vocabulary,
            vec!["alwaysFalse".to_owned(), "failFast".to_owned(), "clientFilter".to_owned()]);
    }

    #[test]
    fn client_filter_neither_fails_nor_rewrites() {
        // `ClientFilter` 两个判定都为假——这是唯一的"两边都不做"的策略。
        // `ClientFilter` answers false to both — the only policy that does neither.
        let report = verify_unsupported_predicate(&request(&["alwaysFalse", "failFast",
            "clientFilter"])).unwrap();
        let client = report.policies.iter().find(|row| row.name == "clientFilter").unwrap();
        assert!(!client.fail_fast);
        assert!(!client.always_false);
        assert!(!client.is_default);
        assert_eq!(report.neutral_policies, vec!["clientFilter".to_owned()]);
        // 另外两个各自只做一件事，而且**互斥**。 / The other two each do exactly one thing, exclusively.
        for row in &report.policies {
            if row.name == "clientFilter" {
                continue;
            }
            assert!(row.fail_fast ^ row.always_false, "{}", row.name);
        }
    }

    #[test]
    fn annotations_keep_order_and_are_not_deduplicated() {
        let report = verify_unsupported_predicate(&UnsupportedPredicateRequest {
            policies: Vec::new(),
            annotations: vec![annotation("hint", "index-scan"), annotation("ttl", "60"),
                annotation("hint", "sequential")] }).unwrap();
        // 原序保留，重复键留两条。 / Original order kept; a repeated key stays twice.
        assert_eq!(report.annotation_count, 3);
        assert_eq!(report.annotation_keys, vec!["hint".to_owned(), "ttl".to_owned(),
            "hint".to_owned()]);
        assert_eq!(report.duplicate_keys, vec!["hint".to_owned()]);
        // 空注解列表合法。 / An empty annotation list is legal.
        let report = verify_unsupported_predicate(&request(&[])).unwrap();
        assert_eq!(report.annotation_count, 0);
        assert!(report.annotation_keys.is_empty());
        assert!(report.duplicate_keys.is_empty());
        assert!(report.policies.is_empty());
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_unsupported_predicate(&request(&["alwaysTrue"])).unwrap_err().message
            .contains("unknown policy"));
        assert!(verify_unsupported_predicate(&UnsupportedPredicateRequest {
            policies: Vec::new(), annotations: vec![annotation("  ", "x")] }).unwrap_err().message
            .contains("must not be blank"));
        let many = UnsupportedPredicateRequest { policies: Vec::new(),
            annotations: (0..=MAX_ENTRIES).map(|index| annotation(&format!("k{index}"), "v"))
                .collect() };
        assert!(verify_unsupported_predicate(&many).unwrap_err().message.contains("at most"));
    }
}
