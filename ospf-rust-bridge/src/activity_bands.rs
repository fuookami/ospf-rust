//! 续挖第 44 单：活动性判定的**四条容差带**——相对容差把 `scale = 0` 也抬高，
//! 且 `slack = -active` 恰好算 `Active` 而不是 `Violated`
//! （`analysis/activity.rs` 的 `ActivityConfig::effective_tolerance` / `classify`）
//! Continuation, delivery 44: the **four tolerance bands** of activity classification — the relative tolerance lifts even
//! `scale = 0`, and `slack = -active` counts as `Active` rather than `Violated` (`ActivityConfig::effective_tolerance`,
//! `classify`).
//!
//! `analysis/activity.rs` 之前只被当作"枚举值"用（`ActivityStatus` 在漏斗与有效性排序里出现），
//! **判定的四段带子从来没量过**。五条实测：
//!
//! 1. **判定的顺序是"先违约、再活动、再接近、最后不活动"**（`order_decides_the_boundaries()`）：
//!    `slack < -active` → `Violated`；`|slack| <= active` → `Active`；`|slack| <= nearly` → `NearlyActive`；
//!    否则 `Inactive`。所以恰好 `slack = -active` 落在**第二档**（`Active`），**不是**违约——
//!    "负 slack 就是违约"是错的，违约要求**严格**超过容差。
//! 2. **相对容差把 `scale = 0` 也抬高**（`relative_tolerance_lifts_zero_scale()`）：
//!    `effective_tolerance = max(tolerance, relative_tolerance * max(|scale|, 1.0))`，
//!    所以 `|scale| < 1` 一律按 `1.0` 算——相对容差在"尺度为零"时**不是零**。
//! 3. **`nearly_active_tolerance` 会被抬到不低于 active 容差**（`nearly_is_lifted_to_active()`）：
//!    `classify` 里对它取 `.max(active_tolerance)`，所以相对容差把 active 顶上去之后，
//!    `NearlyActive` 那条带子会**消失**（没有落在两档之间的 slack）。
//! 4. **非有限 slack 或非有限 scale → `Unknown`**（`non_finite_is_unknown()`），
//!    而且这时**不**看容差——先判有限性。**但这条路出不了 JSON**：`serde_json` 把 NaN/±Infinity 写成 `null`
//!    就再也读不回来，所以入口**直接拒绝**非有限输入（见下）。
//! 5. **`with_tolerance(t)` 令 nearly 恰为 `10t`**（`with_tolerance_scales_by_ten()`）；
//!    `validate()` 要求两者有限、非负且 `nearly >= tolerance`。
//!
//! `analysis/activity.rs` has so far only been used as "a set of enum values" (`ActivityStatus` shows up in the funnel and the
//! effectiveness ranking); **the four classification bands were never measured**. **The decision order is violation, active,
//! nearly-active, inactive** (`order_decides_the_boundaries()`): `slack < -active` gives `Violated`, `|slack| <= active` gives
//! `Active`, `|slack| <= nearly` gives `NearlyActive`, otherwise `Inactive` — so exactly `slack = -active` lands in the
//! **second** band (`Active`) and is **not** a violation: "negative slack means violated" is wrong, and a violation requires
//! **strictly** exceeding the tolerance. **The relative tolerance lifts even `scale = 0`**
//! (`relative_tolerance_lifts_zero_scale()`): `effective_tolerance = max(tolerance, relative_tolerance * max(|scale|, 1.0))`,
//! so any `|scale| < 1` is treated as `1.0` and the relative tolerance is **not** zero at zero scale. **`nearly_active_tolerance`
//! is raised to at least the active tolerance** (`nearly_is_lifted_to_active()`): `classify` takes
//! `.max(active_tolerance)`, so once the relative tolerance pushes active up the `NearlyActive` band **disappears**. **A
//! non-finite slack or scale gives `Unknown`** (`non_finite_is_unknown()`) without consulting the tolerances at all, since
//! finiteness is checked first. And **`with_tolerance(t)` makes nearly exactly `10t`** (`with_tolerance_scales_by_ten()`), while
//! `validate()` requires both to be finite, non-negative and `nearly >= tolerance`.

use ospf_rust_core::analysis::activity::{ActivityConfig, ActivityStatus};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 活动性判定请求。 / The activity classification request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityBandRequest {
    /// active 容差。 / The active tolerance.
    pub tolerance: f64,
    /// nearly-active 容差。 / The nearly-active tolerance.
    pub nearly_active_tolerance: f64,
    /// 相对容差系数。 / The relative tolerance coefficient.
    #[serde(default)]
    pub relative_tolerance: f64,
    /// 要判定的 signed slack。 / The signed slacks to classify.
    #[serde(default)]
    pub slacks: Vec<f64>,
    /// 伴随的尺度。 / The accompanying scales.
    #[serde(default)]
    pub scales: Vec<f64>,
}

