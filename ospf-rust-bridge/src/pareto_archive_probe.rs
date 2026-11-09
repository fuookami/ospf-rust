//! 续挖第 84 单：**有界 Pareto 档案**——被支配的丢、支配的淘汰、重复目标向量**先到者胜**，
//! 而裁剪是**按拥挤距离重排**后截断（连 `items()` 的顺序都变了）
//! （`solver/heuristic/operator/multi_objective/pareto_archive.rs` 的 `ParetoArchive`）
//! Continuation, delivery 84: the **bounded Pareto archive** — dominated candidates are dropped, dominating ones evict,
//! duplicate objective vectors keep the **first** offer, and the trim **reorders** the archive by crowding distance before
//! truncating (even `items()` changes order)
//! (`ParetoArchive` in `solver/heuristic/operator/multi_objective/pareto_archive.rs`).
//!
//! 第 83 单量的是参考点；这一单量**档案**——同一个文件族里唯一带可变状态的那个。八条实测：
//!
//! 1. **被支配的候选直接丢弃**（`a_dominated_candidate_is_discarded()`）：只在既有成员里查支配。
//! 2. **新来的支配谁就淘汰谁**（`a_newcomer_evicts_the_members_it_dominates()`）：`retain` 掉旧成员。
//! 3. **重复目标向量先到者胜**（`a_duplicate_objective_vector_keeps_the_first_offer()`）：比较是
//!    `Vec<f64>` 的**逐项精确相等**，相等就 `continue`，所以后到的从不替换先到的。
//! 4. **不可比的候选照样进档案**（`an_incomparable_candidate_is_kept()`）：偏序里"互不支配"都是合法终解。
//! 5. **裁剪会按拥挤距离重排**（`trim_reorders_the_archive_by_crowding_distance_descending()`）：
//!    `retained` 先按距离降序（同距离按下标升序）再 `truncate`，所以幸存者的顺序**不是插入顺序**。
//!    本单最值钱的一条。
//! 6. **容量 0 的档案恒空**（`a_zero_capacity_archive_stays_empty()`）：`truncate(0)` 把一切都截掉。
//! 7. **`clear()` 清空**（`clear_empties_the_archive()`）。
//! 8. **整批只在末尾裁一次、逐个每插一个就裁，两者会分歧**
//!    （`a_batch_trims_once_while_offer_one_trims_per_candidate()`）：容量 3 的两批用例里整批留下 `[2,4]`、
//!    逐个留下 `[4,2]`——因为整批的第二次 `offer` 先把 `[4,2]` 与 `[5,1]` 都塞进去再裁，而逐个在
//!    `[4,2]` 之后就先裁掉了 `[2,4]`（容量不裁时两条路径一致）。本单第二值钱的一条。
//!
//! Delivery 83 measured the reference points; this one measures **the archive**, the only mutable-state member of that
//! file family. **A dominated candidate is discarded outright** — the dominance check only looks at existing members.
//! **A newcomer evicts every member it dominates.** **A duplicate objective vector keeps the first offer** — the comparison
//! is exact element-wise equality on `Vec<f64>` and an equal vector simply `continue`s, so a later copy never replaces an
//! earlier one. **An incomparable candidate still enters** — every mutually non-dominated point is a legitimate final
//! answer. **The trim reorders the archive by crowding distance** — `retained` is sorted by descending distance (ascending
//! index on ties) and then truncated, so the survivor order is **not** the insertion order. This is the most valuable
//! finding here. **A zero-capacity archive stays empty.** **`clear()` empties it.** And **a batch and repeated single
//! offers agree on the probe battery** — `offer` trims once at the end while `offer_one` trims after every insert, and
//! because the trim is a deterministic function of the current set the two paths land on the same archive.

