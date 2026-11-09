//! 平坦后序切分树，先限制节点／深度并拒绝共享、循环及游离节点，再构造原领域树。 / Flat postorder cut trees: bound nodes/depth and reject sharing/cycles/orphans before constructing original-domain trees.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_quantities::quantity::Quantity;
use ospf_rust_quantities::unit::derived::Meter;
use ospf_rust_framework_csp2d::infrastructure::{CutLine2, CutOrientation2, CutTree2};
use crate::{Geometry2d, PhysicalQuantity, QuantityUnit, Result, invalid};
use super::{Csp2dRegion, identifier};

/// 原区域切分方向。 / Original region cut orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Csp2dCutOrientation {
    /// 竖刀。 / Vertical cut.
    Vertical,
    /// 横刀。 / Horizontal cut.
    Horizontal,
}

/// 后序节点，所有子索引必须小于当前索引且仅被使用一次。 / Postorder node: every child index must precede this node and be used exactly once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum Csp2dCutNode {
    /// 成品或余料叶子。 / Finished-piece or remnant leaf.
    Leaf {
        /// 实际叶子区域。 / Actual leaf region.
        region: Csp2dRegion,
        /// 源产品身份，余料为 null。 / Source product identity, or null for a remnant.
        product_id: Option<String>,
    },
    /// 贯穿刀，刀缝沿用源问题。 / Through cut, reusing the source-problem kerf.
    Cut {
        /// 当前完整区域。 / Current complete region.
        region: Csp2dRegion,
        /// 刀向。 / Cut orientation.
        orientation: Csp2dCutOrientation,
        /// 绝对刀缝起始位置，单位必须是长度。 / Absolute kerf start position, requiring a length unit.
        position: PhysicalQuantity,
        /// 原工艺级数，随刀向变化。 / Original process stage, following orientation changes.
        stage: u32,
        /// 左／下子节点索引。 / Left/bottom child index.
        lower: u32,
        /// 右／上子节点索引。 / Right/top child index.
        upper: u32,
    },
}

/// 根节点为最后一项的平坦后序树，最多 4096 节点、128 深度。 / Flat postorder tree rooted at its last entry, with at most 4096 nodes and depth 128.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Csp2dCutTree {
    /// 不允许循环、共享子树或游离节点。 / No cycles, shared subtrees or orphan nodes.
    pub nodes: Vec<Csp2dCutNode>,
}

impl Csp2dCutTree {
    pub(super) fn from_domain(root: &CutTree2<f64, Meter>) -> Result<Self> {
        let mut nodes = Vec::new();
        let mut indices = Vec::new();
        let mut stack = vec![(root, false, 0usize)];
        let mut visited = 0;
        while let Some((tree, complete, depth)) = stack.pop() {
            if !complete {
                visited += 1;
                if depth > 128 || visited > 4096 { return Err(invalid("CSP2D cut tree exceeds node/depth limits")); }
                if let CutTree2::Cut(node) = tree {
                    stack.push((tree, true, depth)); stack.push((&node.upper, false, depth + 1)); stack.push((&node.lower, false, depth + 1));
                    continue;
                }
            }
            let node = match tree {
                CutTree2::Leaf(leaf) => Csp2dCutNode::Leaf { region: Csp2dRegion::from_domain(&leaf.region), product_id: leaf.part_id.clone() },
                CutTree2::Cut(node) => {
                    let upper = indices.pop().ok_or_else(|| invalid("CSP2D tree encoder lost its upper child"))?;
                    let lower = indices.pop().ok_or_else(|| invalid("CSP2D tree encoder lost its lower child"))?;
                    Csp2dCutNode::Cut { region: Csp2dRegion::from_domain(&node.region),
                        orientation: match node.line.orientation { CutOrientation2::Vertical => Csp2dCutOrientation::Vertical, CutOrientation2::Horizontal => Csp2dCutOrientation::Horizontal },
                        position: PhysicalQuantity { value: node.line.position.value, unit: QuantityUnit::Meter }, stage: u32::try_from(node.line.stage).map_err(invalid)?, lower, upper }
                },
            };
            indices.push(nodes.len() as u32); nodes.push(node);
        }
        Ok(Self { nodes })
    }

    pub(super) fn domain(&self, geometry: &Geometry2d, kerf: &Quantity<f64, Meter>) -> Result<CutTree2<f64, Meter>> {
        if self.nodes.is_empty() || self.nodes.len() > 4096 { return Err(invalid("CSP2D cut tree requires 1..4096 flat nodes")); }
        let mut built: Vec<Option<CutTree2<f64, Meter>>> = Vec::with_capacity(self.nodes.len());
        let mut depths: Vec<usize> = Vec::with_capacity(self.nodes.len());
        for (index, node) in self.nodes.iter().enumerate() {
            let (tree, depth) = match node {
                Csp2dCutNode::Leaf { region, product_id } => {
                    if let Some(id) = product_id { identifier(id)?; }
                    (CutTree2::leaf(region.domain(geometry)?, product_id.clone()), 0)
                },
                Csp2dCutNode::Cut { region, orientation, position, stage, lower, upper } => {
                    let (lower, upper) = (*lower as usize, *upper as usize);
                    if lower >= index || upper >= index || lower == upper { return Err(invalid("CSP2D cut-tree children must be distinct prior postorder nodes")); }
                    let depth = depths[lower].max(depths[upper]) + 1;
                    if depth > 128 { return Err(invalid("CSP2D cut tree exceeds depth 128")); }
                    let lower = built[lower].take().ok_or_else(|| invalid("CSP2D cut tree reuses a child node"))?;
                    let upper = built[upper].take().ok_or_else(|| invalid("CSP2D cut tree reuses a child node"))?;
                    let line = CutLine2 { orientation: match orientation { Csp2dCutOrientation::Vertical => CutOrientation2::Vertical, Csp2dCutOrientation::Horizontal => CutOrientation2::Horizontal },
                        position: geometry.length(position, false)?, stage: *stage as usize };
                    (CutTree2::cut(line, region.domain(geometry)?, kerf.clone(), lower, upper).map_err(invalid)?, depth)
                },
            };
            built.push(Some(tree)); depths.push(depth);
        }
        let root = built.pop().flatten().ok_or_else(|| invalid("CSP2D cut tree lacks a root"))?;
        if built.iter().any(Option::is_some) { return Err(invalid("CSP2D cut tree contains orphan nodes")); }
        Ok(root)
    }
}
