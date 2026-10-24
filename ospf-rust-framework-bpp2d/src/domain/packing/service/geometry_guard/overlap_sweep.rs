use std::collections::BTreeSet;
use super::*;

struct Bounds<V> {
    left: V,
    right: V,
    bottom: V,
    top: V,
}

impl PackingGeometryGuard {
    pub(super) fn first_overlap<V, U>(
        placements: &[Placement2<V, U>],
        kerf: Option<&Quantity<V, U>>,
    ) -> Option<(usize, usize)>
    where
        V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
        U: UnitTrait + CTUnit + Clone,
    {
        if placements.len() < 32 { return Self::first_overlap_pairs(placements, kerf); }
        const PREFIX_END: usize = 9;
        // 常见的首部冲突保留原来的短路，不为错误输入先建完整索引。 / Preserve short-circuiting for common prefix conflicts before building the full index.
        for rhs in 1..placements.len().min(PREFIX_END) {
            if Self::overlapped_with_kerf(&placements[0], &placements[rhs], kerf) { return Some((0, rhs)); }
        }
        let inflation = kerf.filter(|value| quantity_is_positive(value));
        let bounds = placements.iter().map(|placement| {
            let base = Self::box_of(placement, None)?;
            let expanded = inflation.and_then(|value| Self::box_of(placement, Some(value)));
            let expanded = expanded.as_ref().unwrap_or(&base);
            let left = base.x.value;
            let bottom = base.y.value;
            let base_right = base.max_x().ok()?.value;
            let base_top = base.max_y().ok()?.value;
            let expanded_right = expanded.max_x().ok()?.value;
            let expanded_top = expanded.max_y().ok()?.value;
            if ![left, bottom, base_right, base_top, expanded_right, expanded_top].iter().all(|value| value.is_finite()) {
                return None;
            }
            let right = base_right.max(expanded_right);
            let top = base_top.max(expanded_top);
            if right < left || top < bottom { return None; }
            Some(Bounds { left, right, bottom, top })
        }).collect::<Option<Vec<_>>>();
        let Some(bounds) = bounds else { return Self::first_overlap_pairs(placements, kerf); };
        let mut starts = (0..bounds.len()).collect::<Vec<_>>();
        starts.sort_by(|&lhs, &rhs| bounds[lhs].left.partial_cmp(&bounds[rhs].left).unwrap_or(std::cmp::Ordering::Equal));
        let mut ends = (0..bounds.len()).collect::<Vec<_>>();
        ends.sort_by(|&lhs, &rhs| bounds[lhs].right.partial_cmp(&bounds[rhs].right).unwrap_or(std::cmp::Ordering::Equal));
        let mut active = BTreeSet::new();
        let mut next_end = 0;
        let mut best = None;
        for index in starts {
            while next_end < ends.len() && bounds[ends[next_end]].right <= bounds[index].left {
                active.remove(&ends[next_end]);
                next_end += 1;
            }
            for &other in &active {
                let pair = (index.min(other), index.max(other));
                if pair.0 == 0 && pair.1 < PREFIX_END { continue; }
                if best.is_some_and(|best| pair >= best) { continue; }
                if bounds[index].top <= bounds[other].bottom || bounds[other].top <= bounds[index].bottom { continue; }
                // 宽阶段仅排除不可能对，仍按原始索引调用精确谓词。 / The broad phase only rejects impossible pairs; the exact predicate keeps original index order.
                if Self::overlapped_with_kerf(&placements[pair.0], &placements[pair.1], kerf) {
                    best = Some(pair);
                    if pair == (0, PREFIX_END) { return best; }
                }
            }
            if bounds[index].right > bounds[index].left { active.insert(index); }
        }
        best
    }

    pub(super) fn first_overlap_pairs<V, U>(
        placements: &[Placement2<V, U>],
        kerf: Option<&Quantity<V, U>>,
    ) -> Option<(usize, usize)>
    where
        V: ospf_rust_math::algebra::concept::Field + Clone + PartialOrd + num_traits::Float,
        U: UnitTrait + CTUnit + Clone,
    {
        for lhs in 0..placements.len() {
            for rhs in (lhs + 1)..placements.len() {
                if Self::overlapped_with_kerf(&placements[lhs], &placements[rhs], kerf) { return Some((lhs, rhs)); }
            }
        }
        None
    }
}
