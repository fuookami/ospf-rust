//! 续挖第 23 单：排序描述符的组合代数与"空值排序支持"的 4×2 矩阵
//! （`persistence/expression/sort_by.rs`）
//! Continuation, delivery 23: the sort descriptor's composition algebra and the 4×2 null-ordering
//! support matrix.
//!
//! `sort_by.rs` 里真正有信息量的不是 `SortBy` 本身（它就是个 `Vec<SortItem>`），而是两处**容易被误用**的地方：
//!
//! 1. **`NullsOrderSupport::is_supported` 是一张 4×2 的表，而且 `Auto` 与 `Always` 的回答**完全相同**——
//!    都恒为 `true`。也就是说 `Auto`（"由后端决定"）在**这个查询里**并不表示"不知道"，而是**乐观地声称支持**。
//!    真正想"先问后端"的调用方拿不到"未知"这个答案，只能自己额外区分。
//! 2. **`then` / `Add` 只拼不整理**：不排序、不去重，重复路径会原样留在列表里。加上 `then_asc` 这类追加入口，
//!    同一个字段被写两遍是合法的，没有任何告警。
//!
//! 另外 `SortItem::asc` / `desc` 的 `nulls` 是 `None`（"不指定"），与显式 `NullsFirst` / `NullsLast`
//! 是三态——`None` 不等价于任何一侧，调用方必须自己决定默认值。`SortOrder` 是 `SortDirection` 的纯别名。
//!
//! What carries information here is not `SortBy` itself (just a `Vec<SortItem>`) but two easy misuses.
//! **`NullsOrderSupport::is_supported` is a 4×2 table and `Auto` answers exactly like `Always`** — both are constantly
//! `true`, so `Auto` ("decided by the backend") does **not** mean "unknown" in this query, it **optimistically claims
//! support**; a caller that wants to ask the backend first cannot get an "unknown" answer and must distinguish it
//! separately. And **`then` / `Add` only concatenate**: no sorting, no dedupe, so duplicate paths stay in the list,
//! and with the `then_*` appenders it is legal to name the same field twice with no warning. Also `SortItem::asc` /
//! `desc` leave `nulls` at `None` ("unspecified"), a genuine third state next to `NullsFirst` / `NullsLast`, so the
//! caller owns the default; `SortOrder` is a pure alias of `SortDirection`.

use ospf_rust_framework::persistence::expression::sort_by::{
    NullsOrder, NullsOrderSupport, SortBy, SortDirection, SortItem,
};
use ospf_rust_math::symbol::PropertyPath;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个排序项字面量。 / One sort item literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SortItemSpec {
    /// 字段路径。 / The field path.
    pub path: String,
    /// `asc` / `desc`。 / `asc` / `desc`.
    pub direction: String,
    /// `first` / `last` / 不指定。 / `first` / `last` / unspecified.
    #[serde(default)]
    pub nulls: Option<String>,
}

/// 排序请求。 / The sort request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SortByRequest {
    /// 第一个排序描述符。 / The first descriptor.
    #[serde(default)]
    pub left: Vec<SortItemSpec>,
    /// 用 `then` / `+` 拼上去的第二个描述符。 / The second descriptor joined with `then` / `+`.
    #[serde(default)]
    pub right: Vec<SortItemSpec>,
    /// 追加的升序字段。 / The ascending fields to append.
    #[serde(default)]
    pub append_asc: Vec<String>,
    /// 追加的降序字段。 / The descending fields to append.
    #[serde(default)]
    pub append_desc: Vec<String>,
    /// 要探测"支持与否"的方向。 / The direction to probe for support (unused; the table is exhaustive).
    #[serde(default)]
    pub support_probe: String,
}

/// 一个落地出来的排序项。 / One emitted sort item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SortedItemRow {
    /// 字段路径。 / The field path.
    pub path: String,
    /// `asc` / `desc`。 / `asc` / `desc`.
    pub direction: String,
    /// `first` / `last` / 不指定。 / `first` / `last` / unspecified.
    #[serde(default)]
    pub nulls: Option<String>,
}

