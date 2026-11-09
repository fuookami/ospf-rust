//! 邻域搜索入口 / Neighbourhood search entry

use crate::heuristic::composition::neighborhood_search as compose;
use crate::heuristic::composition::TrajectorySolver;
use crate::heuristic::entropy::Entropy;
use crate::heuristic::model::HeuristicValue;
use crate::heuristic::operator::neighborhood::RandomPerturbation;
use crate::heuristic::operator::stopping::StoppingCriterion;

/// 邻域搜索，即轨迹骨架 + `NotWorseAcceptance` / Neighbourhood search: trajectory skeleton + `NotWorseAcceptance`
pub fn neighborhood_search<V>(
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
