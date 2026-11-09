fn revision_fixture(items: usize) -> (MetaModel<f64>, Algorithm) {
    let (mut model, mut algorithm) = fixture();
    let cost = Cost::new((0..items).map(|index| CostItem::new(format!("cost-{index}-业务"),
        Some(0.125), Some("detail:[];\"\\\n".into()))).collect());
    algorithm.add_initial_columns(vec![(0, 0, cost)], &mut model).unwrap();
    algorithm.solver.0 = report(&model, vec![2.0, 0.0, 0.0], vec![-1.0, 0.0, 0.0], -2.0);
    (model, algorithm)
}

fn mutate_revision(context: &mut BasicTaskCompilationContext, case: usize) {
    let compilation = &mut context.compilation;
    match case {
        0 => compilation.columns[0].index += 1,
        1 => compilation.columns[0].task_index = 1,
        2 => compilation.columns[0].executor_index = 1,
        3 => compilation.columns[0].iteration += 1,
        4 => compilation.columns[0].x_model_index += 1,
        5 => compilation.columns[0].cost.cost_sum = Some(99.0),
        6 => compilation.columns[0].cost.items[0].tag.push(':'),
        7 => compilation.columns[0].cost.items[0].cost_quantity = None,
        8 => compilation.columns[0].cost.items[0].message = Some(String::new()),
        9 => { compilation.removed_columns.insert(0); },
        10 => { compilation.fixed_columns.insert(0); },
        11 => compilation.task_assignment_terms[0].push((99, -0.0)),
        12 => compilation.task_compilation_terms[0].push((99, 1.0)),
        13 => compilation.executor_compilation_terms[0].push((99, 2.0)),
        14 => compilation.y_indices.push(99),
        15 => compilation.z_indices.push(99),
        16 => compilation.task_cancel_enabled = !compilation.task_cancel_enabled,
        17 => compilation.with_executor_leisure = !compilation.with_executor_leisure,
        18 => compilation.columns.clear(),
        _ => compilation.columns[0].cost.items.push(CostItem::with_quantity("new", 0.0)),
    }
}

#[test]
fn revision_encoding_is_unambiguous_for_nested_lengths_options_and_strings() {
    let (_, mut algorithm) = revision_fixture(2);
    let mut encoded_to_reference = HashMap::new();
    let mut reference_to_encoded = HashMap::new();
    for case in 0..384 {
        let column = &mut algorithm.context.compilation.columns[0];
        column.index = [0, 1, 127, 128, 255, 16384, usize::MAX][case % 7];
        column.cost.items[0].tag = ["", ":", "00", "\0", "业务", "\"\\\n"][case % 6].repeat(case / 64);
        column.cost.items[0].message = if case % 3 == 0 { None } else { Some(format!("{}:{}", case / 7, case % 5)) };
        column.cost.items[1].cost_quantity = match case % 5 {
            0 => None, 1 => Some(-0.0), 2 => Some(f64::INFINITY),
            3 => Some(f64::from_bits(0x7ff8_0000_0000_0000 | case as u64)), _ => Some(case as f64),
        };
        algorithm.context.compilation.task_assignment_terms = vec![vec![], vec![(case, -0.0)]];
        let encoded = algorithm.context.pricing_revision().unwrap();
        let reference = algorithm.context.pricing_revision_reference().unwrap();
        assert!(encoded.is_ascii());
        if let Some(previous) = encoded_to_reference.insert(encoded.clone(), reference.clone()) {
            assert_eq!(previous, reference);
        }
        if let Some(previous) = reference_to_encoded.insert(reference, encoded.clone()) {
            assert_eq!(previous, encoded);
        }
    }
    let (_, mut algorithm) = revision_fixture(2);
    algorithm.context.compilation.fixed_columns.extend([2, 99, 1]);
    let encoded = algorithm.context.pricing_revision();
    algorithm.context.compilation.fixed_columns.clear();
    algorithm.context.compilation.fixed_columns.extend([99, 1, 2]);
    assert_eq!(encoded, algorithm.context.pricing_revision());
    let moved = BasicTaskCompilationContext { compilation: algorithm.context.compilation };
    assert_eq!(moved.pricing_revision(), encoded);
}

struct RevisionProbe<const STABLE: bool> {
    base: BasicTaskCompilationContext,
    calls: std::sync::atomic::AtomicUsize,
    changed: std::sync::atomic::AtomicBool,
    drift_at: usize,
}

