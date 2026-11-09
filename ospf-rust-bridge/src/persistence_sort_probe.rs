//! 续挖第 112 单：**排序描述是纯追加的构建器，而"空值排序支持"只问方向**——`SortBy::then` 只把新项**追加到末尾**
//! 并且**不合并同路径项**（同一字段写两次会留两条），`Add` 就是 `then`；`is_empty`/`is_not_empty` 互补；
//! `NullsOrderSupport::is_supported` 里 `Auto`/`Always` 恒真、`Never` 恒假、`OnlyAsc` **只认 `Asc`**，
//! 而且**完全不看 `nulls` 有没有真的设**（`nulls: None` 的项也算"支持"）
//! （`persistence/expression/sort_by.rs` 的 `SortBy` / `SortItem` / `NullsOrderSupport`）
//! Continuation, delivery 112: **the sort descriptor is a pure-append builder, and "null ordering support" asks about the
//! direction alone** — `SortBy::then` only **appends to the tail** and **never merges same-path items**, so writing one field
//! twice leaves two entries, and `Add` is `then`; `is_empty` and `is_not_empty` are complementary;
//! `NullsOrderSupport::is_supported` says `Auto`/`Always` are always true, `Never` always false and `OnlyAsc` **accepts `Asc`
//! only**, and it **never looks at whether `nulls` is actually set**, so an item with `nulls: None` still counts as supported
//! (`SortBy` / `SortItem` / `NullsOrderSupport` in `persistence/expression/sort_by.rs`).
//!
//! 第 111 单量的是有效性排序，这一单量**持久化排序描述的构建与支持判据**。八条实测：
//!
//! 1. **`then` 是追加**（`the_then_family_appends_to_the_tail()`）。
//! 2. **`then` 不合并同路径项**（`then_does_not_merge_the_same_path()`）。
//! 3. **`Add` 就是 `then`**（`the_add_operator_is_then()`）。
//! 4. **空/非空互补**（`emptiness_is_complementary()`）。
//! 5. **`is_supported` 只问方向**（`the_support_test_reads_the_direction_only()`）。
//! 6. **`is_supported` 不看 `nulls`**（`the_support_test_ignores_the_nulls_field()`）。
//! 7. **`*_nulls` 与普通构造只差 `nulls`**（`the_nulls_constructors_differ_only_by_the_nulls_field()`）。
//! 8. **`asc_many`/`desc_many` 保留输入顺序**（`the_many_constructors_keep_the_input_order()`）。
//!
//! Delivery 111 measured the effectiveness ranking; this one measures **the persistence sort descriptor's building and
//! support readings**. **The `then` family appends to the tail** and **does not merge the same path**. **`Add` is `then`.**
//! **Emptiness is complementary.** **The support test reads the direction only** and **ignores the nulls field**. **The
//! `*_nulls` constructors differ only by the nulls field.** And **the `*_many` constructors keep the input order.**

use ospf_rust_framework::persistence::expression::sort_by::{
    NullsOrder, NullsOrderSupport, SortBy, SortDirection, SortItem,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个排序项的描述。 / The description of one sort item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistenceSortItemSpec {
    /// 字段路径。 / The field path.
    pub path: String,
    /// 方向名（`Asc` / `Desc`，默认 `Asc`）。 / The direction name.
    #[serde(default)]
    pub direction: Option<String>,
    /// 空值排序名（`NullsFirst` / `NullsLast`，缺省不打 nulls 标记）。 / The null ordering name.
    #[serde(default)]
    pub nulls: Option<String>,
}

/// 排序描述探针请求。 / The sort descriptor probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistenceSortProbeRequest {
    /// 依次构建的排序项（每个都走 `then_*`）。 / The items built in order, each through `then_*`.
    #[serde(default)]
    pub items: Vec<PersistenceSortItemSpec>,
    /// 要测"支持判据"的方向名。 / The direction used for the support test.
    #[serde(default)]
    pub probe_direction: Option<String>,
}

/// 一个已构建排序项的读数。 / One built sort item's reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistenceSortRow {
    /// 字段路径。 / The field path.
    pub path: String,
    /// 方向名。 / The direction name.
    pub direction: String,
    /// 空值排序名（`null` 表示没有）。 / The null ordering name.
    #[serde(default)]
    pub nulls: Option<String>,
}

