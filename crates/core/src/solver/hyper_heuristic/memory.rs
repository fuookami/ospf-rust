//! 档案与工作集分离 / Separate archives and working sets

use crate::solver::heuristic::individual::Individual;
use crate::solver::heuristic::model::HeuristicValue;
use crate::solver::heuristic::operator::multi_objective::CrowdingDistance;
use crate::solver::heuristic::session::{SearchProblem, archive::ObjectiveComponents};
use crate::solver::hyper_heuristic::components::RecoverableComponent;

mod adaptive;

pub use adaptive::{AutoAcceptance, LateAcceptance, RecordToRecordAcceptance};
pub(crate) use adaptive::builtin_acceptance;

/// 接受策略，仅影响工作集，不回滚算法原生状态 / Acceptance policy affecting working sets without rolling back native state
pub trait AcceptancePolicy<O: Clone, V: HeuristicValue>: RecoverableComponent {
    /// 不执行目标调用的接受能力预检 / Preflight acceptance capabilities without objective calls
    fn validate_problem(&self, _problem: &dyn SearchProblem<O, V>) -> crate::solver::heuristic::result::Result<()> { Ok(()) }
    /// 是否接收有效候选 / Whether to accept a valid candidate
    fn accept(
        &mut self,
        problem: &dyn SearchProblem<O, V>,
        working: &[Individual<O, V>],
        candidate: &Individual<O, V>,
    ) -> bool;
}

/// 仅接受未被工作集支配的候选 / Accept candidates not dominated by the working set
pub struct NotWorseAcceptance;

impl RecoverableComponent for NotWorseAcceptance {
    fn stateless(&self) -> bool { true }
}

/// 接受所有有效工作集候选 / Accept every valid working-set candidate
pub struct AlwaysAcceptance;

impl RecoverableComponent for AlwaysAcceptance {
    fn stateless(&self) -> bool { true }
}

impl<O: Clone, V: HeuristicValue> AcceptancePolicy<O, V> for AlwaysAcceptance {
    fn accept(
        &mut self,
        _: &dyn SearchProblem<O, V>,
        _: &[Individual<O, V>],
        _: &Individual<O, V>,
    ) -> bool { true }
}

impl<O: Clone, V: HeuristicValue> AcceptancePolicy<O, V> for NotWorseAcceptance {
    fn accept(
        &mut self,
        problem: &dyn SearchProblem<O, V>,
        working: &[Individual<O, V>],
        candidate: &Individual<O, V>,
    ) -> bool {
        !working.iter().any(|value| problem.dominates(&value.fitness, &candidate.fitness))
    }
}

/// 协调器独占的有效档案与已接受工作集 / Valid archive and accepted working set owned by the coordinator
pub struct SearchMemory<O, V> {
    archive: Vec<Individual<O, V>>,
    working: Vec<Individual<O, V>>,
    capacity: usize,
    working_capacity: usize,
    crowding_truncation: bool,
}

impl<O: ObjectiveComponents, V: HeuristicValue> SearchMemory<O, V> {
    /// 创建有界记忆 / Create bounded memory
    pub fn new(capacity: usize, working_capacity: usize) -> Self {
        Self { archive: Vec::new(), working: Vec::new(), capacity, working_capacity, crowding_truncation: true }
    }

    // 局部会话按到达顺序保留前缀，全局多目标记忆才使用拥挤距离。
    // Local sessions retain the arrival-order prefix; only global multiobjective memory uses crowding distance.
    pub(crate) fn local(capacity: usize, working_capacity: usize) -> Self {
        Self { crowding_truncation: false, ..Self::new(capacity, working_capacity) }
    }

    /// 有效档案 / Valid archive
    pub fn archive(&self) -> &[Individual<O, V>] { &self.archive }
    /// 已接受工作集 / Accepted working set
    pub fn working(&self) -> &[Individual<O, V>] { &self.working }

    pub(crate) fn restore(
        &mut self,
        problem: &dyn SearchProblem<O, V>,
        archive: Vec<Individual<O, V>>,
        working: Vec<Individual<O, V>>,
    ) -> crate::solver::heuristic::result::Result<()> {
        if archive.len() > self.capacity || working.len() > self.working_capacity
            || archive.iter().chain(&working).any(|value| !problem.valid_solution(&value.solution)
                || !problem.valid_objective(&value.fitness) || !problem.admissible(&value.solution) || !problem.feasible(&value.solution)) {
            return Err(super::invalid("invalid checkpoint memory"));
        }
        self.archive = archive;
        self.working = working;
        Ok(())
    }

    /// 相对派发时参考档案计算贡献 / Compute contribution against the dispatch-time reference archive
    pub fn contribution(
        &self,
        problem: &dyn SearchProblem<O, V>,
        reference: &[Individual<O, V>],
        candidates: &[Individual<O, V>],
    ) -> (bool, f64) {
        let merged = self.merged(problem, reference, candidates);
        Self::merged_contribution(problem, reference, &merged, candidates.len())
    }