macro_rules! revision_probe_context {
    ($stable:literal $(, $opt_in:literal)?) => {
        impl IterativeTaskCompilationContext for RevisionProbe<$stable> {
            $(fn pricing_state_is_stable(&self) -> bool { $opt_in })?
            fn pricing_domain(&self) -> Option<TaskPricingDomain> { self.base.pricing_domain() }
            fn pricing_revision(&self) -> Option<String> {
                let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
                Some(format!("{}:{}", self.base.pricing_revision().unwrap(),
                    self.changed.load(Ordering::SeqCst) || call >= self.drift_at))
            }
            fn register(&mut self, model: &mut MetaModel<f64>) -> GanttResult<()> { self.base.register(model) }
            fn add_columns(&mut self, iteration: usize, pairs: Vec<(usize, usize, Cost<f64>)>, model: &mut MetaModel<f64>)
                -> GanttResult<Vec<crate::domain::task_compilation::iterative::AddedTaskColumn>> {
                self.base.add_columns(iteration, pairs, model)
            }
            fn remove_columns(&mut self, indices: &[usize]) { self.base.remove_columns(indices); }
            fn extract_shadow_price(&self, dual: &ospf_rust_framework::solver::column_generation_solver::LinearDualSolution,
                rows: &HashMap<String, usize>) -> HashMap<usize, f64> { self.base.extract_shadow_price(dual, rows) }
            fn task_count_for_shadow_price(&self) -> usize { self.base.task_count_for_shadow_price() }
            fn extract_fixed(&self, solution: &[f64]) -> HashSet<usize> { self.base.extract_fixed(solution) }
            fn extract_kept(&self, solution: &[f64]) -> HashSet<usize> { self.base.extract_kept(solution) }
            fn column_count(&self) -> usize { self.base.column_count() }
        }
    };
}

revision_probe_context!(false);
revision_probe_context!(true, true);

fn probe_request() -> TaskPricingRequest {
    TaskPricingRequest { scope: ColumnGenerationScope { request: String::new(), groups: Default::default(),
        model: String::new(), dual: String::new(), branch: String::new(), global: false, executor_slots: Default::default() },
        iteration: 0, domain: TaskPricingDomain::unslotted(2, 2),
        shadow_prices: Default::default(), row_duals: Default::default(), fixed_columns: Default::default(),
        kept_columns: Default::default(), verified_duals: None }
}

#[test]
fn revision_reuse_is_opt_in_and_does_not_cross_callbacks_or_requests() {
    let (model, algorithm) = fixture();
    let probe = RevisionProbe::<false> { base: algorithm.context, calls: 0.into(), changed: false.into(), drift_at: usize::MAX };
    assert!(!probe.pricing_state_is_stable());
    let mut session = TaskPricingSession::default();
    session.accept_report(&model, &probe, &algorithm.solver.0).unwrap();
    probe.calls.store(0, Ordering::SeqCst);
    session.generate(&model, &probe, probe_request(), &Policy(complete)).unwrap();
    assert_eq!(probe.calls.load(Ordering::SeqCst), 2);
    let mut drifting = probe;
    drifting.drift_at = 2;
    drifting.calls.store(0, Ordering::SeqCst);
    assert!(session.generate(&model, &drifting, probe_request(), &Policy(complete)).is_err());
    assert_eq!(drifting.calls.load(Ordering::SeqCst), 2);

    let (model, algorithm) = fixture();
    let probe = RevisionProbe::<true> { base: algorithm.context, calls: 0.into(), changed: false.into(), drift_at: usize::MAX };
    let mut session = TaskPricingSession::default();
    session.accept_report(&model, &probe, &algorithm.solver.0).unwrap();
    probe.calls.store(0, Ordering::SeqCst);
    session.generate(&model, &probe, probe_request(), &Policy(complete)).unwrap();
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    assert_eq!(session.lower_bound(&model, &probe, &HashSet::new(), &HashSet::new()).unwrap(), Some(-2.0));
    let policy = Policy(|request: &TaskPricingRequest| {
        probe.changed.store(true, Ordering::SeqCst);
        complete(request)
    });
    session.generate(&model, &probe, probe_request(), &policy).unwrap();
    assert_eq!(session.lower_bound(&model, &probe, &HashSet::new(), &HashSet::new()).unwrap(), None);
    assert!(session.generate(&model, &probe, probe_request(), &Policy(complete)).is_err());
}

