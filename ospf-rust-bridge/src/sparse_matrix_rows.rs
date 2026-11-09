//! 续挖第 62 单：约束行与元数据的**锁步语义**——`num_constraints()` 只读 `b`、元数据八个平行向量同进同退
//! （`model/intermediate/basic_linear_triad_model.rs` 的 `SparseMatrix` / `add_constraint*` / `clone_constraints`）
//! Continuation, delivery 62: the **lockstep semantics** of constraint rows and their metadata — `num_constraints()` reads only
//! `b`, and the eight parallel vectors move together (`SparseMatrix`, `add_constraint*`, `clone_constraints`).
//!
//! 第 61 单量过单行与变量界，这一单量**行与元数据**。七条实测（零求解器）：
//!
//! 1. **`add_row` 不做任何校验**（`add_row_validates_nothing()`）：行的列数、索引重复、空行都照收，
//!    行数照样 +1——稀疏矩阵根本没有"列宽"概念。
//! 2. **`add_constraint_with_metadata` 让八个平行向量同时 +1**（`the_eight_vectors_move_together()`）：
//!    `A.rows` / `b` / `names` / `group_ids` / `lazy` / `priorities` / `args` / `source_symbol_ids`。
//! 3. **`num_constraints()` 只读 `b`**（`num_constraints_reads_only_b()`）：直接 `A.add_row` 而不 push `b`，
//!    会出现 **`A.rows() > num_constraints()`** 的不一致状态，且**没有任何报错**。
//! 4. **`add_constraint` 只填行名**（`add_constraint_only_fills_the_name()`）：其余元数据是
//!    `None` / `false` / `0`。
//! 5. **行 entries 原样保留顺序**（`the_row_keeps_its_entry_order()`）：不排序、不合并。
//! 6. **`get_row` 越界返回 `None`**（`get_row_is_bounds_checked()`），空矩阵 `rows() == 0`。
//! 7. **`clone_constraints` 是深拷贝**（`clone_constraints_is_deep()`）：改克隆里的行不影响原模型。
//!
//! Delivery 61 measured one row and variable bounds; this one measures **rows and their metadata**. **`add_row` validates
//! nothing** (`add_row_validates_nothing()`): differing widths, repeated indices and empty rows are all accepted and the row
//! count still grows — a sparse matrix has no notion of column width. **`add_constraint_with_metadata` moves eight parallel
//! vectors together** (`the_eight_vectors_move_together()`). **`num_constraints()` reads only `b`**
//! (`num_constraints_reads_only_b()`): a raw `A.add_row` without a matching `b` push leaves **`A.rows() >
//! num_constraints()`** with **no error at all**. **`add_constraint` fills only the row name**
//! (`add_constraint_only_fills_the_name()`). **A row keeps its entry order** (`the_row_keeps_its_entry_order()`).
//! **`get_row` is bounds checked** (`get_row_is_bounds_checked()`). And **`clone_constraints` is a deep copy**
//! (`clone_constraints_is_deep()`).

use ospf_rust_core::model::intermediate::{
    BasicLinearTriadModel, SparseMatrix, SparseVector,
};
use ospf_rust_core::token::Token;
use ospf_rust_core::variable::{ContinuousVariableItem, VariableId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个稀疏条目。 / One sparse entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatrixEntrySpec {
    pub index: u64,
    pub value: f64,
}

/// 一行。 / One row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatrixRowSpec {
    #[serde(default)]
    pub entries: Vec<MatrixEntrySpec>,
}

/// 行与元数据请求。 / The rows-and-metadata request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatrixRowsRequest {
    #[serde(default)]
    pub rows: Vec<MatrixRowSpec>,
    /// 是否走 `add_constraint_with_metadata`（否则直接用 `A.add_row`）。 
    /// Whether to go through `add_constraint_with_metadata` (otherwise `A.add_row` directly).
    #[serde(default = "default_true")]
    pub with_metadata: bool,
}

/// 缺省为真。 / Defaults to true.
fn default_true() -> bool {
    true
}

