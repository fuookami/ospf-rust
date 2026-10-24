//! 合并同类项模块
//! Combine like terms module
//!
//! 提供多项式同类项合并的优化功能，用于 FastSum 累加后的结果简化。
//! Provides polynomial like-term combining optimization for simplifying
//! results after FastSum accumulation.
//!
//! # 设计思想 / Design Philosophy
//!
//! 当使用 `AddAssign` 累加多个多项式时，结果会包含大量重复的同类项。
//! `CombineTerms` trait 提供了原地合并同类项的能力，减少内存分配。
//!
//! When accumulating multiple polynomials using `AddAssign`, the result
//! contains many duplicate like terms. `CombineTerms` trait provides
//! in-place combining capability, reducing memory allocation.
//!
//! # 示例 / Example
//!
//! ```
//! use ospf_rust_math::symbol::operation::combine::CombineTerms;
//! use ospf_rust_math::symbol::{Linear, LinearMonomial};
//! use ospf_rust_math::symbols_test;
//!
//! symbols_test!(x, y);
//!
//! // 创建多项式：x + x + x + 2y + y
//! let mut poly = Linear::new(vec![
//!     LinearMonomial::new(1.0, x.clone()),
//!     LinearMonomial::new(1.0, x.clone()),
//!     LinearMonomial::new(1.0, x.clone()),
//!     LinearMonomial::new(2.0, y.clone()),
//!     LinearMonomial::new(1.0, y.clone()),
//! ], 0.0);
//!
//! poly.combine_terms(); // 合并为：3x + 3y
//! ```

use crate::operator::Exponent;
use crate::symbol::{
    Canonical, CanonicalMonomial, Linear, LinearMonomial, OwnedSymbol, Quadratic, QuadraticMonomial,
};
use num_traits::Zero;
use std::collections::{HashMap, hash_map::RandomState};
use std::hash::{BuildHasher, Hash};

// ============================================================================
// CombineTerms trait - 合并同类项 trait
// ============================================================================

/// 合并同类项 trait / Combine like terms trait
///
/// 提供多项式同类项合并功能，优化累加后的结果。
/// Provides polynomial like-term combining to optimize accumulated results.
///
/// # 适用场景 / Use Cases
///
/// - `FastSum` 累加后的多项式简化
/// - 多次加法运算后的结果简化
/// - 手动优化多项式表示
pub trait CombineTerms {
    /// 原地合并同类项
    /// Combine like terms in place
    ///
    /// 修改自身，合并所有同类项并移除零系数项。
    /// Modifies self, combining all like terms and removing zero coefficients.
    fn combine_terms(&mut self);

    /// 合并同类项并返回新实例
    /// Combine like terms and return new instance
    ///
    /// 不修改原实例，返回合并后的新实例。
    /// Does not modify original, returns new combined instance.
    fn combined(&self) -> Self
    where
        Self: Clone,
    {
        let mut result = self.clone();
        result.combine_terms();
        result
    }
}

// ============================================================================
// Linear<T> 实现 / Linear<T> Implementation
// ============================================================================

impl<T> CombineTerms for Linear<T>
where
    T: Clone + Zero + PartialEq + std::ops::Add<Output = T>,
{
    fn combine_terms(&mut self) {
        let mut symbol_coefficients: HashMap<OwnedSymbol, T> = HashMap::new();

        // 合并同类项 / Combine like terms
        for monomial in self.monomials.drain(..) {
            if monomial.coefficient.is_zero() {
                continue;
            }
            let entry = symbol_coefficients
                .entry(monomial.symbol)
                .or_insert_with(|| T::zero());
            *entry = entry.clone() + monomial.coefficient;
        }

        // 移除零系数项并重建向量 / Remove zero coefficients and rebuild vector
        self.monomials = symbol_coefficients
            .into_iter()
            .filter(|(_, c)| !c.is_zero())
            .map(|(symbol, coefficient)| LinearMonomial::new(coefficient, symbol))
            .collect();
    }
}

// ============================================================================
// Quadratic<T> 实现 / Quadratic<T> Implementation
// ============================================================================

