//! 二维层生成领域 / 2D layer generation domain
//!
//! 映射 Kotlin `bpp2d-domain-layer-generation-context`：候选层生成、组合上下文与刀缝贯穿。
//! Maps the Kotlin `bpp2d-domain-layer-generation-context` submodule: candidate layer generation, the
//! composite context and kerf propagation.

pub(crate) mod catalog;
pub mod request;
pub mod service;

pub use catalog::LayerColumnKey;
pub use request::{
    Bpp2dLayerGenerationRequest, Bpp2dLayerGenerationResult, Bpp2dLayerGenerator,
    LayerGenerationDemandEntry, add_quantity, max_quantity, order_candidate_sheets,
    subtract_quantity,
};
pub use service::*;

use std::cmp::Ordering;

use ospf_rust_quantities::unit::concept::UnitTrait;
use ospf_rust_quantities::unit::physical_unit::CTUnit;

use crate::domain::item::model::Layer;

// ============================================================================
// LayerGenerationContext - 组合式层生成上下文 / Composite layer generation context
// ============================================================================

/// 组合式层生成上下文 / Composite layer generation context
///
/// 按板材类型展开、限制观察候选数并按评分排序；此 context 本身不去重。
/// Expands per sheet type, caps observed candidates and ranks by score; this context does not deduplicate.
#[derive(Debug, Clone, Default)]
pub struct LayerGenerationContext;

impl LayerGenerationContext {
    /// 原生数值边界的目录引用、旋转、刀缝及物理排布校验；不求解、不读取时钟、不去重输入。
    /// Native numeric-boundary validation of catalog references, rotation, kerf and physical layouts; does not solve, read clocks or deduplicate input.
    ///
    /// 需求数值及完整主模型仍须通过 LayerAssignmentContext::register_model 复验。
    /// Demand values and the complete master still require LayerAssignmentContext::register_model validation.
    pub fn validate_columns(
        &self,
        request: &Bpp2dLayerGenerationRequest<f64, ospf_rust_quantities::unit::derived::Meter>,
        demands: &[crate::domain::layer_assignment::Bpp2dAssignmentDemandEntry],
        layers: &[Layer<f64, ospf_rust_quantities::unit::derived::Meter>],
    ) -> ospf_rust_core::Result<Vec<LayerColumnKey>> {
        let catalog = catalog::ColumnCatalog::new(request)?;
        catalog.require_demands(demands)?;
        layers.iter().map(|layer| catalog.validate(layer)).collect()
    }

    /// 创建上下文 / Create the context
    pub fn new() -> Self {
        Self
    }

    /// 默认生成器集合 / Default generator set
    pub fn default_generators<V, U>() -> Vec<Box<dyn Bpp2dLayerGenerator<V, U>>>
    where
        V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
        U: UnitTrait + CTUnit + Clone,
    {
        vec![
            Box::new(BlockLocalLayerGenerator),
            Box::new(BlockGlobalLayerGenerator),
            Box::new(SpaceSplitLayerGenerator),
            Box::new(BlaLayerGenerator),
            Box::new(PatternLayerGenerator),
            Box::new(CircleGridLayerGenerator),
            Box::new(MixedShapeLayerGenerator),
            Box::new(HistoryLayerGenerator),
        ]
    }

    /// 生成层候选 / Generate layer candidates
    ///
    /// 多板材类型逐类型生成，列自带 `Layer.sheet`；未指定任何板材时保持单次直通，
    /// 使不依赖板材的历史层生成器仍可工作。
    /// Candidates are generated per sheet type and carry `Layer.sheet`; when no sheet is specified the
    /// request is passed through once so the historical generator keeps working.
    /// 保留旧的按名字筛选形状策略；显式扩展链请使用 generate_explicit。
    /// Retains legacy name-based shape selection; use generate_explicit for explicit extension chains.
    pub fn generate<V, U>(
        &self,
        request: &Bpp2dLayerGenerationRequest<V, U>,
        generators: &[Box<dyn Bpp2dLayerGenerator<V, U>>],
    ) -> Vec<Bpp2dLayerGenerationResult<V, U>>
    where
        V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
        U: UnitTrait + CTUnit + Clone,
    {
        let selected = select_generators(generators, request);
        generate_selected(request, selected.into_iter().map(|generator| generator.as_ref()).collect())
    }

