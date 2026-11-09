//! 续挖第 12 单：表达式仓储的两个默认方法的真实走向（`persistence/expression/repository_api.rs`）
//! Continuation, delivery 12: where the expression repository's two default methods really go.
//!
//! `ExpressionRepository` 只要求实现四个方法（`find_with_options` / `count` / `update` / `delete`），另外两个是默认实现。
//! 默认实现的**走向**才是这一单要钉住的东西，因为它决定了后端写错时会发生什么：
//!
//! 1. **`find` 就是"用默认选项调 `find_with_options`"**。`RepositoryQuery::default()` 的 `limit` / `offset` /
//!    `sort_by` **全是 `None`**——也就是"没有上限、没有偏移、没有排序"，它会**把全部命中行交出去**。
//!    所以"默认查询有隐含上限"是想当然；上限只能由后端自己加，而一旦后端加了，`find` 就会静默截断。
//! 2. **`exists` 走 `count`，不碰 `find`**。所以一个自相矛盾的后端（`count()` 说 0、`find()` 却给出行）在
//!    `exists` 眼里就是"不存在"。这不是缺陷而是定义：`exists` 的可信度完全取决于 `count` 的实现。
//! 3. **错误原样穿透**：两个默认方法都是 `?` 转发，不做包装、不吞错，`Self::Error` 从必须方法直接冒出来。
//! 4. **`RepositoryApi` 是空标记 trait**：零方法、零约束，任何类型都能 `impl` 一行接上，不带任何行为。
//!
//! `ExpressionRepository` only requires four methods (`find_with_options` / `count` / `update` / `delete`); the other two
//! are defaults, and **where they go** is what this delivery pins down, because it decides what happens when a backend is
//! written wrong. **`find` is just `find_with_options` with default options**, and `RepositoryQuery::default()` leaves
//! `limit` / `offset` / `sort_by` **all `None`** — no cap, no offset, no ordering — so it **hands back every matching row**;
//! "the default query has an implicit limit" is an assumption, and if a backend adds one, `find` truncates silently.
//! **`exists` goes through `count` and never touches `find`**, so a self-contradictory backend (`count()` says 0 while
//! `find()` returns rows) is "absent" as far as `exists` is concerned — by definition, not by defect: `exists` is only as
//! trustworthy as `count`. **Errors pass through untouched**, both defaults using `?` with no wrapping and no swallowing.
//! And **`RepositoryApi` is an empty marker trait**: no methods, no bounds, any type connects with a one-line impl.

use ospf_rust_framework::persistence::expression::{
    ExpressionRepository, RepositoryApi, RepositoryQuery,
};
use ospf_rust_math::symbol::{BooleanExpression, ExpressionValue};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一行的内容（只需要可比较，不需要真字段）。 / A row's payload (only needs to be comparable).
pub type Row = String;

/// 一个自相矛盾也能跑的测试仓储：`count` 报什么就是什么，`find_with_options` 给什么就是什么。
/// A test repository that stays self-contradictory on purpose: `count` reports one thing and `find_with_options` another.
struct MemoryRepository {
    count: u64,
    rows: Vec<Row>,
    fail: Option<String>,
}

impl MemoryRepository {
    /// 统一的失败出口。 / The single failure exit.
    fn fault(&self) -> std::result::Result<(), String> {
        match &self.fail {
            Some(reason) => Err(reason.clone()),
            None => Ok(()),
        }
    }
}

impl ExpressionRepository<Row> for MemoryRepository {
    type Error = String;

    fn find_with_options(&self, _where_expr: &BooleanExpression<ExpressionValue>,
        options: &RepositoryQuery) -> std::result::Result<Vec<Row>, Self::Error>
    {
        self.fault()?;
        // **只有**后端显式给了 `limit` 才截断：默认查询没有上限。
        // Truncate **only** when the backend was given a `limit`; the default query has no cap.
        match options.limit {
            Some(limit) => Ok(self.rows.iter().take(limit).cloned().collect()),
            None => Ok(self.rows.clone()),
        }
    }