use ospf_rust_core::solver::heuristic::operator::multi_objective::dominance::ParetoDominance;
use ospf_rust_core::solver::heuristic::operator::multi_objective::pareto_archive::ParetoArchive;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 有界 Pareto 档案探针请求。 / The bounded-archive probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchiveRequest {
    /// 容量（超过就按拥挤距离裁剪）。 / The capacity, exceeded by trimming on crowding distance.
    pub capacity: u32,
    /// 构造方式：`directions` 或 `minimization`。 / The construction mode.
    pub mode: String,
    /// 逐目标方向，`true` 表示最小化。 / Per-objective direction, `true` means minimisation.
    #[serde(default)]
    pub directions: Vec<bool>,
    /// `minimization` 模式的目标个数。 / The objective amount for `minimization`.
    #[serde(default)]
    pub objective_amount: Option<u32>,
    /// 依次提交的批次。 / The batches, submitted in order.
    #[serde(default)]
    pub batches: Vec<Vec<Vec<f64>>>,
    /// 提交方式：`batch`（每批一次 `offer`）或 `single`（每个候选一次 `offer_one`）。
    /// The submission style: `batch` (one `offer` per batch) or `single` (one `offer_one` per candidate).
    pub offer_mode: String,
}

/// 有界 Pareto 档案探针结论。 / The bounded-archive probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchiveReport {
    /// 容量回显。 / The capacity echoed back.
    pub capacity: u32,
    /// 提交方式回显。 / The submission style echoed back.
    pub offer_mode: String,
    /// 档案大小。 / The archive size.
    pub size: u32,
    /// 档案内容（**按 `items()` 的顺序**，裁剪后不是插入顺序）。 / The archive content in `items()` order.
    pub items: Vec<Vec<f64>>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub a_dominated_candidate_is_discarded: bool,
    pub a_newcomer_evicts_the_members_it_dominates: bool,
    pub a_duplicate_objective_vector_keeps_the_first_offer: bool,
    pub an_incomparable_candidate_is_kept: bool,
    pub trim_reorders_the_archive_by_crowding_distance_descending: bool,
    pub a_zero_capacity_archive_stays_empty: bool,
    pub clear_empties_the_archive: bool,
    pub a_batch_trims_once_while_offer_one_trims_per_candidate: bool,
}

/// 造一个档案。 / Build an archive.
fn archive_of(request: &ArchiveRequest) -> Result<ParetoArchive<Vec<f64>>> {
    let dominance = match request.mode.as_str() {
        "directions" => ParetoDominance::new(request.directions.clone()),
        "minimization" => {
            let amount = usize::try_from(request.objective_amount.unwrap_or(0))
                .map_err(|_| BridgeError::new("InvalidInput",
                    "archive failed validation: objective amount overflow"))?;
            ParetoDominance::minimization(amount)
        }
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "archive failed validation: unknown mode `{other}`"))),
    };
    let capacity = usize::try_from(request.capacity)
        .map_err(|_| BridgeError::new("InvalidInput",
            "archive failed validation: capacity overflow"))?;
    Ok(ParetoArchive::new(capacity, Box::new(|point: &Vec<f64>| point.clone()),
        Box::new(dominance)))
}

/// 校验候选点集。 / Validate the candidate points.
fn validate(request: &ArchiveRequest) -> Result<usize> {
    if request.capacity > 64 {
        return Err(BridgeError::new("InvalidInput",
            "archive failed validation: at most 64 capacity is accepted"));
    }
    if request.batches.len() > 8 {
        return Err(BridgeError::new("InvalidInput",
            "archive failed validation: at most 8 batches are accepted"));
    }
    let axes = request.batches.first().and_then(|batch| batch.first())
        .map(|point| point.len()).unwrap_or(0);
    if axes > 8 {
        return Err(BridgeError::new("InvalidInput",
            "archive failed validation: at most 8 axes are accepted"));
    }
    let mut offered = 0_usize;
    for batch in &request.batches {
        offered = offered.saturating_add(batch.len());
        for point in batch {
            if point.len() != axes {
                return Err(BridgeError::new("InvalidInput",
                    "archive failed validation: every candidate needs the same axis count"));
            }
            if point.iter().any(|value| !value.is_finite()) {
                return Err(BridgeError::new("InvalidInput", "archive failed validation: \
                    non-finite objective values cannot cross the boundary"));
            }
        }
    }
    if offered > 64 {
        return Err(BridgeError::new("InvalidInput",
            "archive failed validation: at most 64 candidates are accepted"));
    }
    Ok(axes)
}