/// 支持矩阵的一格。 / One cell of the support matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupportRow {
    /// 策略名。 / The policy name.
    pub policy: String,
    /// 对升序项的支持。 / Support for an ascending item.
    pub asc: bool,
    /// 对降序项的支持。 / Support for a descending item.
    pub desc: bool,
}

/// 排序结论。 / The sort verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SortByReport {
    /// `then` 拼接的结果。 / The result of `then`.
    pub combined: Vec<SortedItemRow>,
    /// `+` 拼接的结果（应与 `then` 相同）。 / The result of `+` (should equal `then`).
    pub added: Vec<SortedItemRow>,
    /// 追加升序后的结果。 / The result after appending ascending fields.
    pub appended_asc: Vec<SortedItemRow>,
    /// 追加降序后的结果。 / The result after appending descending fields.
    pub appended_desc: Vec<SortedItemRow>,
    /// 支持矩阵（4 行）。 / The support matrix (four rows).
    pub support: Vec<SupportRow>,
    /// 矩阵格数（8）。 / The number of cells (eight).
    pub support_cells: u64,
    /// `then` 与 `+` 等价（**是**）。 / Whether `then` and `+` agree (**yes**).
    pub then_equals_add: bool,
    /// 拼接只拼不整理：不排序、不去重（**是**）。 / Whether joining only concatenates (**yes**).
    pub join_keeps_order_and_duplicates: bool,
    /// 重复出现的字段路径。 / The paths that occur more than once.
    pub duplicate_paths: Vec<String>,
    /// `Auto` 与 `Always` 的支持回答完全一致（**是**）。 / Whether `Auto` answers exactly like `Always` (**yes**).
    pub auto_answers_like_always: bool,
    /// `Never` 恒为不支持（**是**）。 / Whether `Never` is always unsupported (**yes**).
    pub never_never_supports: bool,
    /// `OnlyAsc` 只看方向（**是**）。 / Whether `OnlyAsc` looks at the direction only (**yes**).
    pub only_asc_follows_direction: bool,
    /// 默认构造的排序项不带空值顺序（**是**）。 / Whether the default constructors leave nulls unspecified (**yes**).
    pub defaults_carry_no_nulls: bool,
}

/// 条目上限（信任边界）。 / The entry cap (a trust boundary).
const MAX_ENTRIES: usize = 4_096;

/// 走一遍排序描述符。非法输入是**错误**。 / Walk the sort descriptor.
pub fn verify_sort_by(request: &SortByRequest) -> Result<SortByReport> {
    let total = request.left.len() + request.right.len()
        + request.append_asc.len() + request.append_desc.len();
    if total > MAX_ENTRIES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "sort by failed validation: at most {MAX_ENTRIES} items are accepted")));
    }
    let left = build(&request.left)?;
    let right = build(&request.right)?;
    let combined = left.clone().then(right.clone());
    let added = left.clone() + right.clone();
    // 追加：`then_asc` / `then_desc` 逐个追。 / Append one by one through `then_asc` / `then_desc`.
    let mut appended_asc = left.clone();
    for path in &request.append_asc {
        appended_asc = appended_asc.then_asc(PropertyPath::parse(path));
    }
    let mut appended_desc = left.clone();
    for path in &request.append_desc {
        appended_desc = appended_desc.then_desc(PropertyPath::parse(path));
    }
    let combined_rows = rows(&combined);
    let added_rows = rows(&added);
    let then_equals_add = combined_rows == added_rows;
    // 拼接只拼不整理：结果就是左右两段按原顺序相接，重复项原样保留。
    // Joining only concatenates: the result is the two halves in order, duplicates included.
    let mut expected = rows(&left);
    expected.extend(rows(&right));
    let join_keeps_order_and_duplicates = combined_rows == expected;
    let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for row in &combined_rows {
        *seen.entry(row.path.clone()).or_insert(0) += 1;
    }
    let duplicate_paths: Vec<String> = seen.iter()
        .filter(|(_, count)| **count > 1).map(|(path, _)| path.clone()).collect();
    // 支持矩阵：4 策略 × 2 方向，穷举。 / The support matrix: four policies by two directions, exhaustively.
    let asc_item = SortItem::asc("probe");
    let desc_item = SortItem::desc("probe");
    let mut support = Vec::with_capacity(4);
    let mut support_cells = 0u64;
    for policy in [NullsOrderSupport::Auto, NullsOrderSupport::Always,
        NullsOrderSupport::Never, NullsOrderSupport::OnlyAsc]
    {
        let asc = policy.is_supported(&asc_item);
        let desc = policy.is_supported(&desc_item);
        support_cells += 2;
        support.push(SupportRow { policy: format!("{policy:?}"), asc, desc });
    }
    let answer = |name: &str, direction: &str| support.iter()
        .find(|row| row.policy == name)
        .map(|row| if direction == "asc" { row.asc } else { row.desc })
        .unwrap_or(false);
    let auto_answers_like_always = answer("Auto", "asc") == answer("Always", "asc")
        && answer("Auto", "desc") == answer("Always", "desc");
    let never_never_supports = !answer("Never", "asc") && !answer("Never", "desc");
    let only_asc_follows_direction = answer("OnlyAsc", "asc") && !answer("OnlyAsc", "desc");
    // 默认构造不带空值顺序；显式构造函数才带。 / The defaults leave nulls unspecified; the explicit ones carry it.
    let defaults_carry_no_nulls = SortItem::asc("a").nulls.is_none()
        && SortItem::desc("b").nulls.is_none()
        && SortBy::asc("a").items.iter().all(|item| item.nulls.is_none())
        && SortBy::desc_nulls("a", NullsOrder::NullsFirst).items[0].nulls
            == Some(NullsOrder::NullsFirst);
    Ok(SortByReport {
        combined: combined_rows,
        added: added_rows,
        appended_asc: rows(&appended_asc),
        appended_desc: rows(&appended_desc),
        support,
        support_cells,
        then_equals_add,
        join_keeps_order_and_duplicates,
        duplicate_paths,
        auto_answers_like_always,
        never_never_supports,
        only_asc_follows_direction,
        defaults_carry_no_nulls,
    })
}

