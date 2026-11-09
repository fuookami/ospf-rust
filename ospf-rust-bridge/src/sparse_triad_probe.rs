//! 续挖第 61 单：稀疏三角模型的**手写语义**——`add` 从不合并、重复 ID 只覆盖索引、`set_bounds` 不校验空域
//! （`model/intermediate/basic_linear_triad_model.rs` 的 `SparseVector` / `BasicLinearTriadModel`）
//! Continuation, delivery 61: the sparse triad model's **hand-written semantics** — `add` never merges, a duplicate ID only
//! overwrites the index, and `set_bounds` accepts an empty domain (`SparseVector` / `BasicLinearTriadModel`).
//!
//! §11.14 点名 `model/intermediate` 的可编码部分。桥接层一直**用**这两个结构（`compile_linear`、弹性构建器），
//! 但从没单独量过它们的手写语义。七条实测（零求解器）：
//!
//! 1. **`SparseVector::add` 从不合并、也不剔除零**（`add_never_merges_or_drops_zeros()`）：重复索引与 `0.0`
//!    都原样留在表里，`len()` 是 **entries 条数**，不是非零元个数。
//! 2. **没有边界的 token 会变成无穷界**（`a_token_without_bounds_becomes_infinite()`）：
//!    `add_variable` 用 `lower_bound().unwrap_or(NEG_INFINITY)`，所以默认拿到 `(-inf, inf)`。
//! 3. **显式界完全覆盖 token 的界**（`explicit_bounds_override_the_token()`）：`add_variable_with_bounds`
//!    不看 token 自己的声明，直接写入给定值。
//! 4. **重复 ID 只覆盖索引**（`a_duplicate_id_overwrites_the_index_only()`）：变量数 +1，但
//!    `find_variable_index` 指向**后加的那个**——先加的那个变成不可达（只能按索引访问）。
//! 5. **行名默认是 `c{行号}`**（`constraint_names_default_to_c()`）：`add_constraint` 自动生成名字，行号自增。
//! 6. **`set_bounds` 接受空域**（`set_bounds_accepts_an_empty_domain()`）：`lb > ub` 不报错，`get_bounds`
//!    原样返回 `(5, 1)`。
//! 7. **`set_bounds` 越界静默忽略**（`set_bounds_out_of_range_is_silently_ignored()`）：不 panic、不改任何值，
//!    而 `get_bounds` / `find_variable` 越界或未命中返回 `None`。
//!
//! The §11.14 audit names the codable part of `model/intermediate`. The bridge has always **used** these two structures
//! (`compile_linear`, the elastic builders) but never measured their hand-written semantics on their own. **`SparseVector::add`
//! never merges and never drops zeros** (`add_never_merges_or_drops_zeros()`): duplicate indices and `0.0` stay in the table, so
//! `len()` counts **entries**, not non-zeros. **A token without bounds becomes unbounded**
//! (`a_token_without_bounds_becomes_infinite()`), because `add_variable` uses
//! `lower_bound().unwrap_or(NEG_INFINITY)`. **Explicit bounds fully override the token's** 
//! (`explicit_bounds_override_the_token()`). **A duplicate ID only overwrites the index**
//! (`a_duplicate_id_overwrites_the_index_only()`): the variable count grows while `find_variable_index` points at the **later**
//! one, leaving the earlier copy reachable only by index. **Row names default to `c{index}`**
//! (`constraint_names_default_to_c()`). **`set_bounds` accepts an empty domain** (`set_bounds_accepts_an_empty_domain()`):
//! `lb > ub` is stored and returned verbatim. And **an out-of-range `set_bounds` is silently ignored**
//! (`set_bounds_out_of_range_is_silently_ignored()`), while `get_bounds` and `find_variable` return `None` instead.

use ospf_rust_core::model::intermediate::{
    BasicLinearTriadModel, SparseVector,
};
use ospf_rust_core::token::Token;
use ospf_rust_core::variable::{ContinuousVariableItem, VariableId, VariableType};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个稀疏条目。 / One sparse entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SparseEntrySpec {
    pub index: u64,
    pub value: f64,
}

