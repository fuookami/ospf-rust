//! 续挖第 88 单：**三种边界处理的退化路径**——`NaN` 三路都落到下界，但 **`+∞` 分道扬镳**（截断给上界、
//! 反射与环绕给下界）；再往外，反射**折 64 期就放弃**，环绕用 `rem_euclid` 所以**负偏移从另一侧回来**
//! （`solver/heuristic/operator/boundary.rs` 的 `ClampBoundary` / `ReflectBoundary` / `WrapBoundary`）
//! Continuation, delivery 88: the **degenerate paths of the three boundary handlers**. `NaN` lands on the lower bound
//! on all three, but **`+∞` splits** (clamping gives the upper bound while reflection and wrapping give the lower one);
//! beyond that, reflection **gives up after 64 periods** and wrapping **reappears from the other side** thanks to
//! `rem_euclid` (`ClampBoundary` / `ReflectBoundary` / `WrapBoundary` in `solver/heuristic/operator/boundary.rs`).
//!
//! 第 81/82 单量的是比较与分层，这一单量**数值回卷本身**——纯 `f64`，一个求解器都不用。八条实测：
//!
//! 1. **`NaN` 三路都落到下界**（`nan_lands_on_the_lower_bound_for_all_three()`）。
//! 2. **`±∞` 让截断分道扬镳**（`the_infinities_diverge_between_clamp_and_the_others()`）：
//!    `+∞` 截断给上界、反射与环绕给下界；`-∞` 三路都给下界。
//! 3. **区间倒置时只有截断 panic**（`an_inverted_range_panics_only_in_clamp()`）：`f64::clamp` 断言
//!    `min <= max`，反射与环绕照旧返下界。本单最值钱的一条。
//! 4. **宽度为零时只有截断给那个值**（`a_zero_width_clamps_to_the_value_itself()`）。
//! 5. **反射只折一次就镜像**（`reflection_folds_once_by_mirroring()`）。
//! 6. **反射折 64 期就放弃**（`reflection_gives_up_after_64_periods()`）：60 个宽度外仍精确，
//!    130 个宽度外被 `.clamp()` 兜住**停在上界**（每两期折回 14，64 期上限只够 448）。
//! 7. **环绕用 `rem_euclid`，负偏移从另一侧回来**（`wrap_uses_rem_euclid()`）。
//! 8. **环绕把区间当半开区间**（`wrapping_treats_the_interval_as_half_open()`）：开区间内三路一致，
//!    但**正好落在上界时环绕跳回下界**，而截断与反射停在上界。
//!
//! Delivery 81/82 measured comparison and layering; this one measures **the numeric roll-back itself**, with no solver.
//! **`NaN` lands low on all three.** **The infinities split the clamp away from the others.** **Only clamping panics on
//! an inverted range** because `f64::clamp` asserts `min <= max` — the most valuable finding here. **A zero width clamps
//! to the value itself.** **Reflection folds once by mirroring.** **Reflection gives up after 64 periods**, so a hundred
//! widths away is still exact while a hundred and thirty is pinned high by the trailing `.clamp()`. **Wrapping uses
//! `rem_euclid`**, so a quarter width below reappears at three quarters up. And **wrapping treats the interval as
//! half-open**: inside it all three agree, yet sitting exactly on the upper bound sends wrapping back to the lower bound
//! while clamping and reflection stay put.