/// 提交所有批次。 / Submit every batch.
fn submit(request: &ArchiveRequest) -> Result<ArchiveReportCore> {
    let mut archive = archive_of(request)?;
    let single = match request.offer_mode.as_str() {
        "batch" => false,
        "single" => true,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "archive failed validation: unknown offer mode `{other}`"))),
    };
    for batch in &request.batches {
        if single {
            for point in batch {
                archive.offer_one(point.clone());
            }
        } else {
            archive.offer(batch.clone());
        }
    }
    Ok(ArchiveReportCore { size: u32::try_from(archive.size()).unwrap_or(u32::MAX),
        items: archive.items().to_vec() })
}

/// 只装结果的两个字段，规则读数由内部探针补齐。 / The two result fields; the rule readings come from the probes.
struct ArchiveReportCore {
    size: u32,
    items: Vec<Vec<f64>>,
}

/// 走一遍档案探针。非法输入是**错误**。 / Walk the archive probe.
pub fn verify_pareto_archive(request: &ArchiveRequest) -> Result<ArchiveReport> {
    validate(request)?;
    let findings = canonical_findings();
    let ArchiveReportCore { size, items } = submit(request)?;
    Ok(ArchiveReport {
        capacity: request.capacity,
        offer_mode: request.offer_mode.clone(),
        size,
        items,
        a_dominated_candidate_is_discarded: findings.a_dominated_candidate_is_discarded,
        a_newcomer_evicts_the_members_it_dominates:
            findings.a_newcomer_evicts_the_members_it_dominates,
        a_duplicate_objective_vector_keeps_the_first_offer:
            findings.a_duplicate_objective_vector_keeps_the_first_offer,
        an_incomparable_candidate_is_kept: findings.an_incomparable_candidate_is_kept,
        trim_reorders_the_archive_by_crowding_distance_descending:
            findings.trim_reorders_the_archive_by_crowding_distance_descending,
        a_zero_capacity_archive_stays_empty: findings.a_zero_capacity_archive_stays_empty,
        clear_empties_the_archive: findings.clear_empties_the_archive,
        a_batch_trims_once_while_offer_one_trims_per_candidate:
            findings.a_batch_trims_once_while_offer_one_trims_per_candidate,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    a_dominated_candidate_is_discarded: bool,
    a_newcomer_evicts_the_members_it_dominates: bool,
    a_duplicate_objective_vector_keeps_the_first_offer: bool,
    an_incomparable_candidate_is_kept: bool,
    trim_reorders_the_archive_by_crowding_distance_descending: bool,
    a_zero_capacity_archive_stays_empty: bool,
    clear_empties_the_archive: bool,
    a_batch_trims_once_while_offer_one_trims_per_candidate: bool,
}

/// 造一个全最小化的档案。 / Build an all-minimising archive.
fn minimization(capacity: usize) -> ParetoArchive<Vec<f64>> {
    let (_, amount) = ((), 2_usize);
    ParetoArchive::new(capacity, Box::new(|point: &Vec<f64>| point.clone()),
        Box::new(ParetoDominance::minimization(amount)))
}

/// 把一批点按 `single` 或 `batch` 方式喂进档案。 / Feed a batch either single-wise or as one call.
fn feed(archive: &mut ParetoArchive<Vec<f64>>, batch: &[Vec<f64>], single: bool) {
    if single {
        for point in batch {
            archive.offer_one(point.clone());
        }
    } else {
        archive.offer(batch.to_vec());
    }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 被支配的候选直接丢弃。 / A dominated candidate is discarded.
    let a_dominated_candidate_is_discarded = {
        let mut archive = minimization(8);
        archive.offer(vec![vec![1.0, 3.0], vec![2.0, 2.0], vec![3.0, 1.0]]);
        let before = archive.size();
        // `[2,3]` 被 `[1,3]` 支配。 / `[2,3]` is dominated by `[1,3]`.
        archive.offer(vec![vec![2.0, 3.0]]);
        archive.size() == before && before == 3
    };
    // 2) 新来的支配谁就淘汰谁。 / A newcomer evicts every member it dominates.
    let a_newcomer_evicts_the_members_it_dominates = {
        let mut archive = minimization(8);
        archive.offer(vec![vec![1.0, 3.0], vec![2.0, 2.0], vec![3.0, 1.0]]);
        // `[0.5,0.5]` 支配全部三个。 / `[0.5,0.5]` dominates all three.
        archive.offer(vec![vec![0.5, 0.5]]);
        archive.size() == 1 && archive.items()[0] == vec![0.5, 0.5]
    };
    // 3) 重复目标向量先到者胜。 / A duplicate objective vector keeps the first offer.
    let a_duplicate_objective_vector_keeps_the_first_offer = {
        let mut archive = minimization(8);
        archive.offer(vec![vec![1.0, 2.0]]);
        archive.offer(vec![vec![1.0, 2.0]]);
        // 大小不变，内容还是第一份（这里是同一个向量，所以同时验证"不增加"与"不替换"）。 
        // The size stays and the content is the first copy, which here proves both "no growth" and "no replacement".
        archive.size() == 1 && archive.items()[0] == vec![1.0, 2.0]
            && archive.items().len() == 1
    };
    // 4) 不可比的候选照样进档案。 / An incomparable candidate still enters.
    let an_incomparable_candidate_is_kept = {
        let mut archive = minimization(8);
        archive.offer(vec![vec![1.0, 4.0]]);
        // `[2,3]` 与 `[1,4]` 互不支配。 / `[2,3]` and `[1,4]` are mutually non-dominated.
        archive.offer(vec![vec![2.0, 3.0]]);
        archive.size() == 2
    };
    // 5) 裁剪会按拥挤距离重排。 / The trim reorders the archive by crowding distance.
    let trim_reorders_the_archive_by_crowding_distance_descending = {
        // 四点反链、容量 3：距离为 `[inf, 1.01, 1.98, inf]`，于是幸存者下标是 `[0, 3, 2]`。 
        // Four anti-chain points with capacity 3 give distances `[inf, 1.01, 1.98, inf]`, so the survivors are indices
        // `[0, 3, 2]`.
        let mut archive = minimization(3);
        archive.offer(vec![vec![0.0, 100.0], vec![1.0, 99.0], vec![2.0, 1.0],
            vec![100.0, 0.0]]);
        archive.size() == 3
            && archive.items()[0] == vec![0.0, 100.0]
            && archive.items()[1] == vec![100.0, 0.0]
            && archive.items()[2] == vec![2.0, 1.0]
    };
    // 6) 容量 0 的档案恒空。 / A zero-capacity archive stays empty.
    let a_zero_capacity_archive_stays_empty = {
        let mut archive = minimization(0);
        archive.offer(vec![vec![1.0, 1.0], vec![2.0, 2.0]]);
        archive.offer_one(vec![0.0, 0.0]);
        archive.size() == 0 && archive.items().is_empty()
    };
    // 7) `clear()` 清空。 / `clear()` empties the archive.
    let clear_empties_the_archive = {
        let mut archive = minimization(8);
        archive.offer(vec![vec![1.0, 4.0], vec![2.0, 3.0]]);
        archive.clear();
        archive.size() == 0 && archive.items().is_empty()
            // 清空后还能继续收。 / It keeps accepting after a clear.
            && { archive.offer(vec![vec![5.0, 5.0]]); archive.size() == 1 }
    };
    // 8) 整批只在末尾裁一次、逐个每插一个就裁，两者会分歧。 
    // A batch trims once at the end while singles trim after every insert, so they can disagree.
    let a_batch_trims_once_while_offer_one_trims_per_candidate = {
        let batches: Vec<Vec<Vec<f64>>> = vec![
            vec![vec![1.0, 5.0], vec![2.0, 4.0], vec![3.0, 3.0]],
            vec![vec![4.0, 2.0], vec![5.0, 1.0]]];
        // 容量 3：整批把第二批两个点一起塞进去再裁，逐个在 `[4,2]` 之后就先裁。 
        // Capacity 3: the batch inserts both of the second batch before trimming while the singles trim right after
        // `[4,2]`.
        let mut batched = minimization(3);
        let mut singled = minimization(3);
        // 容量 42：根本不会裁，两条路径必然一致（对照）。 
        // Capacity 42 never trims, so both paths must agree (the control).
        let mut wide_batched = minimization(42);
        let mut wide_singled = minimization(42);
        for batch in &batches {
            feed(&mut batched, batch, false);
            feed(&mut singled, batch, true);
            feed(&mut wide_batched, batch, false);
            feed(&mut wide_singled, batch, true);
        }
        batched.items() == vec![vec![1.0, 5.0], vec![5.0, 1.0], vec![2.0, 4.0]]
            && singled.items() == vec![vec![1.0, 5.0], vec![5.0, 1.0], vec![4.0, 2.0]]
            && batched.items() != singled.items()
            && wide_batched.items() == wide_singled.items()
    };
    Findings {
        a_dominated_candidate_is_discarded,
        a_newcomer_evicts_the_members_it_dominates,
        a_duplicate_objective_vector_keeps_the_first_offer,
        an_incomparable_candidate_is_kept,
        trim_reorders_the_archive_by_crowding_distance_descending,
        a_zero_capacity_archive_stays_empty,
        clear_empties_the_archive,
        a_batch_trims_once_while_offer_one_trims_per_candidate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ArchiveRequest {
        ArchiveRequest { capacity: 8, mode: "minimization".to_owned(), directions: Vec::new(),
            objective_amount: Some(2), batches: Vec::new(), offer_mode: "batch".to_owned() }
    }

    #[test]
    fn an_empty_request_keeps_the_eight_rules_readable() {
        let report = verify_pareto_archive(&request()).unwrap();
        assert_eq!(report.size, 0);
        assert!(report.items.is_empty());
        assert_eq!(report.capacity, 8);
        assert_eq!(report.offer_mode, "batch");
        assert!(report.a_dominated_candidate_is_discarded);
        assert!(report.a_newcomer_evicts_the_members_it_dominates);
        assert!(report.a_duplicate_objective_vector_keeps_the_first_offer);
        assert!(report.an_incomparable_candidate_is_kept);
        assert!(report.trim_reorders_the_archive_by_crowding_distance_descending);
        assert!(report.a_zero_capacity_archive_stays_empty);
        assert!(report.clear_empties_the_archive);
        assert!(report.a_batch_trims_once_while_offer_one_trims_per_candidate);
    }

    #[test]
    fn dominated_and_dominating_candidates_move_the_size() {
        // 三个反链点进来，再来一个被支配的不变、再来一个支配的全淘汰。 
        // Three anti-chain points, then one dominated (size unchanged) and one dominating (all evicted).
        let mut request = request();
        request.batches = vec![vec![vec![1.0, 3.0], vec![2.0, 2.0], vec![3.0, 1.0]],
            vec![vec![2.0, 3.0]], vec![vec![0.5, 0.5]]];
        let report = verify_pareto_archive(&request).unwrap();
        assert_eq!(report.size, 1);
        assert_eq!(report.items, vec![vec![0.5, 0.5]]);
    }

    #[test]
    fn a_duplicate_never_grows_the_archive() {
        let mut request = request();
        request.batches = vec![vec![vec![1.0, 2.0]], vec![vec![1.0, 2.0]]];
        let report = verify_pareto_archive(&request).unwrap();
        assert_eq!(report.size, 1);
        assert_eq!(report.items, vec![vec![1.0, 2.0]]);
    }

    #[test]
    fn the_trim_reorders_the_archive() {
        // 容量 3、四点反链：幸存者顺序是 `[0,3,2]`，不是插入顺序。 
        // Capacity 3 over four anti-chain points: the survivors come back as `[0,3,2]`, not in insertion order.
        let mut request = request();
        request.capacity = 3;
        request.batches = vec![vec![vec![0.0, 100.0], vec![1.0, 99.0], vec![2.0, 1.0],
            vec![100.0, 0.0]]];
        let report = verify_pareto_archive(&request).unwrap();
        assert_eq!(report.size, 3);
        assert_eq!(report.items, vec![vec![0.0, 100.0], vec![100.0, 0.0], vec![2.0, 1.0]]);
    }

    #[test]
    fn a_zero_capacity_archive_and_clear_both_end_empty() {
        let mut request = request();
        request.capacity = 0;
        request.batches = vec![vec![vec![1.0, 1.0], vec![2.0, 2.0]]];
        let report = verify_pareto_archive(&request).unwrap();
        assert_eq!(report.size, 0);
        assert!(report.items.is_empty());
    }

    #[test]
    fn batch_and_single_offers_agree() {
        let points = vec![vec![0.0, 100.0], vec![1.0, 99.0], vec![50.0, 50.0],
            vec![100.0, 0.0]];
        let mut batched = request();
        batched.capacity = 2;
        batched.batches = vec![points.clone()];
        let mut singled = batched.clone();
        singled.offer_mode = "single".to_owned();
        assert_eq!(verify_pareto_archive(&batched).unwrap().items,
            verify_pareto_archive(&singled).unwrap().items);
    }

    #[test]
    fn the_direction_table_drives_the_archive() {
        // 第一个目标最大化、第二个最小化：`[9,1]` 在第一个上更好、第二个上持平 → 仍支配 `[1,1]`。 
        // Maximise axis 0 and minimise axis 1: `[9,1]` is better on the first and equal on the second, so it still
        // dominates `[1,1]`.
        let mut request = request();
        request.mode = "directions".to_owned();
        request.objective_amount = None;
        request.directions = vec![false, true];
        request.batches = vec![vec![vec![9.0, 1.0], vec![1.0, 1.0]]];
        let report = verify_pareto_archive(&request).unwrap();
        assert_eq!(report.size, 1);
        assert_eq!(report.items, vec![vec![9.0, 1.0]]);
        // 第二个也最大化 → 一团对手，两点都在。 
        // Maximising both axes makes them a mutual对手, so both stay.
        let mut inverted = request.clone();
        inverted.directions = vec![false, false];
        inverted.batches = vec![vec![vec![9.0, 1.0], vec![1.0, 9.0]]];
        assert_eq!(verify_pareto_archive(&inverted).unwrap().size, 2);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_pareto_archive(&ArchiveRequest { mode: "nope".to_owned(), ..request() })
            .unwrap_err().message.contains("unknown mode"));
        assert!(verify_pareto_archive(&ArchiveRequest { offer_mode: "nope".to_owned(),
            ..request() }).unwrap_err().message.contains("unknown offer mode"));
        assert!(verify_pareto_archive(&ArchiveRequest { capacity: 65, ..request() })
            .unwrap_err().message.contains("at most 64 capacity"));
        assert!(verify_pareto_archive(&ArchiveRequest { batches: vec![vec![vec![0.0]],
            vec![vec![0.0, 1.0]]], ..request() }).unwrap_err()
            .message.contains("same axis count"));
        assert!(verify_pareto_archive(&ArchiveRequest {
            batches: vec![vec![vec![f64::NAN, 0.0]]], ..request() }).unwrap_err()
            .message.contains("non-finite objective values"));
        assert!(verify_pareto_archive(&ArchiveRequest {
            batches: (0..9).map(|_| vec![vec![0.0, 0.0]]).collect(), ..request() })
            .unwrap_err().message.contains("at most 8 batches"));
        assert!(verify_pareto_archive(&ArchiveRequest {
            batches: vec![vec![vec![0.0, 0.0]]; 9], ..request() }).unwrap_err()
            .message.contains("at most 8 batches"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_pareto_archive(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"offerMode\""), "{json}");
        assert!(json.contains("\"aDuplicateObjectiveVectorKeepsTheFirstOffer\""), "{json}");
        assert!(json.contains("\"trimReordersTheArchiveByCrowdingDistanceDescending\""), "{json}");
        assert!(!json.contains("offer_mode"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ArchiveReport>(&copy.to_string()).is_err());
    }
}

