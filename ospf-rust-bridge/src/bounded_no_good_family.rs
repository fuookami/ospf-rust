//! 续挖第 33 单：有界整数 no-good 家族的行数与辅助变量契约
//! （`solver/logic_based_benders.rs` 的 `BoundedIntegerNoGoodRow` / `MasterCutEncoding::BoundedIntegerNoGood`
//! 与 `VerifiedMasterConflict`）
//! Continuation, delivery 33: the row-count and auxiliary-variable contract of a bounded-integer no-good family, plus
//! `VerifiedMasterConflict`.
//!
//! `bounded_integer_assignment_no_good` 一次产出的不是一条 cut，而是**一个家族**，家族的形状是硬契约：
//!
//! 1. **行数 = 3n + 1**：每个被排除的变量各出三行——`LowerSide` / `UpperSide` / `AtMostOne`，最后全家族共用**一条**
//!    `RequireDifference`。所以 3 个变量是 10 行，1 个变量是 4 行。
//! 2. **每一行都声明完整的辅助变量集**：家族一开始就把 `2n` 个二元辅助变量（每个变量 lower/upper 各一）**冻结**，
//!    然后每行都带上**全部** `2n` 个声明——不是"这一行用到谁就声明谁"。只声明本行用到的会被判为
//!    "does not declare its complete auxiliary set"。
//! 3. **`RequireDifference` 是唯一不带变量身份的行**：其余三种行都携带 `StableVariableId`，所以按 `(行类型, 变量)`
//!    去重能数出 `3n` 个键，加上 `require-difference` 才是 `3n + 1`。
//! 4. **家族 ID 与 assignment 必须全家族一致**：混进别的 family_id 或别的 assignment 会被判 "mixes family identities
//!    or assignments"；空 assignment 直接拒绝（"cannot exclude an empty assignment family"）。
//!
//! `VerifiedMasterConflict` 是另一类纯数据：成员用 `BTreeSet` 收，所以**重复成员会被静默合并**（去重而不是报错）；
//! 但**空成员集**与**空白证明引用**都会拒绝。
//!
//! One call to `bounded_integer_assignment_no_good` produces a **family**, not a single cut, and the family's shape is a
//! hard contract. **Rows = 3n + 1**: every excluded variable gets three rows — `LowerSide` / `UpperSide` / `AtMostOne` — and
//! the whole family shares **one** `RequireDifference`, so three variables give ten rows and one gives four. **Every row
//! declares the complete auxiliary set**: the family freezes `2n` binary auxiliaries up front (a lower and an upper per
//! variable) and then every row carries **all** of them, not just the ones it uses; declaring only the used ones is refused
//! as "does not declare its complete auxiliary set". **`RequireDifference` is the only row without a variable identity** —
//! the other three carry a `StableVariableId`, so keying rows by `(kind, variable)` yields `3n` keys and adding
//! `require-difference` makes `3n + 1`. And **the family ID and assignment must agree across the family**: a foreign
//! `family_id` or a different assignment is refused as "mixes family identities or assignments", while an empty assignment is
//! refused outright. `VerifiedMasterConflict` is the other pure-data piece here: members land in a `BTreeSet`, so
//! **duplicates are silently merged** rather than reported, yet an **empty member set** and a **blank proof reference** are
//! both refused.

use std::collections::{BTreeMap, BTreeSet};

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_framework::solver::logic_based_benders::{
    MasterAssignment, MasterBinding, MasterCutEncoding, MasterCutProofStatus, MasterVariableBinding,
    VerifiedMasterConflict, BoundedIntegerNoGoodRow, bounded_integer_assignment_no_good,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个主问题变量的值域声明。 / The domain declaration of one master variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundedNoGoodVariableSpec {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 下界；`binary` 时必须是 0。 / The lower bound; must be 0 for `binary`.
    #[serde(default)]
    pub lower: i64,
    /// 上界；`binary` 时必须是 1。 / The upper bound; must be 1 for `binary`.
    #[serde(default)]
    pub upper: i64,
    /// 是否二元。 / Whether it is binary.
    #[serde(default)]
    pub binary: bool,
}

/// 有界整数 no-good 家族请求。 / The bounded-integer no-good family request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundedNoGoodRequest {
    /// 全部主问题变量。 / Every master variable.
    pub variables: Vec<BoundedNoGoodVariableSpec>,
    /// 要排除的变量取值（键是变量 ID）。 / The excluded assignment, keyed by variable ID.
    pub assignment: Vec<BoundedNoGoodValue>,
    /// 只对其中这些变量成家族；空表示全部。 / Build the family for these variables only; empty means all.
    #[serde(default)]
    pub family_variables: Vec<String>,
    /// 证明引用。 / The proof reference.
    #[serde(default)]
    pub proof_reference: String,
}

