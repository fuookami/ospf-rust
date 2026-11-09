//! 续挖第 93 单：**档案的"是否改进"旗标说的不是这批候选，而是档案最优**——首轮哪怕塞进来的是最差的也返回
//! `true`（空 → 有）；**容量 0 的档案把一切吞掉且永远报 `false`**；**等值候选会被存下来却不报改进**
//! （内容变了、旗标说没变）；而历史最优档案**只要这批里有一个更优就返回 `true`**，且**永不收下等值候选**
//! （`solver/heuristic/operator/archive.rs` 的 `BestArchive` / `HistoryBestArchive`）
//! Continuation, delivery 93: **the "did it improve" flag speaks about the archive's best, not about the batch** — the
//! first round returns `true` even when handed the worst candidate (empty to non-empty); **a zero-capacity archive
//! swallows everything and always reports `false`**; **an equal candidate is stored without reporting improvement** (the
//! contents changed while the flag says they did not); and the history archive **returns `true` when any candidate in the
//! batch is better** while **never accepting an equal one**
//! (`BestArchive` / `HistoryBestArchive` in `solver/heuristic/operator/archive.rs`).
//!
//! 第 84 单量的是 Pareto 档案，这一单量**单目标的两条档案路**。八条实测（外加一条写进说明的实测：
//! **更差的候选照样进档案**——`truncate` 只保留前 `amount` 名，所以"没改进"不等于"没被收录"）：
//!
//! 1. **首轮必报改进**（`the_first_update_always_reports_improvement()`）：`None → Some` 就是 `true`，
//!    塞进来的是最差的也照样报。
//! 2. **容量 0 的档案把一切吞掉**（`a_zero_capacity_archive_swallows_everything()`）：`truncate(0)` 之后永远为空、
//!    旗标永远 `false`——连首轮都不报。
//! 3. **等值候选被存下却不报改进**（`an_equal_candidate_is_stored_but_never_reported()`）：
//!    `individuals()` 变长了，而旗标说没变。本单最值钱的一条。
//! 4. **旗标跟踪的是档案最优而不是这批候选**（`the_flag_tracks_the_archive_best()`）：整批都更差 → `false`，
//!    只要有一个更好 → `true`。
//! 5. **并列时先来的排前面**（`a_tie_keeps_the_older_entry_first()`）：稳定排序 + 候选追加在已有条目之后。
//! 6. **`clear()` 会让旗标重新变真**（`clear_resets_the_flag()`）：清空之后的第一次更新又回到 `None → Some`。
//! 7. **历史最优档案对"这批里任何一个更优"都报改进**（`the_history_archive_reports_any_improvement()`）。
//! 8. **历史最优档案永不收下等值候选**（`the_history_archive_never_takes_an_equal_candidate()`）：
//!    严格 `Less` 才算更优，于是等值时**原来的那个个体继续留着**。
//!
//! Delivery 84 measured the Pareto archive; this one measures **the two single-objective archive routes**. **The first
//! update always reports improvement** because empty-to-non-empty is `true`, even for the worst candidate. **A
//! zero-capacity archive swallows everything** — always empty, always `false`, not even the first round reports. **An
//! equal candidate is stored without reporting improvement**, so the contents grew while the flag says nothing changed;
//! this is the most valuable finding here. **The flag tracks the archive's best rather than the batch.** **A tie keeps the
//! older entry first** thanks to the stable sort over appended candidates. **`clear()` resets the flag.** **The history
//! archive reports any improvement in the batch.** And **it never takes an equal candidate**, so the incumbent survives a
//! tie.

use ospf_rust_core::model::callback::Solution;
use ospf_rust_core::solver::heuristic::individual::Individual;
use ospf_rust_core::solver::heuristic::model::{ObjectiveModel, VariableModel};
use ospf_rust_core::solver::heuristic::operator::archive::{
    Archive, BestArchive, HistoryBestArchive,
};
use ospf_rust_core::variable::VariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::cmp::Ordering;

