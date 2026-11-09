//! 一维分切的可移植契约与原领域适配；不在 JavaScript 重写生成算法。
//! Portable cutting-stock contracts and existing-domain adapters, without reimplementing generation in JavaScript.

use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::Unit;
use ospf_rust_quantities::unit::derived::Kilogram;
use ospf_rust_quantities::unit::physical_unit::CTUnit;
use ospf_rust_core::model::MetaModel;
use ospf_rust_framework_csp1d as domain;
use crate::{BridgeError, ModelArtifact, PhysicalQuantity, QuantityUnit, Result, invalid};

const MAX_SAFE_COUNT: u64 = 9_007_199_254_740_991;
const MAX_PLANS: usize = 512;

fn count(text: &str) -> Result<u64> {
    let value = text.parse::<u64>().map_err(invalid)?;
    if value.to_string() != text || value > MAX_SAFE_COUNT { return Err(invalid("CSP1D counts require canonical decimal integers in [0, 2^53 - 1] for the current f64 domain")); }
    Ok(value)
}

fn identity(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 128 || !id.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte)) {
        return Err(invalid("CSP1D identities require 1..128 ASCII letters, digits, underscores, dots or hyphens"));
    }
    Ok(())
}

fn unique<'a>(ids: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for id in ids { identity(id)?; if !seen.insert(id) { return Err(invalid("duplicate CSP1D identity")); } }
    Ok(())
}

fn length(value: &PhysicalQuantity, unit: QuantityUnit) -> Result<Quantity<f64, Unit>> {
    if !matches!(unit, QuantityUnit::Meter | QuantityUnit::Kilometer | QuantityUnit::Centimeter | QuantityUnit::Millimeter) { return Err(invalid("CSP1D normalization unit must be a length unit")); }
    let quantity = value.quantity()?.to_unit(&unit.unit()).map_err(invalid)?;
    if !quantity.value.is_finite() || quantity.value < 0.0 { return Err(invalid("CSP1D normalized lengths must be finite and nonnegative")); }
    Ok(quantity)
}

/// 同单位闭区间，边界由 Rust 显式归一。 / Closed range explicitly normalized to one unit by Rust.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dWidthRange {
    /// 非负下界。 / Nonnegative lower bound.
    pub lower_bound: PhysicalQuantity,
    /// 正上界。 / Positive upper bound.
    pub upper_bound: PhysicalQuantity,
}

impl Csp1dWidthRange {
    fn domain(&self, unit: QuantityUnit) -> Result<domain::WidthRange<f64>> {
        let lower = length(&self.lower_bound, unit)?;
        let upper = length(&self.upper_bound, unit)?;
        if upper.value <= 0.0 { return Err(invalid("CSP1D upper width must be positive")); }
        let range = domain::QuantityRange::with_bounds(lower, upper.clone(), true, true).ok_or_else(|| invalid("invalid CSP1D width range"))?;
        domain::WidthRange::with_step(range, upper).ok_or_else(|| invalid("invalid CSP1D width-range unit"))
    }
}

/// 当前开放离散 Roll/Sheet 产品；其他物性尚未绑定。 / Currently exposed discrete Roll/Sheet product; other physical properties remain unbound.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dProduct {
    /// 稳定产品身份。 / Stable product identity.
    pub id: String,
    /// 显示名，最多 256 字节。 / Display name, at most 256 bytes.
    pub name: String,
    /// 最多八个可选幅宽。 / Up to eight candidate widths.
    pub width: Vec<PhysicalQuantity>,
    /// 可选产品长度；重量口径需求必需。 / Optional product length; required by weight-based demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub length: Option<PhysicalQuantity>,
    /// 可选单位重量；重量口径需求必需。 / Optional unit weight; required by weight-based demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unit_weight: Option<PhysicalQuantity>,
}

/// 配规输入：与产品同构的幅宽/长度/单位重量，但**不产生需求贡献**，只参与配规切片与物料消耗。
/// Costar input, structurally like a product but producing **no demand contribution**; it only participates in costar
/// slices and material consumption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dCostar {
    /// 稳定配规身份。 / Stable costar identity.
    pub id: String,
    /// 显示名，最多 256 字节。 / Display name, at most 256 bytes.
    pub name: String,
    /// 最多八个可选幅宽。 / Up to eight candidate widths.
    pub width: Vec<PhysicalQuantity>,
    /// 可选长度。 / Optional length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub length: Option<PhysicalQuantity>,
    /// 可选单位重量。 / Optional unit weight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unit_weight: Option<PhysicalQuantity>,
}

/// 当前默认物料规则的无损输入。 / Lossless input for current default material rules.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dMaterial {
    /// 稳定物料身份。 / Stable material identity.
    pub id: String,
    /// 显示名。 / Display name.
    pub name: String,
    /// 原领域幅宽范围。 / Existing-domain width range.
    pub width_range: Csp1dWidthRange,
    /// 可选绑定设备，必须存在。 / Optional bound machine, which must exist.
    pub machine_id: Option<String>,
    /// canonical 精确批次数；null 显式表示无限。 / Canonical exact batch count; null explicitly means unlimited.
    pub available_batches: Option<String>,
}

/// 设备幅宽与批次数；不接受尚未正确绑定的产能/换料参数。 / Machine width and batch count, without accepting unbound capacity/switch parameters.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dMachine {
    /// 稳定设备身份。 / Stable machine identity.
    pub id: String,
    /// 显示名。 / Display name.
    pub name: String,
    /// 可选设备幅宽范围。 / Optional machine width range.
    pub width_range: Option<Csp1dWidthRange>,
    /// canonical 精确批次上限；null 表示无限。 / Canonical exact batch limit; null means unlimited.
    pub max_batch_count: Option<String>,
}