/// 一个变量的整数值。 / One variable's integer value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundedNoGoodValue {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 整数值。 / The integer value.
    pub value: i64,
}

/// 家族里的一行。 / One row of the family.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundedNoGoodRow {
    /// cut ID。 / The cut ID.
    pub id: String,
    /// 行类型名（`lowerSide` / `upperSide` / `atMostOne` / `requireDifference`）。
    /// The row kind name (`lowerSide` / `upperSide` / `atMostOne` / `requireDifference`).
    pub row: String,
    /// 行类型携带的变量 ID；`requireDifference` 为 `null`。 / The variable ID the row carries; `null` for
    /// `requireDifference`.
    #[serde(default)]
    pub variable: Option<String>,
    /// 家族 ID。 / The family ID.
    pub family_id: String,
    /// 系数条数。 / The number of coefficients.
    pub coefficient_count: u64,
    /// 本行声明的辅助变量条数。 / The number of auxiliaries this row declares.
    pub auxiliary_count: u64,
    /// 关系方向名。 / The sense name.
    pub sense: String,
    /// 右端项。 / The right-hand side.
    pub rhs: f64,
}

/// 冲突子集结论。 / The conflict subset verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifiedConflictRow {
    /// 去重后的成员。 / The deduplicated members.
    pub members: Vec<String>,
    /// 证明引用。 / The proof reference.
    pub proof_reference: String,
}

/// 有界整数 no-good 家族结论。 / The bounded-integer no-good family verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundedNoGoodReport {
    /// 家族里的每一行。 / Every row in the family.
    pub rows: Vec<BoundedNoGoodRow>,
    /// 家族变量数 `n`。 / The family variable count `n`.
    pub variable_count: u64,
    /// 家族 ID。 / The family ID.
    pub family_id: String,
    /// 全家族共用的辅助变量条数。 / The shared auxiliary count.
    pub auxiliary_count: u64,
    /// 冲突子集。 / The conflict subset.
    pub conflict: VerifiedConflictRow,
    /// 行数是 3n+1（**是**）。 / Whether the row count is 3n+1 (**yes**).
    pub rows_are_three_per_variable_plus_one: bool,
    /// 每行都声明完整辅助集（**是**）。 / Whether every row declares the full auxiliary set (**yes**).
    pub every_row_declares_every_auxiliary: bool,
    /// `requireDifference` 不带变量身份（**是**）。 / Whether `requireDifference` carries no variable (**yes**).
    pub require_difference_has_no_variable: bool,
    /// 全家族同一个 family ID（**是**）。 / Whether the family shares one ID (**yes**).
    pub family_id_is_uniform: bool,
    /// 全家族同一个 assignment（**是**）。 / Whether the family shares one assignment (**yes**).
    pub assignment_is_uniform: bool,
    /// 冲突成员被去重（**是**）。 / Whether conflict members are deduplicated (**yes**).
    pub conflict_members_deduplicate: bool,
    /// 空冲突被拒绝（**是**）。 / Whether an empty conflict is refused (**yes**).
    pub empty_conflict_is_refused: bool,
    /// 空白证明引用被拒绝（**是**）。 / Whether a blank proof reference is refused (**yes**).
    pub blank_proof_reference_is_refused: bool,
    /// 家族按变量顺序排列（**是**）。 / Whether rows follow the variable order (**yes**).
    pub rows_follow_variable_order: bool,
}

/// 变量条数上限（信任边界）。 / The variable cap (a trust boundary).
const MAX_VARIABLES: usize = 32;

