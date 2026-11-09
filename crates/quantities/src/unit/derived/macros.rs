//! 单位定义宏 / Unit definition macros
//!
//! 提供用于定义 SI 单位的宏，包括基本单位定义宏和导出单位定义宏 / Provides macros for defining SI units, including base unit definition macros and derived unit definition macros

/// 定义 SI 单位（运行时 + 编译时）
/// Define SI unit (runtime + compile-time)
///
/// # 参数 / Parameters
/// - `$name`: 单位名称标识符（如 METER） / Unit name identifier (e.g., METER)
/// - `$full_name`: 完整名称（如 "meter"） / Full name (e.g., "meter")
/// - `$symbollic`: 符号（如 "m"） / Symbol (e.g., "m")
/// - `$dim`: 量纲类型（如 Length，来自 dimension::derived） / Dimension type (e.g., Length, from dimension::derived)
/// - `$(, scale = $scale:expr)?`: 可选的缩放因子 / Optional scale factor
///
/// # 生成 / Generates
/// - `$name`: 编译时单位结构体（同时实现 CTUnit 和 UnitTrait）/ Compile-time unit struct (implementing both CTUnit and UnitTrait)
///
/// 使用 `@shared` 前缀仅对纯定义启用具体单位的惰性单例；普通调用不改变初始化语义。
/// The `@shared` prefix opts pure definitions into a lazy concrete-unit singleton; ordinary calls retain their initialization semantics.
#[macro_export]
macro_rules! define_unit {
    // 共享仅对纯定义显式启用，普通宏调用保持逐次初始化。 / Sharing is explicit for pure definitions; ordinary calls retain per-access initialization.
    (@shared $($args:tt)*) => {
        $crate::define_unit!(@share true; $($args)*);
    };
    (@cache $name:ident true) => {
        fn shared_dimension() -> Option<&'static $crate::dimension::derived_quantity::DerivedQuantity> {
            static DIMENSION: once_cell::sync::Lazy<$crate::dimension::derived_quantity::DerivedQuantity> =
                once_cell::sync::Lazy::new(|| {
                    once_cell::sync::Lazy::into_value(<<$name as $crate::unit::physical_unit::CTUnit>::Dimension as $crate::dimension::derived_quantity::CTDerivedQuantity>::INSTANT)
                        .unwrap_or_else(|initialize| initialize())
                });
            Some(&*DIMENSION)
        }

        fn shared_scale() -> Option<&'static $crate::scale::Scale> {
            static SCALE: once_cell::sync::Lazy<$crate::scale::Scale> =
                once_cell::sync::Lazy::new(|| {
                    // 消费新 const 的初始化器，避免先创建再深克隆比例尺。 / Consume the fresh const initializer instead of constructing and deep-cloning its scale.
                    once_cell::sync::Lazy::into_value(<$name as $crate::unit::physical_unit::CTUnit>::SCALE)
                        .unwrap_or_else(|initialize| initialize())
                });
            Some(&*SCALE)
        }

        fn shared_instance() -> Option<&'static $crate::unit::physical_unit::Unit> {
            static INSTANCE: once_cell::sync::Lazy<$crate::unit::physical_unit::Unit> =
                once_cell::sync::Lazy::new(|| {
                    // 比例尺只初始化一次；名称、量纲和偏移量仍按需构造。 / Initialize the scale once; build names, dimensions and offsets only on demand.
                    let scale = <$name as $crate::unit::physical_unit::CTUnit>::shared_scale()
                        .expect("shared scale").clone();
                    let conversion = if *<$name as $crate::unit::physical_unit::CTUnit>::OFFSET == bigdecimal::BigDecimal::from(0) {
                        $crate::unit::physical_unit::UnitConversionRule::linear(scale)
                    } else {
                        $crate::unit::physical_unit::UnitConversionRule::affine(scale, <$name as $crate::unit::physical_unit::CTUnit>::OFFSET.clone())
                    };
                    $crate::unit::physical_unit::Unit::new_with_conversion_and_domain(
                        <$name as $crate::unit::physical_unit::CTUnit>::NAME.to_string(),
                        <$name as $crate::unit::physical_unit::CTUnit>::SYMBOL.to_string(),
                        <$name as $crate::unit::physical_unit::CTUnit>::shared_dimension()
                            .expect("shared dimension").clone(),
                        conversion,
                        <$name as $crate::unit::physical_unit::CTUnit>::DOMAIN,
                    )
                });
            Some(&*INSTANCE)
        }
    };
    // 无缩放参数
    ($(@share $shared:tt;)? $name:ident, $full_name:expr, $symbol:expr, $dim:ty) => {
        $crate::define_unit!(
            $(@share $shared;)?
            $name,
            $full_name,
            $symbol,
            $dim,
            $crate::scale::Scale::new()
        );
    };
    // 带缩放参数
    ($(@share $shared:tt;)? $name:ident, $full_name:expr, $symbol:expr, $dim:ty, $scale:expr) => {
        /// 编译时单位 / Compile-time unit
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct $name;

        impl $crate::unit::physical_unit::CTUnit for $name {
            $($crate::define_unit!(@cache $name $shared);)?
            const NAME: &'static str = $full_name;
            const SYMBOL: &'static str = $symbol;
            const SCALE: once_cell::sync::Lazy<$crate::scale::Scale> =
                once_cell::sync::Lazy::new(|| $scale);
            type Dimension = $dim;
        }

        impl $crate::unit::concept::UnitTrait for $name {
            type Dimension = $dim;

            fn symbol(&self) -> &'static str {
                $symbol
            }

            fn name(&self) -> &'static str {
                $full_name
            }

            fn dimension_symbol(&self) -> String {
                if let Some(dimension) = <Self as $crate::unit::physical_unit::CTUnit>::shared_dimension() {
                    dimension.symbol().to_string()
                } else {
                    <$dim as $crate::dimension::derived_quantity::CTDerivedQuantity>::INSTANT.symbol().to_string()
                }
            }

            fn scale_value(&self) -> bigdecimal::BigDecimal {
                if let Some(scale) = <Self as $crate::unit::physical_unit::CTUnit>::shared_scale() {
                    scale.value().clone()
                } else {
                    Self::SCALE.value().clone()
                }
            }
        }
    };
    // 带缩放和 offset 参数 / With scale and offset parameters
    ($(@share $shared:tt;)? $name:ident, $full_name:expr, $symbol:expr, $dim:ty, $scale:expr, offset = $offset:expr) => {
        /// 编译时单位 / Compile-time unit
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct $name;

        impl $crate::unit::physical_unit::CTUnit for $name {
            $($crate::define_unit!(@cache $name $shared);)?
            const NAME: &'static str = $full_name;
            const SYMBOL: &'static str = $symbol;
            const SCALE: once_cell::sync::Lazy<$crate::scale::Scale> =
                once_cell::sync::Lazy::new(|| $scale);
            const OFFSET: once_cell::sync::Lazy<bigdecimal::BigDecimal> =
                once_cell::sync::Lazy::new(|| $offset);
            type Dimension = $dim;
        }

        impl $crate::unit::concept::UnitTrait for $name {
            type Dimension = $dim;

            fn symbol(&self) -> &'static str {
                $symbol
            }

            fn name(&self) -> &'static str {
                $full_name
            }

            fn dimension_symbol(&self) -> String {
                if let Some(dimension) = <Self as $crate::unit::physical_unit::CTUnit>::shared_dimension() {
                    dimension.symbol().to_string()
                } else {
                    <$dim as $crate::dimension::derived_quantity::CTDerivedQuantity>::INSTANT.symbol().to_string()
                }
            }

            fn scale_value(&self) -> bigdecimal::BigDecimal {
                if let Some(scale) = <Self as $crate::unit::physical_unit::CTUnit>::shared_scale() {
                    scale.value().clone()
                } else {
                    Self::SCALE.value().clone()
                }
            }
        }
    };
    // 带缩放和 domain 参数 / With scale and domain parameters
    ($(@share $shared:tt;)? $name:ident, $full_name:expr, $symbol:expr, $dim:ty, $scale:expr, domain = $domain:expr) => {
        /// 编译时单位 / Compile-time unit
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct $name;

        impl $crate::unit::physical_unit::CTUnit for $name {
            $($crate::define_unit!(@cache $name $shared);)?
            const NAME: &'static str = $full_name;
            const SYMBOL: &'static str = $symbol;
            const SCALE: once_cell::sync::Lazy<$crate::scale::Scale> =
                once_cell::sync::Lazy::new(|| $scale);
            const DOMAIN: $crate::dimension::derived_quantity::QuantityDomain = $domain;
            type Dimension = $dim;
        }

        impl $crate::unit::concept::UnitTrait for $name {
            type Dimension = $dim;

            fn symbol(&self) -> &'static str {
                $symbol
            }

            fn name(&self) -> &'static str {
                $full_name
            }

            fn dimension_symbol(&self) -> String {
                if let Some(dimension) = <Self as $crate::unit::physical_unit::CTUnit>::shared_dimension() {
                    dimension.symbol().to_string()
                } else {
                    <$dim as $crate::dimension::derived_quantity::CTDerivedQuantity>::INSTANT.symbol().to_string()
                }
            }

            fn scale_value(&self) -> bigdecimal::BigDecimal {
                if let Some(scale) = <Self as $crate::unit::physical_unit::CTUnit>::shared_scale() {
                    scale.value().clone()
                } else {
                    Self::SCALE.value().clone()
                }
            }
        }
    };
}