/// 当前已复验的离散需求口径。 / Currently revalidated discrete demand basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Csp1dDemandMode {
    /// 卷数，使用原领域 RollCountUnit。 / Roll count using the domain RollCountUnit.
    Roll,
    /// 张数，使用原领域 SheetCountUnit。 / Sheet count using the domain SheetCountUnit.
    Sheet,
    /// 重量，使用原领域重量口径；产品必须同时声明长度与单位重量。
    /// Weight using the original weight basis; the product must declare both a length and a unit weight.
    Weight,
}

/// 单产品需求，不接受同产品重复需求被静默合并。 / Product demand, rejecting silently merged duplicate demands.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dDemand {
    /// 源产品身份。 / Source product identity.
    pub product_id: String,
    /// 离散单位口径。 / Discrete unit basis.
    pub mode: Csp1dDemandMode,
    /// 正 canonical 十进制文本。 / Positive canonical decimal text.
    pub count: String,
}

/// 有界的物理量问题；数值特化为现有 f64 领域，不宣称任意 u64 可精确求解。
/// Bounded quantity-valued problem specialized to the existing f64 domain, without claiming arbitrary-u64 exact solving.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dProblem {
    /// 最多 64 个产品。 / At most 64 products.
    pub products: Vec<Csp1dProduct>,
    /// 最多 16 种物料。 / At most 16 materials.
    pub materials: Vec<Csp1dMaterial>,
    /// 最多 16 个设备。 / At most 16 machines.
    pub machines: Vec<Csp1dMachine>,
    /// 最多 64 个不同产品的需求。 / At most 64 distinct-product demands.
    pub demands: Vec<Csp1dDemand>,
    /// 最多 64 个配规；不产生需求贡献。 / At most 64 costars; they never contribute to demand.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub costars: Vec<Csp1dCostar>,
    /// 显式长度归一单位。 / Explicit length normalization unit.
    pub length_unit: QuantityUnit,
}

impl Csp1dProblem {
    /// 用原领域模型承接输入，严格拒绝单位混用、引用失效和数值舍入。
    /// Adapt through original domain models, strictly rejecting incompatible units, invalid references and numeric rounding.
    pub fn domain(&self) -> Result<domain::Csp1dProblem<f64>> {
        if self.products.is_empty() || self.products.len() > 64 || self.materials.is_empty() || self.materials.len() > 16
            || self.machines.len() > 16 || self.demands.is_empty() || self.demands.len() > 64 { return Err(invalid("invalid CSP1D input shape or size")); }
        unique(self.products.iter().map(|value| value.id.as_str()))?;
        unique(self.materials.iter().map(|value| value.id.as_str()))?;
        unique(self.machines.iter().map(|value| value.id.as_str()))?;
        unique(self.demands.iter().map(|value| value.product_id.as_str()))?;
        let products = self.products.iter().map(|value| {
            if value.name.len() > 256 || value.width.is_empty() || value.width.len() > 8 { return Err(invalid("invalid CSP1D product widths/name")); }
            let width = value.width.iter().map(|width| {
                let width = length(width, self.length_unit)?;
                if width.value <= 0.0 { return Err(invalid("CSP1D product widths must be positive")); }
                Ok(width)
            }).collect::<Result<Vec<_>>>()?;
            if width.iter().enumerate().any(|(i, value)| width[..i].iter().any(|previous| previous.value == value.value)) { return Err(invalid("duplicate normalized CSP1D product width")); }
            // 长度与单位重量沿用原运行时量值转换；重量口径需求依赖它们。
            // Length and unit weight reuse the original runtime conversions; weight-based demand depends on them.
            let product_length = value.length.as_ref().map(|length_value| length(length_value, self.length_unit)).transpose()?;
            let unit_weight = value.unit_weight.as_ref().map(|weight| {
                let converted = weight.quantity()?.to_unit(&Kilogram::INSTANT).map_err(invalid)?;
                if !converted.value.is_finite() || converted.value < 0.0 { return Err(invalid("CSP1D unit weight must be finite and nonnegative")); }
                Ok(converted)
            }).transpose()?;
            Ok(domain::Product { id: value.id.clone().into(), name: value.name.clone(), width, length: product_length, unit_weight, weight: None,
                max_over_produce_length: None, dynamic_length: false })
        }).collect::<Result<Vec<_>>>()?;
        let machines = self.machines.iter().map(|value| {
            if value.name.len() > 256 { return Err(invalid("oversized CSP1D machine name")); }
            Ok(domain::Machine { id: value.id.clone().into(), name: value.name.clone(), width_range: value.width_range.as_ref().map(|range| range.domain(self.length_unit)).transpose()?,
                max_batch_count: value.max_batch_count.as_deref().map(count).transpose()?, max_switch_count: None, capacity: None })
        }).collect::<Result<Vec<_>>>()?;
        let materials = self.materials.iter().map(|value| {
            if value.name.len() > 256 || value.machine_id.as_ref().is_some_and(|id| !machines.iter().any(|machine| &machine.id == id)) { return Err(invalid("invalid CSP1D material name or machine reference")); }
            Ok(domain::Material { id: value.id.clone().into(), name: value.name.clone(), width_range: value.width_range.domain(self.length_unit)?,
                machine_id: value.machine_id.clone().map(Into::into), available_batches: value.available_batches.as_deref().map(count).transpose()?.unwrap_or(u64::MAX), length: None, unit_weight: None })
        }).collect::<Result<Vec<_>>>()?;
        let demands = self.demands.iter().map(|value| {
            let product = products.iter().find(|product| product.id == value.product_id).ok_or_else(|| invalid("unknown CSP1D demand product"))?.clone();
            let amount = count(&value.count)?;
            if amount == 0 { return Err(invalid("CSP1D demands must be positive")); }
            Ok(match value.mode {
                Csp1dDemandMode::Roll => domain::ProductDemand::legacy_roll(product, amount as f64),
                Csp1dDemandMode::Sheet => domain::ProductDemand::legacy_sheet(product, amount as f64),
                // 重量口径需要产品同时声明长度与单位重量，否则原领域无法换算；这里提前拒绝而不是让下游静默为 0。
                // The weight basis needs both a declared length and unit weight, otherwise the original domain cannot
                // convert; fail here instead of letting downstream silently produce zero.
                Csp1dDemandMode::Weight => {
                    if product.length.is_none() || product.unit_weight.is_none() {
                        return Err(invalid("CSP1D weight demand requires a product length and unit weight"));
                    }
                    match product.weight_for(product.width.first().ok_or_else(|| invalid("CSP1D product has no width"))?, product.length.as_ref()) {
                        Some(quantity) if quantity.value > 0.0 => domain::ProductDemand::weight(product, domain::Csp1dQuantity {
                            value: quantity.value * amount as f64, unit: quantity.unit }),
                        _ => return Err(invalid("CSP1D weight demand cannot be derived from the declared geometry")),
                    }
                }
            })
        }).collect::<Result<Vec<_>>>()?;
        // 配规只参与配规切片与物料消耗，不产生需求贡献；身份与产品共享命名空间以避免切片歧义。
        // Costars only participate in costar slices and material consumption and never contribute to demand; they share
        // the product identifier namespace so slice identities stay unambiguous.
        unique(self.costars.iter().map(|value| value.id.as_str()))?;
        if self.costars.len() > 64 { return Err(invalid("CSP1D accepts at most 64 costars")); }
        let costars = self.costars.iter().map(|value| {
            if value.name.len() > 256 || value.width.is_empty() || value.width.len() > 8 { return Err(invalid("invalid CSP1D costar widths/name")); }
            if products.iter().any(|product| product.id == value.id) { return Err(invalid("CSP1D costar identifiers must not shadow product identifiers")); }
            let width = value.width.iter().map(|width| {
                let width = length(width, self.length_unit)?;
                if width.value <= 0.0 { return Err(invalid("CSP1D costar widths must be positive")); }
                Ok(width)
            }).collect::<Result<Vec<_>>>()?;
            if width.iter().enumerate().any(|(i, value)| width[..i].iter().any(|previous| previous.value == value.value)) { return Err(invalid("duplicate normalized CSP1D costar width")); }
            let length_value = value.length.as_ref().map(|length_value| length(length_value, self.length_unit)).transpose()?;
            // 单位重量沿用原运行时量值：按千克归一，拒绝非有限、负值与跨量纲。
            // Unit weight keeps the original runtime quantity: normalized to kilograms, rejecting non-finite, negative
            // and cross-dimension values.
            let unit_weight = value.unit_weight.as_ref().map(|weight| {
                let converted = weight.quantity()?.to_unit(&Kilogram::INSTANT).map_err(invalid)?;
                if !converted.value.is_finite() || converted.value < 0.0 { return Err(invalid("CSP1D costar unit weight must be finite and nonnegative")); }
                Ok(converted)
            }).transpose()?;
            Ok(domain::Costar { id: value.id.clone().into(), name: value.name.clone(), width, length: length_value, unit_weight })
        }).collect::<Result<Vec<_>>>()?;
        let mut problem = domain::Csp1dProblem::new(products, materials, machines, demands);
        problem.costars = costars;
        Ok(problem)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProblemEnvelope { schema_version: String, problem: Csp1dProblem }

/// 编码 canonical 源问题；整数保持文本并使用领域分离摘要。 / Encode the canonical source problem, retaining integer text and a domain-separated digest.
pub fn encode_csp1d_problem(problem: Csp1dProblem) -> Result<ModelArtifact> {
    problem.domain()?;
    let model_json = serde_json::to_string(&ProblemEnvelope { schema_version: "1.0".into(), problem }).map_err(invalid)?;
    if model_json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "CSP1D problem exceeds 8 MiB")); }
    let fingerprint = ospf_rust_core::solver::sha256_fingerprint("ospf.csp1d.problem.v1", model_json.as_bytes()).value;
    Ok(ModelArtifact { model_json, fingerprint })
}