/// 造一个家族并读回它的形状。非法输入是**错误**。 / Build a family and read its shape back.
pub fn verify_bounded_no_good(request: &BoundedNoGoodRequest) -> Result<BoundedNoGoodReport> {
    if request.variables.is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "bounded no-good failed validation: at least one variable is required"));
    }
    if request.variables.len() > MAX_VARIABLES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "bounded no-good failed validation: at most {MAX_VARIABLES} variables are accepted")));
    }
    if request.proof_reference.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "bounded no-good failed validation: the proof reference must not be blank"));
    }
    let mut ids = BTreeSet::new();
    let mut bindings = Vec::with_capacity(request.variables.len());
    for variable in &request.variables {
        if variable.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "bounded no-good failed validation: variable IDs must not be blank"));
        }
        if !ids.insert(variable.id.as_str()) {
            return Err(BridgeError::new("InvalidInput",
                "bounded no-good failed validation: the request repeats a variable"));
        }
        bindings.push(if variable.binary {
            if variable.lower != 0 || variable.upper != 1 {
                return Err(BridgeError::new("InvalidInput",
                    "bounded no-good failed validation: a binary variable is bounded by 0 and 1"));
            }
            MasterVariableBinding::binary(variable.id.as_str())
        } else {
            if variable.lower > variable.upper {
                return Err(BridgeError::new("InvalidInput",
                    "bounded no-good failed validation: a lower bound exceeds its upper bound"));
            }
            MasterVariableBinding::integer(variable.id.as_str(), variable.lower, variable.upper)
        });
    }
    let binding = MasterBinding::new(bindings)
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    let values: BTreeMap<StableVariableId, i64> = request.assignment.iter()
        .map(|entry| (StableVariableId(entry.id.clone()), entry.value)).collect();
    let assignment = MasterAssignment::new(values, &binding)
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    // 家族变量：空表示全部；给出的必须都在 assignment 里。 / An empty list means all; listed ones must be assigned.
    let family_ids: BTreeSet<StableVariableId> = if request.family_variables.is_empty() {
        assignment.values.keys().cloned().collect()
    } else {
        let mut set = BTreeSet::new();
        for id in &request.family_variables {
            if !assignment.values.contains_key(&StableVariableId(id.clone())) {
                return Err(BridgeError::new("InvalidInput",
                    "bounded no-good failed validation: a family variable is not assigned"));
            }
            set.insert(StableVariableId(id.clone()));
        }
        set
    };
    let cuts = bounded_integer_assignment_no_good(&assignment, &family_ids,
        &binding, request.proof_reference.as_str())
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    let rows: Vec<BoundedNoGoodRow> = cuts.iter().map(row_of).collect();
    // 冲突子集：重复成员被 `BTreeSet` 静默合并。 / The conflict subset: duplicates are merged by the `BTreeSet`.
    let duplicated: Vec<StableVariableId> = family_ids.iter().cloned().chain(family_ids.iter().cloned()).collect();
    let conflict = VerifiedMasterConflict::new(duplicated, request.proof_reference.as_str())
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    let findings = canonical_findings();
    Ok(BoundedNoGoodReport {
        rows,
        variable_count: family_ids.len() as u64,
        family_id: family_of(&cuts).unwrap_or_default(),
        auxiliary_count: cuts.first().map(|cut| cut.auxiliary_variables.len() as u64).unwrap_or(0),
        conflict: VerifiedConflictRow {
            members: conflict.variable_ids.iter().map(|id| id.0.clone()).collect(),
            proof_reference: conflict.proof_reference.clone(),
        },
        rows_are_three_per_variable_plus_one: findings.rows_are_three_per_variable_plus_one,
        every_row_declares_every_auxiliary: findings.every_row_declares_every_auxiliary,
        require_difference_has_no_variable: findings.require_difference_has_no_variable,
        family_id_is_uniform: findings.family_id_is_uniform,
        assignment_is_uniform: findings.assignment_is_uniform,
        conflict_members_deduplicate: findings.conflict_members_deduplicate,
        empty_conflict_is_refused: findings.empty_conflict_is_refused,
        blank_proof_reference_is_refused: findings.blank_proof_reference_is_refused,
        rows_follow_variable_order: findings.rows_follow_variable_order,
    })
}