use ospf_rust_core::solver::heuristic::iteration::Iteration;
use ospf_rust_core::solver::heuristic::operator::boundary::{
    BoundaryHandler, ClampBoundary, ReflectBoundary, WrapBoundary,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 边界处理探针请求。 / The boundary-handling probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundaryRequest {
    /// 待处理的数值。 / The value to handle.
    pub value: f64,
    /// 下界。 / The lower bound.
    pub lower_bound: f64,
    /// 上界（允许小于下界，此时只有截断会 panic）。 / The upper bound; an inverted range makes only the clamp panic.
    pub upper_bound: f64,
}

/// 边界处理探针结论。 / The boundary-handling probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundaryReport {
    /// 输入回显。 / The echoed input.
    pub value: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
    /// 截断结果（`clampPanicked` 为真时为 `null`）。 / The clamp result, `null` when the clamp panicked.
    #[serde(default)]
    pub clamped: Option<f64>,
    /// 反射结果。 / The reflection result.
    pub reflected: f64,
    /// 环绕结果。 / The wrap result.
    pub wrapped: f64,
    /// 截断是否 panic 了。 / Whether the clamp panicked.
    pub clamp_panicked: bool,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub nan_lands_on_the_lower_bound_for_all_three: bool,
    pub the_infinities_diverge_between_clamp_and_the_others: bool,
    pub an_inverted_range_panics_only_in_clamp: bool,
    pub a_zero_width_clamps_to_the_value_itself: bool,
    pub reflection_folds_once_by_mirroring: bool,
    pub reflection_gives_up_after_64_periods: bool,
    pub wrap_uses_rem_euclid: bool,
    pub wrapping_treats_the_interval_as_half_open: bool,
}

/// 三路都跑一遍（截断的 panic 被兜住）。 / Run all three, catching the clamp panic.
fn handle_all(value: f64, lower_bound: f64, upper_bound: f64) -> (Option<f64>, f64, f64, bool) {
    let iteration = Iteration::new();
    // 纯数值：三个算子都不读 `iteration`，这里只是契约要求。
    // Pure numerics: none of the three reads `iteration`, which is only there because the trait demands it.
    let clamped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ClampBoundary.handle(&iteration, 0, value, lower_bound, upper_bound)
    })).ok();
    let reflected = ReflectBoundary.handle(&iteration, 0, value, lower_bound, upper_bound);
    let wrapped = WrapBoundary.handle(&iteration, 0, value, lower_bound, upper_bound);
    (clamped, reflected, wrapped, clamped.is_none())
}

/// 走一遍边界探针。非法输入是**错误**。 / Walk the boundary probe.
pub fn verify_boundary(request: &BoundaryRequest) -> Result<BoundaryReport> {
    // 请求侧只接受有限值：JSON 过不来 `NaN`/`±∞`，非有限语义由内部探针量出。
    // The request side accepts finite values only: JSON cannot carry `NaN` or `±∞`, so the non-finite semantics come
    // from the internal probes.
    if !request.value.is_finite() || !request.lower_bound.is_finite()
        || !request.upper_bound.is_finite()
    {
        return Err(BridgeError::new("InvalidInput", "boundary probe failed validation: \
            non-finite values cannot cross the boundary"));
    }
    let findings = canonical_findings();
    let (clamped, reflected, wrapped, clamp_panicked) =
        handle_all(request.value, request.lower_bound, request.upper_bound);
    // 结果侧同样只出有限值：非有限结果无法跨层，用 `null` 表示。
    // The result side is finite too: a non-finite result cannot cross, so it becomes `null`.
    let finite = |value: f64| value.is_finite().then_some(value);
    Ok(BoundaryReport {
        value: request.value,
        lower_bound: request.lower_bound,
        upper_bound: request.upper_bound,
        clamped: clamped.and_then(finite),
        reflected,
        wrapped,
        clamp_panicked,
        nan_lands_on_the_lower_bound_for_all_three:
            findings.nan_lands_on_the_lower_bound_for_all_three,
        the_infinities_diverge_between_clamp_and_the_others:
            findings.the_infinities_diverge_between_clamp_and_the_others,
        an_inverted_range_panics_only_in_clamp: findings.an_inverted_range_panics_only_in_clamp,
        a_zero_width_clamps_to_the_value_itself: findings.a_zero_width_clamps_to_the_value_itself,
        reflection_folds_once_by_mirroring: findings.reflection_folds_once_by_mirroring,
        reflection_gives_up_after_64_periods: findings.reflection_gives_up_after_64_periods,
        wrap_uses_rem_euclid: findings.wrap_uses_rem_euclid,
        wrapping_treats_the_interval_as_half_open:
            findings.wrapping_treats_the_interval_as_half_open,
    })
}

