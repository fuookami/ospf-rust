//! 续挖第 92 单：**两种岛屿迁移的空岛与并列语义**——`RingMigration` 的 `.max(1)` 让**迁移率 0 也照迁一个**、
//! 空岛只让它的后继拿到 0 个（其余岛照旧）；`BestToWorstMigration` 则是**任一岛为空就整体放弃**，而且
//! **最优岛与最差岛并列时一个都不动**；再往外，**目标岛可能被撑大**（迁移量由来源岛而不是目标岛决定）
//! （`solver/heuristic/operator/migration.rs` 的 `NoMigration` / `RingMigration` / `BestToWorstMigration`）
//! Continuation, delivery 92: the **empty-island and tie semantics of the two migrations** — the `.max(1)` in
//! `RingMigration` makes **a rate of zero migrate one individual anyway** and an empty island merely hands its successor
//! nothing while the others keep going; `BestToWorstMigration` instead **gives up entirely when any island is empty** and
//! **moves nothing when the best and worst leaders tie**; beyond that, **the target island can be grown** because the
//! migrant count comes from the source rather than the target
//! (`NoMigration` / `RingMigration` / `BestToWorstMigration` in `solver/heuristic/operator/migration.rs`).
//!
//! 第 91 单量的是邻域，这一单量**迁移**。八条实测（外加一条写进说明的实测：**两个算子都会"复制"个体**——
//! 送进来的最优是 `cloned()` 进接收岛的，而来源岛只砍掉自己的最差、并不交出原件，所以同一个编号可以在两个岛上
//! 同时出现；区别是环形迁移**总规模守恒**，优岛到劣岛**总规模会变大**）：
//!
//! 1. **单岛永不迁移**（`a_single_island_is_never_migrated()`）：两个算子都在 `len < 2` 时原样返回。
//! 2. **环形迁移率 0 也照迁一个**（`the_ring_rate_of_zero_still_migrates_one()`）：`(len * rate) as usize` 之后
//!    还有 `.max(1)`。
//! 3. **环形迁移让每个岛的规模守恒**（`the_ring_preserves_every_island_size()`）。
//! 4. **环形迁移送进来的是来源岛的最优几个，掉的是自己最差的几个**
//!    （`the_ring_moves_the_best_in_and_drops_the_worst()`）。
//! 5. **空岛只让它的后继拿到 0 个**（`an_empty_island_merely_starves_its_successor()`）：
//!    `amount` 被 `.min(incoming.len())` 压成 0，于是那个岛原样返回。
//! 6. **最优与最差并列时一个都不动**（`a_best_to_worst_tie_migrates_nothing()`）：`source == target` 直接返回。
//! 7. **目标岛可能被撑大**（`the_best_to_worst_target_can_grow()`）：迁移量由**来源岛**的规模决定，
//!    目标岛只保留 `len - amount` 个。本单最值钱的一条。
//! 8. **空岛只在优岛到劣岛里"整体拦住"**（`an_empty_island_blocks_everything_only_in_best_to_worst()`）：
//!    环形迁移在空岛旁边仍然照迁。
//!
//! Delivery 91 measured neighbourhoods; this one measures **migration**. **A single island is never migrated.** **A ring
//! rate of zero still migrates one individual** because of the `.max(1)`. **The ring preserves every island's size.**
//! **It moves the source's best in and drops its own worst.** **An empty island merely starves its successor.** **A tie
//! between the best and worst leaders moves nothing.** **The best-to-worst target can be grown** because the migrant count
//! comes from the source island — the most valuable finding here. And **an empty island blocks everything only in
//! best-to-worst**, while the ring keeps migrating around it.

use ospf_rust_core::model::callback::Solution;
use ospf_rust_core::solver::heuristic::entropy::SeededEntropy;
use ospf_rust_core::solver::heuristic::individual::Individual;
use ospf_rust_core::solver::heuristic::iteration::Iteration;
use ospf_rust_core::solver::heuristic::model::{ObjectiveModel, VariableModel};
use ospf_rust_core::solver::heuristic::operator::migration::{
    BestToWorstMigration, IslandMigration, RingMigration,
};
use ospf_rust_core::variable::VariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::cmp::Ordering;

use crate::{BridgeError, Result};