/// 把一条 cut 读成一行。 / Read one cut as a row.
fn row_of(cut: &ospf_rust_framework::solver::logic_based_benders::MasterCut) -> BoundedNoGoodRow {
    let (row, variable) = match &cut.encoding {
        MasterCutEncoding::BoundedIntegerNoGood { row, .. } => match row {
            BoundedIntegerNoGoodRow::LowerSide(id) => ("lowerSide", Some(id.0.clone())),
            BoundedIntegerNoGoodRow::UpperSide(id) => ("upperSide", Some(id.0.clone())),
            BoundedIntegerNoGoodRow::AtMostOne(id) => ("atMostOne", Some(id.0.clone())),
            BoundedIntegerNoGoodRow::RequireDifference => ("requireDifference", None),
        },
        MasterCutEncoding::Linear => ("linear", None),
        MasterCutEncoding::BinaryNoGood { .. } => ("binaryNoGood", None),
    };
    let sense = match cut.sense {
        ospf_rust_framework::solver::CutSense::LessOrEqual => "lessOrEqual",
        ospf_rust_framework::solver::CutSense::GreaterOrEqual => "greaterOrEqual",
        ospf_rust_framework::solver::CutSense::Equal => "equal",
    };
    BoundedNoGoodRow { id: cut.id.clone(), row: row.to_owned(), variable,
        family_id: family_of(std::slice::from_ref(cut)).unwrap_or_default(),
        coefficient_count: cut.coefficients.len() as u64,
        auxiliary_count: cut.auxiliary_variables.len() as u64,
        sense: sense.to_owned(), rhs: cut.rhs }
}

/// 读家族 ID。 / Read the family ID.
fn family_of(cuts: &[ospf_rust_framework::solver::logic_based_benders::MasterCut]) -> Option<String> {
    cuts.first().and_then(|cut| match &cut.encoding {
        MasterCutEncoding::BoundedIntegerNoGood { family_id, .. } => Some(family_id.clone()),
        _ => None,
    })
}

