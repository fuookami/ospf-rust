//! 续挖第 91 单：**五种邻域结构的边界算术**——**空种群仍然报出邻居 0**、`RingTopology` **把越界下标原样留在
//! 结果里**（邻域却都绕回了）而 `RingKTopology` 一律 `rem_euclid` 绕回、两成员的环**退化成完全图**、
//! 冯·诺依曼网格**不在边界回卷**且**末行不满时会丢掉不存在的邻居**
//! （`solver/heuristic/operator/topology.rs` 的 `GlobalTopology` / `RingTopology` / `RingKTopology` /
//! `VonNeumannTopology` / `RandomTopology`）
//! Continuation, delivery 91: the **boundary arithmetic of five neighbourhood structures** — **an empty population
//! still reports neighbour 0**, `RingTopology` **keeps an out-of-range index verbatim** while wrapping its neighbours
//! (whereas `RingKTopology` wraps everything through `rem_euclid`), a two-member ring **degenerates into a complete
//! graph**, and the Von Neumann grid **never wraps at the edges** while **a partial last row drops the neighbours that do
//! not exist**
//! (`GlobalTopology` / `RingTopology` / `RingKTopology` / `VonNeumannTopology` / `RandomTopology` in
//! `solver/heuristic/operator/topology.rs`).
//!
//! 第 90 单量的是排序，这一单量**邻域下标算术**——纯整数，唯一的随机性来自 `RandomTopology`（用调用方 seed）。
//! 八条实测：
//!
//! 1. **`GlobalTopology` 完全不看 `index`**（`global_topology_ignores_the_index()`）：`0..size` 全给，
//!    `size == 0` 给空。
//! 2. **空种群仍然报出邻居 0**（`an_empty_population_still_reports_neighbour_zero()`）：
//!    `index.min(size.saturating_sub(1))` 在 `size == 0` 时下溢成 0，**返回了一个不存在的邻居**。
//!    本单最值钱的一条。
//! 3. **两成员的环退化成完全图**（`a_two_member_ring_is_complete()`）：`[0, 1]` 互为邻居。
//! 4. **k 邻环饱和到整个种群**（`the_k_ring_saturates_at_the_whole_population()`）：半径够大就返回全部，
//!    半径 0 只返回自己。
//! 5. **`RingTopology` 把越界下标原样留下**（`the_plain_ring_keeps_the_raw_index_while_the_k_ring_wraps_it()`）：
//!    邻居都 `% size` 绕回了，`index` 本身**没绕**；而 `RingKTopology` 对每个偏移都用 `rem_euclid`。
//! 6. **冯·诺依曼网格不在边界回卷**（`the_grid_does_not_wrap_at_the_edges()`）：第 0 列没有左邻居。
//! 7. **末行不满时会丢掉不存在的邻居**（`a_partial_last_row_drops_the_missing_neighbours()`）：
//!    `neighbour < size` 的守卫会让网格末行少几个邻居。
//! 8. **`RandomTopology` 对同一 seed 可复现，且总把自己放在集合里**
//!    （`the_random_topology_is_seeded_and_keeps_the_index_inside()`）：目标数量是
//!    `amount.min(size).max(1)`，结果按升序去重。
//!
//! Delivery 90 measured ranking; this one measures **neighbourhood index arithmetic** — pure integers, with the only
//! randomness coming from `RandomTopology` (via a caller seed). **`GlobalTopology` ignores the index entirely.**
//! **An empty population still reports neighbour zero** because `index.min(size.saturating_sub(1))` underflows to zero,
//! **handing back a neighbour that does not exist** — the most valuable finding here. **A two-member ring degenerates
//! into a complete graph.** **The k-ring saturates at the whole population** once its radius is large enough, and a zero
//! radius returns only the index itself. **The plain ring keeps a raw out-of-range index** while its neighbours wrap
//! through `%`, whereas the k-ring wraps every offset through `rem_euclid`. **The grid does not wrap at the edges.**
//! **A partial last row drops the neighbours that do not exist** because of the `neighbour < size` guard. And **the
//! random topology is reproducible per seed and always keeps the index inside the set**, targeting
//! `amount.min(size).max(1)` sorted and deduped.