/// 通过单位表达式定义单位，可用 `@shared` 显式共享纯定义。 / Define a unit from a unit expression, optionally sharing a pure definition with `@shared`.
#[macro_export]
macro_rules! define_unit_by {
    // 复合纯定义也可选择独立的具体单位缓存。 / Pure compound definitions may opt into a separate concrete-unit cache.
    (@shared $($args:tt)*) => {
        $crate::define_unit_by!(@share true; $($args)*);
    };
    // 单位计算
    ($(@share $shared:tt;)? $name:ident, $full_name:expr, $symbol:expr, $ct_expr:ty) => {
        /// 编译时单位 / Compile-time unit
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct $name;

        impl $crate::unit::physical_unit::CTUnit for $name {
            $($crate::define_unit!(@cache $name $shared);)?
            const NAME: &'static str = $full_name;
            const SYMBOL: &'static str = $symbol;
            const SCALE: once_cell::sync::Lazy<$crate::scale::Scale> = once_cell::sync::Lazy::new(|| <$ct_expr as $crate::unit::physical_unit::CTUnit>::SCALE.clone());
            const OFFSET: once_cell::sync::Lazy<bigdecimal::BigDecimal> = once_cell::sync::Lazy::new(|| <$ct_expr as $crate::unit::physical_unit::CTUnit>::OFFSET.clone());
            const DOMAIN: $crate::dimension::derived_quantity::QuantityDomain = <$ct_expr as $crate::unit::physical_unit::CTUnit>::DOMAIN;
            type Dimension = <$ct_expr as $crate::unit::physical_unit::CTUnit>::Dimension;
        }

        impl $crate::unit::concept::UnitTrait for $name {
            type Dimension = <$ct_expr as $crate::unit::physical_unit::CTUnit>::Dimension;

            fn symbol(&self) -> &'static str {
                $symbol
            }

            fn name(&self) -> &'static str {
                $full_name
            }

            fn dimension_symbol(&self) -> String {
                if let Some(dimension) = <Self as $crate::unit::physical_unit::CTUnit>::shared_dimension() {
                    dimension.symbol().to_string()
                } else {
                    <<$ct_expr as $crate::unit::physical_unit::CTUnit>::Dimension as $crate::dimension::derived_quantity::CTDerivedQuantity>::INSTANT.symbol().to_string()
                }
            }

            fn scale_value(&self) -> bigdecimal::BigDecimal {
                if let Some(scale) = <Self as $crate::unit::physical_unit::CTUnit>::shared_scale() {
                    scale.value().clone()
                } else {
                    Self::SCALE.value().clone()
                }
            }
        }
    };
}