    fn count(&self, _where_expr: &BooleanExpression<ExpressionValue>)
        -> std::result::Result<u64, Self::Error>
    {
        self.fault()?;
        Ok(self.count)
    }

    fn update(&self, _where_expr: &BooleanExpression<ExpressionValue>,
        _assignments: &ospf_rust_framework::persistence::expression::UpdateAssignments<
            ExpressionValue>) -> std::result::Result<u64, Self::Error>
    {
        self.fault()?;
        // 更新 0 行：这一单不连后端，只看接口走向。 / Updates nothing: this delivery only cares where the calls go.
        Ok(0)
    }

    fn delete(&self, _where_expr: &BooleanExpression<ExpressionValue>)
        -> std::result::Result<u64, Self::Error>
    {
        self.fault()?;
        Ok(0)
    }
}

/// 一个空标记 trait 的实现：零方法。 / An empty marker trait impl: zero methods.
struct NoopRepository;
impl RepositoryApi for NoopRepository {}

/// 一条操作的结论。 / One operation's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryCallRow {
    /// 操作名。 / The operation name.
    pub operation: String,
    /// 返回的行数（`find` / `findWithOptions` 有）。 / The number of rows returned (`find*` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub row_count: Option<u32>,
    /// 返回的计数（`count` 有）。 / The returned count (`count` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub count: Option<u64>,
    /// 返回的真值（`exists` / `repositoryApi` 有）。 / The boolean verdict (`exists` / `repositoryApi`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub flag: Option<bool>,
    /// 是否返回了错误。 / Whether the call returned an error.
    pub errored: bool,
    /// 错误文本（原样）。 / The error text, unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
}

/// 仓储走向请求。 / The repository-routing request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryCallRequest {
    /// `count()` 报的数（**故意与 `rows` 无关**）。 / What `count()` reports (**deliberately unrelated** to `rows`).
    pub count: u64,
    /// `find_with_options` 的行（**故意与 `count` 无关**）。 / The rows `find_with_options` returns.
    #[serde(default)]
    pub rows: Vec<String>,
    /// 后端显式上限（`None` 表示默认查询，不截断）。 / An explicit backend cap (`None` = the default, no truncation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<u32>,
    /// 让后端在必须方法里失败（`None` 表示不失败）。 / Make the required methods fail (`None` = no failure).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fail: Option<String>,
    /// 要调用的操作：`find` / `findWithOptions` / `count` / `exists` / `update` / `delete`。
    /// The operations to call.
    #[serde(default)]
    pub operations: Vec<String>,
}

/// 仓储走向结论。 / The repository-routing verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryCallReport {
    /// 逐条结论。 / Each operation's verdict.
    pub calls: Vec<RepositoryCallRow>,
    /// 默认查询是否带上限（**否**）。 / Whether the default query carries a cap (**no**).
    pub default_query_has_limit: bool,
    /// `exists` 是否与 `find` 矛盾（`count = 0` 但 `find` 有行时为真）。 / Whether `exists` contradicts `find`.
    pub exists_contradicts_find: bool,
    /// 空标记 trait 是否零方法即可实现（**是**）。 / Whether the empty marker trait needs zero methods (**yes**).
    pub marker_is_empty: bool,
    /// 操作词表（桥接定义）。 / The operation vocabulary (bridge-defined).
    pub operation_vocabulary: Vec<String>,
}

/// 操作条数上限（信任边界）。 / The operation cap (a trust boundary).
const MAX_OPERATIONS: usize = 1_024;