/// 固定探针只跑一次——`f64::clamp` 的 panic 消息会打到 stderr，缓存后只出现一次。
/// The fixed probes run once only: the `f64::clamp` panic message reaches stderr, so caching keeps it to a single line.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    nan_lands_on_the_lower_bound_for_all_three: bool,
    the_infinities_diverge_between_clamp_and_the_others: bool,
    an_inverted_range_panics_only_in_clamp: bool,
    a_zero_width_clamps_to_the_value_itself: bool,
    reflection_folds_once_by_mirroring: bool,
    reflection_gives_up_after_64_periods: bool,
    wrap_uses_rem_euclid: bool,
    wrapping_treats_the_interval_as_half_open: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值（首次调用后走缓存）。
/// Measure every rule property with fixed probes; the caller's request values are not read, and later calls hit the cache.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(measure_findings)
}

/// 实际的固定探针。 / The actual fixed probes.
fn measure_findings() -> Findings {
    // 1) `NaN` 三路都落到下界。 / `NaN` lands on the lower bound on all three.
    let nan_lands_on_the_lower_bound_for_all_three = {
        let (clamped, reflected, wrapped, panicked) = handle_all(f64::NAN, -2.0, 5.0);
        !panicked && clamped == Some(-2.0) && reflected == -2.0 && wrapped == -2.0
    };
    // 2) `±∞` 让截断分道扬镳。 / The infinities split the clamp away from the others.
    let the_infinities_diverge_between_clamp_and_the_others = {
        let (high_clamped, high_reflected, high_wrapped, _) =
            handle_all(f64::INFINITY, -2.0, 5.0);
        let (low_clamped, low_reflected, low_wrapped, _) =
            handle_all(f64::NEG_INFINITY, -2.0, 5.0);
        high_clamped == Some(5.0) && high_reflected == -2.0 && high_wrapped == -2.0
            && low_clamped == Some(-2.0) && low_reflected == -2.0 && low_wrapped == -2.0
    };
    // 3) 区间倒置时只有截断 panic。 / Only the clamp panics on an inverted range.
    let an_inverted_range_panics_only_in_clamp = {
        // `lower = 5 > upper = -2`：`f64::clamp` 断言失败，另两路照旧返下界。 
        // With lower 5 above upper -2 the `f64::clamp` assertion fails while the other two still return the lower bound.
        let (clamped, reflected, wrapped, panicked) = handle_all(1.0, 5.0, -2.0);
        panicked && clamped.is_none() && reflected == 5.0 && wrapped == 5.0
            && !handle_all(1.0, -2.0, 5.0).3
    };
    // 4) 宽度为零时只有截断给那个值。 / A zero width clamps to the value itself.
    let a_zero_width_clamps_to_the_value_itself = {
        let (clamped, reflected, wrapped, panicked) = handle_all(3.0, 3.0, 3.0);
        !panicked && clamped == Some(3.0) && reflected == 3.0 && wrapped == 3.0
            && { let (_, r, w, _) = handle_all(9.0, 3.0, 3.0); r == 3.0 && w == 3.0 }
    };
    // 5) 反射只折一次就镜像。 / Reflection folds once by mirroring.
    let reflection_folds_once_by_mirroring = {
        let (_, above, _, _) = handle_all(7.0, -2.0, 5.0);
        let (_, below, _, _) = handle_all(-4.0, -2.0, 5.0);
        // 上界外 2 → 3；下界外 2 → 0。 / Two above mirrors to 3; two below mirrors to 0.
        above == 3.0 && below == 0.0
    };
    // 6) 反射折 64 期就放弃。 / Reflection gives up after 64 periods.
    let reflection_gives_up_after_64_periods = {
        // 宽度 7：每两期折回 14，64 期上限只够折回 448，所以 60 个宽度（420）仍精确、
        // 130 个宽度（910）撞上 `periods < 64` 后被 `.clamp()` 兜在上界。 
        // Width 7 folds back 14 per two periods, so the 64-period cap covers only 448: sixty widths (420) stays exact
        // while a hundred and thirty (910) hits the cap and is pinned high by the trailing clamp.
        let width = 7.0;
        let (_, exact, _, _) = handle_all(-2.0 + 60.0 * width, -2.0, 5.0);
        let (_, capped, _, _) = handle_all(-2.0 + 130.0 * width, -2.0, 5.0);
        exact >= -2.0 && exact <= 5.0 && capped == 5.0
            && handle_all(0.0, -2.0, 5.0).1 == 0.0
    };
    // 7) 环绕用 `rem_euclid`，负偏移从另一侧回来。 / Wrapping uses `rem_euclid`.
    let wrap_uses_rem_euclid = {
        let width = 8.0;
        let (_, reflected, wrapped, _) = handle_all(-2.0 - 0.25 * width, -2.0, 6.0);
        // 环绕落在 `lower + 0.75 * width`，反射落在 `lower + 0.25 * width`。 
        // Wrapping lands at `lower + 0.75 * width` while reflection lands at `lower + 0.25 * width`.
        (wrapped - (-2.0 + 0.75 * width)).abs() < 1e-12
            && (reflected - (-2.0 + 0.25 * width)).abs() < 1e-12
            // 上界外正好一个宽度回卷到**下界**（半开区间语义）。
            // Exactly one width above the upper bound wraps to the **lower** bound, the half-open interval semantics.
            && { let (_, _, w, _) = handle_all(6.0 + width, -2.0, 6.0); (w - (-2.0)).abs() < 1e-12 }
    };
    // 8) 环绕把区间当半开区间。 / Wrapping treats the interval as half-open.
    let wrapping_treats_the_interval_as_half_open = {
        // 开区间内三路一致。 / Inside the open interval all three agree.
        let (clamped, reflected, wrapped, panicked) = handle_all(1.5, -2.0, 5.0);
        !panicked && clamped == Some(1.5) && reflected == 1.5 && wrapped == 1.5
            // 正好落在上界：截断与反射停在上界，环绕跳回下界。 
            // Sitting exactly on the upper bound: clamping and reflection stay put while wrapping jumps to the lower bound.
            && { let (c, r, w, p) = handle_all(5.0, -2.0, 5.0);
                !p && c == Some(5.0) && r == 5.0 && w == -2.0 }
    };
    Findings {
        nan_lands_on_the_lower_bound_for_all_three,
        the_infinities_diverge_between_clamp_and_the_others,
        an_inverted_range_panics_only_in_clamp,
        a_zero_width_clamps_to_the_value_itself,
        reflection_folds_once_by_mirroring,
        reflection_gives_up_after_64_periods,
        wrap_uses_rem_euclid,
        wrapping_treats_the_interval_as_half_open,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> BoundaryRequest {
        BoundaryRequest { value: 1.5, lower_bound: -2.0, upper_bound: 5.0 }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_range() {
        let report = verify_boundary(&request()).unwrap();
        assert_eq!(report.clamped, Some(1.5));
        assert_eq!(report.reflected, 1.5);
        assert_eq!(report.wrapped, 1.5);
        assert!(!report.clamp_panicked);
        assert!(report.nan_lands_on_the_lower_bound_for_all_three);
        assert!(report.the_infinities_diverge_between_clamp_and_the_others);
        assert!(report.an_inverted_range_panics_only_in_clamp);
        assert!(report.a_zero_width_clamps_to_the_value_itself);
        assert!(report.reflection_folds_once_by_mirroring);
        assert!(report.reflection_gives_up_after_64_periods);
        assert!(report.wrap_uses_rem_euclid);
        assert!(report.wrapping_treats_the_interval_as_half_open);
    }

    #[test]
    fn above_the_upper_bound_the_three_disagree() {
        let report = verify_boundary(&BoundaryRequest { value: 7.0, lower_bound: -2.0,
            upper_bound: 5.0 }).unwrap();
        // 截断停在上界，反射镜像到 3，环绕回卷到另一侧。 / The clamp stops high, reflection mirrors to 3 and wrapping
        // rolls to the other side.
        assert_eq!(report.clamped, Some(5.0));
        assert_eq!(report.reflected, 3.0);
        // 上界外 2、宽度 7 → 回卷到 `lower + 2`。 / Two above with a width of seven wraps to `lower + 2`.
        assert_eq!(report.wrapped, 0.0);
        assert!(!report.clamp_panicked);
    }

    #[test]
    fn below_the_lower_bound_wrapping_reappears_near_the_upper_bound() {
        let report = verify_boundary(&BoundaryRequest { value: -4.0, lower_bound: -2.0,
            upper_bound: 5.0 }).unwrap();
        assert_eq!(report.clamped, Some(-2.0));
        assert_eq!(report.reflected, 0.0);
        // 下界外 2 个宽度 7 → 落回 `lower + 5`。 / Two below with a width of seven lands at `lower + 5`.
        assert_eq!(report.wrapped, 3.0);
    }

    #[test]
    fn an_inverted_range_panics_the_clamp_only() {
        let report = verify_boundary(&BoundaryRequest { value: 1.0, lower_bound: 5.0,
            upper_bound: -2.0 }).unwrap();
        assert!(report.clamp_panicked);
        assert_eq!(report.clamped, None);
        assert_eq!(report.reflected, 5.0);
        assert_eq!(report.wrapped, 5.0);
    }

    #[test]
    fn a_zero_width_range_lands_on_the_bound() {
        let report = verify_boundary(&BoundaryRequest { value: 9.0, lower_bound: 3.0,
            upper_bound: 3.0 }).unwrap();
        // 截断返 3（`clamp(9, 3, 3)`），反射与环绕的 `width <= 0` 守卫也返下界 3。 
        // The clamp returns 3 and the `width <= 0` guard on the other two also returns the lower bound 3.
        assert_eq!(report.clamped, Some(3.0));
        assert_eq!(report.reflected, 3.0);
        assert_eq!(report.wrapped, 3.0);
    }

    #[test]
    fn far_away_reflection_is_capped() {
        let report = verify_boundary(&BoundaryRequest { value: -2.0 + 130.0 * 7.0,
            lower_bound: -2.0, upper_bound: 5.0 }).unwrap();
        // 折 64 期不够 → 被钳在上界。 / Sixty-four folds are not enough, so the result is pinned high.
        assert_eq!(report.reflected, 5.0);
        // 而 60 个宽度仍在 64 期上限内、精确落回下界。 / Sixty widths still fits the cap and lands exactly low.
        let exact = verify_boundary(&BoundaryRequest { value: -2.0 + 60.0 * 7.0,
            lower_bound: -2.0, upper_bound: 5.0 }).unwrap();
        assert_eq!(exact.reflected, -2.0);
    }

    #[test]
    fn the_upper_bound_sends_wrapping_back_to_the_lower_bound() {
        let report = verify_boundary(&BoundaryRequest { value: 5.0, lower_bound: -2.0,
            upper_bound: 5.0 }).unwrap();
        // 半开区间：上界环绕回下界，截断与反射停在上界。 
        // Half-open: the upper bound wraps to the lower one while clamping and reflection stay put.
        assert_eq!(report.clamped, Some(5.0));
        assert_eq!(report.reflected, 5.0);
        assert_eq!(report.wrapped, -2.0);
    }

    #[test]
    fn non_finite_values_are_refused() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(verify_boundary(&BoundaryRequest { value, lower_bound: -2.0,
                upper_bound: 5.0 }).unwrap_err().message
                .contains("non-finite values cannot cross"));
        }
        assert!(verify_boundary(&BoundaryRequest { value: 1.0, lower_bound: f64::NAN,
            upper_bound: 5.0 }).is_err());
        assert!(verify_boundary(&BoundaryRequest { value: 1.0, lower_bound: -2.0,
            upper_bound: f64::INFINITY }).is_err());
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_boundary(&BoundaryRequest { value: 7.0, lower_bound: -2.0,
            upper_bound: 5.0 }).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"lowerBound\""), "{json}");
        assert!(json.contains("\"clampPanicked\""), "{json}");
        assert!(json.contains("\"theInfinitiesDivergeBetweenClampAndTheOthers\""), "{json}");
        assert!(!json.contains("lower_bound"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<BoundaryReport>(&copy.to_string()).is_err());
    }
}