/// 复验 schema、canonical 字节、数值及问题身份。 / Revalidate schema, canonical bytes, values and problem identity.
pub fn decode_csp1d_problem(artifact: &ModelArtifact) -> Result<(Csp1dProblem, domain::Csp1dProblem<f64>)> {
    if artifact.model_json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "CSP1D problem exceeds 8 MiB")); }
    let envelope: ProblemEnvelope = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
    if envelope.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported CSP1D problem schema")); }
    let encoded = encode_csp1d_problem(envelope.problem.clone())?;
    if encoded.model_json != artifact.model_json || encoded.fingerprint != artifact.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "CSP1D canonical text or fingerprint mismatch")); }
    let domain = envelope.problem.domain()?;
    Ok((envelope.problem, domain))
}

/// 引用源产品的一组同幅宽切片。 / A group of same-width slices referencing a source product.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dSlice {
    /// 源产品身份；配规切片时为空。 / Source product identity; absent for costar slices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub product_id: Option<String>,
    /// 源配规身份；产品切片时为空。 / Source costar identity; absent for product slices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub costar_id: Option<String>,
    /// 声明幅宽之一（产品或配规）。 / One of the declared widths (product or costar).
    pub width: PhysicalQuantity,
    /// 正 canonical 份数，每方案最多 64 刀。 / Positive canonical multiplicity, at most 64 knives per plan.
    pub amount: String,
}

/// 候选方案，无隐式任意需求贡献或回调。 / Candidate plan, without arbitrary implicit demand contributions or callbacks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dPattern {
    /// 方案身份，独立于业务内容去重。 / Plan identity, separate from content deduplication.
    pub id: String,
    /// 源物料身份；设备由物料显式引用。 / Source material identity; machine comes from the material's explicit reference.
    pub material_id: String,
    /// 最多 64 组切片。 / At most 64 slice groups.
    pub slices: Vec<Csp1dSlice>,
}

/// 当前生成策略；两者均不代表穷举完整切割模式空间。 / Current generation strategies, neither implying exhaustive cutting-pattern search.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Csp1dGeneratorKind {
    /// 原 Simple 单产品单刀生成器。 / Original Simple single-product single-knife generator.
    Simple,
    /// 原 N-Same 同产品多刀生成器。 / Original N-Same same-product multi-knife generator.
    NSame,
}

