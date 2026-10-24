#[test]
fn sparse_hessian_preserves_duplicates_generic_powers_and_float_order() {
    let symbols = (0..12).map(|i| make_symbol(&format!("s{i}"), 8000 + i)).collect::<Vec<_>>();
    let mut state = 194321u64;
    for case in 0..48 {
        let monomials = (0..64).map(|index| {
            let mut powers = HashMap::new();
            for _ in 0..case % 13 {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                powers.insert(symbols[(state >> 32) as usize % symbols.len()].clone(), ((state >> 16) % 6) as i32 - 2);
            }
            let coefficient = [1e16, 1.0, -1e16, -0.0, 3.5, f64::INFINITY, f64::NAN][index % 7];
            CanonicalMonomial::new(coefficient, powers)
        }).collect();
        let poly = Canonical::new(monomials, 7.0);
        let requested = if case % 3 == 0 { Vec::new() } else {
            vec![symbols[3].clone(), symbols[0].clone(), symbols[3].clone(),
                make_symbol("absent", 9999), symbols[11].clone(), symbols[2].clone()]
        };
        let expected = hessian_reference(&poly, &requested);
        let actual = poly.hessian(&requested);
        let bits = |matrix: Vec<Vec<f64>>| matrix.into_iter().map(|row| row.into_iter().map(f64::to_bits).collect::<Vec<_>>()).collect::<Vec<_>>();
        assert_eq!(bits(actual), bits(expected), "case={case}");
    }
    let x = symbols[0].clone();
    let y = symbols[1].clone();
    let generic = Canonical::new(vec![CanonicalMonomial::new(3.0f32, HashMap::from([(x.clone(), 2i64)])),
        CanonicalMonomial::new(-2.0, HashMap::from([(x.clone(), 1), (y.clone(), 1)]))], 0.0);
    let requested = [x.clone(), y.clone(), x.clone()];
    assert_eq!(generic.hessian(&requested), hessian_reference(&generic, &requested));
    let fractional = Canonical::new(vec![CanonicalMonomial::new(4.0, HashMap::from([(x.clone(), 0.5f64), (y, 1.0)]))], 0.0);
    assert_eq!(fractional.hessian(&requested), hessian_reference(&fractional, &requested));
}

#[test]
fn sparse_hessian_matches_quadratic_finite_differences() {
    let x = make_symbol("x", 8101);
    let y = make_symbol("y", 8102);
    let poly = Canonical::new(vec![
        CanonicalMonomial::new(3.0, HashMap::from([(x.clone(), 2i32)])),
        CanonicalMonomial::new(2.0, HashMap::from([(x.clone(), 1), (y.clone(), 1)])),
        CanonicalMonomial::new(5.0, HashMap::from([(y.clone(), 2)])),
    ], 7.0);
    let actual = poly.hessian(&[x, y]);
    let f = |x: f64, y: f64| 3.0 * x * x + 2.0 * x * y + 5.0 * y * y + 7.0;
    let (x, y, step) = (1.1, -0.3, 1e-3);
    let diagonal_x = (f(x + step, y) - 2.0 * f(x, y) + f(x - step, y)) / (step * step);
    let diagonal_y = (f(x, y + step) - 2.0 * f(x, y) + f(x, y - step)) / (step * step);
    let cross = (f(x + step, y + step) - f(x + step, y - step) - f(x - step, y + step) + f(x - step, y - step)) / (4.0 * step * step);
    for (value, expected) in [(actual[0][0], diagonal_x), (actual[1][1], diagonal_y), (actual[0][1], cross), (actual[1][0], cross)] {
        assert!((value - expected).abs() < 1e-6);
    }
}

#[test]
#[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
fn performance_b12_canonical_hessian() {
    for (count, terms, arity) in [(16, 128, 2), (64, 256, 2), (256, 512, 2), (32, 64, 32)] {
        let symbols = (0..count).map(|i| make_symbol(&format!("s{i}"), 9000 + i)).collect::<Vec<_>>();
        let poly = Canonical::new((0..terms).map(|i| CanonicalMonomial::new(1.0f64,
            (0..arity).map(|offset| (symbols[(i + offset) % count].clone(), 1i32)).collect())).collect(), 0.0);
        for sample in 0..5 {
            let started = std::time::Instant::now();
            let result = std::hint::black_box(&poly).hessian(std::hint::black_box(&symbols));
            let elapsed = started.elapsed();
            eprintln!("canonical_hessian count={count} terms={terms} arity={arity} sample={sample} elapsed={elapsed:?}");
            std::hint::black_box(result);
        }
    }
}

#[test]
#[ignore = "manual release timing; run with --ignored --nocapture --test-threads=1"]
fn performance_b12_hessian_paired() {
    for (count, terms, arity) in [(2, 4, 2), (16, 128, 2), (64, 256, 2), (256, 512, 2), (32, 64, 32)] {
        let symbols = (0..count).map(|i| make_symbol(&format!("s{i}"), 9000 + i)).collect::<Vec<_>>();
        let poly = Canonical::new((0..terms).map(|i| CanonicalMonomial::new(1.0f64,
            (0..arity).map(|offset| (symbols[(i + offset) % count].clone(), 1i32)).collect())).collect(), 0.0);
        for sample in 0..5 {
            for reference in if sample % 2 == 0 { [true, false] } else { [false, true] } {
                let started = std::time::Instant::now();
                let result = if reference { hessian_reference(std::hint::black_box(&poly), std::hint::black_box(&symbols)) }
                    else { std::hint::black_box(&poly).hessian(std::hint::black_box(&symbols)) };
                let elapsed = started.elapsed().as_nanos();
                std::hint::black_box(result);
                eprintln!("hessian_paired count={count} terms={terms} arity={arity} reference={reference} sample={sample} ns={elapsed}");
            }
        }
    }
}

fn hessian_reference<T, E>(poly: &Canonical<T, E>, symbols: &[OwnedSymbol]) -> Vec<Vec<T>>
where
    T: Clone + Zero + PartialEq + MulRef + num_traits::NumCast,
    E: Exponent + Clone + std::ops::Sub<Output = E> + One + PartialEq + num_traits::ToPrimitive,
{
        let n = symbols.len();
        let mut hessian = vec![vec![T::zero(); n]; n];

        // 遍历所有单项式，计算二阶导数
        // Iterate through all monomials and compute second derivatives
        for monomial in &poly.monomials {
            // 计算每个单项式对 Hessian 的贡献
            // Compute contribution of each monomial to Hessian
            for (i, symbol_i) in symbols.iter().enumerate() {
                for (j, symbol_j) in symbols.iter().enumerate() {
                    // 计算二阶混合偏导 ∂²m/∂xᵢ∂xⱼ
                    // Compute second mixed partial derivative
                    if let Some(contribution) =
                        compute_second_derivative_contribution(monomial, symbol_i, symbol_j)
                    {
                        let lhs = std::mem::replace(&mut hessian[i][j], T::zero());
                        hessian[i][j] = lhs + contribution;
                    }
                }
            }
        }

        hessian
}