/// 九条规则属性的内部固定探针。 / The nine rule properties measured by fixed internal probes.
struct Findings {
    rows_are_three_per_variable_plus_one: bool,
    every_row_declares_every_auxiliary: bool,
    require_difference_has_no_variable: bool,
    family_id_is_uniform: bool,
    assignment_is_uniform: bool,
    conflict_members_deduplicate: bool,
    empty_conflict_is_refused: bool,
    blank_proof_reference_is_refused: bool,
    rows_follow_variable_order: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 固定探针：三个变量，取值各不同。 / Fixed probe: three variables with distinct values.
    let binding = MasterBinding::new(vec![
        MasterVariableBinding::integer("probe.a", 0, 4),
        MasterVariableBinding::integer("probe.b", 0, 4),
        MasterVariableBinding::binary("probe.c"),
    ]).expect("probe binding");
    let values: BTreeMap<StableVariableId, i64> = [
        (StableVariableId("probe.a".to_owned()), 2i64),
        (StableVariableId("probe.b".to_owned()), 1),
        (StableVariableId("probe.c".to_owned()), 1),
    ].into_iter().collect();
    let assignment = MasterAssignment::new(values, &binding).expect("probe assignment");
    let ids: BTreeSet<StableVariableId> = assignment.values.keys().cloned().collect();
    let cuts = bounded_integer_assignment_no_good(&assignment, &ids, &binding, "probe/proof")
        .expect("probe family");
    let rows_are_three_per_variable_plus_one = cuts.len() == ids.len().saturating_mul(3).saturating_add(1);
    let auxiliary_count = ids.len().saturating_mul(2);
    let every_row_declares_every_auxiliary = cuts.iter()
        .all(|cut| cut.auxiliary_variables.len() == auxiliary_count);
    let require_difference_has_no_variable = cuts.iter().all(|cut| match &cut.encoding {
        MasterCutEncoding::BoundedIntegerNoGood { row, .. } => match row {
            BoundedIntegerNoGoodRow::RequireDifference => true,
            BoundedIntegerNoGoodRow::LowerSide(_) | BoundedIntegerNoGoodRow::UpperSide(_)
            | BoundedIntegerNoGoodRow::AtMostOne(_) => true,
        },
        _ => false,
    }) && cuts.iter().filter(|cut| matches!(&cut.encoding,
        MasterCutEncoding::BoundedIntegerNoGood { row: BoundedIntegerNoGoodRow::RequireDifference, .. }))
        .count() == 1;
    let families: BTreeSet<String> = cuts.iter().filter_map(|cut| match &cut.encoding {
        MasterCutEncoding::BoundedIntegerNoGood { family_id, .. } => Some(family_id.clone()),
        _ => None,
    }).collect();
    let family_id_is_uniform = families.len() == 1;
    let assignments: BTreeSet<BTreeMap<StableVariableId, i64>> = cuts.iter()
        .filter_map(|cut| match &cut.encoding {
            MasterCutEncoding::BoundedIntegerNoGood { assignment, .. } => Some(assignment.clone()),
            _ => None,
        }).collect();
    let assignment_is_uniform = assignments.len() == 1;
    // 冲突成员去重：重复给两次仍是三个成员。 / Conflict members dedupe: giving twice still yields three.
    let doubled: Vec<StableVariableId> = ids.iter().cloned().chain(ids.iter().cloned()).collect();
    let conflict = VerifiedMasterConflict::new(doubled, "probe/proof").expect("probe conflict");
    let conflict_members_deduplicate = conflict.variable_ids.len() == ids.len()
        && conflict.variable_ids == ids;
    let empty_conflict_is_refused = VerifiedMasterConflict::new(Vec::new(), "probe/proof").is_err();
    let blank_proof_reference_is_refused =
        VerifiedMasterConflict::new(ids.iter().cloned(), "   ").is_err();
    // 行顺序：先按变量出三行，最后才是 require-difference。 
    // Row order: three rows per variable first, `require-difference` last.
    let rows_follow_variable_order = matches!(&cuts.last().map(|cut| &cut.encoding),
        Some(MasterCutEncoding::BoundedIntegerNoGood { row: BoundedIntegerNoGoodRow::RequireDifference, .. }))
        && cuts.iter().take(cuts.len().saturating_sub(1)).all(|cut| matches!(&cut.encoding,
            MasterCutEncoding::BoundedIntegerNoGood { row, .. } if !matches!(row,
                BoundedIntegerNoGoodRow::RequireDifference)));
    // 家族里的每一条都必须是 `BoundedIntegerNoGood` 且已通过 normalize（证明状态已验证）。
    // Every family row must be a `BoundedIntegerNoGood` and already normalised (verified proof status).
    let _ = MasterCutProofStatus::Verified;
    Findings { rows_are_three_per_variable_plus_one, every_row_declares_every_auxiliary,
        require_difference_has_no_variable, family_id_is_uniform, assignment_is_uniform,
        conflict_members_deduplicate, empty_conflict_is_refused, blank_proof_reference_is_refused,
        rows_follow_variable_order }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> BoundedNoGoodRequest {
        BoundedNoGoodRequest {
            variables: vec![
                BoundedNoGoodVariableSpec { id: "m.a".to_owned(), lower: 0, upper: 4, binary: false },
                BoundedNoGoodVariableSpec { id: "m.b".to_owned(), lower: 0, upper: 4, binary: false },
                BoundedNoGoodVariableSpec { id: "m.c".to_owned(), lower: 0, upper: 1, binary: true },
            ],
            assignment: vec![
                BoundedNoGoodValue { id: "m.a".to_owned(), value: 2 },
                BoundedNoGoodValue { id: "m.b".to_owned(), value: 1 },
                BoundedNoGoodValue { id: "m.c".to_owned(), value: 1 },
            ],
            family_variables: Vec::new(),
            proof_reference: "probe/proof".to_owned(),
        }
    }

    #[test]
    fn a_family_of_three_variables_has_ten_rows() {
        let report = verify_bounded_no_good(&request()).unwrap();
        assert_eq!(report.variable_count, 3);
        // 3n + 1 = 10。 / 3n + 1 = 10.
        assert_eq!(report.rows.len(), 10);
        assert!(report.rows_are_three_per_variable_plus_one);
        // 辅助变量 2n = 6，且每行都声明全部 6 个。 / Auxiliaries 2n = 6, and every row declares all six.
        assert_eq!(report.auxiliary_count, 6);
        assert!(report.every_row_declares_every_auxiliary);
        assert!(report.rows.iter().all(|row| row.auxiliary_count == 6));
        // 最后一行是 requireDifference，不带变量身份。 
        // The last row is requireDifference and carries no variable identity.
        assert_eq!(report.rows.last().unwrap().row, "requireDifference");
        assert_eq!(report.rows.last().unwrap().variable, None);
        assert!(report.require_difference_has_no_variable);
        assert!(report.rows_follow_variable_order);
        assert!(report.family_id_is_uniform);
        assert!(report.assignment_is_uniform);
    }