/// 走一遍仓储接口并回报两个默认方法去了哪里。非法输入是**错误**。
/// Walk the repository interface and report where the two default methods go.
pub fn verify_repository_calls(request: &RepositoryCallRequest) -> Result<RepositoryCallReport> {
    if request.operations.len() > MAX_OPERATIONS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "repository calls failed validation: at most {MAX_OPERATIONS} operations are accepted")));
    }
    let repository = MemoryRepository {
        count: request.count,
        rows: request.rows.clone(),
        fail: request.fail.clone(),
    };
    let where_expr = BooleanExpression::<ExpressionValue>::true_constant();
    let mut calls = Vec::with_capacity(request.operations.len());
    for operation in &request.operations {
        let mut row = RepositoryCallRow { operation: operation.clone(), row_count: None,
            count: None, flag: None, errored: false, error: None };
        match operation.as_str() {
            "find" => match repository.find(&where_expr) {
                Ok(rows) => row.row_count = Some(count(rows.len())),
                Err(reason) => { row.errored = true; row.error = Some(reason); }
            },
            "findWithOptions" => {
                // 只有这里才带上限，用来对比 `find` 的默认查询。 / Only this path carries a cap, to contrast with `find`.
                let mut options = RepositoryQuery::new();
                if let Some(limit) = request.limit {
                    options = options.with_limit(usize::try_from(limit).unwrap_or(usize::MAX));
                }
                match repository.find_with_options(&where_expr, &options) {
                    Ok(rows) => row.row_count = Some(count(rows.len())),
                    Err(reason) => { row.errored = true; row.error = Some(reason); }
                }
            }
            "count" => match repository.count(&where_expr) {
                Ok(value) => row.count = Some(value),
                Err(reason) => { row.errored = true; row.error = Some(reason); }
            },
            "exists" => match repository.exists(&where_expr) {
                Ok(value) => row.flag = Some(value),
                Err(reason) => { row.errored = true; row.error = Some(reason); }
            },
            "update" => match repository.update(&where_expr,
                &ospf_rust_framework::persistence::expression::UpdateAssignments::empty())
            {
                Ok(value) => row.count = Some(value),
                Err(reason) => { row.errored = true; row.error = Some(reason); }
            },
            "delete" => match repository.delete(&where_expr) {
                Ok(value) => row.count = Some(value),
                Err(reason) => { row.errored = true; row.error = Some(reason); }
            },
            other => return Err(BridgeError::new("InvalidInput", &format!(
                "repository calls failed validation: unknown operation `{other}`"))),
        }
        calls.push(row);
    }
    // 直接用默认选项再问一次 `find`：少给行才说明默认查询带了上限。
    // Call `find` once more with default options: only fewer rows would mean the default query has a cap.
    let default_query_has_limit = match repository.find(&where_expr) {
        Ok(rows) => rows.len() < request.rows.len(),
        Err(_) => false,
    };
    // `exists` 与 `find` 是否互相矛盾：`count` 说 0 而 `find` 有行。 / Whether `exists` and `find` disagree.
    let exists_contradicts_find = request.count == 0 && !request.rows.is_empty();
    // 空标记 trait 真的被实现了才为真。 / True only because the marker really was implemented.
    let marker = NoopRepository;
    let marker_is_empty = std::mem::size_of_val(&marker) == 0 && !std::mem::needs_drop::<NoopRepository>();
    let _: &dyn RepositoryApi = &marker;
    Ok(RepositoryCallReport {
        calls,
        // 默认查询**不带**上限：`find` 给出的行数与后端行数一致才说明没截断。
        // The default query carries **no** cap: `find` matching the backend's row count is the evidence.
        default_query_has_limit,
        exists_contradicts_find,
        marker_is_empty,
        operation_vocabulary: vec!["find".to_owned(), "findWithOptions".to_owned(),
            "count".to_owned(), "exists".to_owned(), "update".to_owned(), "delete".to_owned()],
    })
}

/// 计数转 `u32`。 / A count as `u32`.
fn count(value: usize) -> u32 { u32::try_from(value).unwrap_or(u32::MAX) }

#[cfg(test)]
mod tests {
    use super::*;

    fn request(count: u64, rows: &[&str]) -> RepositoryCallRequest {
        RepositoryCallRequest { count, rows: rows.iter().map(|row| (*row).to_owned()).collect(),
            limit: None, fail: None, operations: Vec::new() }
    }

    fn with_operations(mut request: RepositoryCallRequest, names: &[&str])
        -> RepositoryCallRequest
    {
        request.operations = names.iter().map(|name| (*name).to_owned()).collect();
        request
    }

