//! Q 学习入口 / Q-learning entry

use crate::heuristic::composition::population::PopulationSolver;
use crate::heuristic::composition::q_learning as compose;
use crate::heuristic::entropy::Entropy;
use crate::heuristic::model::HeuristicValue;
use crate::heuristic::operator::stopping::StoppingCriterion;
use crate::heuristic::operator::update::{GreyWolfUpdate, SineCosineUpdate};

/// Q 学习驱动的种群搜索，默认在灰狼更新与正弦余弦更新之间选择 / Q-learning driven population search choosing between grey-wolf and sine-cosine updates by default
pub fn q_learning<V>(
    size: usize,
    stopping: Box<dyn StoppingCriterion>,
    entropy: Box<dyn Entropy>,
) -> PopulationSolver<V>
where
    V: HeuristicValue,
{
    compose(
        size,
        stopping,
        entropy,
        vec![
            Box::new(GreyWolfUpdate::default()),
            Box::new(SineCosineUpdate::default()),
        ],
    )
}