    fn merged_contribution(
        problem: &dyn SearchProblem<O, V>,
        reference: &[Individual<O, V>],
        merged: &[Individual<O, V>],
        candidate_count: usize,
    ) -> (bool, f64) {
        let novel = if problem.multi_objective() {
            merged.iter().filter(|candidate| !reference.iter().any(|old|
                problem.dominates(&old.fitness, &candidate.fitness) || problem.equivalent(&old.fitness, &candidate.fitness))).count()
        } else {
            usize::from(merged.first().is_some_and(|candidate|
                reference.first().is_none_or(|old| problem.dominates(&candidate.fitness, &old.fitness))))
        };
        (novel > 0, novel as f64 / candidate_count.max(1) as f64)
    }

    fn merged(
        &self,
        problem: &dyn SearchProblem<O, V>,
        reference: &[Individual<O, V>],
        candidates: &[Individual<O, V>],
    ) -> Vec<Individual<O, V>> {
        let mut all = Vec::new();
        for value in reference.iter().chain(candidates) {
            if problem.valid_solution(&value.solution) && problem.valid_objective(&value.fitness)
                && problem.admissible(&value.solution) && problem.feasible(&value.solution)
                && !all.iter().any(|old: &Individual<O, V>| old.solution == value.solution) {
                all.push(value.clone());
            }
        }
        let mut merged = Vec::new();
        for value in &all {
            if problem.multi_objective() && (all.iter().any(|old| problem.dominates(&old.fitness, &value.fitness))
                || merged.iter().any(|old: &Individual<O, V>| problem.equivalent(&old.fitness, &value.fitness))) { continue; }
            merged.push(value.clone());
        }
        if problem.multi_objective() && merged.len() > self.capacity {
            if !self.crowding_truncation {
                merged.truncate(self.capacity);
                return merged;
            }
            let distances = CrowdingDistance::assign(&merged.iter().map(|value| value.fitness.components()).collect::<Vec<_>>());
            let mut order: Vec<_> = (0..merged.len()).collect();
            order.sort_by(|lhs, rhs| distances[*rhs].total_cmp(&distances[*lhs]).then(lhs.cmp(rhs)));
            order.truncate(self.capacity);
            order.sort_unstable();
            merged = order.into_iter().map(|index| merged[index].clone()).collect();
        } else if !problem.multi_objective() {
            merged.sort_by(|lhs, rhs| {
                if problem.dominates(&lhs.fitness, &rhs.fitness) { std::cmp::Ordering::Less }
                else if problem.dominates(&rhs.fitness, &lhs.fitness) { std::cmp::Ordering::Greater }
                else { std::cmp::Ordering::Equal }
            });
            merged.truncate(self.capacity);
        }
        merged
    }

