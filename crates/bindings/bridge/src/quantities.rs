//! 可移植单位和带单位数值协议。 / Portable units and quantity-valued protocol.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::{CTUnit, Unit};
use ospf_rust_quantities::unit::derived::{Centimeter, CubicMeter, Decimeter, Foot, Gram, Hour, Inch, Kilogram, Kilometer, Liter, Meter, Millimeter, Minute, Pound, Second, SquareCentimeter, SquareDecimeter, SquareKilometer, SquareMeter, SquareMillimeter, Tonne, Yard};
use crate::{Result, invalid};

/// 绑定已开放的单位，量纲检查仍由 Rust quantities 执行。 / Units exposed by the bindings; Rust quantities still performs dimension checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum QuantityUnit {
    /// 米。 / Meter.
    #[serde(rename = "m")]
    Meter,
    /// 千米。 / Kilometer.
    #[serde(rename = "km")]
    Kilometer,
    /// 厘米。 / Centimeter.
    #[serde(rename = "cm")]
    Centimeter,
    /// 毫米。 / Millimeter.
    #[serde(rename = "mm")]
    Millimeter,
    /// 秒。 / Second.
    #[serde(rename = "s")]
    Second,
    /// 分钟。 / Minute.
    #[serde(rename = "min")]
    Minute,
    /// 小时。 / Hour.
    #[serde(rename = "h")]
    Hour,
    /// 千克。 / Kilogram.
    #[serde(rename = "kg")]
    Kilogram,
    /// 克。 / Gram.
    #[serde(rename = "g")]
    Gram,
    /// 立方米。 / Cubic meter.
    #[serde(rename = "m^3")]
    CubicMeter,
    /// 升（1 dm³）。 / Liter (1 dm³).
    #[serde(rename = "L")]
    Liter,
    /// 平方米。 / Square meter.
    #[serde(rename = "m^2")]
    SquareMeter,
    /// 平方千米。 / Square kilometer.
    #[serde(rename = "km^2")]
    SquareKilometer,
    /// 平方厘米。 / Square centimeter.
    #[serde(rename = "cm^2")]
    SquareCentimeter,
    /// 平方毫米。 / Square millimeter.
    #[serde(rename = "mm^2")]
    SquareMillimeter,
    /// 分米。 / Decimeter.
    #[serde(rename = "dm")]
    Decimeter,
    /// 英寸。 / Inch.
    #[serde(rename = "in")]
    Inch,
    /// 英尺。 / Foot.
    #[serde(rename = "ft")]
    Foot,
    /// 码。 / Yard.
    #[serde(rename = "yd")]
    Yard,
    /// 吨。 / Tonne.
    #[serde(rename = "t")]
    Tonne,
    /// 磅。 / Pound.
    #[serde(rename = "lb")]
    Pound,
    /// 平方分米。 / Square decimeter.
    #[serde(rename = "dm^2")]
    SquareDecimeter,
    /// 无量纲（原定义的 `1`），用于产率与同量纲比值。 / Dimensionless (the original `1`), used for yields and same-dimension ratios.
    #[serde(rename = "1")]
    Dimensionless,
}

impl QuantityUnit {
    /// 复用既有单位定义。 / Reuse the existing unit definitions.
    pub fn unit(self) -> Unit {
        match self {
            Self::Meter => Meter::INSTANT.clone(), Self::Kilometer => Kilometer::INSTANT.clone(),
            Self::Centimeter => Centimeter::INSTANT.clone(), Self::Millimeter => Millimeter::INSTANT.clone(),
            Self::Second => Second::INSTANT.clone(), Self::Minute => Minute::INSTANT.clone(), Self::Hour => Hour::INSTANT.clone(),
            Self::Kilogram => Kilogram::INSTANT.clone(), Self::Gram => Gram::INSTANT.clone(),
            Self::CubicMeter => CubicMeter::INSTANT.clone(), Self::Liter => Liter::INSTANT.clone(),
            Self::SquareMeter => SquareMeter::INSTANT.clone(), Self::SquareKilometer => SquareKilometer::INSTANT.clone(),
            Self::SquareCentimeter => SquareCentimeter::INSTANT.clone(), Self::SquareMillimeter => SquareMillimeter::INSTANT.clone(),
            Self::Decimeter => Decimeter::INSTANT.clone(), Self::Inch => Inch::INSTANT.clone(), Self::Foot => Foot::INSTANT.clone(),
            Self::Yard => Yard::INSTANT.clone(), Self::Tonne => Tonne::INSTANT.clone(), Self::Pound => Pound::INSTANT.clone(),
            Self::SquareDecimeter => SquareDecimeter::INSTANT.clone(),
            Self::Dimensionless => ospf_rust_quantities::unit::derived::None::INSTANT.clone(),
        }
    }
}

/// 显式携带单位的 f64 数值，不提供 Rust 编译期量纲保证。 / Explicitly unit-bearing f64 value, without Rust compile-time dimensional guarantees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhysicalQuantity {
    /// 有限数值。 / Finite numeric value.
    pub value: f64,
    /// 单位。 / Unit.
    pub unit: QuantityUnit,
}