/// 一次判定。 / One classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityBandRow {
    /// signed slack。 / The signed slack.
    pub slack: f64,
    /// 尺度。 / The scale.
    pub scale: f64,
    /// active 容差。 / The active tolerance.
    pub active_tolerance: f64,
    /// nearly-active 容差（已被抬到不低于 active）。 / The nearly-active tolerance (lifted to at least active).
    pub nearly_active_tolerance: f64,
    /// 判定结果。 / The classification.
    pub status: String,
}

/// 活动性判定结论。 / The activity classification verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityBandReport {
    /// 逐条判定。 / One row per request pair.
    pub rows: Vec<ActivityBandRow>,
    /// `withTolerance(t)` 给出的 nearly 容差。 / The nearly tolerance `withTolerance(t)` gives.
    pub scaled_nearly: f64,
    /// 配置是否通过 `validate()`。 / Whether the configuration validates.
    pub accepted: bool,
    /// 判定顺序决定边界（**是**）。 / Whether the order decides the boundaries (**yes**).
    pub order_decides_the_boundaries: bool,
    /// 相对容差抬高零尺度（**是**）。 / Whether the relative tolerance lifts zero scale (**yes**).
    pub relative_tolerance_lifts_zero_scale: bool,
    /// nearly 被抬到不低于 active（**是**）。 / Whether nearly is lifted to active (**yes**).
    pub nearly_is_lifted_to_active: bool,
    /// 非有限值给出 Unknown（**是**）。 / Whether non-finite values give Unknown (**yes**).
    pub non_finite_is_unknown: bool,
    /// `withTolerance` 按十倍放大（**是**）。 / Whether `withTolerance` scales by ten (**yes**).
    pub with_tolerance_scales_by_ten: bool,
}

/// 判定条数上限（信任边界）。 / The classification cap (a trust boundary).
const MAX_ROWS: usize = 256;

/// 状态名（`SCREAMING_SNAKE_CASE` 线上形态）。 / The status name (the `SCREAMING_SNAKE_CASE` wire form).
fn status_name(status: ActivityStatus) -> &'static str {
    match status {
        ActivityStatus::Active => "ACTIVE",
        ActivityStatus::NearlyActive => "NEARLY_ACTIVE",
        ActivityStatus::Inactive => "INACTIVE",
        ActivityStatus::SatisfiedWithoutSlackMetric => "SATISFIED_WITHOUT_SLACK_METRIC",
        ActivityStatus::Violated => "VIOLATED",
        ActivityStatus::Unknown => "UNKNOWN",
    }
}

/// 走一遍活动性判定。非法输入是**错误**。 / Walk the activity classification.
pub fn verify_activity_bands(request: &ActivityBandRequest) -> Result<ActivityBandReport> {
    if request.slacks.len() > MAX_ROWS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "activity bands failed validation: at most {MAX_ROWS} slacks are accepted")));
    }
    if request.scales.len() != request.slacks.len() {
        return Err(BridgeError::new("InvalidInput",
            "activity bands failed validation: every slack needs exactly one scale"));
    }
    // **按实测纠正**：`serde_json` 把 NaN / ±Infinity 序列化成 `null`，回读时又被拒——
    // 所以"非有限值 → Unknown"这条路**过不了 JSON 报告**。结论仍然保留在规则属性里（固定探针测得），
    // 但作为输入必须直接拒绝，否则一次成功的计算会生成一份读不回来的报告。
    // Measured correction: `serde_json` writes NaN / ±Infinity as `null` and refuses to read it back, so the
    // "non-finite → Unknown" path **cannot cross a JSON report**. The finding stays (measured by fixed probes), but the input
    // must be refused outright, otherwise a successful run would produce a report nobody can read back.
    if request.slacks.iter().chain(request.scales.iter()).any(|value| !value.is_finite()) {
        return Err(BridgeError::new("InvalidInput",
            "activity bands failed validation: slacks and scales must be finite (JSON cannot carry NaN or infinity)"));
    }
    let config = ActivityConfig { tolerance: request.tolerance,
        nearly_active_tolerance: request.nearly_active_tolerance,
        relative_tolerance: request.relative_tolerance, include_variable_bounds: true };
    let accepted = config.validate().is_ok();
    let rows: Vec<ActivityBandRow> = request.slacks.iter().zip(request.scales.iter())
        .map(|(slack, scale)| ActivityBandRow { slack: *slack, scale: *scale,
            active_tolerance: config.effective_tolerance(*scale),
            nearly_active_tolerance: config.effective_nearly_active_tolerance(*scale)
                .max(config.effective_tolerance(*scale)),
            status: status_name(config.classify(*slack, *scale)).to_owned() })
        .collect();
    let findings = canonical_findings();
    Ok(ActivityBandReport { rows,
        scaled_nearly: ActivityConfig::with_tolerance(request.tolerance).nearly_active_tolerance,
        accepted, order_decides_the_boundaries: findings.order_decides_the_boundaries,
        relative_tolerance_lifts_zero_scale: findings.relative_tolerance_lifts_zero_scale,
        nearly_is_lifted_to_active: findings.nearly_is_lifted_to_active,
        non_finite_is_unknown: findings.non_finite_is_unknown,
        with_tolerance_scales_by_ten: findings.with_tolerance_scales_by_ten })
}