impl<T> CombineTerms for Quadratic<T>
where
    T: Clone + Zero + PartialEq + std::ops::Add<Output = T>,
{
    fn combine_terms(&mut self) {
        let mut combined_monomials: Vec<QuadraticMonomial<T>> = Vec::new();
        let mut index: Option<(RandomState, HashMap<u64, Vec<usize>>)> = None;
        let use_index = self.monomials.len() > 128;

        for monomial in self.monomials.drain(..) {
            if monomial.coefficient.is_zero() { continue; }
            // 少量不同项仍直接扫描；超过阈值后才建索引，不因大量重复项支付建表成本。
            // Scan a few distinct terms directly; build an index only past the threshold, not merely for many duplicates.
            if use_index && index.is_none() && combined_monomials.len() >= 32 {
                let hasher = RandomState::new();
                let mut positions: HashMap<u64, Vec<usize>> = HashMap::new();
                for (position, term) in combined_monomials.iter().enumerate() {
                    positions.entry(hasher.hash_one((&term.symbol1, &term.symbol2))).or_default().push(position);
                }
                index = Some((hasher, positions));
            }
            let hash = index.as_ref().map(|(hasher, _)| hasher.hash_one((&monomial.symbol1, &monomial.symbol2)));
            let same_term = |term: &QuadraticMonomial<T>|
                term.symbol1 == monomial.symbol1 && term.symbol2 == monomial.symbol2;
            let existing = if let (Some((_, positions)), Some(hash)) = (&index, hash) {
                // 哈希仅用于缩小候选范围；碰撞仍比较完整有序符号对，不克隆符号。
                // Hashes only narrow candidates; collisions still compare complete ordered symbol pairs without cloning symbols.
                positions.get(&hash).and_then(|bucket| bucket.iter().copied().find(|position| same_term(&combined_monomials[*position])))
            } else {
                combined_monomials.iter().position(same_term)
            };
            if let Some(position) = existing {
                // 累加系数 / Accumulate coefficient
                let existing = &mut combined_monomials[position];
                existing.coefficient = existing.coefficient.clone() + monomial.coefficient;
            } else {
                if let (Some((_, positions)), Some(hash)) = (&mut index, hash) {
                    positions.entry(hash).or_default().push(combined_monomials.len());
                }
                combined_monomials.push(monomial);
            }
        }

        // 移除零系数项 / Remove zero coefficient terms
        self.monomials = combined_monomials
            .into_iter()
            .filter(|m| !m.coefficient.is_zero())
            .collect();
    }
}

// ============================================================================
// Canonical<T, E> 实现 / Canonical<T, E> Implementation
// ============================================================================

impl<T, E> CombineTerms for Canonical<T, E>
where
    T: Clone + Zero + PartialEq + std::ops::Add<Output = T>,
    E: Exponent + Hash + Eq,
{
    fn combine_terms(&mut self) {
        // 幂次向量的哈希键
        // Hash key for power vectors
        let mut term_coefficients: HashMap<Vec<(OwnedSymbol, E)>, T> = HashMap::new();

        // 合并同类项 / Combine like terms
        for monomial in self.monomials.drain(..) {
            if monomial.coefficient.is_zero() {
                continue;
            }

            // 将幂次映射转换为可哈希的向量
            // Convert power map to hashable vector
            let mut powers: Vec<(OwnedSymbol, E)> = monomial.powers.into_iter().collect();
            // 排序以确保相同幂次的项有相同的键
            // Sort to ensure same powers have same key
            powers.sort_by(|a, b| {
                // 使用 dyn_id 进行排序比较
                // Use dyn_id for comparison
                let id_a = a.0.dyn_id();
                let id_b = b.0.dyn_id();
                // 简单排序：按 parent_id, index
                (id_a.parent_id, id_a.index).cmp(&(id_b.parent_id, id_b.index))
            });

            let entry = term_coefficients.entry(powers).or_insert_with(|| T::zero());
            *entry = entry.clone() + monomial.coefficient;
        }

        // 移除零系数项并重建向量 / Remove zero coefficients and rebuild vector
        self.monomials = term_coefficients
            .into_iter()
            .filter(|(_, c)| !c.is_zero())
            .map(|(powers, coefficient)| {
                let powers: HashMap<OwnedSymbol, E> = powers.into_iter().collect();
                CanonicalMonomial::new(coefficient, powers)
            })
            .collect();
    }
}