/// 有界策略配置，由 Rust 解析，不将任意 TS 函数变成领域策略。 / Bounded strategy configuration parsed by Rust, not arbitrary TS-to-domain callbacks.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dGenerationOptions {
    /// 默认 N-Same。 / Defaults to N-Same.
    #[ts(optional)]
    pub generator: Option<Csp1dGeneratorKind>,
    /// 默认 128，最大 512。 / Defaults to 128, maximum 512.
    #[ts(optional)]
    pub max_plans: Option<u32>,
    /// 默认 16，最大 64；Simple 固定单刀。 / Defaults to 16, maximum 64; Simple always uses one knife.
    #[ts(optional)]
    pub max_knives: Option<u32>,
}

impl Default for Csp1dGenerationOptions { fn default() -> Self { Self { generator: None, max_plans: None, max_knives: None } } }

impl Csp1dGenerationOptions {
    /// 构造原领域生成器，先校验工作量上限。 / Construct an original-domain generator after checking work limits.
    pub fn generator(&self) -> Result<Box<dyn domain::Csp1dInitialCuttingPlanGenerator<f64>>> {
        let plans = self.max_plans.unwrap_or(128);
        let knives = self.max_knives.unwrap_or(16);
        if !(1..=512).contains(&plans) || !(1..=64).contains(&knives) { return Err(invalid("invalid CSP1D generation budgets")); }
        Ok(match self.generator.unwrap_or(Csp1dGeneratorKind::NSame) {
            Csp1dGeneratorKind::Simple => Box::new(domain::SimpleInitialCuttingPlanGenerator),
            Csp1dGeneratorKind::NSame => Box::new(domain::NSameGenerator::with_constraints(domain::GenerationConstraints {
                max_knife_count: Some(u64::from(knives)), ..Default::default()
            }).with_all_amount(true).with_max_plans(u64::from(plans))),
        })
    }
}

/// 生成结果；完整性不提升为源问题证明。 / Generation result, never promoted to a source-problem proof.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dPatterns {
    /// 原领域候选，保留顺序。 / Original-domain candidates, retaining order.
    pub plans: Vec<Csp1dPattern>,
    /// 因显式方案预算或领域停止原因截断。 / Truncated by the explicit plan budget or a domain stop reason.
    pub truncated: bool,
    /// 当前规则不声明全空间完备。 / These rules never declare full-space completeness.
    pub candidate_complete: bool,
    /// 原生成统计计数的精确文本。 / Exact text of original generation counters.
    pub visited_nodes: String,
    /// 实际生成候选数的精确文本。 / Exact text of actual generated candidates.
    pub generated_candidates: String,
}

fn generation_input(problem: &domain::Csp1dProblem<f64>) -> domain::CuttingPlanGenerationInput<f64> {
    // 配规必须进生成输入，否则 CostarFiller 永远拿不到它们；其余字段沿用领域默认值。
    // Costars must reach the generation input, otherwise `CostarFiller` never sees them; other fields keep the domain
    // defaults.
    domain::CuttingPlanGenerationInput { products: problem.products.clone(), materials: problem.materials.clone(), machines: problem.machines.clone(),
        demands: problem.demands.clone(), costars: problem.costars.clone(), ..Default::default() }
}

/// 把原领域方案编码为带单位的引用 DTO。 / Encode an existing-domain plan as a unit-bearing reference DTO.
pub fn encode_csp1d_pattern(plan: &domain::CuttingPlan<f64>, unit: QuantityUnit) -> Result<Csp1dPattern> {
    let slices = plan.slices.iter().map(|slice| {
        // 产品与配规切片都能无损编码；身份字段互斥，绝不两个都填。
        // Product and costar slices both encode losslessly; their identity fields are mutually exclusive.
        let (product_id, costar_id) = match &slice.production {
            domain::CuttingPlanProduction::Product(product) => (Some(product.id.to_string()), None),
            domain::CuttingPlanProduction::Costar(costar) => (None, Some(costar.id.to_string())),
        };
        let width = slice.width.to_unit(&unit.unit()).map_err(invalid)?;
        Ok(Csp1dSlice { product_id, costar_id, width: PhysicalQuantity { value: width.value, unit }, amount: slice.amount.to_string() })
    }).collect::<Result<_>>()?;
    Ok(Csp1dPattern { id: plan.id.to_string(), material_id: plan.material.id.to_string(), slices })
}

/// 委托 Simple/N-Same，真实调用路径不使用时钟、随机数、线程或文件系统。
/// Delegate to Simple/N-Same; actual call paths use no clocks, randomness, threads or filesystem.
pub fn generate_csp1d_patterns(problem: &Csp1dProblem, options: &Csp1dGenerationOptions) -> Result<Csp1dPatterns> {
    let domain = problem.domain()?;
    let report = options.generator()?.generate_with_report(&generation_input(&domain));
    let limit = options.max_plans.unwrap_or(128) as usize;
    let truncated = report.plans.len() > limit || report.statistics.stop_reason != domain::CuttingPlanGenerationStopReason::Exhausted;
    let plans = report.plans.iter().take(limit).map(|plan| encode_csp1d_pattern(plan, problem.length_unit)).collect::<Result<Vec<_>>>()?;
    for plan in &plans { decode_csp1d_pattern(&domain, plan, problem.length_unit)?; }
    Ok(Csp1dPatterns { plans, truncated, candidate_complete: false, visited_nodes: report.statistics.visited_nodes.to_string(),
        generated_candidates: report.statistics.generated_candidates.to_string() })
}

