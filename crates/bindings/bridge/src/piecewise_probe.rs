//! 续挖第 94 单：**分段线性构造器的闸门**——两点的底线、**x 必须严格递增**（等值直接 panic）、坐标必须有限、
//! **结果变量的值域是 y 的包络**（`[min y, max y]`，与 x 的顺序无关）；而 `from_segments` **不校验线段之间的连续性**：
//! 每个断点的 y 一律由**它右边那条线段**算出，于是"同一图像的声明"只是文档，**不一致的线段图会被静默改写**
//! （`symbol/functions/univariate_linear_piecewise.rs` 的 `Point2` / `Segment2` /
//! `UnivariateLinearPiecewiseFunction::new` / `from_segments`）
//! Continuation, delivery 94: **the gates of the piecewise constructor** — a two-point floor, **x must strictly
//! ascend** (an equal value panics outright), finite coordinates, and **the result variable's domain is the y envelope**
//! (`[min y, max y]`, independent of the x order); while `from_segments` **does not check continuity between segments**:
//! every breakpoint's y comes from **the segment on its right**, so the "same graph" claim is documentation only and **an
//! inconsistent segment graph is silently rewritten**
//! (`Point2` / `Segment2` / `UnivariateLinearPiecewiseFunction::new` / `from_segments` in
//! `symbol/functions/univariate_linear_piecewise.rs`).
//!
//! 第 93 单量的是档案，这一单量**分段线性插值函数的构造闸门**（纯 `f64`，不注册符号也不建模型）。八条实测：
//!
//! 1. **至少要两个点**（`at_least_two_points_are_required()`）：0 与 1 都 panic。
//! 2. **x 必须严格递增**（`the_x_values_must_strictly_ascend()`）：等值与降序都 panic。
//! 3. **坐标必须有限**（`non_finite_coordinates_are_refused()`）：x 或 y 非有限都 panic。
//! 4. **结果变量的值域是 y 的包络**（`the_result_range_is_the_y_envelope()`）：`[min y, max y]`，与 x 的顺序无关
//!    （x 递增是前提，所以"包络"看的是给出的 y 集合）。
//! 5. **λ 与选择器的数量都跟着点数走**（`the_lambda_and_selector_counts_follow_the_point_count()`）：
//!    λ 有 n 个（都在 `[0, 1]`），选择器有 n-1 个。
//! 6. **线段数必须正好比断点数少一**（`segments_must_match_the_breakpoint_count()`）：两个向量长度都要对上。
//! 7. **不一致的线段图会被静默改写**（`an_inconsistent_segment_graph_is_silently_rewritten()`）：
//!    每个断点取**右边**那条线段的斜率与截距，于是"同一图像"的声明没有校验。本单最值钱的一条。
//! 8. **`Display` 是 `ulp(<名字>)`**（`the_display_wraps_the_name()`）。
//!
//! Delivery 93 measured archives; this one measures **the constructor gates of the piecewise-linear function** (pure
//! `f64`, with no symbol registration and no model). **At least two points are required**, and zero or one panics. **The
//! x values must strictly ascend**, so equal and descending values panic. **Coordinates must be finite.** **The result
//! variable's domain is the y envelope**, independent of the x order. **The lambda and selector counts follow the point
//! count** — n lambdas in `[0, 1]` and n−1 selectors. **The segment count must be exactly one less than the breakpoint
//! count.** **An inconsistent segment graph is silently rewritten** because each breakpoint takes the slope and intercept
//! of **the segment on its right**, so the "same graph" claim is never checked — the most valuable finding here. And
//! **`Display` wraps the name as `ulp(<name>)`**.

use ospf_rust_core::symbol::functions::univariate_linear_piecewise::{
    Point2, UnivariateLinearPiecewiseFunction,
};
use ospf_rust_core::symbol::flatten::Linear;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个断点。 / One breakpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PiecewisePointSpec {
    /// x 坐标。 / The x coordinate.
    pub x: f64,
    /// y 坐标（函数值）。 / The y coordinate.
    pub y: f64,
}

/// 分段线性探针请求。 / The piecewise-linear probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PiecewiseRequest {
    /// 函数名（决定 `display()` 的文本）。 / The function name, which decides `display()`.
    #[serde(default)]
    pub name: Option<String>,
    /// 点形式构造时给出的点。 / The points handed to the point form.
    #[serde(default)]
    pub points: Vec<PiecewisePointSpec>,
    /// 线段形式构造时给出的断点。 / The breakpoints handed to the segment form.
    #[serde(default)]
    pub breakpoints: Vec<f64>,
    /// 线段形式构造时给出的斜率。 / The segment slopes.
    #[serde(default)]
    pub slopes: Vec<f64>,
    /// 线段形式构造时给出的截距。 / The segment intercepts.
    #[serde(default)]
    pub intercepts: Vec<f64>,
    /// 是否走线段形式（否则走点形式）。 / Whether to take the segment form instead of the point form.
    #[serde(default)]
    pub use_segments: Option<bool>,
}