/// 行与元数据结论。 / The rows-and-metadata verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatrixRowsReport {
    /// `A.rows()`。 / `A.rows()`.
    pub matrix_rows: u64,
    /// `num_constraints()`（只读 `b`）。 / `num_constraints()` (reads only `b`).
    pub num_constraints: u64,
    /// 八个平行向量的长度。 / The eight parallel vectors' lengths.
    pub parallel_lengths: Vec<u64>,
    /// 第一行的索引序列。 / The first row's entry indices.
    pub first_row_indices: Vec<u64>,
    /// `get_row` 越界的结果是否为 `None`。 / Whether `get_row` returned `None` out of range.
    pub get_row_out_of_range_is_none: bool,
    /// `add_row` 不做任何校验（**是**）。 / Whether `add_row` validates nothing (**yes**).
    pub add_row_validates_nothing: bool,
    /// 八个平行向量同时 +1（**是**）。 / Whether the eight vectors move together (**yes**).
    pub the_eight_vectors_move_together: bool,
    /// `num_constraints` 只读 `b`（**是**）。 / Whether `num_constraints` reads only `b` (**yes**).
    pub num_constraints_reads_only_b: bool,
    /// `add_constraint` 只填行名（**是**）。 / Whether `add_constraint` fills only the name (**yes**).
    pub add_constraint_only_fills_the_name: bool,
    /// 行 entries 保持输入顺序（**是**）。 / Whether the row keeps its entry order (**yes**).
    pub the_row_keeps_its_entry_order: bool,
    /// `get_row` 有界检查（**是**）。 / Whether `get_row` is bounds checked (**yes**).
    pub get_row_is_bounds_checked: bool,
    /// `clone_constraints` 是深拷贝（**是**）。 / Whether `clone_constraints` is deep (**yes**).
    pub clone_constraints_is_deep: bool,
}

/// 造一个变量 token。 / Build one variable token.
fn token_of(id: &str, group: usize) -> Token<f64> {
    Token::from_generic(ContinuousVariableItem::create(VariableId::standalone(group), id), group)
}

/// 造一行。 / Build one row.
fn row_of(entries: &[MatrixEntrySpec]) -> SparseVector<f64> {
    let mut row = SparseVector::<f64>::new();
    for entry in entries {
        row.add(usize::try_from(entry.index).unwrap_or(usize::MAX), entry.value);
    }
    row
}

/// 走一遍行与元数据。非法输入是**错误**。 / Walk the rows and their metadata.
pub fn verify_matrix_rows(request: &MatrixRowsRequest) -> Result<MatrixRowsReport> {
    if request.rows.len() > 16 {
        return Err(BridgeError::new("InvalidInput",
            "matrix rows failed validation: at most 16 rows are accepted"));
    }
    for row in &request.rows {
        if row.entries.len() > 64 {
            return Err(BridgeError::new("InvalidInput",
                "matrix rows failed validation: at most 64 entries per row are accepted"));
        }
        for entry in &row.entries {
            if !entry.value.is_finite() {
                return Err(BridgeError::new("InvalidInput",
                    "matrix rows failed validation: entry values must be finite"));
            }
        }
    }
    let mut model = BasicLinearTriadModel::new("matrix_probe");
    model.add_variable(token_of("x", 0));
    for (position, row) in request.rows.iter().enumerate() {
        if request.with_metadata {
            let _ = model.add_constraint_with_metadata(row_of(&row.entries),
                position as f64, format!("row{position}"), Some(position as u64), false,
                position as u32, Some(format!("arg{position}")), Some(position as u64));
        } else {
            model.A.add_row(row_of(&row.entries));
        }
    }
    let matrix_rows = model.A.rows() as u64;
    let num_constraints = model.num_constraints() as u64;
    let parallel_lengths = vec![model.A.rows() as u64, model.b.len() as u64,
        model.constraint_names.len() as u64, model.constraint_group_ids.len() as u64,
        model.constraint_lazy_flags.len() as u64, model.constraint_priorities.len() as u64,
        model.constraint_args.len() as u64, model.constraint_source_symbol_ids.len() as u64];
    let first_row_indices = model.A.get_row(0)
        .map(|row| row.entries.iter().map(|(index, _)| *index as u64).collect::<Vec<_>>())
        .unwrap_or_default();
    let get_row_out_of_range_is_none = model.A.get_row(999).is_none();
    let findings = canonical_findings();
    Ok(MatrixRowsReport { matrix_rows, num_constraints, parallel_lengths, first_row_indices,
        get_row_out_of_range_is_none,
        add_row_validates_nothing: findings.add_row_validates_nothing,
        the_eight_vectors_move_together: findings.the_eight_vectors_move_together,
        num_constraints_reads_only_b: findings.num_constraints_reads_only_b,
        add_constraint_only_fills_the_name: findings.add_constraint_only_fills_the_name,
        the_row_keeps_its_entry_order: findings.the_row_keeps_its_entry_order,
        get_row_is_bounds_checked: findings.get_row_is_bounds_checked,
        clone_constraints_is_deep: findings.clone_constraints_is_deep })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    add_row_validates_nothing: bool,
    the_eight_vectors_move_together: bool,
    num_constraints_reads_only_b: bool,
    add_constraint_only_fills_the_name: bool,
    the_row_keeps_its_entry_order: bool,
    get_row_is_bounds_checked: bool,
    clone_constraints_is_deep: bool,
}

