use std::collections::BTreeSet;
use super::*;

struct SliceBounds {
    bottom: f64,
    top: f64,
    left: f64,
    right: f64,
}

pub(super) struct SliceSweep {
    bounds: Vec<SliceBounds>,
    starts: Vec<usize>,
    ends: Vec<usize>,
    next_start: usize,
    next_end: usize,
    active: BTreeSet<usize>,
}

impl SliceSweep {
    pub(super) fn new<V, U>(placements: &[Placement2<V, U>], width: f64) -> Option<Self>
    where
        V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
        U: UnitTrait + CTUnit + Clone,
    {
        let mut bounds = Vec::with_capacity(placements.len());
        for placement in placements {
            let bottom = placement.y().value.to_f64()?;
            let top = placement.max_y().ok()?.value.to_f64()?;
            let left = placement.x().value.to_f64()?;
            let right = placement.max_x().ok()?.value.to_f64()?;
            if ![bottom, top, left, right].iter().all(|value| value.is_finite()) || top < bottom || right < left {
                return None;
            }
            bounds.push(SliceBounds { bottom, top, left: left.clamp(0.0, width), right: right.clamp(0.0, width) });
        }
        // 稳定的横向秩同时保留相同左边界时的输入顺序。 / Stable horizontal ranks retain input order for equal left edges.
        bounds.sort_by(|lhs, rhs| lhs.left.partial_cmp(&rhs.left).unwrap_or(std::cmp::Ordering::Equal));
        let mut starts = (0..bounds.len()).collect::<Vec<_>>();
        starts.sort_by(|&lhs, &rhs| bounds[lhs].bottom.partial_cmp(&bounds[rhs].bottom).unwrap_or(std::cmp::Ordering::Equal));
        let mut ends = (0..bounds.len()).collect::<Vec<_>>();
        ends.sort_by(|&lhs, &rhs| bounds[lhs].top.partial_cmp(&bounds[rhs].top).unwrap_or(std::cmp::Ordering::Equal));
        Some(Self { bounds, starts, ends, next_start: 0, next_end: 0, active: BTreeSet::new() })
    }

    pub(super) fn write_used(
        &mut self,
        bottom: f64,
        top: f64,
        used: &mut Vec<(f64, f64)>,
    ) {
        while self.next_start < self.starts.len() {
            let index = self.starts[self.next_start];
            let bound = &self.bounds[index];
            if bound.bottom >= top - 1e-9 { break; }
            self.next_start += 1;
            // 窄片可能在进入活动集前就已过期。 / A thin interval may expire before it ever enters the active set.
            if bound.top > bottom + 1e-9 { self.active.insert(index); }
        }
        while self.next_end < self.ends.len() {
            let index = self.ends[self.next_end];
            if self.bounds[index].top > bottom + 1e-9 { break; }
            self.next_end += 1;
            self.active.remove(&index);
        }
        used.extend(self.active.iter().map(|&index| (self.bounds[index].left, self.bounds[index].right)));
    }
}