/// 分段线性探针结论。 / The piecewise-linear probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PiecewiseReport {
    /// 是否构造成功（失败即 panic 被兜住）。 / Whether construction succeeded; a failure means the panic was caught.
    pub constructed: bool,
    /// 结果变量的下界（`null` 表示无下界）。 / The result variable's lower bound, `null` when unbounded.
    #[serde(default)]
    pub result_lower: Option<f64>,
    /// 结果变量的上界（`null` 表示无上界）。 / The result variable's upper bound.
    #[serde(default)]
    pub result_upper: Option<f64>,
    /// λ 变量的数量。 / The lambda variable count.
    pub lambda_count: u32,
    /// 选择器变量的数量。 / The selector variable count.
    pub selector_count: u32,
    /// 构造器内部的点（线段形式下能看到"右边那条线段"的取值）。
    /// The points the constructor ended up with; in the segment form they reveal the right-hand segment's value.
    pub points: Vec<PiecewisePointSpec>,
    /// `display()` 的文本。 / What `display()` says.
    pub display: String,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub at_least_two_points_are_required: bool,
    pub the_x_values_must_strictly_ascend: bool,
    pub non_finite_coordinates_are_refused: bool,
    pub the_result_range_is_the_y_envelope: bool,
    pub the_lambda_and_selector_counts_follow_the_point_count: bool,
    pub segments_must_match_the_breakpoint_count: bool,
    pub an_inconsistent_segment_graph_is_silently_rewritten: bool,
    pub the_display_wraps_the_name: bool,
}

/// 走一遍点形式。panic 被兜住，返回构造与否与读数。 / Walk the point form, catching the panic.
fn build_points(name: &str, points: &[PiecewisePointSpec])
    -> Option<UnivariateLinearPiecewiseFunction<f64>>
{
    let input = Linear::new(Vec::new(), 0.0_f64);
    let domain: Vec<Point2<f64>> = points.iter()
        .map(|point| Point2::new(point.x, point.y)).collect();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        UnivariateLinearPiecewiseFunction::new(0, name, input, domain)
    })).ok()
}

/// 走一遍线段形式。 / Walk the segment form.
fn build_segments(name: &str, breakpoints: &[f64], slopes: &[f64], intercepts: &[f64])
    -> Option<UnivariateLinearPiecewiseFunction<f64>>
{
    let input = Linear::new(Vec::new(), 0.0_f64);
    let breakpoints = breakpoints.to_vec();
    let slopes = slopes.to_vec();
    let intercepts = intercepts.to_vec();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        UnivariateLinearPiecewiseFunction::from_segments(0, name, input, breakpoints, slopes,
            intercepts)
    })).ok()
}

/// 把构造出来的函数转成读数。 / Turn a built function into the readable face.
fn face(name: &str, built: Option<UnivariateLinearPiecewiseFunction<f64>>)
    -> (bool, Option<f64>, Option<f64>, u32, u32, Vec<PiecewisePointSpec>, String)
{
    match built {
        None => (false, None, None, 0, 0, Vec::new(), String::new()),
        Some(function) => {
            let range = function.result_variable().range();
            let points = function.points().iter()
                .map(|point| PiecewisePointSpec { x: point.x, y: point.y }).collect();
            (true, range.lower_bound, range.upper_bound,
                u32::try_from(function.lambda_variables().len()).unwrap_or(u32::MAX),
                u32::try_from(function.selector_variables().len()).unwrap_or(u32::MAX),
                points, format!("{function}"))
        }
    }
}