use ospf_rust_core_plugin_heuristic::heuristic::entropy::SeededEntropy;
use ospf_rust_core_plugin_heuristic::heuristic::operator::topology::{
    GlobalTopology, RingKTopology, RingTopology, Topology, VonNeumannTopology,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 邻域结构探针请求。 / The topology probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopologyRequest {
    /// 待查的个体下标（允许越界，越界的语义正是本单要量的）。
    /// The individual index to query; out-of-range values are allowed and are exactly what this delivery measures.
    pub index: u32,
    /// 种群规模。 / The population size.
    pub size: u32,
    /// k 邻环的半径。 / The k-ring neighbourhood radius.
    #[serde(default)]
    pub radius: Option<u32>,
    /// 冯·诺依曼网格宽度。 / The Von Neumann grid width.
    #[serde(default)]
    pub grid_width: Option<u32>,
    /// 随机拓扑的邻居数量。 / The random topology's neighbour amount.
    #[serde(default)]
    pub random_amount: Option<u32>,
    /// 随机拓扑的种子。 / The random topology's seed.
    #[serde(default)]
    pub seed: Option<u64>,
}

/// 邻域结构探针结论。 / The topology probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopologyReport {
    /// 输入回显。 / The echoed input.
    pub index: u32,
    pub size: u32,
    /// 全局拓扑。 / The global topology.
    pub global: Vec<u32>,
    /// 环形拓扑。 / The ring topology.
    pub ring: Vec<u32>,
    /// k 邻环形拓扑。 / The k-neighbour ring topology.
    pub ring_k: Vec<u32>,
    /// 冯·诺依曼网格拓扑。 / The Von Neumann grid topology.
    pub von_neumann: Vec<u32>,
    /// 随机拓扑。 / The random topology.
    pub random: Vec<u32>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub global_topology_ignores_the_index: bool,
    pub an_empty_population_still_reports_neighbour_zero: bool,
    pub a_two_member_ring_is_complete: bool,
    pub the_k_ring_saturates_at_the_whole_population: bool,
    pub the_plain_ring_keeps_the_raw_index_while_the_k_ring_wraps_it: bool,
    pub the_grid_does_not_wrap_at_the_edges: bool,
    pub a_partial_last_row_drops_the_missing_neighbours: bool,
    pub the_random_topology_is_seeded_and_keeps_the_index_inside: bool,
}

/// 五种拓扑各查一遍。 / Query all five topologies.
fn neighbours(index: usize, size: usize, radius: usize, width: usize, amount: usize, seed: u64)
    -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>)
{
    let as_u32 = |values: Vec<usize>| values.into_iter()
        .map(|value| u32::try_from(value).unwrap_or(u32::MAX)).collect::<Vec<_>>();
    let mut entropy = SeededEntropy::new(seed);
    // 逐个拓扑各用一条独立随机流，互不干扰。 / Each random topology reads its own stream so they cannot interfere.
    let global = as_u32(GlobalTopology.neighbours(index, size, &mut entropy));
    let ring = as_u32(RingTopology.neighbours(index, size, &mut entropy));
    let ring_k = as_u32(RingKTopology::new(radius).neighbours(index, size, &mut entropy));
    let von_neumann = as_u32(VonNeumannTopology::new(width).neighbours(index, size, &mut entropy));
    let mut random_stream = SeededEntropy::new(seed);
    let random = as_u32(ospf_rust_core_plugin_heuristic::heuristic::operator::topology::RandomTopology::new(
        amount).neighbours(index, size, &mut random_stream));
    (global, ring, ring_k, von_neumann, random)
}