/// 五条规则属性的内部固定探针。 / The five rule properties measured by fixed internal probes.
struct Findings {
    order_decides_the_boundaries: bool,
    relative_tolerance_lifts_zero_scale: bool,
    nearly_is_lifted_to_active: bool,
    non_finite_is_unknown: bool,
    with_tolerance_scales_by_ten: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let plain = ActivityConfig::new(1e-7, 1e-5);
    // 1) 恰好 `slack = -active` 是 `Active`，再减一点才是 `Violated`。 
    // 1) Exactly `slack = -active` is `Active`; a bit further is `Violated`.
    let active = plain.effective_tolerance(0.0);
    let order_decides_the_boundaries = plain.classify(0.0, 0.0) == ActivityStatus::Active
        && plain.classify(-active, 0.0) == ActivityStatus::Active
        && plain.classify(-active * 1.5, 0.0) == ActivityStatus::Violated
        && plain.classify(active * 1.5, 0.0) == ActivityStatus::NearlyActive
        && plain.classify(1.0, 0.0) == ActivityStatus::Inactive;
    // 2) 相对容差在 `scale = 0` 时仍按 `max(|0|, 1) = 1` 生效。 
    // 2) The relative tolerance still applies at `scale = 0` via `max(|0|, 1) = 1`.
    let relative = ActivityConfig::new(1e-7, 1e-5).with_relative_tolerance(1e-3);
    let relative_tolerance_lifts_zero_scale = relative.effective_tolerance(0.0) == 1e-3
        && relative.effective_tolerance(0.5) == 1e-3
        && relative.effective_tolerance(10.0) == 1e-2;
    // 3) 相对容差把 active 顶到 nearly 之上后，`NearlyActive` 那条带子消失。 
    // 3) Once the relative tolerance lifts active above nearly, the `NearlyActive` band disappears.
    let lifted = ActivityConfig::new(1e-7, 1e-5).with_relative_tolerance(1.0);
    let nearly_is_lifted_to_active = {
        let active = lifted.effective_tolerance(1.0);
        let nearly = lifted.effective_nearly_active_tolerance(1.0).max(active);
        active == 1.0 && nearly == 1.0
            && lifted.classify(1.5, 1.0) == ActivityStatus::Inactive
            && lifted.classify(0.5, 1.0) == ActivityStatus::Active
    };
    // 4) 非有限值 → `Unknown`（且不看容差）。 / Non-finite → `Unknown`, tolerances are never consulted.
    let non_finite_is_unknown = plain.classify(f64::NAN, 0.0) == ActivityStatus::Unknown
        && plain.classify(f64::INFINITY, 0.0) == ActivityStatus::Unknown
        && plain.classify(0.0, f64::NAN) == ActivityStatus::Unknown
        && plain.classify(0.0, f64::NEG_INFINITY) == ActivityStatus::Unknown;
    // 5) `with_tolerance(t)` 的 nearly 恒为 `10t`。 / `with_tolerance(t)` always gives nearly `10t`.
    let with_tolerance_scales_by_ten = {
        let scaled = ActivityConfig::with_tolerance(2.5e-6);
        // **按实测纠正**：nearly 是 `10 * tolerance`，我第一次拿 `2.5e-7 * 10` 比，比错了对象。 
        // Measured correction: nearly is `10 * tolerance`; my first probe compared against `2.5e-7 * 10`.
        scaled.tolerance == 2.5e-6
            && scaled.nearly_active_tolerance == scaled.tolerance * 10.0
            && scaled.relative_tolerance == 0.0 && scaled.include_variable_bounds
    };
    Findings { order_decides_the_boundaries, relative_tolerance_lifts_zero_scale,
        nearly_is_lifted_to_active, non_finite_is_unknown, with_tolerance_scales_by_ten }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ActivityBandRequest {
        ActivityBandRequest { tolerance: 1e-7, nearly_active_tolerance: 1e-5,
            relative_tolerance: 0.0, slacks: vec![0.0, -1e-7, -1.5e-7, 1.5e-7, 1.0],
            scales: vec![0.0, 0.0, 0.0, 0.0, 0.0] }
    }

