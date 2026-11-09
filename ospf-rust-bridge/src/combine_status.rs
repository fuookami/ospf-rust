//! 续挖第 17 单：分析状态的合并是一个四元格（`analysis/pipeline.rs::combine_status`）
//! Continuation, delivery 17: combining analysis statuses is a four-element lattice.
//!
//! `combine_status(current, next)` 只有 12 行，但它是整个报告"状态收敛"的唯一规则，四条性质可以**穷举**验证：
//!
//! 1. **`None` 是恒等元**：`combine_status(current, None) == current`，一个"没有新结论"的步骤不会动状态。
//! 2. **合并是取 `rank` 更小者**（悲观优先）：`Reachable(3) > Unreachable(2) > Unknown(1) > Unsupported(0)`。
//!    所以 `Reachable` 是**最弱**的状态——遇到任何一个别的状态都会被换掉。
//! 3. **`Unsupported` 是最强势的坏消息**：它的 rank 最低（0），一旦出现就**压住一切**，包括 `Unknown`。
//!    这很反直觉：直觉上"未知"比"不支持"更不确定，但合并不按确定性排，按 rank 排。
//! 4. **`None` 之外的三条代数律都成立**：交换、结合、幂等。所以"一串状态折叠起来"与顺序无关——
//!    这也是报告能分块累积、最后再合并的原因。**但 `None` 破坏交换律**（它只能当右侧恒等元）。
//!
//! `combine_status(current, next)` is twelve lines, but it is the single rule behind the report's status convergence, and its
//! four properties are exhaustively checkable. **`None` is the identity**: `combine_status(current, None) == current`, so a step
//! with no new conclusion never moves the status. **The combination keeps the smaller `rank`** — pessimistic first:
//! `Reachable(3) > Unreachable(2) > Unknown(1) > Unsupported(0)` — which makes `Reachable` the **weakest** status, replaced by
//! anything else. **`Unsupported` is the strongest bad news**: rank 0, so once present it **overrides everything**, `Unknown`
//! included — counter-intuitive, since "unknown" feels less certain than "unsupported", but the merge orders by rank, not by
//! confidence. And **the three algebraic laws hold off `None`**: commutative, associative, idempotent, so folding a sequence of
//! statuses is order-independent — which is why the report can accumulate in blocks and merge at the end. **`None` does break
//! commutativity**, since it only works as the right-hand identity.

use ospf_rust_core::analysis::pipeline::combine_status;
use ospf_rust_core::analysis::AnalysisStatus;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个状态名。 / A status name.
pub type StatusName = String;

/// 合并请求。 / The merge request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CombineStatusRequest {
    /// 要折叠的序列（从第一个开始，逐个与后续合并）。 / The sequence to fold.
    #[serde(default)]
    pub sequence: Vec<String>,
}

/// 一格结论。 / One cell of the table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CombineCell {
    /// 左侧状态名。 / The left status.
    pub current: String,
    /// 右侧状态名（`none` 表示无新结论）。 / The right status (`none` for no new conclusion).
    pub next: String,
    /// 合并结果。 / The combined status.
    pub combined: String,
}

/// 状态合并结论。 / The status-merge verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CombineStatusReport {
    /// 状态词表（按 rank 从高到低）。 / The status vocabulary, ordered by rank descending.
    pub vocabulary: Vec<String>,
    /// 全表（4 个左侧 × 5 个右侧 = 20 格）。 / The full table (4 × 5 = 20 cells).
    pub table: Vec<CombineCell>,
    /// 折叠结果。 / The folded result.
    pub folded: String,
    /// 折叠过程（每一步之后的状态）。 / The fold trace, one entry per step.
    pub trace: Vec<String>,
    /// `None` 是否是恒等元（**是**）。 / Whether `None` is the identity (**yes**).
    pub none_is_identity: bool,
    /// 两个具体状态是否可交换（**是**）。 / Whether two statuses commute (**yes**).
    pub commutative: bool,
    /// 是否结合（**是**）。 / Whether it is associative (**yes**).
    pub associative: bool,
    /// 是否幂等（**是**）。 / Whether it is idempotent (**yes**).
    pub idempotent: bool,
    /// `Unsupported` 是否压住一切（**是**）。 / Whether `Unsupported` overrides everything (**yes**).
    pub unsupported_dominates: bool,
    /// `Reachable` 是否被一切换掉（**是**）。 / Whether `Reachable` loses to everything (**yes**).
    pub reachable_is_weakest: bool,
    /// `Unknown` 是否压住 `Unsupported`（**否**）。 / Whether `Unknown` overrides `Unsupported` (**no**).
    pub unknown_beats_unsupported: bool,
}