/// 把字面量转成排序项。 / Turn literals into items.
fn build(specs: &[SortItemSpec]) -> Result<SortBy> {
    let mut items = Vec::with_capacity(specs.len());
    for spec in specs {
        if spec.path.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "sort by failed validation: an item needs a path"));
        }
        let direction = match spec.direction.as_str() {
            "asc" => SortDirection::Asc,
            "desc" => SortDirection::Desc,
            other => return Err(BridgeError::new("InvalidInput", &format!(
                "sort by failed validation: unknown sort direction `{other}`"))),
        };
        let nulls = match spec.nulls.as_deref() {
            None | Some("") => None,
            Some("first") => Some(NullsOrder::NullsFirst),
            Some("last") => Some(NullsOrder::NullsLast),
            Some(other) => return Err(BridgeError::new("InvalidInput", &format!(
                "sort by failed validation: unknown null ordering `{other}`"))),
        };
        items.push(SortItem::new(PropertyPath::parse(&spec.path), direction, nulls));
    }
    Ok(SortBy { items })
}

/// 把排序描述符摊平成行。 / Flatten a descriptor into rows.
fn rows(sort: &SortBy) -> Vec<SortedItemRow> {
    sort.items.iter().map(|item| SortedItemRow {
        path: item.path.value().to_owned(),
        direction: match item.direction {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        }.to_owned(),
        nulls: item.nulls.as_ref().map(|nulls| match nulls {
            NullsOrder::NullsFirst => "first",
            NullsOrder::NullsLast => "last",
        }.to_owned()),
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str, direction: &str, nulls: Option<&str>) -> SortItemSpec {
        SortItemSpec { path: path.to_owned(), direction: direction.to_owned(),
            nulls: nulls.map(|value| value.to_owned()) }
    }

    fn request(left: Vec<SortItemSpec>, right: Vec<SortItemSpec>) -> SortByRequest {
        SortByRequest { left, right, append_asc: Vec::new(), append_desc: Vec::new(),
            support_probe: String::new() }
    }

    #[test]
    fn the_support_matrix_is_four_by_two_and_auto_answers_like_always() {
        let report = verify_sort_by(&request(Vec::new(), Vec::new())).unwrap();
        assert_eq!(report.support_cells, 8);
        assert_eq!(report.support.len(), 4);
        let row = |policy: &str| report.support.iter().find(|row| row.policy == policy).unwrap();
        // `Auto` 与 `Always` 回答完全一致。 / `Auto` answers exactly like `Always`.
        assert!(row("Auto").asc && row("Auto").desc);
        assert!(row("Always").asc && row("Always").desc);
        assert!(report.auto_answers_like_always);
        // `Never` 恒不支持。 / `Never` never supports.
        assert!(!row("Never").asc && !row("Never").desc);
        assert!(report.never_never_supports);
        // `OnlyAsc` 只看方向。 / `OnlyAsc` looks at the direction.
        assert!(row("OnlyAsc").asc && !row("OnlyAsc").desc);
        assert!(report.only_asc_follows_direction);
    }

    #[test]
    fn joining_only_concatenates_and_keeps_duplicates() {
        let report = verify_sort_by(&request(
            vec![item("created_at", "desc", Some("last")), item("id", "asc", None)],
            vec![item("id", "asc", None), item("name", "asc", None)])).unwrap();
        assert_eq!(report.combined.len(), 4);
        assert!(report.then_equals_add);
        assert!(report.join_keeps_order_and_duplicates);
        // 顺序就是左右相接。 / The order is the halves joined.
        assert_eq!(report.combined[0].path, "created_at");
        assert_eq!(report.combined[0].nulls.as_deref(), Some("last"));
        assert_eq!(report.combined[1].path, "id");
        assert_eq!(report.combined[2].path, "id");
        assert_eq!(report.combined[3].path, "name");
        // 重复路径被原样保留，只是被列出来。 / Duplicates are kept and merely listed.
        assert_eq!(report.duplicate_paths, vec!["id".to_owned()]);
    }

    #[test]
    fn appending_adds_to_the_end_without_dedupe() {
        let mut document = request(vec![item("id", "asc", None)], Vec::new());
        document.append_asc = vec!["name".to_owned(), "id".to_owned()];
        document.append_desc = vec!["total".to_owned()];
        let report = verify_sort_by(&document).unwrap();
        let asc: Vec<&str> = report.appended_asc.iter().map(|row| row.path.as_str()).collect();
        assert_eq!(asc, vec!["id", "name", "id"]);
        let desc: Vec<&str> = report.appended_desc.iter().map(|row| row.path.as_str()).collect();
        assert_eq!(desc, vec!["id", "total"]);
        assert_eq!(report.appended_desc[1].direction, "desc");
    }

    #[test]
    fn the_defaults_carry_no_null_ordering() {
        let report = verify_sort_by(&request(Vec::new(), Vec::new())).unwrap();
        assert!(report.defaults_carry_no_nulls);
        // 显式构造才带。 / Only the explicit constructor carries it.
        let report = verify_sort_by(&request(vec![item("a", "asc", Some("first"))],
            Vec::new())).unwrap();
        assert_eq!(report.combined[0].nulls.as_deref(), Some("first"));
        let report = verify_sort_by(&request(vec![item("a", "desc", None)], Vec::new())).unwrap();
        assert_eq!(report.combined[0].nulls, None);
    }

    #[test]
    fn bad_input_is_refused() {
        let blank = request(vec![item("", "asc", None)], Vec::new());
        assert!(verify_sort_by(&blank).unwrap_err().message.contains("needs a path"));
        let wrong = request(vec![item("a", "up", None)], Vec::new());
        assert!(verify_sort_by(&wrong).unwrap_err().message.contains("unknown sort direction"));
        let nulls = request(vec![item("a", "asc", Some("middle"))], Vec::new());
        assert!(verify_sort_by(&nulls).unwrap_err().message.contains("unknown null ordering"));
        let many = request(vec![item("a", "asc", None); MAX_ENTRIES + 1], Vec::new());
        assert!(verify_sort_by(&many).unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_sort_by(&request(vec![item("id", "asc", Some("last"))],
            Vec::new())).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"supportCells\""), "{json}");
        assert!(json.contains("\"autoAnswersLikeAlways\""), "{json}");
        assert!(json.contains("\"duplicatePaths\""), "{json}");
        assert!(!json.contains("support_cells"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<SortByReport>(&copy.to_string()).is_err());
    }
}