/// 排序描述探针结论。 / The sort descriptor probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistenceSortProbeReport {
    /// 构建出的排序项（按顺序）。 / The built items, in order.
    pub items: Vec<PersistenceSortRow>,
    /// 是否为空。 / Whether the descriptor is empty.
    pub empty: bool,
    /// 是否非空。 / Whether the descriptor is not empty.
    pub not_empty: bool,
    /// 四种支持策略对探针方向的判定。 / The four policies' verdicts for the probe direction.
    pub auto_supported: bool,
    pub always_supported: bool,
    pub never_supported: bool,
    pub only_asc_supported: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_then_family_appends_to_the_tail: bool,
    pub then_does_not_merge_the_same_path: bool,
    pub the_add_operator_is_then: bool,
    pub emptiness_is_complementary: bool,
    pub the_support_test_reads_the_direction_only: bool,
    pub the_support_test_ignores_the_nulls_field: bool,
    pub the_nulls_constructors_differ_only_by_the_nulls_field: bool,
    pub the_many_constructors_keep_the_input_order: bool,
}

/// 按名字取方向。 / Resolve a direction by name.
fn direction_named(name: &str) -> Option<SortDirection> {
    match name {
        "Asc" => Some(SortDirection::Asc),
        "Desc" => Some(SortDirection::Desc),
        _ => None,
    }
}

/// 按名字取空值排序。 / Resolve a null ordering by name.
fn nulls_named(name: &str) -> Option<NullsOrder> {
    match name {
        "NullsFirst" => Some(NullsOrder::NullsFirst),
        "NullsLast" => Some(NullsOrder::NullsLast),
        _ => None,
    }
}

/// 方向名。 / The direction name.
fn direction_name(direction: SortDirection) -> String {
    match direction {
        SortDirection::Asc => "Asc",
        SortDirection::Desc => "Desc",
    }.to_owned()
}

/// 空值排序名。 / The null ordering name.
fn nulls_name(nulls: &NullsOrder) -> String {
    match nulls {
        NullsOrder::NullsFirst => "NullsFirst",
        NullsOrder::NullsLast => "NullsLast",
    }.to_owned()
}

/// 造一个排序项。 / Build one sort item.
fn item_of(spec: &PersistenceSortItemSpec) -> Result<SortItem> {
    let direction = match spec.direction.as_deref() {
        None => SortDirection::Asc,
        Some(name) => direction_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
            "persistence sort probe failed validation: unknown direction"))?,
    };
    let nulls = match spec.nulls.as_deref() {
        None => None,
        Some(name) => Some(nulls_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
            "persistence sort probe failed validation: unknown null ordering"))?),
    };
    Ok(SortItem { path: spec.path.as_str().into(), direction, nulls })
}