/// 走一遍分段线性探针。非法输入是**错误**；结构违规由构造器的 assert 表达（被兜住后报 `constructed: false`）。
/// Walk the piecewise probe. Illegal *input* is an error; a structural violation is expressed by the constructor's
/// assert, which is caught and reported as `constructed: false`.
pub fn verify_piecewise(request: &PiecewiseRequest) -> Result<PiecewiseReport> {
    if request.points.len() > 8 || request.breakpoints.len() > 8 {
        return Err(BridgeError::new("InvalidInput",
            "piecewise probe failed validation: at most eight points or breakpoints"));
    }
    let name = request.name.clone().unwrap_or_else(|| "probe".to_owned());
    let findings = canonical_findings();
    let built = if request.use_segments.unwrap_or(false) {
        build_segments(&name, &request.breakpoints, &request.slopes, &request.intercepts)
    } else {
        build_points(&name, &request.points)
    };
    let (constructed, result_lower, result_upper, lambda_count, selector_count, points, display) =
        face(&name, built);
    Ok(PiecewiseReport {
        constructed,
        result_lower,
        result_upper,
        lambda_count,
        selector_count,
        points,
        display,
        at_least_two_points_are_required: findings.at_least_two_points_are_required,
        the_x_values_must_strictly_ascend: findings.the_x_values_must_strictly_ascend,
        non_finite_coordinates_are_refused: findings.non_finite_coordinates_are_refused,
        the_result_range_is_the_y_envelope: findings.the_result_range_is_the_y_envelope,
        the_lambda_and_selector_counts_follow_the_point_count:
            findings.the_lambda_and_selector_counts_follow_the_point_count,
        segments_must_match_the_breakpoint_count:
            findings.segments_must_match_the_breakpoint_count,
        an_inconsistent_segment_graph_is_silently_rewritten:
            findings.an_inconsistent_segment_graph_is_silently_rewritten,
        the_display_wraps_the_name: findings.the_display_wraps_the_name,
    })
}

/// 固定探针只跑一次（assert 的 panic 消息会打到 stderr）。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    at_least_two_points_are_required: bool,
    the_x_values_must_strictly_ascend: bool,
    non_finite_coordinates_are_refused: bool,
    the_result_range_is_the_y_envelope: bool,
    the_lambda_and_selector_counts_follow_the_point_count: bool,
    segments_must_match_the_breakpoint_count: bool,
    an_inconsistent_segment_graph_is_silently_rewritten: bool,
    the_display_wraps_the_name: bool,
}