/// 一个界设置。 / One bound assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SparseBoundsSpec {
    pub index: u64,
    pub lower: f64,
    pub upper: f64,
}

/// 稀疏三角模型探针请求。 / The sparse triad probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SparseTriadRequest {
    /// 一个稀疏行里的条目。 / The entries of one sparse row.
    #[serde(default)]
    pub entries: Vec<SparseEntrySpec>,
    /// 是否加两个同 ID 变量。 / Whether to add two variables with the same ID.
    #[serde(default)]
    pub duplicate_variable: bool,
    /// 追加的约束行数。 / How many constraint rows to append.
    #[serde(default)]
    pub constraint_rows: Option<u64>,
    #[serde(default)]
    pub set_bounds: Option<SparseBoundsSpec>,
}

/// 稀疏三角模型探针结论。 / The sparse triad probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SparseTriadReport {
    /// 稀疏行的 entries 条数。 / The entry count of the sparse row.
    pub entry_count: u64,
    /// 稀疏行的索引序列。 / The entry indices.
    pub entry_indices: Vec<u64>,
    /// 变量数。 / The variable count.
    pub variables: u64,
    /// 行名序列。 / The row names.
    pub constraint_names: Vec<String>,
    /// 每个变量的界。 / Each variable's bounds.
    pub bounds: Vec<SparseBoundsRow>,
    /// 第一个变量按 ID 查到的索引。 / The index found by the first variable's ID.
    #[serde(default)]
    pub first_index: Option<u64>,
    /// 一个不存在的 ID 的查询结果（应为 `None`）。 / A lookup for a missing ID (`null` expected).
    #[serde(default)]
    pub missing_index: Option<u64>,
    /// 越界 `getBounds` 的结果（应为 `None`）。 / An out-of-range `getBounds` (`null` expected).
    #[serde(default)]
    pub out_of_range_bounds: Option<SparseBoundsRow>,
    /// `add` 从不合并、也不剔除零（**是**）。 / Whether `add` never merges or drops zeros (**yes**).
    pub add_never_merges_or_drops_zeros: bool,
    /// 无边界的 token 变成无穷界（**是**）。 / Whether a boundless token becomes unbounded (**yes**).
    pub a_token_without_bounds_becomes_infinite: bool,
    /// 显式界覆盖 token 的界（**是**）。 / Whether explicit bounds override the token's (**yes**).
    pub explicit_bounds_override_the_token: bool,
    /// 重复 ID 只覆盖索引（**是**）。 / Whether a duplicate ID only overwrites the index (**yes**).
    pub a_duplicate_id_overwrites_the_index_only: bool,
    /// 行名默认 `c{行号}`（**是**）。 / Whether row names default to `c{index}` (**yes**).
    pub constraint_names_default_to_c: bool,
    /// `setBounds` 接受空域（**是**）。 / Whether `setBounds` accepts an empty domain (**yes**).
    pub set_bounds_accepts_an_empty_domain: bool,
    /// 越界 `setBounds` 静默忽略（**是**）。 / Whether an out-of-range `setBounds` is ignored (**yes**).
    pub set_bounds_out_of_range_is_silently_ignored: bool,
}

/// 一对界，以文本表示（无穷界写成 `-inf` / `inf`，因为非有限 `f64` 不能跨 JSON）。
/// One pair of bounds as text (`-inf` / `inf` for the infinite ones, since a non-finite `f64` cannot cross JSON).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SparseBoundsRow {
    pub lower: String,
    pub upper: String,
}

/// 界文本。 / Bound text.
fn bound_text(value: f64) -> String {
    if value == f64::NEG_INFINITY { "-inf".to_owned() }
    else if value == f64::INFINITY { "inf".to_owned() }
    else { format!("{value}") }
}

/// 把一对界转成 row。 / Turn a bound pair into a row.
fn bounds_row(lower: f64, upper: f64) -> SparseBoundsRow {
    SparseBoundsRow { lower: bound_text(lower), upper: bound_text(upper) }
}