    /// 归并有效候选，不触发目标重评估 / Merge valid candidates without re-evaluating objectives
    ///
    /// 同次调用的分类和比较结果必须稳定；可以缓存或计数，但不得依调用次数改变判断。
    /// Classification and comparison results must remain stable during a call; caching or counting must not change decisions.
    pub fn offer(
        &mut self,
        problem: &dyn SearchProblem<O, V>,
        candidates: Vec<Individual<O, V>>,
        acceptance: &mut dyn AcceptancePolicy<O, V>,
    ) -> bool {
        let valid: Vec<_> = candidates.into_iter().filter(|value| problem.valid_solution(&value.solution)
            && problem.valid_objective(&value.fitness) && problem.admissible(&value.solution) && problem.feasible(&value.solution)).collect();
        let merged = self.merged(problem, &self.archive, &valid);
        let improved = Self::merged_contribution(problem, &self.archive, &merged, valid.len()).0;
        self.archive = merged;
        for candidate in &valid {
            if acceptance.accept(problem, &self.working, candidate) {
                self.working.retain(|old| old.solution != candidate.solution);
                self.working.insert(0, candidate.clone());
                self.working.truncate(self.working_capacity);
            }
        }
        improved
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use crate::model::callback::Solution;
    use crate::variable::{VariableId, new_standalone_id};
    use super::*;

    struct CountedProblem {
        variable: VariableId,
        vector: bool,
        checks: Cell<usize>,
    }

    impl SearchProblem<Vec<f64>, f64> for CountedProblem {
        fn id(&self) -> &str { "counted-memory" }
        fn fingerprint(&self) -> &str { "counted-memory:1" }
        fn variable_order(&self) -> Vec<VariableId> { vec![self.variable] }
        fn initial_solutions(&self, _: usize) -> Vec<Solution<f64>> { vec![] }
        fn objective(&self, _: &Solution<f64>) -> Option<Vec<f64>> { panic!("memory must not evaluate objectives") }
        fn dominates(&self, left: &Vec<f64>, right: &Vec<f64>) -> bool {
            if self.vector { left.iter().zip(right).all(|(a, b)| a <= b) && left.iter().zip(right).any(|(a, b)| a < b) }
            else { left[0] < right[0] }
        }
        fn equivalent(&self, left: &Vec<f64>, right: &Vec<f64>) -> bool { left == right }
        fn valid_objective(&self, fitness: &Vec<f64>) -> bool {
            self.checks.set(self.checks.get() + 1);
            fitness.len() == 2 && fitness.iter().all(|value| value.is_finite())
        }
        fn multi_objective(&self) -> bool { self.vector }
    }

    fn copy_memory(memory: &SearchMemory<Vec<f64>, f64>) -> SearchMemory<Vec<f64>, f64> {
        SearchMemory { archive: memory.archive.clone(), working: memory.working.clone(), capacity: memory.capacity,
            working_capacity: memory.working_capacity, crowding_truncation: memory.crowding_truncation }
    }

    fn original_offer(
        memory: &mut SearchMemory<Vec<f64>, f64>,
        problem: &CountedProblem,
        candidates: Vec<Individual<Vec<f64>, f64>>,
    ) -> bool {
        let valid = candidates.into_iter().filter(|value| problem.valid_solution(&value.solution)
            && problem.valid_objective(&value.fitness) && problem.admissible(&value.solution) && problem.feasible(&value.solution)).collect::<Vec<_>>();
        let first_merge = memory.merged(problem, &memory.archive, &valid);
        let improved = if problem.multi_objective() {
            first_merge.iter().filter(|candidate| !memory.archive.iter().any(|old|
                problem.dominates(&old.fitness, &candidate.fitness) || problem.equivalent(&old.fitness, &candidate.fitness))).count() > 0
        } else {
            first_merge.first().is_some_and(|candidate| memory.archive.first().is_none_or(|old|
                problem.dominates(&candidate.fitness, &old.fitness)))
        };
        drop(first_merge);
        memory.archive = memory.merged(problem, &memory.archive, &valid);
        for candidate in &valid {
            memory.working.retain(|old| old.solution != candidate.solution);
            memory.working.insert(0, candidate.clone());
            memory.working.truncate(memory.working_capacity);
        }
        improved
    }

    #[test]
    fn offer_reuses_one_merge_for_scalar_vector_crowding_and_local_memories() {
        for vector in [false, true] {
            for local in [false, true] {
                for capacity in [0, 1, 3, 8] {
                    let problem = CountedProblem { variable: new_standalone_id(), vector, checks: Cell::new(0) };
                    let individual = |id, fitness| Individual::new(Solution::from([(problem.variable, id)]), fitness);
                    let mut memory = if local { SearchMemory::local(capacity, 2) } else { SearchMemory::new(capacity, 2) };
                    memory.offer(&problem, vec![individual(0.0, vec![0.0, 4.0]), individual(2.0, vec![4.0, 0.0])], &mut AlwaysAcceptance);
                    let candidates = vec![individual(1.0, vec![1.0, 1.0]), individual(1.0, vec![1.0, 1.0]),
                        individual(3.0, vec![9.0, 9.0]), individual(4.0, vec![f64::NAN, 0.0])];
                    let mut expected = copy_memory(&memory);
                    let expected_improvement = original_offer(&mut expected, &problem, candidates.clone());
                    let reference_count = memory.archive().len();
                    problem.checks.set(0);
                    assert_eq!(memory.offer(&problem, candidates, &mut AlwaysAcceptance), expected_improvement);
                    assert_eq!(problem.checks.get(), 4 + reference_count + 3);
                    let values = |entries: &[Individual<Vec<f64>, f64>]| entries.iter()
                        .map(|entry| (entry.solution.clone(), entry.fitness.clone())).collect::<Vec<_>>();
                    assert_eq!(values(memory.archive()), values(expected.archive()));
                    assert_eq!(values(memory.working()), values(expected.working()));
                    assert_eq!(memory.working().len(), 2);
                    assert_eq!(memory.working()[0].solution[&problem.variable], 3.0);
                    assert_eq!(memory.working()[1].solution[&problem.variable], 1.0);
                }
            }
        }
    }

    #[test]
    #[ignore = "manual release-mode archive merge timings"]
    fn performance_b03_memory_offer() {
        use std::hint::black_box;
        use std::time::Instant;

        for vector in [false, true] {
            let problem = CountedProblem { variable: new_standalone_id(), vector, checks: Cell::new(0) };
            let individual = |id: usize| Individual::new(Solution::from([(problem.variable, id as f64)]),
                vec![id as f64, (200 - id) as f64]);
            let mut reference = SearchMemory::new(64, 8);
            reference.offer(&problem, (0..64).map(individual).collect(), &mut AlwaysAcceptance);
            let candidates = (64..128).map(individual).collect::<Vec<_>>();
            for reused in [false, true] {
                let mut samples = Vec::new();
                for _ in 0..5 {
                    let mut memory = copy_memory(&reference);
                    let input = candidates.clone();
                    let start = Instant::now();
                    let improved = if reused { memory.offer(&problem, black_box(input), &mut AlwaysAcceptance) }
                        else { original_offer(&mut memory, &problem, black_box(input)) };
                    black_box(improved);
                    samples.push(start.elapsed().as_nanos());
                    black_box(memory);
                }
                samples.sort_unstable();
                println!("P05 vector={vector}, reused={reused}, reference=64,candidates=64: median={} ns; samples={samples:?}", samples[2]);
            }
        }
    }
}