/// 迁移探针请求。 / The migration probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MigrationRequest {
    /// 每个岛的适应度列表（空岛写空数组）。 / Each island's fitness list; an empty island is an empty array.
    #[serde(default)]
    pub islands: Vec<Vec<f64>>,
    /// 环形迁移的比例（默认 0.1）。 / The ring migration rate (default 0.1).
    #[serde(default)]
    pub rate: Option<f64>,
    /// 优岛到劣岛每次迁移的数量（默认 1）。 / How many individuals best-to-worst migrates at a time.
    #[serde(default)]
    pub amount: Option<u32>,
}

/// 迁移探针结论。 / The migration probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MigrationReport {
    /// 请求里每个岛的规模（读面用它校验环迁移的规模守恒）。 / The request's island sizes.
    pub input_sizes: Vec<u32>,
    /// 环形迁移之后的各岛（元素是**全局个体编号**）。 / The islands after the ring migration, as global ids.
    pub ring: Vec<Vec<u32>>,
    /// 优岛到劣岛迁移之后的各岛。 / The islands after the best-to-worst migration.
    pub best_to_worst: Vec<Vec<u32>>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub a_single_island_is_never_migrated: bool,
    pub the_ring_rate_of_zero_still_migrates_one: bool,
    pub the_ring_preserves_every_island_size: bool,
    pub the_ring_moves_the_best_in_and_drops_the_worst: bool,
    pub an_empty_island_merely_starves_its_successor: bool,
    pub a_best_to_worst_tie_migrates_nothing: bool,
    pub the_best_to_worst_target_can_grow: bool,
    pub an_empty_island_blocks_everything_only_in_best_to_worst: bool,
}

/// 探针模型：目标值是 `(全局编号, 适应度)` 这个对，于是读面能逐条认出个体来源。
/// The probe model uses the `(global id, fitness)` pair as its objective so the read face can name every individual.
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

/// 把请求里的适应度铺成带全局编号的个体。 / Lay out the request's fitness values as globally tagged individuals.
fn individuals(islands: &[Vec<f64>]) -> Vec<Vec<Individual<(u32, f64), f64>>> {
    let mut next = 0_u32;
    islands.iter().map(|island| island.iter().map(|fitness| {
        let id = next;
        next = next.saturating_add(1);
        Individual::new(Solution::new(), (id, *fitness))
    }).collect()).collect()
}

/// 把迁移结果转成编号矩阵。 / Turn a migration result into the id matrix.
fn ids(islands: &[Vec<Individual<(u32, f64), f64>>]) -> Vec<Vec<u32>> {
    islands.iter().map(|island| island.iter().map(|entry| entry.fitness.0).collect())
        .collect()
}

/// 跑两种迁移。 / Run both migrations.
fn migrate_all(islands: &[Vec<f64>], rate: f64, amount: usize)
    -> (Vec<Vec<u32>>, Vec<Vec<u32>>)
{
    let model = ProbeModel;
    let iteration = Iteration::new();
    let mut entropy = SeededEntropy::new(0);
    let input = individuals(islands);
    let ring = RingMigration::new(rate).migrate(&iteration, input.clone(), &model, &mut entropy);
    let best_to_worst = BestToWorstMigration::new(amount).migrate(&iteration, input, &model,
        &mut entropy);
    (ids(&ring), ids(&best_to_worst))
}