/// 序列长度上限（信任边界）。 / The sequence cap (a trust boundary).
const MAX_SEQUENCE: usize = 4_096;

/// 列出状态与 rank。 / The statuses with their ranks.
const RANKED: [(&str, AnalysisStatus); 4] = [
    ("reachable", AnalysisStatus::Reachable),
    ("unreachable", AnalysisStatus::Unreachable),
    ("unknown", AnalysisStatus::Unknown),
    ("unsupported", AnalysisStatus::Unsupported),
];

/// 走一遍状态合并：20 格全表 + 三条代数律 + 折叠。非法输入是**错误**。
/// Walk the merge: the full twenty-cell table, the three laws and a fold.
pub fn verify_combine_status(request: &CombineStatusRequest) -> Result<CombineStatusReport> {
    if request.sequence.len() > MAX_SEQUENCE {
        return Err(BridgeError::new("InvalidInput", &format!(
            "combine status failed validation: at most {MAX_SEQUENCE} statuses are accepted")));
    }
    // 全表：4 个左侧 ×（4 个右侧 + `None`）。 / The full table: 4 lefts × (4 rights + `None`).
    let mut table = Vec::with_capacity(RANKED.len() * (RANKED.len() + 1));
    for (left_name, left) in RANKED {
        for (right_name, right) in RANKED {
            table.push(CombineCell { current: left_name.to_owned(), next: right_name.to_owned(),
                combined: name_of(combine_status(left, Some(right))) });
        }
        table.push(CombineCell { current: left_name.to_owned(), next: "none".to_owned(),
            combined: name_of(combine_status(left, None)) });
    }
    // 四条代数性质，穷举验证而不是嘴上说。 / Four algebraic properties, checked exhaustively.
    let mut none_is_identity = true;
    let mut commutative = true;
    let mut associative = true;
    let mut idempotent = true;
    for (_, left) in RANKED {
        none_is_identity &= combine_status(left, None) == left;
        idempotent &= combine_status(left, Some(left)) == left;
        for (_, right) in RANKED {
            commutative &= combine_status(left, Some(right)) == combine_status(right, Some(left));
            for (_, third) in RANKED {
                associative &= combine_status(combine_status(left, Some(right)), Some(third))
                    == combine_status(left, Some(combine_status(right, Some(third))));
            }
        }
    }
    // 两条"谁压谁"的实测。 / The two dominance findings.
    let unsupported = AnalysisStatus::Unsupported;
    let reachable = AnalysisStatus::Reachable;
    let unsupported_dominates = RANKED.iter()
        .all(|(_, other)| combine_status(*other, Some(unsupported)) == unsupported
            && combine_status(unsupported, Some(*other)) == unsupported);
    let reachable_is_weakest = RANKED.iter()
        .all(|(name, other)| *name == "reachable"
            || combine_status(reachable, Some(*other)) == *other);
    let unknown_beats_unsupported = combine_status(AnalysisStatus::Unknown,
        Some(AnalysisStatus::Unsupported)) == AnalysisStatus::Unknown;
    // 折叠：从第一个开始，逐个与后续合并。 / Fold from the first element onwards.
    let mut trace = Vec::with_capacity(request.sequence.len());
    let mut folded = AnalysisStatus::Unknown;
    for (position, name) in request.sequence.iter().enumerate() {
        let status = status_of(name)?;
        folded = if position == 0 { status } else { combine_status(folded, Some(status)) };
        trace.push(name_of(folded));
    }
    Ok(CombineStatusReport {
        vocabulary: RANKED.iter().map(|(name, _)| (*name).to_owned()).collect(),
        table,
        folded: name_of(folded),
        trace,
        none_is_identity,
        commutative,
        associative,
        idempotent,
        unsupported_dominates,
        reachable_is_weakest,
        unknown_beats_unsupported,
    })
}

/// 状态名 → 枚举。 / A status name into the enum.
fn status_of(name: &str) -> Result<AnalysisStatus> {
    RANKED.iter().find(|(candidate, _)| *candidate == name).map(|(_, status)| *status)
        .ok_or_else(|| BridgeError::new("InvalidInput", &format!(
            "combine status failed validation: unknown status `{name}`")))
}