/// 造一条探针行。 / Build one probe row.
fn probe_row(entries: &[(usize, f64)]) -> SparseVector<f64> {
    let mut row = SparseVector::<f64>::new();
    for (index, value) in entries {
        row.add(*index, *value);
    }
    row
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) `add_row` 不做任何校验：不同宽度、重复索引、空行都收。 
    // 1) `add_row` validates nothing: differing widths, duplicate indices and empty rows are accepted.
    let add_row_validates_nothing = {
        let mut matrix = SparseMatrix::<f64>::new();
        matrix.add_row(probe_row(&[(0, 1.0)]));
        matrix.add_row(probe_row(&[(0, 2.0), (0, 3.0), (7, 4.0)]));
        matrix.add_row(probe_row(&[]));
        matrix.rows() == 3
    };
    // 2) 八个平行向量同时 +1。 / The eight parallel vectors move together.
    let the_eight_vectors_move_together = {
        let mut model = BasicLinearTriadModel::new("lockstep");
        model.add_variable(token_of("x", 0));
        for position in 0..3 {
            let _ = model.add_constraint_with_metadata(probe_row(&[(0, 1.0)]),
                position as f64, format!("row{position}"), Some(position as u64), false,
                position as u32, Some(format!("arg{position}")), Some(position as u64));
        }
        model.A.rows() == 3 && model.b.len() == 3 && model.constraint_names.len() == 3
            && model.constraint_group_ids.len() == 3 && model.constraint_lazy_flags.len() == 3
            && model.constraint_priorities.len() == 3 && model.constraint_args.len() == 3
            && model.constraint_source_symbol_ids.len() == 3
    };
    // 3) `num_constraints` 只读 `b`：直接加行会让两者不一致且不报错。 
    // 3) `num_constraints` reads only `b`: a raw row push leaves them inconsistent without an error.
    let num_constraints_reads_only_b = {
        let mut model = BasicLinearTriadModel::new("b-only");
        model.add_variable(token_of("x", 0));
        model.A.add_row(probe_row(&[(0, 1.0)]));
        model.A.add_row(probe_row(&[(0, 2.0)]));
        model.num_constraints() == 0 && model.A.rows() == 2
            && model.b.is_empty() && model.constraint_names.is_empty()
    };
    // 4) `add_constraint` 只填行名。 / `add_constraint` fills only the row name.
    let add_constraint_only_fills_the_name = {
        let mut model = BasicLinearTriadModel::new("name-only");
        model.add_variable(token_of("x", 0));
        let index = model.add_constraint(probe_row(&[(0, 1.0)]), 2.5);
        index == 0 && model.constraint_names == vec!["c0".to_owned()]
            && model.b == vec![2.5] && model.constraint_group_ids == vec![None]
            && model.constraint_lazy_flags == vec![false]
            && model.constraint_priorities == vec![0] && model.constraint_args == vec![None]
            && model.constraint_source_symbol_ids == vec![None]
    };
    // 5) 行 entries 保持输入顺序。 / A row keeps its entry order.
    let the_row_keeps_its_entry_order = {
        let row = probe_row(&[(5, 1.0), (1, 2.0), (3, 3.0), (1, 4.0)]);
        row.entries == vec![(5usize, 1.0f64), (1usize, 2.0f64), (3usize, 3.0f64),
            (1usize, 4.0f64)]
    };
    // 6) `get_row` 有界检查。 / `get_row` is bounds checked.
    let get_row_is_bounds_checked = {
        let empty = SparseMatrix::<f64>::new();
        let mut matrix = SparseMatrix::<f64>::new();
        matrix.add_row(probe_row(&[(0, 1.0)]));
        empty.rows() == 0 && empty.get_row(0).is_none() && matrix.get_row(0).is_some()
            && matrix.get_row(1).is_none()
    };
    // 7) `clone_constraints` 是深拷贝。 / `clone_constraints` is deep.
    let clone_constraints_is_deep = {
        let mut model = BasicLinearTriadModel::new("deep");
        model.add_variable(token_of("x", 0));
        model.add_constraint(probe_row(&[(0, 1.0)]), 1.0);
        let (mut cloned, cloned_b) = model.clone_constraints();
        if let Some(row) = cloned.rows.first_mut() {
            row.add(9, 2.0);
        }
        let source_untouched = model.A.get_row(0).is_some_and(|row| row.entries.len() == 1);
        let cloned_changed = cloned.rows.first().is_some_and(|row| row.entries.len() == 2);
        cloned_b == model.b && source_untouched && cloned_changed
    };
    Findings { add_row_validates_nothing, the_eight_vectors_move_together,
        num_constraints_reads_only_b, add_constraint_only_fills_the_name,
        the_row_keeps_its_entry_order, get_row_is_bounds_checked, clone_constraints_is_deep }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(entries: &[(u64, f64)]) -> MatrixRowSpec {
        MatrixRowSpec { entries: entries.iter().map(|(index, value)| MatrixEntrySpec {
            index: *index, value: *value }).collect() }
    }

    fn request() -> MatrixRowsRequest {
        MatrixRowsRequest { rows: vec![row(&[(5, 1.0), (1, 2.0), (3, 3.0)]),
            row(&[(0, 4.0), (0, 5.0)]), row(&[])], with_metadata: true }
    }

    #[test]
    fn the_metadata_vectors_stay_in_lockstep() {
        let report = verify_matrix_rows(&request()).unwrap();
        assert_eq!(report.matrix_rows, 3);
        assert_eq!(report.num_constraints, 3);
        assert_eq!(report.parallel_lengths, vec![3, 3, 3, 3, 3, 3, 3, 3]);
        assert_eq!(report.first_row_indices, vec![5, 1, 3]);
        assert!(report.get_row_out_of_range_is_none);
        assert!(report.add_row_validates_nothing);
        assert!(report.the_eight_vectors_move_together);
        assert!(report.num_constraints_reads_only_b);
        assert!(report.add_constraint_only_fills_the_name);
        assert!(report.the_row_keeps_its_entry_order);
        assert!(report.get_row_is_bounds_checked);
        assert!(report.clone_constraints_is_deep);
    }

    #[test]
    fn a_raw_row_push_leaves_the_counts_inconsistent() {
        let report = verify_matrix_rows(&MatrixRowsRequest { with_metadata: false,
            ..request() }).unwrap();
        assert_eq!(report.matrix_rows, 3);
        // `num_constraints()` 只读 `b`，所以是 0。 / `num_constraints()` reads only `b`, so it is 0.
        assert_eq!(report.num_constraints, 0);
        assert_eq!(report.parallel_lengths, vec![3, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(report.first_row_indices, vec![5, 1, 3]);
    }

    #[test]
    fn differing_widths_and_empty_rows_are_accepted() {
        let report = verify_matrix_rows(&MatrixRowsRequest { rows: vec![row(&[(0, 1.0)]),
            row(&[(0, 1.0), (1, 2.0), (2, 3.0)]), row(&[])], with_metadata: true })
            .unwrap();
        assert_eq!(report.matrix_rows, 3);
        assert_eq!(report.first_row_indices, vec![0]);
    }

    #[test]
    fn no_rows_at_all_is_fine() {
        let report = verify_matrix_rows(&MatrixRowsRequest { rows: Vec::new(),
            with_metadata: true }).unwrap();
        assert_eq!(report.matrix_rows, 0);
        assert_eq!(report.num_constraints, 0);
        assert!(report.first_row_indices.is_empty());
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_matrix_rows(&MatrixRowsRequest {
            rows: (0..17).map(|_| row(&[(0, 1.0)])).collect(), with_metadata: true })
            .unwrap_err().message.contains("at most 16 rows"));
        assert!(verify_matrix_rows(&MatrixRowsRequest {
            rows: vec![MatrixRowSpec { entries: (0..65).map(|index| MatrixEntrySpec { index,
                value: 1.0 }).collect() }], with_metadata: true }).unwrap_err().message
            .contains("at most 64 entries"));
        assert!(verify_matrix_rows(&MatrixRowsRequest {
            rows: vec![row(&[(0, f64::NAN)])], with_metadata: true }).unwrap_err().message
            .contains("must be finite"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_matrix_rows(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"matrixRows\""), "{json}");
        assert!(json.contains("\"numConstraintsReadsOnlyB\""), "{json}");
        assert!(json.contains("\"theEightVectorsMoveTogether\""), "{json}");
        assert!(!json.contains("matrix_rows"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<MatrixRowsReport>(&copy.to_string()).is_err());
    }
}