/// 造一个变量 token。 / Build one variable token.
fn token_of(id: &str, group: usize) -> Token<f64> {
    Token::from_generic(ContinuousVariableItem::create(VariableId::standalone(group), id), group)
}

/// 走一遍稀疏三角模型。非法输入是**错误**。 / Walk the sparse triad model.
pub fn verify_sparse_triad(request: &SparseTriadRequest) -> Result<SparseTriadReport> {
    if request.entries.len() > 64 {
        return Err(BridgeError::new("InvalidInput",
            "sparse triad failed validation: at most 64 entries are accepted"));
    }
    let rows = usize::try_from(request.constraint_rows.unwrap_or(2)).unwrap_or(2);
    if rows > 16 {
        return Err(BridgeError::new("InvalidInput",
            "sparse triad failed validation: at most 16 constraint rows are accepted"));
    }
    for entry in &request.entries {
        if !entry.value.is_finite() {
            return Err(BridgeError::new("InvalidInput",
                "sparse triad failed validation: entry values must be finite"));
        }
    }
    let mut row = SparseVector::<f64>::new();
    for entry in &request.entries {
        row.add(usize::try_from(entry.index).unwrap_or(usize::MAX), entry.value);
    }
    let entry_count = row.len() as u64;
    let entry_indices = row.entries.iter().map(|(index, _)| *index as u64).collect::<Vec<_>>();
    let mut model = BasicLinearTriadModel::new("sparse_probe");
    // 第一个变量用 token 自己的界（没有声明 → 无穷界）。 
    // The first variable uses the token's own bounds (none declared → unbounded).
    model.add_variable(token_of("x", 0));
    if request.duplicate_variable {
        model.add_variable(token_of("x", 0));
    }
    for index in 0..rows {
        model.add_constraint(row.clone(), index as f64);
    }
    if let Some(bounds) = &request.set_bounds {
        model.set_bounds(usize::try_from(bounds.index).unwrap_or(usize::MAX), bounds.lower,
            bounds.upper);
    }
    let variables = model.num_variables() as u64;
    let constraint_names = model.constraint_names.clone();
    let bounds = (0..model.num_variables()).filter_map(|index| model.get_bounds(index)
        .map(|(lower, upper)| bounds_row(lower, upper))).collect::<Vec<_>>();
    let first_index = model.find_variable_index(VariableId::standalone(0)).map(|index| index as u64);
    let missing_index = model.find_variable_index(VariableId::standalone(99))
        .map(|index| index as u64);
    let out_of_range_bounds = model.get_bounds(999).map(|(lower, upper)| bounds_row(lower, upper));
    let findings = canonical_findings();
    Ok(SparseTriadReport { entry_count, entry_indices, variables, constraint_names, bounds,
        first_index, missing_index, out_of_range_bounds,
        add_never_merges_or_drops_zeros: findings.add_never_merges_or_drops_zeros,
        a_token_without_bounds_becomes_infinite: findings.a_token_without_bounds_becomes_infinite,
        explicit_bounds_override_the_token: findings.explicit_bounds_override_the_token,
        a_duplicate_id_overwrites_the_index_only:
            findings.a_duplicate_id_overwrites_the_index_only,
        constraint_names_default_to_c: findings.constraint_names_default_to_c,
        set_bounds_accepts_an_empty_domain: findings.set_bounds_accepts_an_empty_domain,
        set_bounds_out_of_range_is_silently_ignored:
            findings.set_bounds_out_of_range_is_silently_ignored })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    add_never_merges_or_drops_zeros: bool,
    a_token_without_bounds_becomes_infinite: bool,
    explicit_bounds_override_the_token: bool,
    a_duplicate_id_overwrites_the_index_only: bool,
    constraint_names_default_to_c: bool,
    set_bounds_accepts_an_empty_domain: bool,
    set_bounds_out_of_range_is_silently_ignored: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) `add` 不合并重复索引、不剔除零。 / `add` neither merges duplicate indices nor drops zeros.
    let add_never_merges_or_drops_zeros = {
        let mut row = SparseVector::<f64>::new();
        row.add(3, 1.0);
        row.add(3, 2.0);
        row.add(0, 0.0);
        row.len() == 3
            && row.entries == vec![(3usize, 1.0f64), (3usize, 2.0f64), (0usize, 0.0f64)]
            && !row.is_empty()
    };
    // 2) 没有边界的 token → 无穷界。 / A boundless token becomes unbounded.
    let a_token_without_bounds_becomes_infinite = {
        let mut model = BasicLinearTriadModel::new("bounds");
        model.add_variable(token_of("x", 0));
        model.get_bounds(0).is_some_and(|(lower, upper)| lower == f64::NEG_INFINITY
            && upper == f64::INFINITY)
    };
    // 3) 显式界覆盖 token 的界。 / Explicit bounds override the token's.
    let explicit_bounds_override_the_token = {
        let mut model = BasicLinearTriadModel::new("explicit");
        model.add_variable_with_bounds(token_of("x", 0), 1.5, 2.5, VariableType::Continuous);
        model.get_bounds(0) == Some((1.5, 2.5))
    };
    // 4) 重复 ID 只覆盖索引：变量数 +1，索引指向后加的那个。 
    // 4) A duplicate ID only overwrites the index: the count grows and the index points at the later one.
    let a_duplicate_id_overwrites_the_index_only = {
        let mut model = BasicLinearTriadModel::new("duplicate");
        model.add_variable(token_of("x", 0));
        let first = model.add_variable(token_of("x", 0));
        model.num_variables() == 2
            && model.find_variable_index(VariableId::standalone(0)) == Some(first)
            && first == 1
    };
    // 5) 行名默认 `c{行号}`。 / Row names default to `c{index}`.
    let constraint_names_default_to_c = {
        let mut model = BasicLinearTriadModel::new("names");
        model.add_variable(token_of("x", 0));
        let first = model.add_constraint(SparseVector::new(), 0.0);
        let second = model.add_constraint(SparseVector::new(), 1.0);
        first == 0 && second == 1 && model.constraint_names == vec!["c0".to_owned(),
            "c1".to_owned()] && model.num_constraints() == 2
    };
    // 6) `set_bounds` 接受空域（`lb > ub`），原样存回。 
    // 6) `set_bounds` accepts an empty domain (`lb > ub`) and stores it verbatim.
    let set_bounds_accepts_an_empty_domain = {
        let mut model = BasicLinearTriadModel::new("empty-domain");
        model.add_variable(token_of("x", 0));
        model.set_bounds(0, 5.0, 1.0);
        model.get_bounds(0) == Some((5.0, 1.0))
    };
    // 7) 越界 `set_bounds` 静默忽略；`get_bounds` / `find_variable` 返回 `None`。 
    // 7) An out-of-range `set_bounds` is ignored; `get_bounds` / `find_variable` return `None`.
    let set_bounds_out_of_range_is_silently_ignored = {
        let mut model = BasicLinearTriadModel::new("out-of-range");
        model.add_variable(token_of("x", 0));
        model.set_bounds(0, 1.0, 2.0);
        model.set_bounds(9, 8.0, 9.0);
        model.get_bounds(0) == Some((1.0, 2.0))
            && model.get_bounds(9).is_none()
            && model.find_variable(VariableId::standalone(42)).is_none()
            && model.num_variables() == 1
    };
    Findings { add_never_merges_or_drops_zeros, a_token_without_bounds_becomes_infinite,
        explicit_bounds_override_the_token, a_duplicate_id_overwrites_the_index_only,
        constraint_names_default_to_c, set_bounds_accepts_an_empty_domain,
        set_bounds_out_of_range_is_silently_ignored }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SparseTriadRequest {
        SparseTriadRequest {
            entries: vec![SparseEntrySpec { index: 3, value: 1.0 },
                SparseEntrySpec { index: 3, value: 2.0 },
                SparseEntrySpec { index: 0, value: 0.0 }],
            duplicate_variable: false, constraint_rows: Some(2), set_bounds: None }
    }

    #[test]
    fn the_sparse_row_keeps_every_entry() {
        let report = verify_sparse_triad(&request()).unwrap();
        assert_eq!(report.entry_count, 3);
        assert_eq!(report.entry_indices, vec![3, 3, 0]);
        assert_eq!(report.variables, 1);
        assert_eq!(report.constraint_names, vec!["c0".to_owned(), "c1".to_owned()]);
        assert_eq!(report.bounds, vec![SparseBoundsRow { lower: "-inf".to_owned(),
            upper: "inf".to_owned() }]);
        assert_eq!(report.first_index, Some(0));
        assert_eq!(report.missing_index, None);
        assert_eq!(report.out_of_range_bounds, None);
        assert!(report.add_never_merges_or_drops_zeros);
        assert!(report.a_token_without_bounds_becomes_infinite);
        assert!(report.explicit_bounds_override_the_token);
        assert!(report.a_duplicate_id_overwrites_the_index_only);
        assert!(report.constraint_names_default_to_c);
        assert!(report.set_bounds_accepts_an_empty_domain);
        assert!(report.set_bounds_out_of_range_is_silently_ignored);
    }

    #[test]
    fn a_duplicate_id_leaves_two_variables_but_one_index() {
        let report = verify_sparse_triad(&SparseTriadRequest { duplicate_variable: true,
            ..request() }).unwrap();
        assert_eq!(report.variables, 2);
        assert_eq!(report.bounds.len(), 2);
        // 索引指向后加的那个。 / The index points at the later variable.
        assert_eq!(report.first_index, Some(1));
    }

    #[test]
    fn an_empty_domain_is_stored_verbatim() {
        let report = verify_sparse_triad(&SparseTriadRequest {
            set_bounds: Some(SparseBoundsSpec { index: 0, lower: 5.0, upper: 1.0 }),
            ..request() }).unwrap();
        assert_eq!(report.bounds, vec![SparseBoundsRow { lower: "5".to_owned(),
            upper: "1".to_owned() }]);
    }

    #[test]
    fn an_out_of_range_set_bounds_changes_nothing() {
        let report = verify_sparse_triad(&SparseTriadRequest {
            set_bounds: Some(SparseBoundsSpec { index: 9, lower: 8.0, upper: 9.0 }),
            ..request() }).unwrap();
        assert_eq!(report.bounds, vec![SparseBoundsRow { lower: "-inf".to_owned(),
            upper: "inf".to_owned() }]);
        assert_eq!(report.variables, 1);
    }

    #[test]
    fn an_empty_row_and_no_rows_are_allowed() {
        let report = verify_sparse_triad(&SparseTriadRequest { entries: Vec::new(),
            constraint_rows: Some(0), ..request() }).unwrap();
        assert_eq!(report.entry_count, 0);
        assert!(report.entry_indices.is_empty());
        assert!(report.constraint_names.is_empty());
    }

    #[test]
    fn bad_input_is_refused() {
        let too_many = SparseTriadRequest {
            entries: (0..65).map(|index| SparseEntrySpec { index, value: 1.0 }).collect(),
            ..request() };
        assert!(verify_sparse_triad(&too_many).unwrap_err().message
            .contains("at most 64 entries"));
        assert!(verify_sparse_triad(&SparseTriadRequest { constraint_rows: Some(17),
            ..request() }).unwrap_err().message.contains("at most 16 constraint rows"));
        assert!(verify_sparse_triad(&SparseTriadRequest {
            entries: vec![SparseEntrySpec { index: 0, value: f64::INFINITY }],
            ..request() }).unwrap_err().message.contains("must be finite"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_sparse_triad(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"entryCount\""), "{json}");
        assert!(json.contains("\"addNeverMergesOrDropsZeros\""), "{json}");
        assert!(json.contains("\"setBoundsAcceptsAnEmptyDomain\""), "{json}");
        assert!(!json.contains("entry_count"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<SparseTriadReport>(&copy.to_string()).is_err());
    }
}