/// 走一遍邻域结构探针。非法输入是**错误**。 / Walk the topology probe.
pub fn verify_topology(request: &TopologyRequest) -> Result<TopologyReport> {
    let index = usize::try_from(request.index)
        .map_err(|_| BridgeError::new("InvalidInput",
            "topology probe failed validation: index overflow"))?;
    let size = usize::try_from(request.size)
        .map_err(|_| BridgeError::new("InvalidInput",
            "topology probe failed validation: population size overflow"))?;
    let radius = usize::try_from(request.radius.unwrap_or(1))
        .map_err(|_| BridgeError::new("InvalidInput",
            "topology probe failed validation: radius overflow"))?;
    let width = usize::try_from(request.grid_width.unwrap_or(3))
        .map_err(|_| BridgeError::new("InvalidInput",
            "topology probe failed validation: grid width overflow"))?;
    let amount = usize::try_from(request.random_amount.unwrap_or(2))
        .map_err(|_| BridgeError::new("InvalidInput",
            "topology probe failed validation: neighbour amount overflow"))?;
    // 上限压在能一眼看清邻域的量级。 / The caps keep every neighbourhood small enough to read at a glance.
    if size > 64 || index > 1_000 || radius > 32 || width > 64 || amount > 64 {
        return Err(BridgeError::new("InvalidInput",
            "topology probe failed validation: the population stays at 64 and the knobs stay small"));
    }
    let findings = canonical_findings();
    let (global, ring, ring_k, von_neumann, random) =
        neighbours(index, size, radius, width, amount, request.seed.unwrap_or(0));
    Ok(TopologyReport {
        index: request.index,
        size: request.size,
        global,
        ring,
        ring_k,
        von_neumann,
        random,
        global_topology_ignores_the_index: findings.global_topology_ignores_the_index,
        an_empty_population_still_reports_neighbour_zero:
            findings.an_empty_population_still_reports_neighbour_zero,
        a_two_member_ring_is_complete: findings.a_two_member_ring_is_complete,
        the_k_ring_saturates_at_the_whole_population:
            findings.the_k_ring_saturates_at_the_whole_population,
        the_plain_ring_keeps_the_raw_index_while_the_k_ring_wraps_it:
            findings.the_plain_ring_keeps_the_raw_index_while_the_k_ring_wraps_it,
        the_grid_does_not_wrap_at_the_edges: findings.the_grid_does_not_wrap_at_the_edges,
        a_partial_last_row_drops_the_missing_neighbours:
            findings.a_partial_last_row_drops_the_missing_neighbours,
        the_random_topology_is_seeded_and_keeps_the_index_inside:
            findings.the_random_topology_is_seeded_and_keeps_the_index_inside,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    global_topology_ignores_the_index: bool,
    an_empty_population_still_reports_neighbour_zero: bool,
    a_two_member_ring_is_complete: bool,
    the_k_ring_saturates_at_the_whole_population: bool,
    the_plain_ring_keeps_the_raw_index_while_the_k_ring_wraps_it: bool,
    the_grid_does_not_wrap_at_the_edges: bool,
    a_partial_last_row_drops_the_missing_neighbours: bool,
    the_random_topology_is_seeded_and_keeps_the_index_inside: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) `GlobalTopology` 完全不看 `index`。 / `GlobalTopology` ignores the index entirely.
    let global_topology_ignores_the_index = {
        let (_, ring_a, ..) = neighbours(0, 4, 1, 2, 2, 0);
        let (global_zero, ..) = neighbours(0, 4, 1, 2, 2, 0);
        let (global_three, ..) = neighbours(3, 4, 1, 2, 2, 0);
        // 下标 0 与 3 拿到同一份 `0..4`，而环形拓扑对两者不同。 
        // Indices zero and three get the same `0..4`, while the ring differs between them.
        global_zero == vec![0, 1, 2, 3] && global_three == global_zero && ring_a != global_zero
            // `size == 0` 给空。 / A zero size gives an empty list.
            && { let (empty, ..) = neighbours(0, 0, 1, 2, 2, 0); empty.is_empty() }
    };
    // 2) 空种群仍然报出邻居 0。 / An empty population still reports neighbour zero.
    let an_empty_population_still_reports_neighbour_zero = {
        let (_, ring, ring_k, von_neumann, _) = neighbours(0, 0, 1, 3, 2, 0);
        // 三条都返回 `[0]`——一个不存在的邻居。 / All three report `[0]`, a neighbour that does not exist.
        ring == vec![0] && ring_k == vec![0] && von_neumann == vec![0]
            // 单元素种群同样落到 0。 / A one-member population lands on zero as well.
            && { let (_, ring, ..) = neighbours(5, 1, 1, 3, 2, 0); ring == vec![0] }
    };
    // 3) 两成员的环退化成完全图。 / A two-member ring degenerates into a complete graph.
    let a_two_member_ring_is_complete = {
        let (_, ring_zero, ..) = neighbours(0, 2, 1, 2, 2, 0);
        let (_, ring_one, ..) = neighbours(1, 2, 1, 2, 2, 0);
        // 两者都看到整圈。 / Each one sees the whole ring.
        ring_zero == vec![0, 1] && ring_one == vec![0, 1]
            // 对照：三成员的环每个只看到三个下标里的三个（也是全图），四成员才真正变环。 
            // Control: a three-member ring also sees everything, while four members finally form a ring.
            && { let (_, ring, ..) = neighbours(0, 4, 1, 2, 2, 0); ring == vec![0, 1, 3] }
    };
    // 4) k 邻环饱和到整个种群。 / The k-ring saturates at the whole population.
    let the_k_ring_saturates_at_the_whole_population = {
        let (_, _, ring_k_zero, ..) = neighbours(2, 6, 0, 2, 2, 0);
        let (_, _, ring_k_one, ..) = neighbours(2, 6, 1, 2, 2, 0);
        let (_, _, ring_k_big, ..) = neighbours(2, 6, 3, 2, 2, 0);
        // 半径 0 只有自己；半径 1 是自己加减一；半径 3 已经覆盖 6 个里的全部。 
        // Radius zero is the index alone, radius one adds its two sides, and radius three already covers all six.
        ring_k_zero == vec![2] && ring_k_one == vec![1, 2, 3]
            && ring_k_big == vec![0, 1, 2, 3, 4, 5]
            // 再大也只是同一份全集。 / Anything larger is the same full set.
            && { let (_, _, ring_k_huge, ..) = neighbours(2, 6, 30, 2, 2, 0);
                ring_k_huge == ring_k_big }
    };
    // 5) 普通环留下越界下标，k 邻环绕回。 / The plain ring keeps the raw index while the k-ring wraps it.
    let the_plain_ring_keeps_the_raw_index_while_the_k_ring_wraps_it = {
        let (_, ring, ring_k, ..) = neighbours(7, 3, 1, 2, 2, 0);
        // 邻居都绕回了（0 与 2），而 `7` 本身**原样留下**。 
        // The neighbours wrap to 0 and 2 while 7 itself is kept verbatim.
        ring == vec![0, 2, 7] && ring_k.iter().all(|value| *value < 3)
    };
    // 6) 网格不在边界回卷。 / The grid does not wrap at the edges.
    let the_grid_does_not_wrap_at_the_edges = {
        let (_, _, _, corner, _) = neighbours(0, 6, 1, 3, 2, 0);
        let (_, _, _, middle, _) = neighbours(4, 6, 1, 3, 2, 0);
        // 左上角只有自己、右、下三个；中间那个才有上下左右。 
        // The top-left corner has only itself, the right and the below, while the middle one has all four sides.
        corner == vec![0, 1, 3] && middle == vec![1, 3, 4, 5]
    };
    // 7) 末行不满时丢掉不存在的邻居。 / A partial last row drops the neighbours that do not exist.
    let a_partial_last_row_drops_the_missing_neighbours = {
        // 宽度 3、规模 5：下标 4 在第 1 行第 1 列，右邻 5 与下邻 7 都不存在。 
        // Width three with five members: index four sits at row one, column one, and both the right neighbour five and the
        // below neighbour seven do not exist.
        let (_, _, _, partial, _) = neighbours(4, 5, 1, 3, 2, 0);
        partial == vec![1, 3, 4]
            // 对照：满行时不丢。 / Control: a full row loses nothing.
            && { let (_, _, _, full, _) = neighbours(4, 6, 1, 3, 2, 0);
                full == vec![1, 3, 4, 5] }
    };
    // 8) 随机拓扑对同一 seed 可复现，且总把自己放在集合里。 
    // The random topology is reproducible per seed and always keeps the index inside the set.
    let the_random_topology_is_seeded_and_keeps_the_index_inside = {
        let (.., first) = neighbours(2, 6, 1, 3, 3, 11);
        let (.., second) = neighbours(2, 6, 1, 3, 3, 11);
        // 同 seed 一致、含自己、升序、去重、数量不超过 `size`。 
        // Same seed agrees, the index is present, the list ascends, has no duplicates and never exceeds `size`.
        first == second && first.contains(&2) && first.len() <= 6
            && first.windows(2).all(|pair| pair[0] < pair[1])
            // `amount = 0` 被抬成 1：只留自己。 / An amount of zero is lifted to one, leaving only the index.
            && { let (.., alone) = neighbours(2, 6, 1, 3, 0, 11); alone == vec![2] }
    };
    Findings {
        global_topology_ignores_the_index,
        an_empty_population_still_reports_neighbour_zero,
        a_two_member_ring_is_complete,
        the_k_ring_saturates_at_the_whole_population,
        the_plain_ring_keeps_the_raw_index_while_the_k_ring_wraps_it,
        the_grid_does_not_wrap_at_the_edges,
        a_partial_last_row_drops_the_missing_neighbours,
        the_random_topology_is_seeded_and_keeps_the_index_inside,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> TopologyRequest {
        TopologyRequest { index: 0, size: 6, radius: Some(1), grid_width: Some(3),
            random_amount: Some(2), seed: Some(0) }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_query() {
        let report = verify_topology(&request()).unwrap();
        assert!(report.global_topology_ignores_the_index);
        assert!(report.an_empty_population_still_reports_neighbour_zero);
        assert!(report.a_two_member_ring_is_complete);
        assert!(report.the_k_ring_saturates_at_the_whole_population);
        assert!(report.the_plain_ring_keeps_the_raw_index_while_the_k_ring_wraps_it);
        assert!(report.the_grid_does_not_wrap_at_the_edges);
        assert!(report.a_partial_last_row_drops_the_missing_neighbours);
        assert!(report.the_random_topology_is_seeded_and_keeps_the_index_inside);
        // 六成员、下标 0、半径 1：环形是 `[0, 1, 5]`，网格是左上角 `[0, 1, 3]`。 
        // Six members at index zero with radius one: the ring is `[0, 1, 5]` and the grid corner is `[0, 1, 3]`.
        assert_eq!(report.global, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(report.ring, vec![0, 1, 5]);
        assert_eq!(report.ring_k, vec![0, 1, 5]);
        assert_eq!(report.von_neumann, vec![0, 1, 3]);
    }

    #[test]
    fn an_empty_population_still_reports_a_neighbour() {
        let report = verify_topology(&TopologyRequest { size: 0, ..request() }).unwrap();
        // 全局拓扑给空，其余三条给 `[0]`——一个不存在的邻居。 
        // The global topology is empty while the other three hand back `[0]`, a neighbour that does not exist.
        assert!(report.global.is_empty());
        assert_eq!(report.ring, vec![0]);
        assert_eq!(report.ring_k, vec![0]);
        assert_eq!(report.von_neumann, vec![0]);
    }

    #[test]
    fn an_out_of_range_index_survives_in_the_plain_ring() {
        let report = verify_topology(&TopologyRequest { index: 7, size: 3, ..request() }).unwrap();
        // 邻居绕回 0 与 2，而 7 原样留下；k 邻环则全部绕回。 
        // The neighbours wrap to 0 and 2 while 7 stays verbatim; the k-ring wraps everything.
        assert_eq!(report.ring, vec![0, 2, 7]);
        assert!(report.ring_k.iter().all(|value| *value < 3));
        // 全局拓扑仍是整圈 `0..3`。 / The global topology is still the whole `0..3`.
        assert_eq!(report.global, vec![0, 1, 2]);
    }

    #[test]
    fn the_k_ring_saturates_and_the_grid_spans() {
        let wide = verify_topology(&TopologyRequest { index: 2, size: 6, radius: Some(30),
            ..request() }).unwrap();
        assert_eq!(wide.ring_k, vec![0, 1, 2, 3, 4, 5]);
        let partial = verify_topology(&TopologyRequest { index: 4, size: 5, grid_width: Some(3),
            ..request() }).unwrap();
        // 末行不满：右邻 5 与下邻 7 都不存在。 / A partial last row loses the right neighbour 5 and the below 7.
        assert_eq!(partial.von_neumann, vec![1, 3, 4]);
    }

    #[test]
    fn the_random_topology_is_reproducible() {
        let first = verify_topology(&TopologyRequest { index: 2, size: 6,
            random_amount: Some(3), seed: Some(11), ..request() }).unwrap();
        let second = verify_topology(&TopologyRequest { index: 2, size: 6,
            random_amount: Some(3), seed: Some(11), ..request() }).unwrap();
        assert_eq!(first.random, second.random);
        assert!(first.random.contains(&2));
        assert!(first.random.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_topology(&TopologyRequest { size: 65, ..request() }).unwrap_err()
            .message.contains("the population stays at 64"));
        assert!(verify_topology(&TopologyRequest { index: 2_000, ..request() }).unwrap_err()
            .message.contains("the population stays at 64"));
        assert!(verify_topology(&TopologyRequest { radius: Some(40), ..request() }).unwrap_err()
            .message.contains("the population stays at 64"));
        assert!(verify_topology(&TopologyRequest { random_amount: Some(70), ..request() })
            .unwrap_err().message.contains("the population stays at 64"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_topology(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"ringK\""), "{json}");
        assert!(json.contains("\"vonNeumann\""), "{json}");
        assert!(json.contains("\"globalTopologyIgnoresTheIndex\""), "{json}");
        assert!(!json.contains("von_neumann"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<TopologyReport>(&copy.to_string()).is_err());
    }
}