// ============================================================================
// AddAssignCombine 扩展 trait / AddAssignCombine Extension Trait
// ============================================================================

/// 带合并的加法赋值 trait / Add assign with combining trait
///
/// 在累加后自动合并同类项。
/// Automatically combines like terms after accumulation.
pub trait AddAssignCombine<Rhs = Self>: CombineTerms {
    /// 累加并合并同类项
    /// Add and combine like terms
    fn add_assign_combine(&mut self, rhs: Rhs);
}

impl<T> AddAssignCombine for Linear<T>
where
    T: Clone + Zero + PartialEq + std::ops::Add<Output = T>,
{
    fn add_assign_combine(&mut self, rhs: Self) {
        self.monomials.extend(rhs.monomials);
        self.constant = self.constant.clone() + rhs.constant;
        self.combine_terms();
    }
}

impl<T> AddAssignCombine for Quadratic<T>
where
    T: Clone + Zero + PartialEq + std::ops::Add<Output = T>,
{
    fn add_assign_combine(&mut self, rhs: Self) {
        self.monomials.extend(rhs.monomials);
        self.constant = self.constant.clone() + rhs.constant;
        self.combine_terms();
    }
}

impl<T, E> AddAssignCombine for Canonical<T, E>
where
    T: Clone + Zero + PartialEq + std::ops::Add<Output = T>,
    E: Exponent + Hash + Eq,
{
    fn add_assign_combine(&mut self, rhs: Self) {
        self.monomials.extend(rhs.monomials);
        self.constant = self.constant.clone() + rhs.constant;
        self.combine_terms();
    }
}