use crate::{BridgeError, Result};

/// 一轮更新的读数。 / One update round's reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchiveStepRow {
    /// `update` 的返回值。 / What `update` returned.
    pub improved: bool,
    /// 这一轮之后档案里的个体编号。 / The archived ids after this round.
    pub members: Vec<u32>,
}

/// 档案探针请求。 / The archive probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchiveProbeRequest {
    /// 逐批送进来的适应度。 / The fitness values handed in, batch by batch.
    #[serde(default)]
    pub batches: Vec<Vec<f64>>,
    /// `BestArchive` 的容量（默认 2）。 / The `BestArchive` capacity (default 2).
    #[serde(default)]
    pub amount: Option<u32>,
}

/// 档案探针结论。 / The archive probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchiveProbeReport {
    /// `BestArchive` 每轮的读数。 / The `BestArchive` reading per round.
    pub best: Vec<ArchiveStepRow>,
    /// `HistoryBestArchive` 每轮的读数。 / The `HistoryBestArchive` reading per round.
    pub history: Vec<ArchiveStepRow>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_first_update_always_reports_improvement: bool,
    pub a_zero_capacity_archive_swallows_everything: bool,
    pub an_equal_candidate_is_stored_but_never_reported: bool,
    pub the_flag_tracks_the_archive_best: bool,
    pub a_tie_keeps_the_older_entry_first: bool,
    pub clear_resets_the_flag: bool,
    pub the_history_archive_reports_any_improvement: bool,
    pub the_history_archive_never_takes_an_equal_candidate: bool,
}

/// 探针模型：目标值是 `(全局编号, 适应度)`，于是读面能逐条认出个体来源。
/// The probe model uses `(global id, fitness)` so the read face can name every individual.
struct ProbeModel;

impl VariableModel<f64> for ProbeModel {
    fn num_variables(&self) -> usize { 0 }
    fn variable_order(&self) -> Vec<VariableId> { Vec::new() }
    fn lower_bound(&self, _index: usize) -> Option<f64> { None }
    fn upper_bound(&self, _index: usize) -> Option<f64> { None }
    fn coerce_in(&self, _index: usize, value: f64) -> f64 { value }
    fn initial_solutions(&self, _amount: usize) -> Vec<Solution<f64>> { Vec::new() }
    fn constraint_satisfied(&self, _solution: &Solution<f64>) -> Option<bool> { Some(true) }
}

impl ObjectiveModel<(u32, f64), f64> for ProbeModel {
    fn default_objective(&self) -> Option<(u32, f64)> { None }
    fn objective(&self, _solution: &Solution<f64>) -> Option<(u32, f64)> { None }
    fn compare_objective(&self, lhs: &(u32, f64), rhs: &(u32, f64)) -> Option<Ordering> {
        lhs.1.partial_cmp(&rhs.1)
    }
}

/// 跑两条档案路。 / Run both archive routes.
fn update_all(batches: &[Vec<f64>], amount: usize) -> (Vec<ArchiveStepRow>, Vec<ArchiveStepRow>) {
    let model = ProbeModel;
    let mut next = 0_u32;
    let mut best = BestArchive::new(amount);
    let mut history = HistoryBestArchive::new();
    let mut best_rows = Vec::with_capacity(batches.len());
    let mut history_rows = Vec::with_capacity(batches.len());
    for batch in batches {
        let candidates: Vec<Individual<(u32, f64), f64>> = batch.iter().map(|fitness| {
            let id = next;
            next = next.saturating_add(1);
            Individual::new(Solution::new(), (id, *fitness))
        }).collect();
        let best_improved = best.update(&model, candidates.clone());
        best_rows.push(ArchiveStepRow { improved: best_improved,
            members: best.individuals().iter().map(|entry| entry.fitness.0).collect() });
        let history_improved = history.update(&model, candidates);
        history_rows.push(ArchiveStepRow { improved: history_improved,
            members: history.individuals().iter().map(|entry| entry.fitness.0).collect() });
    }
    (best_rows, history_rows)
}