impl PhysicalQuantity {
    /// 校验数值后构造 Rust 运行时物理量。 / Construct a Rust runtime quantity after checking the value.
    pub fn quantity(&self) -> Result<Quantity<f64, Unit>> {
        if !self.value.is_finite() { return Err(invalid("quantity must be finite")); }
        Ok(Quantity::new(self.value, self.unit.unit()))
    }
}

#[cfg(test)]
mod volume_tests {
    use super::*;
    use ospf_rust_quantities::quantity::Quantity as RawQuantity;
    use ospf_rust_quantities::unit::physical_unit::Unit as _;

    /// 体积单位沿用原定义：1 L = 1 dm³ = 0.001 m³，且与长度量纲不可互换。
    /// Volume units reuse the original definitions: 1 L = 1 dm³ = 0.001 m³, and they never interchange with lengths.
    #[test]
    fn volume_units_convert_through_the_original_definitions() {
        let liter = QuantityUnit::Liter.unit();
        let cubic_meter = QuantityUnit::CubicMeter.unit();
        assert_eq!(liter.symbol(), "L");
        assert_eq!(cubic_meter.symbol(), "m³");
        let one_liter = RawQuantity::new(1.0, liter).to_unit(&cubic_meter.clone()).unwrap();
        assert!((one_liter.value - 0.001).abs() < 1e-12, "1 L must be 0.001 m³, got {}", one_liter.value);
        let thousand = RawQuantity::new(1.0, cubic_meter.clone()).to_unit(&QuantityUnit::Liter.unit()).unwrap();
        assert!((thousand.value - 1000.0).abs() < 1e-9, "1 m³ must be 1000 L, got {}", thousand.value);
        // 体积与长度、面积都不兼容。 / Volume is incompatible with length and area.
        assert!(RawQuantity::new(1.0, QuantityUnit::Meter.unit()).to_unit(&cubic_meter).is_err());
        assert!(RawQuantity::new(1.0, QuantityUnit::SquareMeter.unit()).to_unit(&cubic_meter).is_err());
        assert!(RawQuantity::new(1.0, QuantityUnit::Liter.unit()).to_unit(&QuantityUnit::Kilogram.unit()).is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_quantities::unit::UnitTrait;

    fn converted(value: f64, from: QuantityUnit, to: QuantityUnit) -> f64 {
        PhysicalQuantity { value, unit: from }.quantity().unwrap().to_unit(&to.unit()).unwrap().value
    }

    #[test]
    fn added_units_convert_through_the_original_definitions() {
        // 每一项都对照领域内置定义后的直觉值，容差只吸收 f64 表示误差。
        // Each case matches the value implied by the original domain definition, with a tolerance that only
        // absorbs f64 representation error.
        assert!((converted(1.0, QuantityUnit::Decimeter, QuantityUnit::Meter) - 0.1).abs() < 1e-15);
        assert!((converted(1.0, QuantityUnit::Inch, QuantityUnit::Centimeter) - 2.54).abs() < 1e-12);
        assert!((converted(1.0, QuantityUnit::Foot, QuantityUnit::Meter) - 0.3048).abs() < 1e-12);
        assert!((converted(1.0, QuantityUnit::Yard, QuantityUnit::Foot) - 3.0).abs() < 1e-12);
        assert!((converted(1.0, QuantityUnit::Tonne, QuantityUnit::Kilogram) - 1000.0).abs() < 1e-9);
        assert!((converted(1.0, QuantityUnit::Pound, QuantityUnit::Kilogram) - 0.45359237).abs() < 1e-15);
        assert!((converted(1.0, QuantityUnit::SquareDecimeter, QuantityUnit::SquareMeter) - 0.01).abs() < 1e-15);
        // 量纲仍由 Rust quantities 判定：新单位不能跨量纲转换。
        // Dimensions are still decided by Rust quantities: the new units do not cross dimensions.
        assert!(PhysicalQuantity { value: 1.0, unit: QuantityUnit::Pound }.quantity().unwrap()
            .to_unit(&QuantityUnit::Meter.unit()).is_err());
        assert!(PhysicalQuantity { value: 1.0, unit: QuantityUnit::Inch }.quantity().unwrap()
            .to_unit(&QuantityUnit::SquareDecimeter.unit()).is_err());
        // 符号必须与领域定义一致，避免 SDK 自造写法。
        // Symbols must match the domain definitions so the SDK does not invent its own spelling.
        for (unit, symbol) in [(QuantityUnit::Decimeter, "dm"), (QuantityUnit::Inch, "in"), (QuantityUnit::Foot, "ft"),
            (QuantityUnit::Yard, "yd"), (QuantityUnit::Tonne, "t"), (QuantityUnit::Pound, "lb"), (QuantityUnit::SquareDecimeter, "dm^2")] {
            assert_eq!(unit.unit().symbol(), symbol);
        }
    }
}