/// 以源实体重建方案，需求贡献只由原 Rust 公式计算，不能由外部伪造。
/// Rebuild plans from source entities; demand contributions come only from original Rust formulas, never external claims.
pub fn decode_csp1d_pattern(
    problem: &domain::Csp1dProblem<f64>,
    pattern: &Csp1dPattern,
    unit: QuantityUnit,
) -> Result<domain::CuttingPlan<f64>> {
    if pattern.id.is_empty() || pattern.id.len() > 512 || !pattern.id.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte)) {
        return Err(invalid("invalid CSP1D plan identity"));
    }
    if pattern.slices.is_empty() || pattern.slices.len() > 64 { return Err(invalid("invalid CSP1D plan shape")); }
    let material = problem.materials.iter().find(|material| material.id == pattern.material_id).ok_or_else(|| invalid("unknown CSP1D plan material"))?.clone();
    let mut slices = Vec::new();
    let mut contributions = Vec::new();
    let mut keys = BTreeSet::new();
    let mut knives = 0u64;
    for slice in &pattern.slices {
        // 产品切片贡献需求；配规切片**不贡献**任何需求，只占物料与刀数——这是原领域语义。
        // Product slices contribute demand; costar slices contribute none and only consume material and knives, which is
        // the original domain semantics.
        let (production, identity, declared) = match (&slice.product_id, &slice.costar_id) {
            (Some(id), None) => {
                let demand = problem.demands.iter().find(|demand| demand.product.id == *id).ok_or_else(|| invalid("unknown CSP1D slice demand"))?;
                (domain::CuttingPlanProduction::Product(demand.product.clone()), id.clone(), demand.product.width.clone())
            }
            (None, Some(id)) => {
                let costar = problem.costars.iter().find(|costar| costar.id == *id).ok_or_else(|| invalid("unknown CSP1D slice costar"))?;
                (domain::CuttingPlanProduction::Costar(costar.clone()), id.clone(), costar.width.clone())
            }
            _ => return Err(invalid("CSP1D slice must name exactly one of productId or costarId")),
        };
        let width = length(&slice.width, unit)?;
        let amount = count(&slice.amount)?;
        knives = knives.checked_add(amount).ok_or_else(|| invalid("CSP1D knife count overflow"))?;
        if amount == 0 || knives > 64 || !declared.iter().any(|source| source.value == width.value && source.unit == width.unit)
            || !keys.insert((identity, width.value.to_bits())) { return Err(invalid("invalid or duplicate CSP1D plan slice")); }
        if let domain::CuttingPlanProduction::Product(product) = &production {
            let demand = problem.demands.iter().find(|demand| demand.product.id == product.id).ok_or_else(|| invalid("unknown CSP1D slice demand"))?;
            contributions.push(domain::CuttingPlanDemandContribution::from_demand(demand, &width, amount, None));
        }
        slices.push(domain::CuttingPlanSlice { production, width, amount });
    }
    let plan = domain::CuttingPlan { id: pattern.id.clone().into(), machine_id: material.machine_id.clone(), material, slices, demand_contributions: contributions, capacity_consumption: None };
    let used = plan.used_width().ok_or_else(|| invalid("CSP1D plan has no width"))?;
    if !used.value.is_finite() || used.value <= 0.0 || !plan.material.enabled(&plan, &problem.machines) { return Err(invalid("infeasible CSP1D plan width/machine")); }
    Ok(plan)
}

/// 方案与精确整数使用次数。 / Plan and exact integer usage count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dUsage {
    /// 独立拥有的完整方案。 / Independently owned complete plan.
    pub plan: Csp1dPattern,
    /// 正 canonical 使用次数。 / Positive canonical usage count.
    pub count: String,
}

/// 通过原 context、源约束及精确计数共同复验的离散生产结果。
/// Discrete production revalidated by the original context, source constraints and exact counts together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp1dProduction {
    /// 规范化的方案使用量。 / Normalized plan usages.
    pub usages: Vec<Csp1dUsage>,
    /// 精确批次数。 / Exact batch count.
    pub batch_count: String,
    /// 按源产品口径的精确产量。 / Exact output in each source product's demand basis.
    pub produced: BTreeMap<String, String>,
    /// 物料精确批次数。 / Exact material batches.
    pub material_batches: BTreeMap<String, String>,
    /// 设备精确批次数。 / Exact machine batches.
    pub machine_batches: BTreeMap<String, String>,
    /// 原领域计算的总用宽。 / Total used width computed by the original domain.
    pub used_width: PhysicalQuantity,
    /// 原领域计算的总余宽。 / Total trim width computed by the original domain.
    pub trim_width: PhysicalQuantity,
}

fn add_count(values: &mut BTreeMap<String, u64>, id: &str, value: u64) -> Result<()> {
    let entry = values.entry(id.to_owned()).or_default();
    *entry = entry.checked_add(value).ok_or_else(|| invalid("CSP1D exact aggregate count overflow"))?;
    Ok(())
}

