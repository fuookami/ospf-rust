use super::cycle_crossover_once;
use crate::solver::heuristic::model::HeuristicValue;

#[test]
fn cycle_index_matches_reference_for_permutations_and_special_values() {
    let bits = |values: Vec<f64>| values.into_iter().map(f64::to_bits).collect::<Vec<_>>();
    let mut state = 817351u64;
    for count in [0, 1, 2, 8, 32, 64, 127, 512, 1024] {
        for _ in 0..32 {
            let first = (0..count).map(|i| i as f64).collect::<Vec<_>>();
            let mut second = first.clone();
            for i in (1..count).rev() {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                second.swap(i, (state >> 32) as usize % (i + 1));
            }
            for (left, right) in [(&first, &second), (&second, &first)] {
                assert_eq!(bits(cycle_crossover_once(left, right)), bits(cycle_crossover_reference(left, right)));
            }
        }
    }
    for value in [f64::NAN, f64::from_bits(0x7ff8_0000_0000_0123), f64::INFINITY,
        f64::NEG_INFINITY, -0.0, 1.0, 999.0, f64::MIN_POSITIVE, f64::MAX] {
        let first = (0..1024).map(|i| i as f64).collect::<Vec<_>>();
        let mut second = first.clone();
        second.rotate_left(1);
        second[7] = value;
        for (left, right) in [(&first, &second), (&second, &first)] {
            assert_eq!(bits(cycle_crossover_once(left, right)), bits(cycle_crossover_reference(left, right)));
        }
    }
    let first = (0..1024).map(|i| i as f32).collect::<Vec<_>>();
    let mut second = first.clone();
    second.rotate_left(1);
    assert_eq!(cycle_crossover_once(&first, &second), cycle_crossover_reference(&first, &second));
    for value in [f64::INFINITY, f64::NEG_INFINITY, -0.0, f64::MIN_POSITIVE, f64::MAX] {
        let mut first = (0..1024).map(|i| i as f64 + 1.0).collect::<Vec<_>>();
        first[0] = value;
        let mut second = first.clone();
        second.rotate_left(1);
        assert_eq!(bits(cycle_crossover_once(&first, &second)), bits(cycle_crossover_reference(&first, &second)));
    }
    let (test, parents) = setup();
    let children = CycleCrossover::new().update(&test.model, &Iteration::new(), &parents,
        &mut FnEntropy::new(|| panic!("cycle crossover must not consume entropy")));
    assert_eq!(children.len(), 2);
}

#[test]
#[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
fn performance_b12_cycle_paired() {
    for count in [16, 128, 512, 1024, 8192] {
        for mode in ["identity", "short", "long"] {
            let first = (0..count).map(|i| i as f64).collect::<Vec<_>>();
            let mut second = first.clone();
            match mode { "short" => second.swap(0, 1), "long" => second.rotate_left(1), _ => {} }
            let iterations = if count < 1024 { 100 } else { 10 };
            for sample in 0..5 {
                for reference in if sample % 2 == 0 { [true, false] } else { [false, true] } {
                    let started = std::time::Instant::now();
                    for _ in 0..iterations {
                        let result = if reference { cycle_crossover_reference(std::hint::black_box(&first), std::hint::black_box(&second)) }
                            else { cycle_crossover_once(std::hint::black_box(&first), std::hint::black_box(&second)) };
                        std::hint::black_box(result);
                    }
                    eprintln!("cycle_paired count={count} mode={mode} reference={reference} sample={sample} iterations={iterations} ns={}", started.elapsed().as_nanos());
                }
            }
        }
    }
}

#[test]
#[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
fn performance_b12_cycle_crossover() {
    for count in [16, 128, 1024, 8192] {
        let first = (0..count).map(|i| i as f64).collect::<Vec<_>>();
        let mut second = first.clone();
        second.rotate_left(1);
        for sample in 0..5 {
            let started = std::time::Instant::now();
            let result = cycle_crossover_once(std::hint::black_box(&first), std::hint::black_box(&second));
            let elapsed = started.elapsed();
            eprintln!("cycle_crossover count={count} sample={sample} elapsed={elapsed:?}");
            std::hint::black_box(result);
        }
    }
}

fn cycle_crossover_reference<V: HeuristicValue>(cycle_source: &[V], other_source: &[V]) -> Vec<V> {
    let size = cycle_source.len();
    let mut child = other_source.to_vec();
    let mut assigned = vec![false; size];
    let mut index = 0;
    let mut guard = 0;
    while index < size && !assigned[index] && guard <= size {
        assigned[index] = true;
        child[index] = cycle_source[index];
        let next = other_source
            .iter()
            .position(|value| *value == cycle_source[index]);
        match next {
            Some(next) => {
                index = next;
                guard += 1;
            }
            None => break,
        }
    }
    child
}
