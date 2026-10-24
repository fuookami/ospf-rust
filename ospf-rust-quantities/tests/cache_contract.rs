use std::hint::black_box;
use std::sync::{Arc, Barrier};
use std::time::Instant;
use ospf_rust_quantities::Quantity;
use ospf_rust_quantities::unit::derived::{Kilometer, Meter};
use ospf_rust_quantities::unit::{CTUnit, ct_conversion_factor};

fn report(label: &str, mut run: impl FnMut()) {
    let mut samples = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        run();
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    println!("{label}: median_ms={:.6} range_ms={:.6}..{:.6}", samples[2], samples[0], samples[4]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use bigdecimal::BigDecimal;
    use ospf_rust_quantities::{define_unit, define_unit_by, QuantityTrait};
    use ospf_rust_quantities::dimension::derived::{Length, Time};
    use ospf_rust_quantities::dimension::derived_quantity::{QuantityDomain, CTDerivedQuantity};
    use ospf_rust_quantities::scale::Scale;
    use ospf_rust_quantities::unit::UnitTrait;
    use ospf_rust_quantities::unit::derived::{Celsius, Fahrenheit, Inch, Kelvin, Second};
    use ospf_rust_quantities::unit::{CTUnitMul, UnitConversionCalculation};

    static UNCHANGED: AtomicUsize = AtomicUsize::new(0);
    static SHARED: AtomicUsize = AtomicUsize::new(0);
    static SHARED_FAILURES: AtomicUsize = AtomicUsize::new(0);
    static ORDINARY_FAILURES: AtomicUsize = AtomicUsize::new(0);
    static SCALE_ONLY: AtomicUsize = AtomicUsize::new(0);
    static FULL_METADATA: AtomicUsize = AtomicUsize::new(0);

    define_unit!(@shared ScaleOnly, "scale only", "so", Length, {
        SCALE_ONLY.fetch_add(1, Ordering::SeqCst);
        Scale::from_int(1000)
    }, offset = {
        FULL_METADATA.fetch_add(1, Ordering::SeqCst);
        BigDecimal::from(0)
    });

    #[test]
    fn scale_only_cold_access_does_not_build_full_unit_metadata() {
        assert_eq!(ScaleOnly.dimension_symbol(), Length::INSTANT.symbol());
        assert_eq!(Quantity::<f64, ScaleOnly>::new_ct(1.0).dimension_symbol(), Length::INSTANT.symbol());
        assert_eq!(SCALE_ONLY.load(Ordering::SeqCst), 0);
        assert_eq!(FULL_METADATA.load(Ordering::SeqCst), 0);
        let barrier = Barrier::new(8);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    barrier.wait();
                    for _ in 0..20 {
                        assert_eq!(ct_conversion_factor::<ScaleOnly, Meter>(), BigDecimal::from(1000));
                        assert_eq!(ScaleOnly.scale_value(), BigDecimal::from(1000));
                        assert_eq!(Quantity::<f64, ScaleOnly>::new_ct(1.0).scale_value(), BigDecimal::from(1000));
                    }
                });
            }
        });
        assert_eq!(SCALE_ONLY.load(Ordering::SeqCst), 1);
        assert_eq!(FULL_METADATA.load(Ordering::SeqCst), 0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    assert_eq!(ScaleOnly::convert_value_to::<f64, Meter>(1.0), Some(1000.0));
                    assert_eq!(ct_conversion_factor::<ScaleOnly, Meter>(), BigDecimal::from(1000));
                });
            }
        });
        assert_eq!(SCALE_ONLY.load(Ordering::SeqCst), 1);
        assert_eq!(FULL_METADATA.load(Ordering::SeqCst), 1);
        let dimension = ScaleOnly::shared_dimension().unwrap();
        assert!(std::ptr::eq(dimension, ScaleOnly::shared_dimension().unwrap()));
        assert_eq!(dimension.symbol().as_ptr(), ScaleOnly::shared_instance().unwrap().dimension().symbol().as_ptr());
    }

    define_unit!(UnsharedCounted, "counted", "ct", Length, {
        UNCHANGED.fetch_add(1, Ordering::SeqCst);
        Scale::from_int(1000)
    });
    define_unit!(@shared SharedCounted, "shared", "sc", Length, {
        SHARED.fetch_add(1, Ordering::SeqCst);
        Scale::from_int(1000)
    });
    define_unit!(@shared SharedSecond, "second", "s", Time);
    define_unit!(@shared SharedDiscrete, "discrete", "d", Length, Scale::new(), domain = QuantityDomain::Discrete);
    define_unit!(@shared SharedAffine, "affine", "a", Length, Scale::new(), offset = BigDecimal::from(2));
    define_unit_by!(@shared SharedArea, "area", "a2", CTUnitMul<Kilometer, Meter>);
    define_unit!(@shared FailingShared, "failure", "fs", Length, {
        SHARED_FAILURES.fetch_add(1, Ordering::SeqCst);
        panic!("shared initializer failure")
    });
    define_unit!(RetryOrdinary, "retry", "ro", Length, {
        if ORDINARY_FAILURES.fetch_add(1, Ordering::SeqCst) == 0 { panic!("ordinary initializer failure"); }
        Scale::new()
    });

    struct ManualShared;

    impl UnitTrait for ManualShared {
        type Dimension = Length;

        fn symbol(&self) -> &'static str { "manual" }

        fn name(&self) -> &'static str { "manual" }

        fn dimension_symbol(&self) -> String { Length::INSTANT.symbol().to_string() }

        fn scale_value(&self) -> BigDecimal { BigDecimal::from(1) }
    }

    impl CTUnit for ManualShared {
        type Dimension = Length;

        fn shared_instance() -> Option<&'static ospf_rust_quantities::unit::Unit> {
            static UNIT: once_cell::sync::Lazy<ospf_rust_quantities::unit::Unit> =
                once_cell::sync::Lazy::new(|| ManualShared::INSTANT.clone());
            Some(&*UNIT)
        }
    }

    #[test]
    fn existing_manual_shared_implementations_supply_the_default_shared_scale() {
        assert!(ManualShared::shared_dimension().is_none());
        assert!(std::ptr::eq(ManualShared::shared_scale().unwrap(), ManualShared::shared_instance().unwrap().scale()));
        assert_eq!(ct_conversion_factor::<ManualShared, Meter>(), BigDecimal::from(1));
    }

    #[test]
    fn shared_failure_is_poisoned_without_changing_ordinary_retry_semantics() {
        assert_eq!(FailingShared.dimension_symbol(), Length::INSTANT.symbol());
        assert!(FailingShared::dim_eq::<Meter>());
        assert!(!FailingShared::dim_eq::<Second>());
        assert!(!FailingShared::unit_eq::<Second>());
        assert_eq!(FailingShared::convert_value_to::<f64, Second>(1.0), None);
        assert_eq!(FailingShared::conversion_factor_to::<Second>(), None);
        assert_eq!(SHARED_FAILURES.load(Ordering::SeqCst), 0);
        assert!(std::panic::catch_unwind(FailingShared::runtime_unit).is_err());
        assert!(std::panic::catch_unwind(FailingShared::runtime_unit).is_err());
        assert!(std::panic::catch_unwind(|| ct_conversion_factor::<FailingShared, Meter>()).is_err());
        assert_eq!(SHARED_FAILURES.load(Ordering::SeqCst), 1);
        assert_eq!(FailingShared.dimension_symbol(), Length::INSTANT.symbol());
        assert_eq!(FailingShared::convert_value_to::<f64, Second>(1.0), None);
        assert_eq!(FailingShared::conversion_factor_to::<Second>(), None);
        assert!(std::panic::catch_unwind(RetryOrdinary::runtime_unit).is_err());
        assert_eq!(RetryOrdinary::runtime_unit().scale().value(), &BigDecimal::from(1));
        assert_eq!(ORDINARY_FAILURES.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn ordinary_macro_and_direct_consts_preserve_reinitialization() {
        assert!(UnsharedCounted::shared_instance().is_none());
        for _ in 0..10 { assert_eq!(ct_conversion_factor::<UnsharedCounted, Meter>(), BigDecimal::from(1000)); }
        assert_eq!(UNCHANGED.load(Ordering::SeqCst), 10);
        for _ in 0..10 { assert_eq!(UnsharedCounted::convert_value_to::<f64, Meter>(1.0), Some(1000.0)); }
        assert_eq!(UNCHANGED.load(Ordering::SeqCst), 20);
        for _ in 0..10 { assert_eq!(UnsharedCounted::runtime_unit().scale().value(), &BigDecimal::from(1000)); }
        assert_eq!(UNCHANGED.load(Ordering::SeqCst), 30);
        // 直接访问保留的关联 const 仍是新实例。 / Direct access to the retained associated const still creates a fresh instance.
        for _ in 0..10 { assert_eq!(UnsharedCounted::SCALE.value(), &BigDecimal::from(1000)); }
        assert_eq!(UNCHANGED.load(Ordering::SeqCst), 40);
        assert!(!UnsharedCounted::unit_eq::<Meter>());
        assert_eq!(UNCHANGED.load(Ordering::SeqCst), 41);
        assert!(UnsharedCounted::unit_eq::<UnsharedCounted>());
        assert_eq!(UNCHANGED.load(Ordering::SeqCst), 43);
        assert!(!UnsharedCounted::unit_eq::<Second>());
        assert_eq!(UNCHANGED.load(Ordering::SeqCst), 43);
    }

    #[test]
    fn shared_macro_is_lazy_and_initializes_once_across_threads() {
        let _unused = SharedCounted;
        assert_eq!(SHARED.load(Ordering::SeqCst), 0);
        let barrier = Barrier::new(8);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    barrier.wait();
                    for _ in 0..20 {
                        assert_eq!(SharedCounted::convert_value_to::<f64, Meter>(1.0), Some(1000.0));
                        assert_eq!(ct_conversion_factor::<SharedCounted, Meter>(), BigDecimal::from(1000));
                    }
                });
            }
        });
        assert_eq!(SHARED.load(Ordering::SeqCst), 1);
        assert!(SharedCounted::unit_eq::<SharedCounted>());
        assert!(!SharedCounted::unit_eq::<Meter>());
        assert_eq!(SHARED.load(Ordering::SeqCst), 1);
        let first = SharedCounted::shared_instance().unwrap();
        assert!(std::ptr::eq(first, SharedCounted::shared_instance().unwrap()));
        assert!(std::ptr::eq(first.scale(), SharedCounted::runtime_unit().scale()));
    }

    fn legacy_convert<From: CTUnit, To: CTUnit>(value: BigDecimal) -> Option<BigDecimal> {
        let from = From::INSTANT;
        let to = To::INSTANT;
        let standard = from.conversion().to_standard_value_checked(value)?;
        to.conversion().value_from_standard_checked(standard)
    }

    #[test]
    fn shared_paths_preserve_exact_fractional_and_affine_arithmetic() {
        for input in ["-10.125", "0", "1.25", "1000000000000000000000000.00001"] {
            let value: BigDecimal = input.parse().unwrap();
            assert_eq!(Inch::convert_value_to::<_, Meter>(value.clone()), legacy_convert::<Inch, Meter>(value.clone()));
            assert_eq!(Celsius::convert_value_to::<_, Fahrenheit>(value.clone()), legacy_convert::<Celsius, Fahrenheit>(value.clone()));
            assert_eq!(Fahrenheit::convert_value_to::<_, Kelvin>(value.clone()), legacy_convert::<Fahrenheit, Kelvin>(value));
        }
        for input in [-0.0f64, 0.0, -32.125, f64::INFINITY, f64::NAN] {
            let from = Celsius::INSTANT;
            let to = Fahrenheit::INSTANT;
            let expected: Option<f64> = from.conversion().to_standard_value_checked(input)
                .and_then(|value| to.conversion().value_from_standard_checked(value));
            let actual = Celsius::convert_value_to::<_, Fahrenheit>(input);
            match (actual, expected) {
                (Some(actual), Some(expected)) if expected.is_nan() => assert!(actual.is_nan()),
                (Some(actual), Some(expected)) => assert_eq!(actual.to_bits(), expected.to_bits()),
                (None, None) => {},
                _ => panic!("checked conversion mismatch"),
            }
        }
        assert_eq!(Celsius::conversion_factor_to::<Kelvin>(), None);
        assert_eq!(Meter::convert_value_to::<f64, Second>(1.0), None);
        let one = num_rational::BigRational::from_integer(1.into());
        // Inch 既有定义来自二进制 f64，不能擅自替换为十进制 127/5000。 / The existing Inch definition comes from binary f64, not exact decimal 127/5000.
        let legacy = Inch::INSTANT.convert_value_to(one.clone(), &Meter::INSTANT);
        assert_eq!(Inch::convert_value_to::<_, Meter>(one), legacy);
    }

    #[test]
    #[ignore = "新进程中单独运行的首次访问测量 / First-access measurement run alone in a fresh process"]
    fn performance_cache_cold_first_use() {
        let start = Instant::now();
        let actual = Quantity::<f64, Kilometer>::new_ct(black_box(1.25)).to::<Meter>();
        let shared_us = start.elapsed().as_secs_f64() * 1e6;
        let start = Instant::now();
        assert_eq!(<Kilometer as CTUnit>::Dimension::INSTANT.symbol(), <Meter as CTUnit>::Dimension::INSTANT.symbol());
        let from = Kilometer::INSTANT;
        let to = Meter::INSTANT;
        let standard: f64 = from.conversion().to_standard_value_checked(black_box(1.25)).unwrap();
        let expected: f64 = to.conversion().value_from_standard_checked(standard).unwrap();
        let legacy_us = start.elapsed().as_secs_f64() * 1e6;
        assert_eq!(actual.value.to_bits(), expected.to_bits());
        println!("cold_quantity_one shared_us={shared_us:.3} legacy_reference_us={legacy_us:.3}");
    }

    #[test]
    fn shared_definitions_cover_all_macro_forms_and_keep_types_separate() {
        assert!(SharedSecond::shared_instance().is_some());
        assert_eq!(SharedDiscrete::shared_instance().unwrap().domain(), QuantityDomain::Discrete);
        assert_eq!(SharedAffine::convert_value_to::<f64, Meter>(1.0), Some(3.0));
        assert_eq!(SharedArea::runtime_unit().scale().value(), &BigDecimal::from(1000));
        assert_eq!(SharedSecond.dimension_symbol(), Time::INSTANT.symbol());
        assert_eq!(SharedDiscrete.scale_value(), BigDecimal::from(1));
        assert!(!std::ptr::eq(Meter::shared_instance().unwrap(), Kilometer::shared_instance().unwrap()));
        // 泛型复合单位不使用可能被不同单态共享的普通 static。 / Generic compound units do not use a plain static shared across monomorphizations.
        assert!(CTUnitMul::<Meter, Meter>::shared_instance().is_none());
        assert_eq!(CTUnitMul::<Meter, Meter>::runtime_unit().scale().value(), &BigDecimal::from(1));
        assert_eq!(CTUnitMul::<Kilometer, Meter>::runtime_unit().scale().value(), &BigDecimal::from(1000));
    }

    #[test]
    #[ignore = "手动 release 缓存测量 / Manual release cache measurement"]
    fn performance_cache_conversion() {
        report("factor_10000", || {
            for _ in 0..10_000 { black_box(ct_conversion_factor::<Kilometer, Meter>()); }
        });
        report("quantity_checked_10000", || {
            for index in 0..10_000 {
                let quantity = Quantity::<f64, Kilometer>::new_ct(black_box(index as f64));
                black_box(quantity.to::<Meter>());
            }
        });
        let from = Kilometer::INSTANT.clone();
        let to = Meter::INSTANT.clone();
        report("runtime_frozen_units_10000", || {
            for index in 0..10_000 { black_box(from.convert_value_to(index as f64, &to)); }
        });
        for workers in [1, 2, 4] {
            report(&format!("quantity_checked_total4096_workers{workers}"), || {
                let barrier = Arc::new(Barrier::new(workers));
                std::thread::scope(|scope| {
                    for _ in 0..workers {
                        let barrier = barrier.clone();
                        scope.spawn(move || {
                            barrier.wait();
                            for index in 0..4096 / workers {
                                let quantity = Quantity::<f64, Kilometer>::new_ct(black_box(index as f64));
                                black_box(quantity.to::<Meter>());
                            }
                        });
                    }
                });
            });
        }
    }
}