/// 走一遍档案探针。非法输入是**错误**。 / Walk the archive probe.
pub fn verify_archive_probe(request: &ArchiveProbeRequest) -> Result<ArchiveProbeReport> {
    if request.batches.len() > 8 {
        return Err(BridgeError::new("InvalidInput",
            "archive probe failed validation: at most eight batches are accepted"));
    }
    if request.batches.iter().any(|batch| batch.len() > 8) {
        return Err(BridgeError::new("InvalidInput",
            "archive probe failed validation: at most eight candidates per batch"));
    }
    if request.batches.iter().flatten().any(|fitness| !fitness.is_finite()) {
        return Err(BridgeError::new("InvalidInput", "archive probe failed validation: \
            non-finite fitness cannot cross the boundary"));
    }
    let amount = usize::try_from(request.amount.unwrap_or(2))
        .map_err(|_| BridgeError::new("InvalidInput",
            "archive probe failed validation: the capacity overflowed"))?;
    if amount > 8 {
        return Err(BridgeError::new("InvalidInput",
            "archive probe failed validation: the capacity stays at eight"));
    }
    let findings = canonical_findings();
    let (best, history) = update_all(&request.batches, amount);
    Ok(ArchiveProbeReport {
        best,
        history,
        the_first_update_always_reports_improvement:
            findings.the_first_update_always_reports_improvement,
        a_zero_capacity_archive_swallows_everything:
            findings.a_zero_capacity_archive_swallows_everything,
        an_equal_candidate_is_stored_but_never_reported:
            findings.an_equal_candidate_is_stored_but_never_reported,
        the_flag_tracks_the_archive_best: findings.the_flag_tracks_the_archive_best,
        a_tie_keeps_the_older_entry_first: findings.a_tie_keeps_the_older_entry_first,
        clear_resets_the_flag: findings.clear_resets_the_flag,
        the_history_archive_reports_any_improvement:
            findings.the_history_archive_reports_any_improvement,
        the_history_archive_never_takes_an_equal_candidate:
            findings.the_history_archive_never_takes_an_equal_candidate,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_first_update_always_reports_improvement: bool,
    a_zero_capacity_archive_swallows_everything: bool,
    an_equal_candidate_is_stored_but_never_reported: bool,
    the_flag_tracks_the_archive_best: bool,
    a_tie_keeps_the_older_entry_first: bool,
    clear_resets_the_flag: bool,
    the_history_archive_reports_any_improvement: bool,
    the_history_archive_never_takes_an_equal_candidate: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 首轮必报改进。 / The first update always reports improvement.
    let the_first_update_always_reports_improvement = {
        // 塞进来的是最差的也报（`None → Some`）。 / Even the worst candidate reports, since `None` becomes `Some`.
        let (best, history) = update_all(&[vec![100.0]], 2);
        best[0].improved && history[0].improved && best[0].members == vec![0]
    };
    // 2) 容量 0 的档案把一切吞掉。 / A zero-capacity archive swallows everything.
    let a_zero_capacity_archive_swallows_everything = {
        let (best, _) = update_all(&[vec![1.0, 2.0], vec![3.0]], 0);
        // 永远为空、永远 `false`——连首轮都不报。 / Always empty and always false, not even the first round reports.
        best.iter().all(|row| !row.improved && row.members.is_empty())
            // 对照：容量 1 时首轮就报真。 / Control: a capacity of one reports true on the first round.
            && update_all(&[vec![1.0, 2.0]], 1).0[0].improved
    };
    // 3) 等值候选被存下却不报改进。 / An equal candidate is stored but never reported.
    let an_equal_candidate_is_stored_but_never_reported = {
        let (best, _) = update_all(&[vec![5.0], vec![5.0]], 3);
        // 第一轮报真、第二轮报假，而档案从 1 个变成 2 个。 
        // The first round reports true, the second false, yet the archive grows from one member to two.
        best[0].improved && !best[1].improved
            && best[0].members == vec![0] && best[1].members == vec![0, 1]
    };
    // 4) 旗标跟踪的是档案最优而不是这批候选。 / The flag tracks the archive best rather than the batch.
    let the_flag_tracks_the_archive_best = {
        let (best, _) = update_all(&[vec![5.0], vec![9.0, 8.0], vec![4.0]], 3);
        // 整批都更差 → 假；只要有一个更好 → 真（第三批的个体是编号 3，排到档案首位）。 
        // An all-worse batch is false while any better one is true; the third batch's individual is id 3 and heads the
        // archive.
        !best[1].improved && best[2].improved
            && best[2].members == vec![3, 0, 2]
    };
    // 5) 并列时先来的排前面。 / A tie keeps the older entry first.
    let a_tie_keeps_the_older_entry_first = {
        // 同一批里两个等值候选：稳定排序保插入顺序。 
        // Two equal candidates in one batch keep their insertion order through the stable sort.
        let (best, _) = update_all(&[vec![7.0, 7.0, 7.0]], 3);
        best[0].members == vec![0, 1, 2]
    };
    // 6) `clear()` 会让旗标重新变真。 / `clear()` resets the flag.
    let clear_resets_the_flag = {
        let model = ProbeModel;
        let mut best = BestArchive::new(2);
        let first = best.update(&model, vec![Individual::new(Solution::new(), (0_u32, 5.0))]);
        let second = best.update(&model, vec![Individual::new(Solution::new(), (1_u32, 9.0))]);
        best.clear();
        let after = best.update(&model, vec![Individual::new(Solution::new(), (2_u32, 100.0))]);
        // 首轮真、差批假、清空后又真。 / True, then false on a worse batch, then true again after the clear.
        first && !second && after && best.individuals().len() == 1
    };
    // 7) 历史最优档案对"这批里任何一个更优"都报改进。 
    // The history archive reports any improvement in the batch.
    let the_history_archive_reports_any_improvement = {
        // 第一轮种下 5，第二轮 `[9, 3]`：9 不更优、3 更优 → 真，最终留下的是**编号 2**（适应度 3）。 
        // The first round seeds five; the second hands in `[9, 3]`, where nine is not better but three is, so the flag is
        // true and **id 2** (fitness three) wins.
        let (_, history) = update_all(&[vec![5.0], vec![9.0, 3.0]], 2);
        history[0].improved && history[1].improved && history[1].members == vec![2]
            // 整批都更差 → 假。 / An all-worse batch is false.
            && { let (_, all_worse) = update_all(&[vec![5.0], vec![9.0, 8.0]], 2);
                !all_worse[1].improved && all_worse[1].members == vec![0] }
    };
    // 8) 历史最优档案永不收下等值候选。 / The history archive never takes an equal candidate.
    let the_history_archive_never_takes_an_equal_candidate = {
        // 第二轮是等值 → 假，且**原来的编号 0 继续留着**。 
        // The second round ties, so the flag is false and the original id 0 survives.
        let (_, history) = update_all(&[vec![5.0], vec![5.0]], 2);
        !history[1].improved && history[1].members == vec![0]
            // 对照：档案可能同时存下等值（上面第 3 条），两者语义不同。 
            // Control: the elite archive may store the equal candidate instead, as in rule three above.
            && update_all(&[vec![5.0], vec![5.0]], 2).0[1].members == vec![0, 1]
    };
    Findings {
        the_first_update_always_reports_improvement,
        a_zero_capacity_archive_swallows_everything,
        an_equal_candidate_is_stored_but_never_reported,
        the_flag_tracks_the_archive_best,
        a_tie_keeps_the_older_entry_first,
        clear_resets_the_flag,
        the_history_archive_reports_any_improvement,
        the_history_archive_never_takes_an_equal_candidate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ArchiveProbeRequest {
        ArchiveProbeRequest { batches: vec![vec![5.0], vec![9.0, 8.0], vec![4.0]],
            amount: Some(3) }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = verify_archive_probe(&request()).unwrap();
        assert!(report.the_first_update_always_reports_improvement);
        assert!(report.a_zero_capacity_archive_swallows_everything);
        assert!(report.an_equal_candidate_is_stored_but_never_reported);
        assert!(report.the_flag_tracks_the_archive_best);
        assert!(report.a_tie_keeps_the_older_entry_first);
        assert!(report.clear_resets_the_flag);
        assert!(report.the_history_archive_reports_any_improvement);
        assert!(report.the_history_archive_never_takes_an_equal_candidate);
        assert_eq!(report.best[0].members, vec![0]);
        // 更差的候选照样进档案（只是不报改进）：`[0(5), 2(8), 1(9)]`。 
        // Worse candidates still enter the archive, they merely report no improvement: `[0(5), 2(8), 1(9)]`.
        assert_eq!(report.best[1].members, vec![0, 2, 1]);
        assert_eq!(report.best[2].members, vec![3, 0, 2]);
        assert!(report.best[0].improved);
        assert!(!report.best[1].improved);
        assert!(report.best[2].improved);
        assert_eq!(report.history[0].members, vec![0]);
        assert_eq!(report.history[2].members, vec![3]);
    }

    #[test]
    fn a_first_round_with_the_worst_candidate_still_reports_true() {
        let report = verify_archive_probe(&ArchiveProbeRequest { batches: vec![vec![100.0]],
            amount: Some(2) }).unwrap();
        assert!(report.best[0].improved);
        assert!(report.history[0].improved);
    }

    #[test]
    fn a_zero_capacity_archive_swallows_everything() {
        let report = verify_archive_probe(&ArchiveProbeRequest { batches: vec![vec![1.0, 2.0],
            vec![3.0]], amount: Some(0) }).unwrap();
        assert!(report.best.iter().all(|row| !row.improved && row.members.is_empty()));
        assert!(report.history[0].improved);
        assert_eq!(report.history[1].members, vec![0]);
    }

    #[test]
    fn an_equal_candidate_grows_the_elite_archive_without_reporting() {
        let report = verify_archive_probe(&ArchiveProbeRequest { batches: vec![vec![5.0],
            vec![5.0]], amount: Some(3) }).unwrap();
        assert!(report.best[0].improved);
        assert!(!report.best[1].improved);
        assert_eq!(report.best[1].members, vec![0, 1]);
        assert!(!report.history[1].improved);
        assert_eq!(report.history[1].members, vec![0]);
    }

    #[test]
    fn the_capacity_truncates_the_tail() {
        let report = verify_archive_probe(&ArchiveProbeRequest {
            batches: vec![vec![5.0, 6.0, 7.0]], amount: Some(2) }).unwrap();
        assert_eq!(report.best[0].members, vec![0, 1]);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_archive_probe(&ArchiveProbeRequest { batches: vec![Vec::new(); 9],
            amount: Some(1) }).unwrap_err().message.contains("at most eight batches"));
        assert!(verify_archive_probe(&ArchiveProbeRequest { batches: vec![vec![1.0; 9]],
            amount: Some(1) }).unwrap_err().message.contains("at most eight candidates"));
        assert!(verify_archive_probe(&ArchiveProbeRequest { batches: vec![vec![f64::NAN]],
            amount: Some(1) }).unwrap_err().message.contains("non-finite fitness"));
        assert!(verify_archive_probe(&ArchiveProbeRequest { batches: vec![vec![1.0]],
            amount: Some(9) }).unwrap_err().message.contains("the capacity stays at eight"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_archive_probe(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"improved\""), "{json}");
        assert!(json.contains("\"theFlagTracksTheArchiveBest\""), "{json}");
        assert!(json.contains("\"aZeroCapacityArchiveSwallowsEverything\""), "{json}");
        assert!(!json.contains("the_flag_tracks"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ArchiveProbeReport>(&copy.to_string()).is_err());
    }
}