/// 造一个点。 / Build one point.
fn point(x: f64, y: f64) -> PiecewisePointSpec {
    PiecewisePointSpec { x, y }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 至少要两个点。 / At least two points are required.
        let at_least_two_points_are_required = {
            build_points("p", &[]).is_none()
                && build_points("p", &[point(0.0, 1.0)]).is_none()
                && build_points("p", &[point(0.0, 1.0), point(1.0, 2.0)]).is_some()
        };
        // 2) x 必须严格递增。 / The x values must strictly ascend.
        let the_x_values_must_strictly_ascend = {
            // 等值与降序都 panic。 / Equal and descending values both panic.
            build_points("p", &[point(1.0, 0.0), point(1.0, 5.0)]).is_none()
                && build_points("p", &[point(2.0, 0.0), point(1.0, 5.0)]).is_none()
                && build_points("p", &[point(0.0, 0.0), point(1.0, 5.0),
                    point(2.0, 1.0)]).is_some()
        };
        // 3) 坐标必须有限。 / Coordinates must be finite.
        let non_finite_coordinates_are_refused = {
            build_points("p", &[point(f64::NAN, 0.0), point(1.0, 1.0)]).is_none()
                && build_points("p", &[point(0.0, f64::INFINITY), point(1.0, 1.0)]).is_none()
                && build_points("p", &[point(0.0, 0.0), point(f64::NEG_INFINITY, 1.0)]).is_none()
        };
        // 4) 结果变量的值域是 y 的包络。 / The result range is the y envelope.
        let the_result_range_is_the_y_envelope = {
            // y 值 `[5, 1, 9]` → `[1, 9]`，与 x 递增顺序无关。 
            // The y values `[5, 1, 9]` give `[1, 9]` regardless of the ascending x order.
            let built = build_points("p", &[point(0.0, 5.0), point(1.0, 1.0), point(2.0, 9.0)]);
            let range = built.as_ref().map(|function| function.result_variable().range().clone());
            range.as_ref().and_then(|value| value.lower_bound) == Some(1.0)
                && range.and_then(|value| value.upper_bound) == Some(9.0)
                // 两点一样高时值域退化成一点。 / Equal heights collapse the range to a single value.
                && { let flat = build_points("p", &[point(0.0, 4.0), point(1.0, 4.0)]);
                    flat.map(|function| {
                        let range = function.result_variable().range();
                        range.lower_bound == Some(4.0) && range.upper_bound == Some(4.0)
                    }) == Some(true) }
        };
        // 5) λ 与选择器的数量跟着点数走。 / The lambda and selector counts follow the point count.
        let the_lambda_and_selector_counts_follow_the_point_count = {
            let four = build_points("p", &[point(0.0, 0.0), point(1.0, 1.0), point(2.0, 4.0),
                point(3.0, 9.0)]);
            four.map(|function| {
                let lambdas = function.lambda_variables();
                function.lambda_variables().len() == 4
                    && function.selector_variables().len() == 3
                    // 每个 λ 都在 `[0, 1]`。 / Every lambda sits in `[0, 1]`.
                    && lambdas.iter().all(|lambda| {
                        let range = lambda.range();
                        range.lower_bound == Some(0.0) && range.upper_bound == Some(1.0)
                    })
            }) == Some(true)
        };
        // 6) 线段数必须正好比断点数少一。 / The segment count must be one less than the breakpoint count.
        let segments_must_match_the_breakpoint_count = {
            // 断点 3 个 → 斜率与截距各 2 个。 / Three breakpoints need two slopes and two intercepts.
            let ok = build_segments("p", &[0.0, 1.0, 2.0], &[1.0, 2.0], &[0.0, 0.0]);
            // 斜率少一个 → panic；截距少一个 → panic；断点只有一个 → panic。 
            // One slope short panics, one intercept short panics and a single breakpoint panics.
            ok.is_some()
                && build_segments("p", &[0.0, 1.0, 2.0], &[1.0], &[0.0, 0.0]).is_none()
                && build_segments("p", &[0.0, 1.0, 2.0], &[1.0, 2.0], &[0.0]).is_none()
                && build_segments("p", &[0.0], &[], &[]).is_none()
        };
        // 7) 不一致的线段图会被静默改写。 / An inconsistent segment graph is silently rewritten.
        let an_inconsistent_segment_graph_is_silently_rewritten = {
            // 段 0：`y = x`（0 → 0、1 → 1）；段 1：`y = 10x - 9`（1 → 1、2 → 11）。两者在 1 处相接。 
            // Segment 0 is `y = x` (0 to 0, 1 to 1) and segment 1 is `y = 10x - 9` (1 to 1, 2 to 11): they meet at one.
            let consistent = build_segments("p", &[0.0, 1.0, 2.0], &[1.0, 10.0], &[0.0, -9.0]);
            // 段 1 改成 `y = x + 100`：与段 0 在 1 处**断开**（段 0 说 1，段 1 说 101），而构造器不报错，
            // 直接取"右边那条"的值，于是断点 1 的 y 变成 101。 
            // Changing segment 1 to `y = x + 100` **breaks** the graph at one (segment 0 says 1, segment 1 says 101),
            // yet the constructor raises nothing and simply takes the right-hand segment, so breakpoint one becomes 101.
            let broken = build_segments("p", &[0.0, 1.0, 2.0], &[1.0, 1.0], &[0.0, 100.0]);
            consistent.is_some() && broken.as_ref().map(|function| {
                let points = function.points();
                points.len() == 3
                    && (points[0].y - 0.0).abs() < 1e-12
                    // 断点 1 拿到的是段 1 的值 101，段 0 的 1 被丢掉。 
                    // Breakpoint one takes 101 from segment 1 while segment 0's 1 is dropped.
                    && (points[1].y - 101.0).abs() < 1e-12
                    && (points[2].y - 102.0).abs() < 1e-12
            }) == Some(true)
        };
        // 8) `Display` 是 `ulp(<名字>)`。 / `Display` wraps the name as `ulp(<name>)`.
        let the_display_wraps_the_name = {
            build_points("alpha", &[point(0.0, 0.0), point(1.0, 1.0)])
                .map(|function| format!("{function}") == "ulp(alpha)") == Some(true)
        };
        Findings {
            at_least_two_points_are_required,
            the_x_values_must_strictly_ascend,
            non_finite_coordinates_are_refused,
            the_result_range_is_the_y_envelope,
            the_lambda_and_selector_counts_follow_the_point_count,
            segments_must_match_the_breakpoint_count,
            an_inconsistent_segment_graph_is_silently_rewritten,
            the_display_wraps_the_name,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> PiecewiseRequest {
        PiecewiseRequest { name: Some("f".to_owned()),
            points: vec![point(0.0, 0.0), point(1.0, 1.0), point(2.0, 4.0)],
            breakpoints: Vec::new(), slopes: Vec::new(), intercepts: Vec::new(),
            use_segments: None }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = verify_piecewise(&request()).unwrap();
        assert!(report.constructed);
        assert!(report.at_least_two_points_are_required);
        assert!(report.the_x_values_must_strictly_ascend);
        assert!(report.non_finite_coordinates_are_refused);
        assert!(report.the_result_range_is_the_y_envelope);
        assert!(report.the_lambda_and_selector_counts_follow_the_point_count);
        assert!(report.segments_must_match_the_breakpoint_count);
        assert!(report.an_inconsistent_segment_graph_is_silently_rewritten);
        assert!(report.the_display_wraps_the_name);
        // 三点：λ 三个、选择器两个，值域 `[0, 4]`。 / Three points give three lambdas and two selectors.
        assert_eq!(report.lambda_count, 3);
        assert_eq!(report.selector_count, 2);
        assert_eq!(report.result_lower, Some(0.0));
        assert_eq!(report.result_upper, Some(4.0));
        assert_eq!(report.display, "ulp(f)");
        assert_eq!(report.points.len(), 3);
    }

    #[test]
    fn fewer_than_two_points_panics() {
        let empty = verify_piecewise(&PiecewiseRequest { points: Vec::new(),
            ..request() }).unwrap();
        assert!(!empty.constructed);
        assert_eq!(empty.display, "");
        let single = verify_piecewise(&PiecewiseRequest { points: vec![point(0.0, 1.0)],
            ..request() }).unwrap();
        assert!(!single.constructed);
    }

    #[test]
    fn a_non_ascending_x_panics() {
        let equal = verify_piecewise(&PiecewiseRequest { points: vec![point(1.0, 0.0),
            point(1.0, 5.0)], ..request() }).unwrap();
        assert!(!equal.constructed);
        let descending = verify_piecewise(&PiecewiseRequest { points: vec![point(2.0, 0.0),
            point(1.0, 5.0)], ..request() }).unwrap();
        assert!(!descending.constructed);
    }

    #[test]
    fn the_range_is_the_y_envelope() {
        let report = verify_piecewise(&PiecewiseRequest { points: vec![point(0.0, 5.0),
            point(1.0, 1.0), point(2.0, 9.0)], ..request() }).unwrap();
        assert_eq!(report.result_lower, Some(1.0));
        assert_eq!(report.result_upper, Some(9.0));
        let flat = verify_piecewise(&PiecewiseRequest { points: vec![point(0.0, 4.0),
            point(1.0, 4.0)], ..request() }).unwrap();
        assert_eq!(flat.result_lower, Some(4.0));
        assert_eq!(flat.result_upper, Some(4.0));
    }

    #[test]
    fn an_inconsistent_segment_graph_is_accepted_and_rewritten() {
        // 段 1 与段 0 在 1 处断开（1 对 101），构造器不报错，取右边那条。 
        // Segment 1 breaks from segment 0 at one (1 versus 101); the constructor raises nothing and takes the right side.
        let report = verify_piecewise(&PiecewiseRequest { use_segments: Some(true),
            breakpoints: vec![0.0, 1.0, 2.0], slopes: vec![1.0, 1.0],
            intercepts: vec![0.0, 100.0], ..request() }).unwrap();
        assert!(report.constructed);
        assert_eq!(report.points.len(), 3);
        assert!((report.points[0].y - 0.0).abs() < 1e-12);
        assert!((report.points[1].y - 101.0).abs() < 1e-12);
        assert!((report.points[2].y - 102.0).abs() < 1e-12);
    }

    #[test]
    fn mismatched_segment_counts_panic() {
        let short_slopes = verify_piecewise(&PiecewiseRequest { use_segments: Some(true),
            breakpoints: vec![0.0, 1.0, 2.0], slopes: vec![1.0], intercepts: vec![0.0, 0.0],
            ..request() }).unwrap();
        assert!(!short_slopes.constructed);
        let short_intercepts = verify_piecewise(&PiecewiseRequest { use_segments: Some(true),
            breakpoints: vec![0.0, 1.0, 2.0], slopes: vec![1.0, 2.0], intercepts: vec![0.0],
            ..request() }).unwrap();
        assert!(!short_intercepts.constructed);
        let ok = verify_piecewise(&PiecewiseRequest { use_segments: Some(true),
            breakpoints: vec![0.0, 1.0, 2.0], slopes: vec![1.0, 2.0], intercepts: vec![0.0, 0.0],
            ..request() }).unwrap();
        assert!(ok.constructed);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_piecewise(&PiecewiseRequest { points: (0..9).map(|index|
            point(index as f64, 0.0)).collect(), ..request() }).unwrap_err()
            .message.contains("at most eight points"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_piecewise(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"resultLower\""), "{json}");
        assert!(json.contains("\"lambdaCount\""), "{json}");
        assert!(json.contains("\"anInconsistentSegmentGraphIsSilentlyRewritten\""), "{json}");
        assert!(!json.contains("result_lower"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<PiecewiseReport>(&copy.to_string()).is_err());
    }
}
