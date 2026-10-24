//! 服务路径规模观测契约，不是生产负载或耗时基准。 / Service workload observation contracts, not production traffic or timing benchmarks.

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use ospf_rust_base::error::Ret;
    use ospf_rust_quantities::quantity::Quantity;
    use ospf_rust_quantities::unit::derived::Meter;
    use ospf_rust_core::solver::config::SolverConfig;
    use ospf_rust_framework_csp2d::application::{
        Csp2dHeuristicMasterSolver, Csp2dMasterSolveResult, Csp2dMasterSolver,
        Csp2dProblem, Csp2dSolution, Csp2dSolvers, Csp2dTerminationReason,
    };
    use ospf_rust_framework_csp2d::domain::material::{
        DemandMode2, Material2, Product2, ProductDemand2, material_id2, product_id2,
    };
    use ospf_rust_framework_csp2d::domain::produce::Csp2dMasterProblem;
    use ospf_rust_framework_csp2d::infrastructure::error::solving_error;

    #[derive(Clone, Copy, Debug)]
    enum Reply {
        Delegate,
        NonDeliverable,
        WrongLength,
        Error,
        ZeroThenDelegate,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Observation {
        plans: usize,
        demands: usize,
        area_demands: usize,
        placements: usize,
    }

    #[derive(Debug)]
    struct RecordingSolver {
        reply: Reply,
        observations: Mutex<Vec<Observation>>,
    }

    impl RecordingSolver {
        fn new(reply: Reply) -> Self {
            Self { reply, observations: Mutex::new(Vec::new()) }
        }

        fn observations(&self) -> Vec<Observation> {
            self.observations.lock().unwrap().clone()
        }
    }

    impl Csp2dMasterSolver for RecordingSolver {
        fn name(&self) -> &str { "workload-recording" }

        fn solve_master(&self, master: &Csp2dMasterProblem) -> Ret<Csp2dMasterSolveResult> {
            let call = {
                let mut observations = self.observations.lock().unwrap();
                observations.push(Observation {
                    plans: master.assignment.len(),
                    demands: master.demands.len(),
                    area_demands: master.demands.iter()
                        .filter(|demand| demand.mode == DemandMode2::Area).count(),
                    placements: master.assignment.plans.iter()
                        .map(|plan| plan.pattern.placements.len()).sum(),
                });
                observations.len()
            };
            // 锁不跨越真实求解器调用，不改变其回调或阻塞边界。 / Release the recording lock before invoking the real solver or its callbacks.
            match self.reply {
                Reply::NonDeliverable => Ok(Csp2dMasterSolveResult::infeasible()),
                Reply::WrongLength => Ok(Csp2dMasterSolveResult::optimal(
                    vec![0.0; master.assignment.len() + 1], None,
                )),
                Reply::Error => Err(solving_error("injected observation failure")),
                Reply::ZeroThenDelegate if call == 1 => Ok(Csp2dMasterSolveResult::optimal(
                    vec![0.0; master.assignment.len()], None,
                )),
                _ => Csp2dHeuristicMasterSolver::new().solve_master(master),
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum Path { Milp, ColumnGeneration }

    impl Path {
        fn solve(
            self,
            service: &Csp2dSolvers,
            problem: Csp2dProblem,
        ) -> Ret<Csp2dSolution> {
            match self {
                Self::Milp => service.solve_milp(problem),
                Self::ColumnGeneration => service.solve_column_generation(problem),
            }
        }
    }

    fn meters(value: f64) -> Quantity<f64, Meter> { Quantity::new_ct(value) }

    fn problem() -> Csp2dProblem {
        Csp2dProblem::single(
            Material2::sheet(material_id2("m1"), "sheet", meters(10.0), meters(8.0), 100),
            Product2::new(product_id2("a"), "a", meters(2.0), meters(2.0)),
            20,
            meters(0.0),
        )
    }

    fn cases() -> Vec<(&'static str, Csp2dProblem)> {
        let single = problem();
        let mut area = single.clone();
        area.demands = vec![ProductDemand2::area(area.products[0].clone(), 40.0)];
        let mut multi = single.clone();
        multi.materials.push(Material2::sheet(
            material_id2("m2"), "small", meters(6.0), meters(4.0), 100,
        ));
        let mut mixed = single.clone();
        let product = Product2::new(product_id2("b"), "b", meters(1.0), meters(2.0));
        mixed.products.push(product.clone());
        mixed.demands.push(ProductDemand2::area(product, 20.0));
        vec![("amount", single), ("area", area), ("multi_material", multi), ("mixed", mixed)]
    }

    fn assert_same(actual: &Csp2dSolution, expected: &Csp2dSolution) {
        let usage = |solution: &Csp2dSolution| solution.usage.iter()
            .map(|(plan, amount)| (plan.identity_key(), *amount)).collect::<Vec<_>>();
        assert_eq!(usage(actual), usage(expected));
        assert_eq!(actual.coverage, expected.coverage);
        assert_eq!(actual.material_usage, expected.material_usage);
        assert_eq!(actual.fully_covered, expected.fully_covered);
        assert_eq!(actual.termination, expected.termination);
        assert_eq!(actual.total_waste_area().to_bits(), expected.total_waste_area().to_bits());
        let iterations = |solution: &Csp2dSolution| solution.iterations.iter()
            .map(|entry| (entry.iteration, entry.plan_count, entry.added_plan_count, entry.covered))
            .collect::<Vec<_>>();
        assert_eq!(iterations(actual), iterations(expected));
    }

    #[test]
    fn recording_delegate_preserves_complete_service_paths() {
        for (case, problem) in cases() {
            for path in [Path::Milp, Path::ColumnGeneration] {
                let expected = path.solve(&Csp2dSolvers::new(), problem.clone()).unwrap();
                let recorder = Arc::new(RecordingSolver::new(Reply::Delegate));
                let service = Csp2dSolvers::new().with_master_solver(recorder.clone());
                let actual = path.solve(&service, problem.clone()).unwrap();
                assert_same(&actual, &expected);
                let observations = recorder.observations();
                let calls = match path {
                    Path::Milp => 1,
                    Path::ColumnGeneration => actual.iterations.len() + 1,
                };
                assert_eq!(observations.len(), calls);
                assert!(observations.iter().all(|entry| entry.plans > 0 && entry.demands > 0));
                println!("WORKLOAD case={case} path={path:?} reply=Delegate master_calls={calls} iterations={} observations={observations:?}", actual.iterations.len());
            }
        }
    }

    #[test]
    fn rejected_injected_results_preserve_service_fallback() {
        for path in [Path::Milp, Path::ColumnGeneration] {
            let expected = path.solve(&Csp2dSolvers::new(), problem()).unwrap();
            for reply in [Reply::NonDeliverable, Reply::WrongLength, Reply::Error] {
                let recorder = Arc::new(RecordingSolver::new(reply));
                let service = Csp2dSolvers::new().with_master_solver(recorder.clone());
                let actual = path.solve(&service, problem()).unwrap();
                assert_same(&actual, &expected);
                let observations = recorder.observations();
                let calls = match path {
                    Path::Milp => 1,
                    Path::ColumnGeneration => actual.iterations.len() + 1,
                };
                assert_eq!(observations.len(), calls);
                println!("WORKLOAD case=fallback path={path:?} reply={reply:?} master_calls={calls} observations={observations:?}");
            }
        }
    }

    #[test]
    fn invalid_and_no_candidate_inputs_do_not_invoke_master() {
        for path in [Path::Milp, Path::ColumnGeneration] {
            let recorder = Arc::new(RecordingSolver::new(Reply::Delegate));
            let service = Csp2dSolvers::new().with_master_solver(recorder.clone());
            let mut invalid = problem();
            invalid.materials.clear();
            assert!(path.solve(&service, invalid).is_err());
            let mut impossible = problem();
            let large = Product2::new(product_id2("large"), "large", meters(20.0), meters(20.0));
            impossible.products = vec![large.clone()];
            impossible.demands = vec![ProductDemand2::amount(large, 1)];
            let result = path.solve(&service, impossible).unwrap();
            assert_eq!(result.termination, Csp2dTerminationReason::NoCandidatePlan);
            assert!(recorder.observations().is_empty());
        }
    }

    #[test]
    fn final_master_is_not_skipped_when_solver_state_changes() {
        let recorder = Arc::new(RecordingSolver::new(Reply::ZeroThenDelegate));
        let service = Csp2dSolvers::new().with_master_solver(recorder.clone());
        let result = service.solve_column_generation(problem()).unwrap();
        let observations = recorder.observations();
        assert_eq!(observations.len(), result.iterations.len() + 1);
        assert!(observations.len() >= 2);
        assert!(!result.iterations[0].covered);
        assert!(result.fully_covered);
        println!("WORKLOAD case=stateful path=ColumnGeneration master_calls={} observations={observations:?}", observations.len());
    }

    #[test]
    fn zero_iteration_budget_still_invokes_final_master_once() {
        let recorder = Arc::new(RecordingSolver::new(Reply::Delegate));
        let service = Csp2dSolvers::new().with_master_solver(recorder.clone());
        let mut problem = problem();
        problem.configuration.iteration_limit = 0;
        let result = service.solve_column_generation(problem).unwrap();
        assert!(result.iterations.is_empty());
        assert!(result.fully_covered);
        assert_eq!(recorder.observations().len(), 1);
    }

    #[test]
    fn concurrent_callers_share_the_recorder_without_changing_results() {
        let recorder = Arc::new(RecordingSolver::new(Reply::Delegate));
        let service = Csp2dSolvers::new().with_master_solver(recorder.clone());
        let expected = Csp2dSolvers::new().solve_milp(problem()).unwrap();
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4).map(|_| scope.spawn(|| {
                service.solve_milp(problem()).unwrap()
            })).collect();
            for handle in handles {
                assert_same(&handle.join().unwrap(), &expected);
            }
        });
        let observations = recorder.observations();
        assert_eq!(observations.len(), 4);
        assert!(observations.iter().all(|entry| *entry == observations[0]));
    }

    #[test]
    fn configured_thread_counts_are_not_a_process_budget() {
        let available = std::thread::available_parallelism().unwrap().get();
        let unset = SolverConfig::new("workload-observation");
        assert_eq!(unset.threads, None);
        let recommended = SolverConfig::recommended_defaults("workload-observation");
        assert!(recommended.threads.is_some_and(|count| count > 0 && count <= 32));
        let explicit = unset.with_threads(2).with_recommended_defaults();
        assert_eq!(explicit.threads, Some(2));
        // 这里只检查配置，不启动原生求解器或把配置数当成活跃线程数。 / Inspect configuration only; do not launch native solvers or equate configured and active threads.
        println!("WORKLOAD_CPU available={available} unset=None recommended={:?} explicit={:?} scope=per_solver_config", recommended.threads, explicit.threads);
    }
}