    /// 严格使用调用方给出的生成器链，不按生成器名字推断形状能力。
    /// Honor the caller's explicit generator chain without inferring shape capabilities from generator names.
    ///
    /// 仍逐板材展开并限制观察候选数；正式原生路径会进一步校验所有观察排布。
    /// Still expands per sheet and caps observed candidates; the formal native path additionally validates every observed layout.
    pub fn generate_explicit<V, U>(
        &self,
        request: &Bpp2dLayerGenerationRequest<V, U>,
        generators: &[Box<dyn Bpp2dLayerGenerator<V, U>>],
    ) -> Vec<Bpp2dLayerGenerationResult<V, U>>
    where
        V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
        U: UnitTrait + CTUnit + Clone,
    {
        generate_selected(request, generators.iter().map(|generator| generator.as_ref()).collect())
    }
}

fn generate_selected<V, U>(
    request: &Bpp2dLayerGenerationRequest<V, U>,
    generators: Vec<&dyn Bpp2dLayerGenerator<V, U>>,
) -> Vec<Bpp2dLayerGenerationResult<V, U>>
where
    V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
    U: UnitTrait + CTUnit + Clone,
{
    if generators.is_empty() || request.max_candidates == 0 {
        return Vec::new();
    }
    let sheets = request.candidate_sheets();
    let sheet_requests: Vec<Bpp2dLayerGenerationRequest<V, U>> = if sheets.is_empty() {
        vec![request.clone()]
    } else {
        sheets
            .into_iter()
            .map(|sheet| Bpp2dLayerGenerationRequest {
                sheet: Some(sheet),
                ..request.clone()
            })
            .collect()
    };

    let mut all = Vec::new();
    for sheet_request in &sheet_requests {
        if all.len() >= request.max_candidates {
            break;
        }
        for generator in &generators {
            all.extend(generator.generate(sheet_request));
            if all.len() >= request.max_candidates {
                break;
            }
        }
    }
    all.truncate(request.max_candidates);
    rank_by_area(all)
}

/// 按请求的物品形状挑选生成器 / Pick generators by the requested item shapes
///
/// 纯矩形请求排除圆形专用生成器；纯圆形请求只保留圆网格与历史；混合请求保留全部。
/// Pure rectangle requests exclude the circle-only generators; pure circle requests keep only the circle
/// grid and history; mixed requests keep everything.
fn select_generators<'a, V, U>(
    generators: &'a [Box<dyn Bpp2dLayerGenerator<V, U>>],
    request: &Bpp2dLayerGenerationRequest<V, U>,
) -> Vec<&'a Box<dyn Bpp2dLayerGenerator<V, U>>>
where
    V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
    U: UnitTrait + CTUnit + Clone,
{
    let has_circle = request.has_circle_items();
    let has_rectangle = request.has_rectangle_items();
    generators
        .iter()
        .filter(|generator| {
            let name = generator.name();
            let is_circle_only = name == "circle-grid" || name == "mixed-shape";
            if !has_circle && has_rectangle {
                // 纯矩形请求排除圆形专用生成器。
                return !is_circle_only;
            }
            if has_circle && !has_rectangle {
                // 纯圆形请求只走圆网格与历史生成器。
                return name == "circle-grid" || name == "history";
            }
            true
        })
        .collect()
}

/// 按面积降序排序候选层 / Rank candidate layers by descending area
///
/// 无影子价格评分时面积是唯一可比较的通用口径。
/// Without a shadow-price score, area is the only universally comparable measure.
fn rank_by_area<V, U>(
    results: Vec<Bpp2dLayerGenerationResult<V, U>>,
) -> Vec<Bpp2dLayerGenerationResult<V, U>>
where
    V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
    U: UnitTrait + CTUnit + Clone,
{
    let mut results = results;
    results.sort_by(|lhs, rhs| {
        let left = lhs
            .score
            .as_ref()
            .map(|score| score.clone())
            .unwrap_or_else(|| V::zero());
        let right = rhs
            .score
            .as_ref()
            .map(|score| score.clone())
            .unwrap_or_else(|| V::zero());
        let left_value = if lhs.score.is_some() {
            left
        } else {
            lhs.layer.area().value.clone()
        };
        let right_value = if rhs.score.is_some() {
            right
        } else {
            rhs.layer.area().value.clone()
        };
        right_value
            .partial_cmp(&left_value)
            .unwrap_or(Ordering::Equal)
    });
    results
}

/// 去重后的候选层 / Deduplicated candidate layers
///
/// 按列身份键去重：板材类型、来源、包围尺寸与放置集合共同参与。
/// Deduplicates by the column identity key: sheet type, source, bounding size and placement set together.
pub fn deduplicate_layers<V, U>(layers: Vec<Layer<V, U>>) -> Vec<Layer<V, U>>
where
    V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
    U: UnitTrait + CTUnit + Clone,
{
    let mut seen = std::collections::HashSet::new();
    layers
        .into_iter()
        .filter(|layer| seen.insert(layer.identity_key()))
        .collect()
}