/// 走一遍排序描述探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_persistence_sort_probe(
    request: &PersistenceSortProbeRequest,
) -> Result<PersistenceSortProbeReport> {
    if request.items.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "persistence sort probe failed validation: \
            at most eight items are accepted"));
    }
    let findings = canonical_findings();
    let mut sort = SortBy::empty();
    for spec in &request.items {
        let item = item_of(spec)?;
        // 每一项都走 `then`，让"纯追加"这一步真的被调用。 
        // Each item goes through `then`, so the pure-append step really runs.
        sort = sort.then(SortBy { items: vec![item] });
    }
    let probe = match request.probe_direction.as_deref() {
        None => SortItem { path: "probe".into(), direction: SortDirection::Asc, nulls: None },
        Some(name) => {
            let direction = direction_named(name).ok_or_else(|| BridgeError::new("InvalidInput",
                "persistence sort probe failed validation: unknown probe direction"))?;
            SortItem { path: "probe".into(), direction, nulls: None }
        }
    };
    Ok(PersistenceSortProbeReport {
        items: sort.items.iter().map(|item| PersistenceSortRow {
            path: item.path.to_string(),
            direction: direction_name(item.direction),
            nulls: item.nulls.as_ref().map(nulls_name),
        }).collect(),
        empty: sort.is_empty(),
        not_empty: sort.is_not_empty(),
        auto_supported: NullsOrderSupport::Auto.is_supported(&probe),
        always_supported: NullsOrderSupport::Always.is_supported(&probe),
        never_supported: NullsOrderSupport::Never.is_supported(&probe),
        only_asc_supported: NullsOrderSupport::OnlyAsc.is_supported(&probe),
        the_then_family_appends_to_the_tail: findings.the_then_family_appends_to_the_tail,
        then_does_not_merge_the_same_path: findings.then_does_not_merge_the_same_path,
        the_add_operator_is_then: findings.the_add_operator_is_then,
        emptiness_is_complementary: findings.emptiness_is_complementary,
        the_support_test_reads_the_direction_only: findings.the_support_test_reads_the_direction_only,
        the_support_test_ignores_the_nulls_field:
            findings.the_support_test_ignores_the_nulls_field,
        the_nulls_constructors_differ_only_by_the_nulls_field:
            findings.the_nulls_constructors_differ_only_by_the_nulls_field,
        the_many_constructors_keep_the_input_order:
            findings.the_many_constructors_keep_the_input_order,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_then_family_appends_to_the_tail: bool,
    then_does_not_merge_the_same_path: bool,
    the_add_operator_is_then: bool,
    emptiness_is_complementary: bool,
    the_support_test_reads_the_direction_only: bool,
    the_support_test_ignores_the_nulls_field: bool,
    the_nulls_constructors_differ_only_by_the_nulls_field: bool,
    the_many_constructors_keep_the_input_order: bool,
}

/// 取路径序列。 / Take the path sequence.
fn paths(sort: &SortBy) -> Vec<String> {
    sort.items.iter().map(|item| item.path.to_string()).collect()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) then 追加到末尾。 / `then` appends to the tail.
        let the_then_family_appends_to_the_tail = {
            let sort = SortBy::asc("a").then_desc("b").then_asc_nulls("c", NullsOrder::NullsLast);
            paths(&sort) == vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
                && sort.items[1].direction == SortDirection::Desc
                && sort.items[2].nulls == Some(NullsOrder::NullsLast)
        };
        // 2) then 不合并同路径。 / `then` does not merge the same path.
        let then_does_not_merge_the_same_path = {
            let sort = SortBy::asc("a").then_desc("a");
            // 同一字段两条记录，方向各自保留。 / One field keeps two records with their own directions.
            sort.items.len() == 2
                && sort.items[1].direction == SortDirection::Desc
                && paths(&sort) == vec!["a".to_owned(), "a".to_owned()]
        };
        // 3) Add 就是 then。 / `Add` is `then`.
        let the_add_operator_is_then = {
            let added = SortBy::asc("a") + SortBy::desc("b");
            let chained = SortBy::asc("a").then(SortBy::desc("b"));
            added == chained && paths(&added) == vec!["a".to_owned(), "b".to_owned()]
        };
        // 4) 空/非空互补。 / Emptiness is complementary.
        let emptiness_is_complementary = {
            let blank = SortBy::empty();
            blank.is_empty() && !blank.is_not_empty()
                && {
                    let one = SortBy::asc("a");
                    !one.is_empty() && one.is_not_empty()
                }
        };
        // 5) 支持判据只看方向。 / The support test reads the direction only.
        let the_support_test_reads_the_direction_only = {
            let asc = SortItem::asc("a");
            let desc = SortItem::desc("a");
            NullsOrderSupport::Auto.is_supported(&asc) && NullsOrderSupport::Auto.is_supported(&desc)
                && NullsOrderSupport::Always.is_supported(&asc)
                && !NullsOrderSupport::Never.is_supported(&asc)
                && !NullsOrderSupport::Never.is_supported(&desc)
                && NullsOrderSupport::OnlyAsc.is_supported(&asc)
                && !NullsOrderSupport::OnlyAsc.is_supported(&desc)
                // 默认策略是 Auto。 / The default policy is auto.
                && NullsOrderSupport::default() == NullsOrderSupport::Auto
        };
        // 6) 支持判据不看 nulls 字段。 / The support test ignores the nulls field.
        let the_support_test_ignores_the_nulls_field = {
            // 没有 nulls 的项也被判"支持"。 / An item without nulls is still called supported.
            let bare = SortItem::asc("a");
            let nulls = SortItem::new("a", SortDirection::Asc, Some(NullsOrder::NullsFirst));
            bare.nulls.is_none() && nulls.nulls.is_some()
                && NullsOrderSupport::OnlyAsc.is_supported(&bare)
                && NullsOrderSupport::OnlyAsc.is_supported(&nulls)
        };
        // 7) *_nulls 与普通构造只差 nulls。 / The nulls constructors differ only by the nulls field.
        let the_nulls_constructors_differ_only_by_the_nulls_field = {
            let plain = SortItem::asc("a");
            let with_nulls = SortItem::new("a", SortDirection::Asc, Some(NullsOrder::NullsLast));
            plain.direction == with_nulls.direction && plain.path == with_nulls.path
                && plain.nulls.is_none() && with_nulls.nulls == Some(NullsOrder::NullsLast)
                && {
                    let plain_desc = SortItem::desc("a");
                    let nulls_desc = SortItem::new("a", SortDirection::Desc, Some(NullsOrder::NullsFirst));
                    plain_desc.direction == nulls_desc.direction
                        && nulls_desc.nulls == Some(NullsOrder::NullsFirst)
                }
        };
        // 8) *_many 保留输入顺序。 / The many constructors keep the input order.
        let the_many_constructors_keep_the_input_order = {
            let ascending = SortBy::asc_many(["z", "a", "m"]);
            let descending = SortBy::desc_many(["z", "a"]);
            paths(&ascending) == vec!["z".to_owned(), "a".to_owned(), "m".to_owned()]
                && ascending.items.iter().all(|item| item.direction == SortDirection::Asc)
                && ascending.items.iter().all(|item| item.nulls.is_none())
                && descending.items.iter().all(|item| item.direction == SortDirection::Desc)
        };
        Findings {
            the_then_family_appends_to_the_tail,
            then_does_not_merge_the_same_path,
            the_add_operator_is_then,
            emptiness_is_complementary,
            the_support_test_reads_the_direction_only,
            the_support_test_ignores_the_nulls_field,
            the_nulls_constructors_differ_only_by_the_nulls_field,
            the_many_constructors_keep_the_input_order,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: PersistenceSortProbeRequest) -> PersistenceSortProbeReport {
        verify_persistence_sort_probe(&request).unwrap()
    }

    fn spec(path: &str, direction: Option<&str>, nulls: Option<&str>)
        -> PersistenceSortItemSpec
    {
        PersistenceSortItemSpec { path: path.to_owned(),
            direction: direction.map(str::to_owned), nulls: nulls.map(str::to_owned) }
    }

    fn request() -> PersistenceSortProbeRequest {
        PersistenceSortProbeRequest { items: vec![spec("a", Some("Asc"), None),
            spec("b", Some("Desc"), Some("NullsLast"))], probe_direction: Some("Asc".to_owned()) }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request());
        assert!(!report.empty);
        assert!(report.not_empty);
        assert_eq!(report.items.len(), 2);
        assert_eq!(report.items[0].path, "a");
        assert_eq!(report.items[0].direction, "Asc");
        assert_eq!(report.items[0].nulls, None);
        assert_eq!(report.items[1].direction, "Desc");
        assert_eq!(report.items[1].nulls, Some("NullsLast".to_owned()));
        assert!(report.auto_supported);
        assert!(report.always_supported);
        assert!(!report.never_supported);
        assert!(report.only_asc_supported);
        assert!(report.the_then_family_appends_to_the_tail);
        assert!(report.then_does_not_merge_the_same_path);
        assert!(report.the_add_operator_is_then);
        assert!(report.emptiness_is_complementary);
        assert!(report.the_support_test_reads_the_direction_only);
        assert!(report.the_support_test_ignores_the_nulls_field);
        assert!(report.the_nulls_constructors_differ_only_by_the_nulls_field);
        assert!(report.the_many_constructors_keep_the_input_order);
    }

    #[test]
    fn the_support_test_reads_only_the_direction() {
        let ascending = ask(PersistenceSortProbeRequest {
            probe_direction: Some("Asc".to_owned()), ..request() });
        assert!(ascending.only_asc_supported);
        let descending = ask(PersistenceSortProbeRequest {
            probe_direction: Some("Desc".to_owned()), ..request() });
        assert!(!descending.only_asc_supported);
        // Auto / Always / Never 与方向无关。 / The other three do not depend on the direction.
        assert!(descending.auto_supported && descending.always_supported);
        assert!(!descending.never_supported);
    }

    #[test]
    fn the_same_path_is_never_merged() {
        let report = ask(PersistenceSortProbeRequest {
            items: vec![spec("a", Some("Asc"), None), spec("a", Some("Desc"), None)],
            ..request() });
        assert_eq!(report.items.len(), 2);
        assert_eq!(report.items[1].path, "a");
        assert_eq!(report.items[1].direction, "Desc");
    }

    #[test]
    fn an_empty_descriptor_reports_both_flags() {
        let report = ask(PersistenceSortProbeRequest { items: Vec::new(), ..request() });
        assert!(report.empty);
        assert!(!report.not_empty);
        assert!(report.items.is_empty());
        // 支持判据与描述内容无关。 / The support test is independent of the descriptor.
        assert!(report.only_asc_supported);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_persistence_sort_probe(&PersistenceSortProbeRequest {
            items: (0..9).map(|index| spec(&format!("f{index}"), None, None)).collect(),
            ..request() }).unwrap_err().message.contains("at most eight items"));
        assert!(verify_persistence_sort_probe(&PersistenceSortProbeRequest {
            items: vec![spec("a", Some("Sideways"), None)], ..request() }).unwrap_err()
            .message.contains("unknown direction"));
        assert!(verify_persistence_sort_probe(&PersistenceSortProbeRequest {
            items: vec![spec("a", None, Some("Middle"))], ..request() }).unwrap_err()
            .message.contains("unknown null ordering"));
        assert!(verify_persistence_sort_probe(&PersistenceSortProbeRequest {
            probe_direction: Some("Sideways".to_owned()), ..request() }).unwrap_err()
            .message.contains("unknown probe direction"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"notEmpty\""), "{json}");
        assert!(json.contains("\"onlyAscSupported\""), "{json}");
        assert!(json.contains("\"thenDoesNotMergeTheSamePath\""), "{json}");
        assert!(!json.contains("not_empty"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<PersistenceSortProbeReport>(&copy.to_string()).is_err());
    }
}