    fn row<'a>(report: &'a RepositoryCallReport, name: &str) -> &'a RepositoryCallRow {
        report.calls.iter().find(|row| row.operation == name)
            .unwrap_or_else(|| panic!("operation {name}"))
    }

    #[test]
    fn find_is_find_with_options_and_the_default_query_has_no_cap() {
        // `find` 走默认选项：即使给了后端上限，它也**不截断**——默认查询没有上限。
        // `find` uses default options: even with a backend cap, it does **not** truncate.
        let mut document = request(2, &["a", "b", "c"]);
        document.limit = Some(1);
        let document = with_operations(document, &["find", "findWithOptions"]);
        let report = verify_repository_calls(&document).unwrap();
        assert_eq!(row(&report, "find").row_count, Some(3));
        assert_eq!(row(&report, "findWithOptions").row_count, Some(1));
        assert!(!report.default_query_has_limit);
        // 空仓储：`find` 给零行，也不报错。 / An empty repository returns nothing and no error.
        let report = verify_repository_calls(&with_operations(request(0, &[]), &["find"])).unwrap();
        assert_eq!(row(&report, "find").row_count, Some(0));
    }

    #[test]
    fn exists_goes_through_count_and_not_through_find() {
        // 自相矛盾的后端：`count()` 说 0，`find()` 却有三行。 / A contradictory backend: count 0, three rows.
        let report = verify_repository_calls(&with_operations(request(0, &["a", "b", "c"]),
            &["count", "exists", "find"])).unwrap();
        assert_eq!(row(&report, "count").count, Some(0));
        assert_eq!(row(&report, "find").row_count, Some(3));
        // `exists` 跟着 `count` 走 → 假。 / `exists` follows `count` → false.
        assert_eq!(row(&report, "exists").flag, Some(false));
        assert!(report.exists_contradicts_find);
        // 反过来：`count()` 有数但 `find()` 空 → `exists` 为真。 / The other way round: exists is true.
        let report = verify_repository_calls(&with_operations(request(5, &[]),
            &["exists", "find"])).unwrap();
        assert_eq!(row(&report, "exists").flag, Some(true));
        assert_eq!(row(&report, "find").row_count, Some(0));
        assert!(!report.exists_contradicts_find);
    }

    #[test]
    fn errors_pass_through_untouched() {
        let mut document = request(1, &["a"]);
        document.fail = Some("backend is down".to_owned());
        let document = with_operations(document, &["find", "count", "exists", "update", "delete"]);
        let report = verify_repository_calls(&document).unwrap();
        for name in ["find", "count", "exists", "update", "delete"] {
            let row = row(&report, name);
            assert!(row.errored, "{name}");
            // 原样穿透，不包装、不吞。 / Passed through unchanged, unwrapped and unswallowed.
            assert_eq!(row.error.as_deref(), Some("backend is down"), "{name}");
            assert_eq!(row.row_count, None, "{name}");
            assert_eq!(row.count, None, "{name}");
            assert_eq!(row.flag, None, "{name}");
        }
    }

    #[test]
    fn the_marker_trait_is_empty_and_the_vocabulary_is_six() {
        let report = verify_repository_calls(&with_operations(request(1, &["a"]),
            &["count", "update", "delete"])).unwrap();
        // 空标记 trait：零大小、无析构，`&dyn` 直接接上。 / An empty marker: zero-sized, no drop, `&dyn` works.
        assert!(report.marker_is_empty);
        assert_eq!(report.operation_vocabulary.len(), 6);
        // `update` / `delete` 这一单只回报行数。 / `update` / `delete` only report their row counts here.
        assert_eq!(row(&report, "update").count, Some(0));
        assert_eq!(row(&report, "delete").count, Some(0));
    }

    #[test]
    fn bad_input_is_refused() {
        let report = verify_repository_calls(&with_operations(request(0, &[]), &["peek"]));
        assert!(report.unwrap_err().message.contains("unknown operation"));
        let mut many = request(0, &[]);
        many.operations = vec!["count".to_owned(); MAX_OPERATIONS + 1];
        assert!(verify_repository_calls(&many).unwrap_err().message.contains("at most"));
        // 空操作列表合法。 / An empty operation list is legal.
        let report = verify_repository_calls(&request(0, &[])).unwrap();
        assert!(report.calls.is_empty());
    }
}