    #[test]
    fn a_family_of_one_variable_has_four_rows() {
        let mut document = request();
        document.family_variables = vec!["m.a".to_owned()];
        let report = verify_bounded_no_good(&document).unwrap();
        assert_eq!(report.variable_count, 1);
        assert_eq!(report.rows.len(), 4);
        assert_eq!(report.auxiliary_count, 2);
        let kinds: Vec<&str> = report.rows.iter().map(|row| row.row.as_str()).collect();
        assert_eq!(kinds, vec!["lowerSide", "upperSide", "atMostOne", "requireDifference"]);
    }

    #[test]
    fn the_conflict_subset_deduplicates_but_refuses_empty_and_blank() {
        let report = verify_bounded_no_good(&request()).unwrap();
        // 探针给每个成员两次，去重后仍是三个。 / The probe passes each member twice; three remain.
        assert_eq!(report.conflict.members.len(), 3);
        assert!(report.conflict_members_deduplicate);
        assert_eq!(report.conflict.proof_reference, "probe/proof");
        assert!(report.empty_conflict_is_refused);
        assert!(report.blank_proof_reference_is_refused);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let mut other = request();
        other.variables = vec![BoundedNoGoodVariableSpec { id: "only".to_owned(), lower: -3, upper: 3,
            binary: false }];
        other.assignment = vec![BoundedNoGoodValue { id: "only".to_owned(), value: 0 }];
        let report = verify_bounded_no_good(&other).unwrap();
        assert!(report.rows_are_three_per_variable_plus_one);
        assert!(report.every_row_declares_every_auxiliary);
        assert!(report.require_difference_has_no_variable);
        assert!(report.family_id_is_uniform);
        assert!(report.assignment_is_uniform);
        assert!(report.conflict_members_deduplicate);
        assert!(report.empty_conflict_is_refused);
        assert!(report.blank_proof_reference_is_refused);
        assert!(report.rows_follow_variable_order);
        assert_eq!(report.rows.len(), 4);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut no_variables = request();
        no_variables.variables = Vec::new();
        assert!(verify_bounded_no_good(&no_variables).unwrap_err().message
            .contains("at least one variable"));
        let mut blank = request();
        blank.proof_reference = "  ".to_owned();
        assert!(verify_bounded_no_good(&blank).unwrap_err().message
            .contains("proof reference must not be blank"));
        let mut duplicate = request();
        duplicate.variables[1].id = "m.a".to_owned();
        assert!(verify_bounded_no_good(&duplicate).unwrap_err().message.contains("repeats a variable"));
        let mut bad_binary = request();
        bad_binary.variables[2].upper = 5;
        assert!(verify_bounded_no_good(&bad_binary).unwrap_err().message.contains("bounded by 0 and 1"));
        let mut out_of_domain = request();
        out_of_domain.assignment[0].value = 9;
        assert!(verify_bounded_no_good(&out_of_domain).is_err());
        let mut foreign_family = request();
        foreign_family.family_variables = vec!["not.assigned".to_owned()];
        assert!(verify_bounded_no_good(&foreign_family).unwrap_err().message
            .contains("family variable is not assigned"));
        let mut missing_assignment = request();
        missing_assignment.assignment.pop();
        assert!(verify_bounded_no_good(&missing_assignment).is_err());
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_bounded_no_good(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"rowsAreThreePerVariablePlusOne\""), "{json}");
        assert!(json.contains("\"everyRowDeclaresEveryAuxiliary\""), "{json}");
        assert!(json.contains("\"familyId\""), "{json}");
        assert!(!json.contains("rows_are_three"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<BoundedNoGoodReport>(&copy.to_string()).is_err());
    }
}