/// 走一遍迁移探针。非法输入是**错误**。 / Walk the migration probe.
pub fn verify_migration(request: &MigrationRequest) -> Result<MigrationReport> {
    if request.islands.len() > 4 {
        return Err(BridgeError::new("InvalidInput",
            "migration probe failed validation: at most four islands are accepted"));
    }
    if request.islands.iter().any(|island| island.len() > 8) {
        return Err(BridgeError::new("InvalidInput",
            "migration probe failed validation: at most eight individuals per island"));
    }
    if request.islands.iter().flatten().any(|fitness| !fitness.is_finite()) {
        return Err(BridgeError::new("InvalidInput", "migration probe failed validation: \
            non-finite fitness cannot cross the boundary"));
    }
    let rate = request.rate.unwrap_or(0.1);
    if !rate.is_finite() || rate < 0.0 || rate > 1.0 {
        return Err(BridgeError::new("InvalidInput", "migration probe failed validation: \
            the rate must stay inside [0, 1]"));
    }
    let amount = usize::try_from(request.amount.unwrap_or(1))
        .map_err(|_| BridgeError::new("InvalidInput",
            "migration probe failed validation: the amount overflowed"))?;
    if amount > 8 {
        return Err(BridgeError::new("InvalidInput",
            "migration probe failed validation: at most eight individuals migrate at a time"));
    }
    let findings = canonical_findings();
    let (ring, best_to_worst) = migrate_all(&request.islands, rate, amount);
    Ok(MigrationReport {
        input_sizes: request.islands.iter()
            .map(|island| u32::try_from(island.len()).unwrap_or(u32::MAX)).collect(),
        ring,
        best_to_worst,
        a_single_island_is_never_migrated: findings.a_single_island_is_never_migrated,
        the_ring_rate_of_zero_still_migrates_one:
            findings.the_ring_rate_of_zero_still_migrates_one,
        the_ring_preserves_every_island_size: findings.the_ring_preserves_every_island_size,
        the_ring_moves_the_best_in_and_drops_the_worst:
            findings.the_ring_moves_the_best_in_and_drops_the_worst,
        an_empty_island_merely_starves_its_successor:
            findings.an_empty_island_merely_starves_its_successor,
        a_best_to_worst_tie_migrates_nothing: findings.a_best_to_worst_tie_migrates_nothing,
        the_best_to_worst_target_can_grow: findings.the_best_to_worst_target_can_grow,
        an_empty_island_blocks_everything_only_in_best_to_worst:
            findings.an_empty_island_blocks_everything_only_in_best_to_worst,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    a_single_island_is_never_migrated: bool,
    the_ring_rate_of_zero_still_migrates_one: bool,
    the_ring_preserves_every_island_size: bool,
    the_ring_moves_the_best_in_and_drops_the_worst: bool,
    an_empty_island_merely_starves_its_successor: bool,
    a_best_to_worst_tie_migrates_nothing: bool,
    the_best_to_worst_target_can_grow: bool,
    an_empty_island_blocks_everything_only_in_best_to_worst: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 单岛永不迁移。 / A single island is never migrated.
    let a_single_island_is_never_migrated = {
        let (ring, best_to_worst) = migrate_all(&[vec![1.0, 2.0, 3.0]], 0.5, 2);
        // 编号保持 `[0, 1, 2]`。 / The ids stay `[0, 1, 2]`.
        ring == vec![vec![0, 1, 2]] && best_to_worst == vec![vec![0, 1, 2]]
            // 空输入也不炸。 / An empty input does not blow up either.
            && migrate_all(&[], 0.5, 1) == (Vec::new(), Vec::new())
    };
    // 2) 迁移率 0 也照迁一个。 / A rate of zero still migrates one.
    let the_ring_rate_of_zero_still_migrates_one = {
        let islands = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let zero = migrate_all(&islands, 0.0, 1).0;
        let small = migrate_all(&islands, 0.1, 1).0;
        // `(3 * 0.0) as usize == 0`，但 `.max(1)` 把它抬成 1，所以与 0.1 的结果一致。 
        // `(3 * 0.0) as usize` is zero, yet `.max(1)` lifts it, so the result matches a rate of 0.1.
        zero == small
            // 岛 0 得到了岛 1 的最优个体（编号 3），并丢掉了自己的最差个体（编号 2）。 
            // Island 0 receives island 1's best (id 3) and drops its own worst (id 2).
            && zero[0] == vec![0, 1, 3]
    };
    // 3) 环形迁移让每个岛的规模守恒。 / The ring preserves every island's size.
    let the_ring_preserves_every_island_size = {
        let islands = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0], vec![7.0, 8.0]];
        let ring = migrate_all(&islands, 0.5, 1).0;
        // 三个岛分别仍是 3 / 3 / 2 个。 / The three islands still hold 3, 3 and 2 members.
        ring.iter().map(Vec::len).collect::<Vec<_>>() == vec![3, 3, 2]
            // 极端比例也不改变规模。 / An extreme rate changes no size either.
            && { let full = migrate_all(&islands, 1.0, 0).0;
                full.iter().map(Vec::len).collect::<Vec<_>>() == vec![3, 3, 2] }
    };
    // 4) 送进来的是来源岛的最优几个，掉的是自己最差的几个。 
    // The source's best come in and the island's own worst is dropped.
    let the_ring_moves_the_best_in_and_drops_the_worst = {
        // 岛 1 的适应度是 `[9, 1, 5]`（编号 3, 4, 5），最优是编号 4。 
        // Island 1 holds fitnesses `[9, 1, 5]` (ids 3, 4, 5) and its best is id 4.
        let islands = vec![vec![1.0, 2.0, 3.0], vec![9.0, 1.0, 5.0]];
        let ring = migrate_all(&islands, 0.34, 1).0;
        // 岛 0 保留最优两个 `[0, 1]`、掉最差 `2`，换来编号 `4`。 
        // Island 0 keeps its two best `[0, 1]`, drops the worst `2` and gains id 4.
        ring[0] == vec![0, 1, 4]
            // 岛 1 同理：来源是岛 0 的最优 `0`，掉的是自己的最差 `3`。 
            // Island 1 mirrors that: it gains island 0's best `0` and drops its own worst `3`.
            && ring[1] == vec![4, 5, 0]
    };
    // 5) 空岛只让它的后继拿到 0 个。 / An empty island merely starves its successor.
    let an_empty_island_merely_starves_its_successor = {
        // 岛 1 是空的，于是岛 0 的后继（也就是岛 1）给它 0 个：`amount` 被压成 0，岛 0 原样返回。 
        // Island 1 is empty, so island 0's incoming set holds nothing, `amount` collapses to zero and island 0 is
        // returned untouched.
        let islands = vec![vec![1.0, 2.0], Vec::new()];
        let ring = migrate_all(&islands, 0.5, 1).0;
        ring == vec![vec![0, 1], Vec::new()]
    };
    // 6) 最优与最差并列时一个都不动。 / A tie between the best and worst leaders moves nothing.
    let a_best_to_worst_tie_migrates_nothing = {
        // 两个岛的最优个体都是适应度 5 → `source == target` → 原样返回。 
        // Both islands' leaders have fitness five, so `source == target` and the input comes back unchanged.
        let islands = vec![vec![5.0, 5.0], vec![5.0, 5.0]];
        let best_to_worst = migrate_all(&islands, 0.1, 1).1;
        best_to_worst == vec![vec![0, 1], vec![2, 3]]
            // 而环形迁移在同样输入上照旧搬动。 / The ring migration still moves individuals on the same input.
            && migrate_all(&islands, 0.5, 1).0 != vec![vec![0, 1], vec![2, 3]]
    };
    // 7) 目标岛可能被撑大。 / The best-to-worst target can be grown.
    let the_best_to_worst_target_can_grow = {
        // 岛 0 最优（适应度 1）且更大（5 个），岛 1 最差（适应度 8）且更小（2 个）。 
        // Island 0 leads (fitness one) and is larger, island 1 trails (fitness eight) and is smaller.
        let islands = vec![vec![1.0, 2.0, 3.0, 4.0, 5.0], vec![9.0, 8.0]];
        let best_to_worst = migrate_all(&islands, 0.1, 5).1;
        // 迁移量由**来源岛**决定（5），目标岛只保留 `2 - 2 = 0` 个，于是被撑到 5 个。 
        // The migrant count comes from the **source** island (five), the target keeps `2 - 2 = 0` and thus grows to five.
        best_to_worst[1].len() == 5 && best_to_worst[0].len() == 5
            // 来源岛一个都没少。 / The source island loses nothing.
            && best_to_worst[0] == vec![0, 1, 2, 3, 4]
            // 撑进来的正是来源岛的最优 5 个。 / What arrives is exactly the source's five best.
            && best_to_worst[1] == vec![0, 1, 2, 3, 4]
    };
    // 8) 空岛只在优岛到劣岛里"整体拦住"。 / An empty island blocks everything only in best-to-worst.
    let an_empty_island_blocks_everything_only_in_best_to_worst = {
        // 第三个岛是空的：优岛到劣岛直接原样返回，而环形迁移仍然在岛 1 上搬动。 
        // The third island is empty: best-to-worst returns everything untouched while the ring still moves island 1.
        let islands = vec![vec![1.0, 2.0], vec![3.0], Vec::new()];
        let (ring, best_to_worst) = migrate_all(&islands, 0.5, 1);
        best_to_worst == vec![vec![0, 1], vec![2], Vec::new()]
            && ring[1] == vec![0]
            && ring[0] == vec![0, 1]
    };
    Findings {
        a_single_island_is_never_migrated,
        the_ring_rate_of_zero_still_migrates_one,
        the_ring_preserves_every_island_size,
        the_ring_moves_the_best_in_and_drops_the_worst,
        an_empty_island_merely_starves_its_successor,
        a_best_to_worst_tie_migrates_nothing,
        the_best_to_worst_target_can_grow,
        an_empty_island_blocks_everything_only_in_best_to_worst,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> MigrationRequest {
        MigrationRequest { islands: vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]],
            rate: Some(0.34), amount: Some(1) }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_pair() {
        let report = verify_migration(&request()).unwrap();
        assert!(report.a_single_island_is_never_migrated);
        assert!(report.the_ring_rate_of_zero_still_migrates_one);
        assert!(report.the_ring_preserves_every_island_size);
        assert!(report.the_ring_moves_the_best_in_and_drops_the_worst);
        assert!(report.an_empty_island_merely_starves_its_successor);
        assert!(report.a_best_to_worst_tie_migrates_nothing);
        assert!(report.the_best_to_worst_target_can_grow);
        assert!(report.an_empty_island_blocks_everything_only_in_best_to_worst);
        // 两个三元素岛、比例 0.34 → 每个岛只动一个。 / Two three-member islands at 0.34 move one each.
        assert_eq!(report.ring, vec![vec![0, 1, 3], vec![3, 4, 0]]);
    }

    #[test]
    fn a_rate_of_zero_still_moves_one_individual() {
        let zero = verify_migration(&MigrationRequest { rate: Some(0.0), ..request() }).unwrap();
        let small = verify_migration(&MigrationRequest { rate: Some(0.1), ..request() }).unwrap();
        assert_eq!(zero.ring, small.ring);
        assert_eq!(zero.ring[0], vec![0, 1, 3]);
    }

    #[test]
    fn a_single_island_is_left_alone() {
        let report = verify_migration(&MigrationRequest { islands: vec![vec![3.0, 1.0, 2.0]],
            ..request() }).unwrap();
        assert_eq!(report.ring, vec![vec![0, 1, 2]]);
        assert_eq!(report.best_to_worst, vec![vec![0, 1, 2]]);
    }

    #[test]
    fn an_empty_island_starves_the_ring_but_only_blocks_best_to_worst() {
        let pair = verify_migration(&MigrationRequest { islands: vec![vec![1.0, 2.0],
            Vec::new()], ..request() }).unwrap();
        // 后继为空 → 环迁移不动。 / An empty successor starves the ring.
        assert_eq!(pair.ring, vec![vec![0, 1], Vec::new()]);
        // 三岛带一个空岛：环迁移仍在岛 1 上搬动，优岛到劣岛整体放弃。 
        // With three islands and one empty: the ring still moves island 1 while best-to-worst gives up entirely.
        let triple = verify_migration(&MigrationRequest { islands: vec![vec![1.0, 2.0], vec![3.0],
            Vec::new()], ..request() }).unwrap();
        assert_eq!(triple.best_to_worst, vec![vec![0, 1], vec![2], Vec::new()]);
        assert_eq!(triple.ring[1], vec![0]);
    }

    #[test]
    fn a_tie_moves_nothing_but_the_ring_still_does() {
        let report = verify_migration(&MigrationRequest { islands: vec![vec![5.0, 5.0],
            vec![5.0, 5.0]], rate: Some(0.5), amount: Some(1) }).unwrap();
        assert_eq!(report.best_to_worst, vec![vec![0, 1], vec![2, 3]]);
        assert_ne!(report.ring, vec![vec![0, 1], vec![2, 3]]);
    }

    #[test]
    fn the_target_grows_to_the_source_size() {
        let report = verify_migration(&MigrationRequest { islands: vec![
            vec![1.0, 2.0, 3.0, 4.0, 5.0], vec![9.0, 8.0]], amount: Some(5), ..request() })
            .unwrap();
        // 目标岛从 2 个被撑到 5 个。 / The target grows from two members to five.
        assert_eq!(report.best_to_worst[1].len(), 5);
        assert_eq!(report.best_to_worst[1], vec![0, 1, 2, 3, 4]);
        assert_eq!(report.best_to_worst[0], vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_migration(&MigrationRequest { islands: vec![Vec::new(); 5], ..request() })
            .unwrap_err().message.contains("at most four islands"));
        assert!(verify_migration(&MigrationRequest { islands: vec![vec![1.0; 9]], ..request() })
            .unwrap_err().message.contains("at most eight individuals"));
        assert!(verify_migration(&MigrationRequest { islands: vec![vec![f64::NAN]],
            ..request() }).unwrap_err().message.contains("non-finite fitness"));
        assert!(verify_migration(&MigrationRequest { rate: Some(2.0), ..request() })
            .unwrap_err().message.contains("the rate must stay inside [0, 1]"));
        assert!(verify_migration(&MigrationRequest { amount: Some(9), ..request() })
            .unwrap_err().message.contains("at most eight individuals migrate"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_migration(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"bestToWorst\""), "{json}");
        assert!(json.contains("\"theRingPreservesEveryIslandSize\""), "{json}");
        assert!(json.contains("\"theBestToWorstTargetCanGrow\""), "{json}");
        assert!(!json.contains("best_to_worst"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<MigrationReport>(&copy.to_string()).is_err());
    }
}
