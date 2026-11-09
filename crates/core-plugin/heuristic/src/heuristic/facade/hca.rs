//! 爬山算法入口 / Hill climbing entry

use crate::heuristic::composition::hill_climbing as compose;
use crate::heuristic::composition::TrajectorySolver;
use crate::heuristic::entropy::Entropy;
use crate::heuristic::model::HeuristicValue;
use crate::heuristic::operator::neighborhood::RandomPerturbation;
use crate::heuristic::operator::stopping::StoppingCriterion;

/// 爬山算法，即轨迹骨架 + `BetterOnlyAcceptance` / Hill climbing: trajectory skeleton + `BetterOnlyAcceptance`
///
/// 与 Kotlin 侧 `hillClimbing` 一一对应：`stopping` 决定搜索长度，邻域默认均匀扰动。
/// Mirrors the Kotlin `hillClimbing`: `stopping` decides the search length and the neighbourhood defaults to uniform
/// perturbation.
pub fn hill_climbing<V>(
    stopping: Box<dyn StoppingCriterion>,
    entropy: Box<dyn Entropy>,
) -> TrajectorySolver<V>
where
    V: HeuristicValue,
{
    compose(
        stopping,
        entropy,
        Box::new(RandomPerturbation::default()),
    )
}