/// 枚举 → 状态名。 / The enum into a status name.
fn name_of(status: AnalysisStatus) -> String {
    match status {
        AnalysisStatus::Reachable => "reachable",
        AnalysisStatus::Unreachable => "unreachable",
        AnalysisStatus::Unknown => "unknown",
        AnalysisStatus::Unsupported => "unsupported",
    }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(sequence: &[&str]) -> CombineStatusRequest {
        CombineStatusRequest { sequence: sequence.iter().map(|name| (*name).to_owned()).collect() }
    }

    fn cell<'a>(report: &'a CombineStatusReport, current: &str, next: &str)
        -> &'a CombineCell
    {
        report.table.iter()
            .find(|cell| cell.current == current && cell.next == next)
            .unwrap_or_else(|| panic!("cell {current}/{next}"))
    }

    #[test]
    fn the_table_is_twenty_cells_and_none_is_the_identity() {
        let report = verify_combine_status(&request(&[])).unwrap();
        assert_eq!(report.table.len(), 20);
        assert_eq!(report.vocabulary,
            vec!["reachable".to_owned(), "unreachable".to_owned(), "unknown".to_owned(),
                "unsupported".to_owned()]);
        assert!(report.none_is_identity);
        // `None` 列原样返回当前状态。 / The `none` column returns the current status untouched.
        for name in ["reachable", "unreachable", "unknown", "unsupported"] {
            assert_eq!(cell(&report, name, "none").combined, name);
        }
        // 同状态合并是它自己。 / A status combined with itself is itself.
        assert!(report.idempotent);
        assert_eq!(cell(&report, "unknown", "unknown").combined, "unknown");
    }

    #[test]
    fn unsupported_dominates_and_reachable_is_the_weakest() {
        let report = verify_combine_status(&request(&[])).unwrap();
        assert!(report.unsupported_dominates);
        // `Unsupported` 压住一切，包括 `Unknown` 与 `Reachable`。 / It overrides everything, `Unknown` included.
        for name in ["reachable", "unreachable", "unknown"] {
            assert_eq!(cell(&report, name, "unsupported").combined, "unsupported", "{name}");
            assert_eq!(cell(&report, "unsupported", name).combined, "unsupported", "{name}");
        }
        // 反直觉的一条：`Unknown` **不**压住 `Unsupported`。 / `Unknown` does **not** override `Unsupported`.
        assert!(!report.unknown_beats_unsupported);
        assert_eq!(cell(&report, "unknown", "unsupported").combined, "unsupported");
        // `Reachable` 是最弱的：遇到任何别的状态都被换掉。 / `Reachable` is the weakest.
        assert!(report.reachable_is_weakest);
        assert_eq!(cell(&report, "reachable", "unreachable").combined, "unreachable");
        assert_eq!(cell(&report, "reachable", "unknown").combined, "unknown");
        // `Unreachable` 只被 `Unsupported` 换掉。 / `Unreachable` only loses to `Unsupported`.
        assert_eq!(cell(&report, "unreachable", "reachable").combined, "unreachable");
        assert_eq!(cell(&report, "unreachable", "unknown").combined, "unknown");
        assert_eq!(cell(&report, "unreachable", "unsupported").combined, "unsupported");
    }

    #[test]
    fn the_three_laws_hold_and_folding_ignores_order() {
        let report = verify_combine_status(&request(&[])).unwrap();
        assert!(report.commutative);
        assert!(report.associative);
        // 折叠：顺序无关，只看有没有 `Unsupported`。 / The fold is order-independent.
        let a = verify_combine_status(&request(&["reachable", "unknown", "unreachable"])).unwrap();
        assert_eq!(a.folded, "unknown");
        assert_eq!(a.trace, vec!["reachable".to_owned(), "unknown".to_owned(),
            "unknown".to_owned()]);
        let b = verify_combine_status(&request(&["unreachable", "unknown", "reachable"])).unwrap();
        assert_eq!(b.folded, a.folded);
        // 一旦出现 `Unsupported`，后面再也翻不了身。 / An early `Unsupported` is final.
        let c = verify_combine_status(&request(&["reachable", "unsupported", "reachable"])).unwrap();
        assert_eq!(c.folded, "unsupported");
        assert_eq!(c.trace, vec!["reachable".to_owned(), "unsupported".to_owned(),
            "unsupported".to_owned()]);
        // 单元素序列：就是它自己。 / A single-element sequence is itself.
        let d = verify_combine_status(&request(&["unreachable"])).unwrap();
        assert_eq!(d.folded, "unreachable");
        assert_eq!(d.trace.len(), 1);
        // 空序列：没有状态可合并，回报 `unknown`。 / An empty sequence reports `unknown`.
        let e = verify_combine_status(&request(&[])).unwrap();
        assert_eq!(e.folded, "unknown");
        assert!(e.trace.is_empty());
    }

    #[test]
    fn bad_input_is_refused() {
        let report = verify_combine_status(&request(&["reachable", "unknownish"]));
        assert!(report.unwrap_err().message.contains("unknown status"));
        let many = request(&vec!["unknown"; MAX_SEQUENCE + 1]);
        assert!(verify_combine_status(&many).unwrap_err().message.contains("at most"));
    }
}
