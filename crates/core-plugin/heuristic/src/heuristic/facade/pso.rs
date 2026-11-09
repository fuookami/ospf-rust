//! 粒子群入口 / Particle swarm entry

use crate::heuristic::composition::population::particle_swarm as compose;
use crate::heuristic::composition::{ParticleSwarmSolver, SwarmConfig};
use crate::heuristic::entropy::Entropy;
use crate::heuristic::model::HeuristicValue;
use crate::heuristic::operator::initializer::ModelInitializer;
use crate::heuristic::operator::stopping::StoppingCriterion;

/// 粒子群优化，即粒子群骨架 + 默认惯性权重与学习因子 / Particle swarm optimization: particle swarm skeleton + default inertia and learning factors
pub fn particle_swarm<V>(
    size: usize,
    stopping: Box<dyn StoppingCriterion>,
    entropy: Box<dyn Entropy>,
) -> ParticleSwarmSolver<V>
where
    V: HeuristicValue,
{
    compose(
        "pso",
        SwarmConfig::new(size, stopping, entropy, Box::new(ModelInitializer)),
    )
}