    #[test]
    fn the_order_decides_every_boundary() {
        let report = verify_activity_bands(&request()).unwrap();
        let status = |index: usize| report.rows[index].status.as_str();
        // 恰好等于 -active → Active；再往下才 Violated。 
        // Exactly -active → Active; only further down is Violated.
        assert_eq!(status(0), "ACTIVE");
        assert_eq!(status(1), "ACTIVE");
        assert_eq!(status(2), "VIOLATED");
        assert_eq!(status(3), "NEARLY_ACTIVE");
        assert_eq!(status(4), "INACTIVE");
        assert!(report.order_decides_the_boundaries);
        assert!(report.accepted);
    }

    #[test]
    fn the_relative_tolerance_lifts_zero_scale() {
        let mut document = request();
        document.relative_tolerance = 1e-3;
        let report = verify_activity_bands(&document).unwrap();
        // `scale = 0` 也按 `1.0` 放大。 / `scale = 0` is lifted by `1.0` as well.
        assert_eq!(report.rows[0].active_tolerance, 1e-3);
        assert_eq!(report.rows[0].nearly_active_tolerance, 1e-3);
        assert!(report.relative_tolerance_lifts_zero_scale);
        assert!(report.nearly_is_lifted_to_active);
        // 容差被抬高后，`0.0` 仍是 Active，`1.0` 变成 Active（1.0 恰好等于容差）。 
        // With the lifted tolerance `0.0` stays Active and `1.0` becomes Active (exactly at the tolerance).
        assert_eq!(report.rows[0].status, "ACTIVE");
    }

    /// **按实测纠正**：非有限输入必须**拒绝**——报告过不了 JSON，不能让一次成功的运行产出读不回来的结论。
    /// Measured correction: non-finite input must be **refused** — the report cannot cross JSON, and a successful run must
    /// never produce a verdict nobody can read back.
    #[test]
    fn non_finite_input_is_refused_though_the_rule_holds() {
        let mut document = request();
        document.slacks = vec![f64::NAN];
        document.scales = vec![0.0];
        assert!(verify_activity_bands(&document).unwrap_err().message
            .contains("must be finite"));
        let mut infinite = request();
        infinite.slacks = vec![0.0];
        infinite.scales = vec![f64::INFINITY];
        assert!(verify_activity_bands(&infinite).is_err());
        // 规则本身仍然成立（固定探针测得）。 / The rule itself still holds (measured by fixed probes).
        let report = verify_activity_bands(&request()).unwrap();
        assert!(report.non_finite_is_unknown);
    }

    #[test]
    fn with_tolerance_scales_by_ten() {
        let mut document = request();
        document.tolerance = 2.5e-6;
        let report = verify_activity_bands(&document).unwrap();
        assert_eq!(report.scaled_nearly, 2.5e-5);
        assert!(report.with_tolerance_scales_by_ten);
    }

    #[test]
    fn an_invalid_configuration_is_refused_not_panicked() {
        let mut document = request();
        document.nearly_active_tolerance = 0.0;
        let report = verify_activity_bands(&document).unwrap();
        // `nearly < tolerance` 被 `validate()` 拒绝，但判定仍然给出行。 
        // `nearly < tolerance` is refused by `validate()`, yet the rows are still produced.
        assert!(!report.accepted);
        assert!(!report.rows.is_empty());
        let mut negative = request();
        negative.relative_tolerance = -1.0;
        assert!(!verify_activity_bands(&negative).unwrap().accepted);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let report = verify_activity_bands(&ActivityBandRequest { tolerance: 0.5,
            nearly_active_tolerance: 2.0, relative_tolerance: 0.0,
            slacks: vec![-9.0], scales: vec![1.0] }).unwrap();
        assert!(report.order_decides_the_boundaries);
        assert!(report.relative_tolerance_lifts_zero_scale);
        assert!(report.nearly_is_lifted_to_active);
        assert!(report.non_finite_is_unknown);
        assert!(report.with_tolerance_scales_by_ten);
        assert_eq!(report.rows[0].status, "VIOLATED");
    }

    #[test]
    fn bad_input_is_refused() {
        let mut mismatched = request();
        mismatched.scales.pop();
        assert!(verify_activity_bands(&mismatched).unwrap_err().message
            .contains("every slack needs exactly one scale"));
        let mut too_many = request();
        too_many.slacks = vec![0.0; 257];
        too_many.scales = vec![0.0; 257];
        assert!(verify_activity_bands(&too_many).unwrap_err().message
            .contains("at most 256 slacks"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_activity_bands(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"orderDecidesTheBoundaries\""), "{json}");
        assert!(json.contains("\"activeTolerance\""), "{json}");
        assert!(json.contains("\"scaledNearly\""), "{json}");
        assert!(!json.contains("order_decides"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ActivityBandReport>(&copy.to_string()).is_err());
    }
}