#[test]
fn exact_revisions_detect_direct_public_mutations_and_preserve_restore_equivalence() {
    let _literal = BasicTaskCompilationContext {
        compilation: crate::domain::task_compilation::iterative::IterativeTaskCompilation::new(2, 2, false, false),
    };
    for case in 0..20 {
        let (model, mut algorithm) = revision_fixture(2);
        let revision = algorithm.context.pricing_revision();
        let reference = algorithm.context.pricing_revision_reference();
        algorithm.solve_rmp_lp(&model).unwrap();
        generate(&mut algorithm, &model).unwrap();
        assert_eq!(algorithm.certified_lower_bound(&model).unwrap(), Some(-2.0));
        mutate_revision(&mut algorithm.context, case);
        assert_ne!(algorithm.context.pricing_revision_reference(), reference, "case={case}");
        assert_ne!(algorithm.context.pricing_revision(), revision, "case={case}");
        assert_eq!(algorithm.certified_lower_bound(&model).unwrap(), None, "case={case}");
        assert!(generate(&mut algorithm, &model).is_err(), "case={case}");
    }
    let (_, mut algorithm) = revision_fixture(2);
    let original = algorithm.context.pricing_revision();
    let old = algorithm.context.pricing_revision_reference();
    algorithm.context.compilation.columns[0].cost.cost_sum = Some(-0.0);
    let negative_zero = algorithm.context.pricing_revision();
    algorithm.context.compilation.columns[0].cost.cost_sum = Some(0.0);
    assert_ne!(algorithm.context.pricing_revision(), negative_zero);
    algorithm.context.compilation.columns[0].cost.cost_sum = Some(0.25);
    assert_eq!(algorithm.context.pricing_revision(), original);
    assert_eq!(algorithm.context.pricing_revision_reference(), old);
    algorithm.context.compilation.columns[0].cost.cost_sum = Some(f64::NAN);
    let nan = algorithm.context.pricing_revision();
    let nan_reference = algorithm.context.pricing_revision_reference();
    algorithm.context.compilation.columns[0].cost.cost_sum = Some(f64::from_bits(0xfff8_0000_0000_0321));
    assert_eq!(algorithm.context.pricing_revision_reference(), nan_reference);
    assert_eq!(algorithm.context.pricing_revision(), nan);
    for tasks in [true, false] {
        let (model, mut algorithm) = revision_fixture(2);
        algorithm.solve_rmp_lp(&model).unwrap();
        generate(&mut algorithm, &model).unwrap();
        if tasks { algorithm.context.compilation.n_tasks += 1; }
        else { algorithm.context.compilation.n_executors += 1; }
        assert_eq!(algorithm.certified_lower_bound(&model).unwrap(), None);
        assert!(generate(&mut algorithm, &model).is_err());
    }
}

#[test]
#[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
fn performance_b12_revision_paired() {
    for items in [0, 32, 256, 2048] {
        let (_, algorithm) = revision_fixture(items);
        for sample in 0..5 {
            for reference in if sample % 2 == 0 { [true, false] } else { [false, true] } {
                let started = std::time::Instant::now();
                let mut length = 0;
                for _ in 0..10 {
                    let result = if reference { algorithm.context.pricing_revision_reference() } else { algorithm.context.pricing_revision() };
                    length = result.as_ref().unwrap().len();
                    std::hint::black_box(result);
                }
                eprintln!("revision_paired items={items} reference={reference} sample={sample} iterations=10 length={length} ns={}", started.elapsed().as_nanos());
            }
        }
    }
}

#[test]
#[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
fn performance_b12_pricing_revision() {
    for items in [32, 256, 2048] {
        let (model, mut algorithm) = revision_fixture(items);
        algorithm.solve_rmp_lp(&model).unwrap();
        for sample in 0..5 {
            let started = std::time::Instant::now();
            for _ in 0..10 { std::hint::black_box(algorithm.context.pricing_revision()); }
            eprintln!("pricing_revision items={items} sample={sample} elapsed={:?}", started.elapsed());
            let started = std::time::Instant::now();
            for _ in 0..10 { std::hint::black_box(generate(&mut algorithm, std::hint::black_box(&model)).unwrap()); }
            eprintln!("pricing_generate items={items} sample={sample} elapsed={:?}", started.elapsed());
        }
    }
}