// ============================================================================
// 测试 / Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{DynSymbol, LinearMonomial, QuadraticMonomial, SymbolDynId};
    use std::any::Any;

    /// 测试用的简单符号 / Simple symbol for testing
    #[derive(Debug, Clone)]
    struct TestSymbol {
        id: usize,
        name: String,
    }

    impl std::fmt::Display for TestSymbol {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.name)
        }
    }

    impl TestSymbol {
        fn new(name: &str, id: usize) -> Self {
            Self {
                id,
                name: name.to_string(),
            }
        }
    }

    impl DynSymbol for TestSymbol {
        fn name(&self) -> &str {
            &self.name
        }
        fn display_name(&self) -> &str {
            &self.name
        }
        fn dyn_id(&self) -> SymbolDynId<'_> {
            SymbolDynId::standalone(self.id)
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    fn make_symbol(name: &str, id: usize) -> OwnedSymbol {
        OwnedSymbol::new(TestSymbol::new(name, id))
    }

    fn original_combine<T>(polynomial: &mut Quadratic<T>)
    where
        T: Clone + Zero + PartialEq + std::ops::Add<Output = T>,
    {
        let mut combined: Vec<QuadraticMonomial<T>> = Vec::new();
        for monomial in polynomial.monomials.drain(..) {
            if monomial.coefficient.is_zero() { continue; }
            if let Some(existing) = combined.iter_mut().find(|term|
                term.symbol1 == monomial.symbol1 && term.symbol2 == monomial.symbol2)
            {
                existing.coefficient = existing.coefficient.clone() + monomial.coefficient;
            } else { combined.push(monomial); }
        }
        polynomial.monomials = combined.into_iter().filter(|term| !term.coefficient.is_zero()).collect();
    }

    #[test]
    fn indexed_quadratic_combine_matches_reference_without_commuting_or_reordering_terms() {
        let x = make_symbol("x", 1);
        let y = make_symbol("y", 2);
        let mut polynomial = Quadratic::new(vec![
            QuadraticMonomial::quadratic(1e16, x.clone(), y.clone()),
            QuadraticMonomial::quadratic(2.0, y.clone(), x.clone()),
            QuadraticMonomial::quadratic(1.0, x.clone(), y.clone()),
            QuadraticMonomial::quadratic(-1e16, x.clone(), y.clone()),
            QuadraticMonomial::linear(-0.0, x.clone()), QuadraticMonomial::linear(1.0, x.clone()),
            QuadraticMonomial::linear(-1.0, x.clone()), QuadraticMonomial::linear(3.0, x.clone()),
        ], 7.0);
        let mut expected = polynomial.clone();
        original_combine(&mut expected);
        polynomial.combine_terms();
        assert_eq!(polynomial, expected);
        assert_eq!(polynomial.monomials.len(), 2);
        assert_eq!(polynomial.monomials[0].symbol1, y);
        assert_eq!(polynomial.monomials[0].symbol2, Some(x));
        for amount in [0, 1, 8, 128, 129] {
            let symbol = make_symbol("special", 1);
            let mut actual = Quadratic::new((0..amount).map(|i| QuadraticMonomial::linear(
                [1e16, 1.0, -1e16, -0.0, f64::NAN][i % 5], symbol.clone(),
            )).collect(), 0.0);
            let mut expected = actual.clone();
            original_combine(&mut expected);
            actual.combine_terms();
            let bits = |polynomial: Quadratic<f64>| polynomial.monomials.into_iter()
                .map(|term| (term.symbol1, term.symbol2, term.coefficient.to_bits())).collect::<Vec<_>>();
            assert_eq!(bits(actual), bits(expected));
        }
        for case in 0..32 {
            let symbols = (0..64).map(|id| make_symbol(&format!("s-{id}"), id)).collect::<Vec<_>>();
            let terms = (0..512).map(|i| QuadraticMonomial::new(((i * 17 + case) % 23) as f64 - 11.0,
                symbols[(i + case) % 64].clone(), (i % 5 != 0).then(|| symbols[(i * 3 + case) % 64].clone()))).collect();
            let mut actual = Quadratic::new(terms, 5.0);
            let mut exact = Quadratic::new(actual.monomials.iter().map(|term| QuadraticMonomial::new(
                bigdecimal::BigDecimal::from(term.coefficient as i32), term.symbol1.clone(), term.symbol2.clone(),
            )).collect(), bigdecimal::BigDecimal::from(5));
            let mut expected = actual.clone();
            original_combine(&mut expected);
            actual.combine_terms();
            assert_eq!(actual, expected);
            let mut expected = exact.clone();
            original_combine(&mut expected);
            exact.combine_terms();
            assert_eq!(exact, expected);
        }
    }

    #[test]
    #[ignore = "manual release-mode term combining timings"]
    fn performance_b04_quadratic_combine() {
        use std::hint::black_box;
        use std::time::Instant;

        for (amount, repetitions) in [(1, 4), (8, 4), (32, 4), (256, 4), (2048, 4), (1, 8192), (8, 1024), (32, 256), (64, 128)] {
            let symbols = (0..amount).map(|id| make_symbol(&format!("x-{id}"), id)).collect::<Vec<_>>();
            let polynomial = Quadratic::new((0..amount * repetitions).map(|i| QuadraticMonomial::quadratic(1.0,
                symbols[i % amount].clone(), symbols[(i * 17 + 5) % amount].clone())).collect(), 0.0);
            for optimized in [false, true] {
                let mut samples = Vec::new();
                for _ in 0..5 {
                    let mut input = polynomial.clone();
                    let start = Instant::now();
                    if optimized { black_box(&mut input).combine_terms(); } else { original_combine(black_box(&mut input)); }
                    samples.push(start.elapsed().as_nanos());
                    assert_eq!(input.monomials.len(), amount);
                    black_box(input);
                }
                samples.sort_unstable();
                println!("P17 unique={amount}, terms={}, optimized={optimized}: median={} ns; samples={samples:?}", amount * repetitions, samples[2]);
            }
        }
    }

    #[test]
    fn test_linear_combine_terms() {
        let x = make_symbol("x", 1);
        let y = make_symbol("y", 2);

        // 创建多项式：x + x + x + 2y + y + 5
        // Create polynomial: x + x + x + 2y + y + 5
        let mut poly: Linear<f64> = Linear::new(
            vec![
                LinearMonomial::new(1.0, x.clone()),
                LinearMonomial::new(1.0, x.clone()),
                LinearMonomial::new(1.0, x.clone()),
                LinearMonomial::new(2.0, y.clone()),
                LinearMonomial::new(1.0, y.clone()),
            ],
            5.0,
        );

        poly.combine_terms();

        // 应合并为：3x + 3y + 5
        // Should combine to: 3x + 3y + 5
        assert_eq!(poly.monomials.len(), 2);

        // 检查系数 / Check coefficients
        for m in &poly.monomials {
            if m.symbol == x {
                assert!((m.coefficient - 3.0_f64).abs() < 1e-10);
            } else if m.symbol == y {
                assert!((m.coefficient - 3.0_f64).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_linear_combine_zero_terms() {
        let x = make_symbol("x", 1);

        // 创建多项式：x + (-x) + 5
        // Create polynomial: x + (-x) + 5
        let mut poly: Linear<f64> = Linear::new(
            vec![
                LinearMonomial::new(1.0, x.clone()),
                LinearMonomial::new(-1.0, x.clone()),
            ],
            5.0,
        );

        poly.combine_terms();

        // 应合并为：5（零系数项被移除）
        // Should combine to: 5 (zero coefficient terms removed)
        assert_eq!(poly.monomials.len(), 0);
        assert_eq!(poly.constant, 5.0_f64);
    }

    #[test]
    fn test_quadratic_combine_terms() {
        let x = make_symbol("x", 1);
        let y = make_symbol("y", 2);

        // 创建多项式：x*x + 2x*y + x*x + 3x + 2x
        // Create polynomial: x*x + 2x*y + x*x + 3x + 2x
        let mut poly: Quadratic<f64> = Quadratic::new(
            vec![
                QuadraticMonomial::quadratic(1.0, x.clone(), x.clone()), // x*x
                QuadraticMonomial::quadratic(2.0, x.clone(), y.clone()), // 2x*y
                QuadraticMonomial::quadratic(1.0, x.clone(), x.clone()), // x*x
                QuadraticMonomial::linear(3.0, x.clone()),               // 3x
                QuadraticMonomial::linear(2.0, x.clone()),               // 2x
            ],
            0.0,
        );

        poly.combine_terms();

        // 应合并为：2x*x + 2x*y + 5x
        // Should combine to: 2x*x + 2x*y + 5x
        assert_eq!(poly.monomials.len(), 3);
    }

    #[test]
    fn test_add_assign_combine() {
        let x = make_symbol("x", 1);

        // p1 = x + 1
        let p1: Linear<f64> = Linear::new(vec![LinearMonomial::new(1.0, x.clone())], 1.0);
        // p2 = x + 2
        let p2: Linear<f64> = Linear::new(vec![LinearMonomial::new(1.0, x.clone())], 2.0);

        let mut result = p1;
        result.add_assign_combine(p2);

        // 应合并为：2x + 3
        // Should combine to: 2x + 3
        assert_eq!(result.monomials.len(), 1);
        assert!((result.monomials[0].coefficient - 2.0_f64).abs() < 1e-10);
        assert!((result.constant - 3.0_f64).abs() < 1e-10);
    }

    #[test]
    fn test_combined_method() {
        let x = make_symbol("x", 1);

        // 原多项式：x + x
        let poly: Linear<f64> = Linear::new(
            vec![
                LinearMonomial::new(1.0, x.clone()),
                LinearMonomial::new(1.0, x.clone()),
            ],
            0.0,
        );

        // 使用 combined() 返回新实例
        let combined = poly.combined();

        // 原实例不变 / Original instance unchanged
        assert_eq!(poly.monomials.len(), 2);
        // 新实例已合并 / New instance combined
        assert_eq!(combined.monomials.len(), 1);
    }
}