/// 复用领域 context 注册和完整源向量校验；整数合计独立检查，不进行逐件展开。
/// Reuse domain-context registration and complete source-vector validation; independently check exact totals without piece expansion.
pub fn verify_csp1d_usages(problem: &Csp1dProblem, usages: &[Csp1dUsage]) -> Result<Csp1dProduction> {
    use domain::Csp1dModelContext;
    if usages.is_empty() || usages.len() > MAX_PLANS { return Err(invalid("invalid CSP1D usage count")); }
    let ids = usages.iter().map(|usage| &usage.plan.id).collect::<BTreeSet<_>>();
    if ids.len() != usages.len() { return Err(invalid("duplicate CSP1D usage identity")); }
    let source = problem.domain()?;
    let mut keys = BTreeSet::new();
    let mut selected = Vec::new();
    let (mut produced, mut material_batches, mut machine_batches) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let (mut batches, mut used_width, mut trim_width) = (0u64, 0.0, 0.0);
    for usage in usages {
        let plan = decode_csp1d_pattern(&source, &usage.plan, problem.length_unit)?;
        let amount = count(&usage.count)?;
        if amount == 0 || !keys.insert(plan.canonical_key()) { return Err(invalid("zero or duplicate CSP1D usage")); }
        batches = batches.checked_add(amount).ok_or_else(|| invalid("CSP1D batch total overflow"))?;
        add_count(&mut material_batches, &plan.material.id, amount)?;
        if let Some(machine) = &plan.machine_id { add_count(&mut machine_batches, machine, amount)?; }
        for slice in &plan.slices {
            let pieces = slice.amount.checked_mul(amount).ok_or_else(|| invalid("CSP1D production overflow"))?;
            add_count(&mut produced, slice.production.id(), pieces)?;
        }
        used_width += plan.used_width().ok_or_else(|| invalid("missing CSP1D used width"))?.value * amount as f64;
        trim_width += plan.rest_width().ok_or_else(|| invalid("missing CSP1D trim width"))?.value * amount as f64;
        selected.push(domain::CuttingPlanUsage { plan, amount });
    }
    if !used_width.is_finite() || !trim_width.is_finite() { return Err(invalid("CSP1D width total overflow")); }
    for demand in &problem.demands {
        if produced.get(&demand.product_id).copied().unwrap_or_default() < count(&demand.count)? { return Err(invalid("CSP1D exact demand is unmet")); }
    }
    for material in &problem.materials {
        if let Some(available) = &material.available_batches {
            if material_batches.get(&material.id).copied().unwrap_or_default() > count(available)? { return Err(invalid("CSP1D exact material stock exceeded")); }
        }
    }
    for machine in &problem.machines {
        if let Some(available) = &machine.max_batch_count {
            if machine_batches.get(&machine.id).copied().unwrap_or_default() > count(available)? { return Err(invalid("CSP1D exact machine batch limit exceeded")); }
        }
    }
    let input = domain::ProduceInput { cutting_plans: selected.iter().map(|usage| usage.plan.clone()).collect(), demands: source.demands,
        materials: source.materials, machines: source.machines, warm_start_plan_usages: Vec::new() };
    let mut builder = domain::Csp1dProduceContextBuilder::new(input);
    builder.mode(domain::Csp1dModelingMode::MILP);
    let mut context = builder.build().map_err(invalid)?;
    let mut model = MetaModel::new("csp1d_verify");
    context.register(&mut model).map_err(invalid)?;
    let values = selected.iter().enumerate().map(|(index, usage)| {
        let token = context.produce.plan_variable_index(index).and_then(|index| model.tokens().get(index)).ok_or_else(|| invalid("CSP1D plan lost its source variable"))?;
        Ok((token.id(), usage.amount as f64))
    }).collect::<Result<BTreeMap<_, _>>>()?;
    model.linear_model_snapshot().map_err(invalid)?.validate_solution(&values, &1e-7).map_err(invalid)?;
    let normalized = selected.iter().map(|usage| Ok(Csp1dUsage { plan: encode_csp1d_pattern(&usage.plan, problem.length_unit)?, count: usage.amount.to_string() })).collect::<Result<_>>()?;
    let strings = |values: BTreeMap<String, u64>| values.into_iter().map(|(id, count)| (id, count.to_string())).collect();
    Ok(Csp1dProduction { usages: normalized, batch_count: batches.to_string(), produced: strings(produced), material_batches: strings(material_batches),
        machine_batches: strings(machine_batches), used_width: PhysicalQuantity { value: used_width, unit: problem.length_unit },
        trim_width: PhysicalQuantity { value: trim_width, unit: problem.length_unit } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::Csp1dInitialCuttingPlanGenerator;

    fn problem() -> Csp1dProblem {
        serde_json::from_str(r#"{"products":[{"id":"p","name":"product","width":[{"value":30,"unit":"mm"}]}],"materials":[{"id":"m","name":"material","widthRange":{"lowerBound":{"value":0,"unit":"m"},"upperBound":{"value":10,"unit":"cm"}},"machineId":"mc","availableBatches":"3"}],"machines":[{"id":"mc","name":"machine","widthRange":null,"maxBatchCount":"3"}],"demands":[{"productId":"p","mode":"roll","count":"5"}],"lengthUnit":"cm"}"#).unwrap()
    }

    /// 配规参与切片与物料消耗，但**不产生需求贡献**；身份字段互斥，未知名必须拒绝。
    /// Costars take part in slices and material consumption but produce **no demand contribution**; identity fields are
    /// mutually exclusive and unknown names must be rejected.
    #[test]
    fn costars_enter_the_plan_without_contributing_demand() {
        let mut source = problem();
        source.costars = vec![Csp1dCostar { id: "c".into(), name: "costar".into(),
            width: vec![PhysicalQuantity { value: 2.0, unit: QuantityUnit::Centimeter }], length: None, unit_weight: None }];
        let domain = source.domain().unwrap();
        assert_eq!(domain.costars.len(), 1);
        assert_eq!(domain.costars[0].width.len(), 1);
        // 配规切片编码后回填，需求贡献保持为空。 / A costar slice round-trips with no demand contribution.
        // 复用既有可行方案（30 mm 产品 × 3 刀，物料范围 0..10 cm），只把其中一刀换成配规。
        // Reuse the existing feasible plan (30 mm product × 3 knives within the 0..10 cm material range) and replace one
        // knife group with the costar.
        let usage = Csp1dUsage { plan: Csp1dPattern { id: "plan".into(), material_id: "m".into(), slices: vec![
            Csp1dSlice { product_id: None, costar_id: Some("c".into()), width: PhysicalQuantity { value: 2.0, unit: QuantityUnit::Centimeter }, amount: "1".into() },
            Csp1dSlice { product_id: Some("p".into()), costar_id: None, width: PhysicalQuantity { value: 30.0, unit: QuantityUnit::Millimeter }, amount: "2".into() },
        ] }, count: "2".into() };
        let plan = decode_csp1d_pattern(&domain, &usage.plan, QuantityUnit::Centimeter).unwrap();
        assert_eq!(plan.demand_contributions.len(), 1, "only the product slice contributes demand");
        assert!(matches!(plan.slices[0].production, domain::CuttingPlanProduction::Costar(_)));
        // 产品身份复用配规名必须拒绝；两侧都填或都空也必须拒绝。
        // Shadowing a product id must fail, and so must naming both or neither identity.
        source.costars[0].id = "p".into();
        assert!(source.domain().is_err());
        let mut both = usage.clone(); both.plan.slices[0].product_id = Some("c".into());
        assert!(decode_csp1d_pattern(&domain, &both.plan, QuantityUnit::Centimeter).is_err());
        let mut neither = usage.clone(); neither.plan.slices[0].costar_id = None;
        assert!(decode_csp1d_pattern(&domain, &neither.plan, QuantityUnit::Centimeter).is_err());
        let mut unknown = usage.clone(); unknown.plan.slices[0].costar_id = Some("ghost".into());
        assert!(decode_csp1d_pattern(&domain, &unknown.plan, QuantityUnit::Centimeter).is_err());
    }

    /// 重量口径需求由产品几何换算；缺长度或单位重量必须在边界拒绝，不能静默成 0。
    /// Weight-based demand converts from the product geometry; missing length or unit weight must fail at the boundary
    /// instead of silently becoming zero.
    #[test]
    fn weight_demand_requires_declared_geometry_and_computes_real_quantities() {
        let mut source = problem();
        source.length_unit = QuantityUnit::Centimeter;
        source.products[0].length = Some(PhysicalQuantity { value: 100.0, unit: QuantityUnit::Centimeter });
        source.products[0].unit_weight = Some(PhysicalQuantity { value: 500.0, unit: QuantityUnit::Gram });
        source.demands[0].mode = Csp1dDemandMode::Weight;
        let domain = source.domain().unwrap();
        // 30 mm 宽 × 100 cm 长 × 500 g/m² 类比换算：重量口径必须为正且有限。
        // The converted weight must be positive and finite for the declared 30 mm width, 100 cm length and 500 g unit
        // weight.
        assert_eq!(domain.demands[0].mode, Some(domain::DemandMode::Weight));
        assert!(domain.demands[0].quantity.value > 0.0 && domain.demands[0].quantity.value.is_finite());
        // 缺长度或缺单位重量都必须拒绝。 / Missing length or unit weight must fail.
        let mut missing_length = source.clone();
        missing_length.products[0].length = None;
        assert_eq!(missing_length.domain().unwrap_err().code, "InvalidInput");
        let mut missing_weight = source.clone();
        missing_weight.products[0].unit_weight = None;
        assert_eq!(missing_weight.domain().unwrap_err().code, "InvalidInput");
        // 负单位重量同样拒绝。 / A negative unit weight is rejected too.
        let mut negative = source.clone();
        negative.products[0].unit_weight = Some(PhysicalQuantity { value: -1.0, unit: QuantityUnit::Gram });
        assert_eq!(negative.domain().unwrap_err().code, "InvalidInput");
        // 卷/张口径不需要几何声明。 / Roll and sheet bases need no geometry.
        let mut catalog = problem();
        catalog.demands[0].mode = Csp1dDemandMode::Roll;
        assert!(catalog.domain().is_ok());
    }

    fn usage() -> Csp1dUsage {
        Csp1dUsage { plan: Csp1dPattern { id: "plan".into(), material_id: "m".into(), slices: vec![Csp1dSlice {
            product_id: Some("p".into()), costar_id: None, width: PhysicalQuantity { value: 0.03, unit: QuantityUnit::Meter }, amount: "3".into(),
        }] }, count: "2".into() }
    }

    #[test]
    fn canonical_problem_normalizes_real_units_and_preserves_discrete_demand_bases() {
        let artifact = encode_csp1d_problem(problem()).unwrap();
        let (dto, source) = decode_csp1d_problem(&artifact).unwrap();
        assert_eq!(source.products[0].width[0].value, 3.0);
        assert_eq!(source.materials[0].width_range.upper_bound.value, 10.0);
        assert_eq!(source.demands[0].quantity.unit, domain::roll_count_unit());
        let mut sheet = dto; sheet.demands[0].mode = Csp1dDemandMode::Sheet;
        assert_eq!(sheet.domain().unwrap().demands[0].quantity.unit, domain::sheet_count_unit());
        let mut foreign = artifact.clone(); foreign.fingerprint = "foreign".into();
        assert_eq!(decode_csp1d_problem(&foreign).unwrap_err().code, "FingerprintMismatch");
        foreign = artifact.clone(); foreign.model_json.push(' ');
        assert_eq!(decode_csp1d_problem(&foreign).unwrap_err().code, "FingerprintMismatch");
        foreign.model_json = artifact.model_json.replace("\"schemaVersion\":\"1.0\"", "\"schemaVersion\":\"2.0\"");
        assert_eq!(decode_csp1d_problem(&foreign).unwrap_err().code, "UnsupportedProtocolVersion");
    }

    #[test]
    fn portable_generation_is_identical_to_bounded_original_n_same_not_global_search() {
        let problem = problem();
        let result = generate_csp1d_patterns(&problem, &Default::default()).unwrap();
        let source = problem.domain().unwrap();
        let original = domain::NSameGenerator::with_constraints(domain::GenerationConstraints { max_knife_count: Some(16), ..Default::default() })
            .with_all_amount(true).with_max_plans(128).generate_with_report(&generation_input(&source));
        assert_eq!(result.plans, original.plans.iter().map(|plan| encode_csp1d_pattern(plan, problem.length_unit).unwrap()).collect::<Vec<_>>());
        assert_eq!(result.plans.len(), 3); assert!(!result.candidate_complete); assert!(!result.truncated);
        assert!(generate_csp1d_patterns(&problem, &Csp1dGenerationOptions { max_plans: Some(1), ..Default::default() }).unwrap().truncated);
        let simple = generate_csp1d_patterns(&problem, &Csp1dGenerationOptions { generator: Some(Csp1dGeneratorKind::Simple), ..Default::default() }).unwrap();
        assert_eq!(simple.plans[0].slices[0].amount, "1");
        for options in [Csp1dGenerationOptions { max_knives: Some(65), ..Default::default() },
            Csp1dGenerationOptions { max_plans: Some(0), ..Default::default() }, Csp1dGenerationOptions { max_plans: Some(513), ..Default::default() }] {
            assert!(generate_csp1d_patterns(&problem, &options).is_err());
        }
    }

    #[test]
    fn original_context_and_exact_counts_revalidate_normalized_production_without_expansion() {
        let result = verify_csp1d_usages(&problem(), &[usage()]).unwrap();
        assert_eq!(result.batch_count, "2"); assert_eq!(result.produced["p"], "6");
        assert_eq!(result.material_batches["m"], "2"); assert_eq!(result.machine_batches["mc"], "2");
        assert_eq!(result.used_width.value, 18.0); assert_eq!(result.trim_width.value, 2.0);
        assert_eq!(result.usages[0].plan.slices[0].width, PhysicalQuantity { value: 3.0, unit: QuantityUnit::Centimeter });
        assert_eq!(verify_csp1d_usages(&problem(), &result.usages).unwrap(), result);
        let mut large = problem(); large.materials[0].available_batches = None; large.machines[0].max_batch_count = None;
        let mut exact = usage(); exact.count = MAX_SAFE_COUNT.to_string();
        assert_eq!(verify_csp1d_usages(&large, &[exact]).unwrap().produced["p"], "27021597764222973");
    }

    #[test]
    fn units_counts_shape_unknown_fields_and_references_fail_explicitly() {
        for text in ["01", "+1", "-1", "1.0", "9007199254740992", "18446744073709551615"] {
            let mut invalid = problem(); invalid.demands[0].count = text.into(); assert!(invalid.domain().is_err(), "{text}");
        }
        for variant in 0..9 {
            let mut invalid = problem();
            match variant {
                0 => invalid.products[0].width[0].unit = QuantityUnit::Kilogram,
                1 => invalid.length_unit = QuantityUnit::Second,
                2 => invalid.products[0].width[0].value = f64::INFINITY,
                3 => invalid.products[0].width[0].value = 0.0,
                4 => invalid.materials[0].machine_id = Some("missing".into()),
                5 => invalid.demands[0].product_id = "missing".into(),
                6 => invalid.products.push(invalid.products[0].clone()),
                7 => invalid.materials[0].width_range.lower_bound.value = 100.0,
                _ => invalid.products[0].id = "canonical|collision".into(),
            }
            assert!(encode_csp1d_problem(invalid).is_err(), "variant {variant}");
        }
        let mut json = serde_json::to_value(problem()).unwrap(); json["weight"] = 1.into();
        assert!(serde_json::from_value::<Csp1dProblem>(json).is_err());
    }

    #[test]
    fn exact_verification_rejects_one_piece_shortfall_and_u64_aggregate_overflow() {
        let mut problem = problem();
        problem.materials[0].available_batches = None;
        problem.machines[0].max_batch_count = None;
        problem.demands[0].count = MAX_SAFE_COUNT.to_string();
        let mut short = usage();
        short.plan.slices[0].amount = "1".into();
        short.count = (MAX_SAFE_COUNT - 1).to_string();
        assert!(verify_csp1d_usages(&problem, &[short]).unwrap_err().message.contains("exact demand is unmet"));

        problem.demands[0].count = "1".into();
        problem.products[0].width = (1..=8).map(|index| PhysicalQuantity { value: index as f64 / 10.0, unit: QuantityUnit::Millimeter }).collect();
        let material = problem.materials[0].clone();
        problem.materials = (0..5).map(|index| Csp1dMaterial { id: format!("m{index}"), ..material.clone() }).collect();
        let usages = (0..33).map(|index| Csp1dUsage {
            plan: Csp1dPattern { id: format!("plan-{index}"), material_id: format!("m{}", index / 8),
                slices: vec![Csp1dSlice { product_id: Some("p".into()), costar_id: None, width: problem.products[0].width[index % 8].clone(), amount: "64".into() }] },
            count: MAX_SAFE_COUNT.to_string(),
        }).collect::<Vec<_>>();
        assert!(verify_csp1d_usages(&problem, &usages).unwrap_err().message.contains("aggregate count overflow"));
    }

    #[test]
    fn demand_stock_machine_width_duplicate_and_foreign_usages_are_never_silently_repaired() {
        assert!(verify_csp1d_usages(&problem(), &[]).is_err());
        assert!(verify_csp1d_usages(&problem(), &[usage(), usage()]).is_err());
        for variant in 0..9 {
            let mut invalid = usage();
            match variant {
                0 => invalid.count = "1".into(),
                1 => invalid.count = "4".into(),
                2 => invalid.plan.material_id = "foreign".into(),
                3 => invalid.plan.slices[0].product_id = Some("foreign".into()),
                4 => invalid.plan.slices[0].amount = "4".into(),
                5 => invalid.plan.slices[0].width.value = 0.04,
                6 => invalid.plan.slices[0].amount = "0".into(),
                7 => invalid.plan.slices.push(invalid.plan.slices[0].clone()),
                _ => invalid.plan.slices[0].width.unit = QuantityUnit::Second,
            }
            assert!(verify_csp1d_usages(&problem(), &[invalid]).is_err(), "variant {variant}");
        }
        let mut machine = problem(); machine.machines[0].max_batch_count = Some("1".into());
        assert!(verify_csp1d_usages(&machine, &[usage()]).is_err());
        machine.machines[0].max_batch_count = None;
        machine.machines[0].width_range = Some(Csp1dWidthRange { lower_bound: PhysicalQuantity { value: 0.0, unit: QuantityUnit::Centimeter },
            upper_bound: PhysicalQuantity { value: 9.0, unit: QuantityUnit::Centimeter } });
        assert!(verify_csp1d_usages(&machine, &[usage()]).is_err());
    }
}
