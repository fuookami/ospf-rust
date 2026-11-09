//! 续挖第 122 单：**超体积增量有三段式生命周期，而且"精确"与"采样"各自给出不同的证据**——校准阶段
//! （观测数不足 `calibrationObservations`）返回 **`delta = 0` 且 `calibrated = false`**；校准刚好凑满的那一次
//! 会**自动冻结参考点**并同样返回 `delta = 0`，但 `calibrated = true`；此后 `exact` 模式的增量
//! **非负且带 `work` 计数**，`sampled` 模式给出 `exact = false` 与**标准误/99% Hoeffding 界**；
//! **空候选（全被已有参考支配）给 `delta = 0`**；维度不一致与非有限坐标**直接拒**；`maxWork = 0` 与
//! `samples = 0` 在构造时就被拒；**一次测量失败不覆盖上一次成功的 `lastMeasurement`**
//! （`reward.rs` 的 `HypervolumeDeltaReward` / `HypervolumeMeasurement`）
//! Continuation, delivery 122: **the hypervolume increment has a three-phase lifecycle, and "exact" and "sampled" each bring
//! different evidence** — during calibration (fewer observations than `calibrationObservations`) it returns **`delta = 0` with
//! `calibrated = false`**; the call that completes calibration **freezes an automatic reference** and also returns
//! `delta = 0`, but with `calibrated = true`; afterwards an `exact` increment is **non-negative and carries a `work` count**,
//! while `sampled` reports `exact = false` plus a **standard error and a 99% Hoeffding bound**; **empty candidates** (all
//! dominated by the existing reference) give `delta = 0`; inconsistent dimensions and non-finite coordinates are **refused
//! outright**; `maxWork = 0` and `samples = 0` are refused at construction; and **a failed measurement never overwrites the last
//! successful `lastMeasurement`** (`HypervolumeDeltaReward` / `HypervolumeMeasurement` in `reward.rs`).
//!
//! 第 121 单量的是接受准则，这一单量**超体积增量的三段式生命周期**。八条实测：
//!
//! 1. **校准不足时 `delta = 0` 且 `calibrated = false`**（`the_calibration_phase_reports_zero_uncertain()`）。
//! 2. **凑满校准的那一次冻结参考点且 `calibrated = true`**（`the_calibration_freezes_the_reference()`）。
//! 3. **`exact` 增量非负且记 `work`**（`the_exact_increment_is_nonnegative_and_counted()`）。
//! 4. **`sampled` 给 `exact = false` 与两个误差量**（`the_sampled_increment_carries_uncertainty()`）。
//! 5. **空候选给 `delta = 0`**（`an_empty_increment_is_zero()`）。
//! 6. **维度不一致与非有限坐标被拒**（`bad_coordinates_are_refused()`）。
//! 7. **`maxWork = 0` 与 `samples = 0` 在构造时被拒**（`the_geometry_budget_is_hardcoded()`）。
//! 8. **失败不覆盖上一次成功的测量**（`a_failure_keeps_the_last_measurement()`）。
//!
//! Delivery 121 measured the acceptance policies; this one measures **the hypervolume increment's three-phase lifecycle**.
//! **The calibration phase reports zero and uncertain**, and **the call that fills the calibration freezes the reference**.
//! **The exact increment is non-negative and counted**, while **the sampled increment carries two uncertainty figures**.
//! **An empty increment is zero**, **bad coordinates are refused**, **the geometry budget is hardcoded**, and **a failure keeps
//! the last measurement**.

use ospf_rust_core::solver::hyper_heuristic::reward::{
    HypervolumeConfig, HypervolumeDeltaReward, HypervolumeMode,
};
use ospf_rust_core::solver::hyper_heuristic::{RecoverableComponent, RewardContext, RewardPolicy};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条坐标。 / One coordinate point.
pub type HypervolumePoint = Vec<f64>;

/// 超体积探针请求。 / The hypervolume probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HypervolumeProbeRequest {
    /// 模式（`Exact` / `Sampled`，缺省 `Exact`）。 / The mode.
    #[serde(default)]
    pub mode: Option<String>,
    /// 固定劣参考点；缺省则走自动校准。 / The fixed reference; empty calibrates automatically.
    #[serde(default)]
    pub reference: Vec<f64>,
    /// 几何操作额度（默认 1_000_000）。 / The geometry allowance.
    #[serde(default)]
    pub max_work: Option<u64>,
    /// 采样数（默认 16384）。 / The sample count.
    #[serde(default)]
    pub samples: Option<u32>,
    /// 随机种子（默认 1）。 / The seed.
    #[serde(default)]
    pub seed: Option<i64>,
    /// 自动参考点所需的候选数（默认 32）。 / The calibration requirement.
    #[serde(default)]
    pub calibration_observations: Option<u32>,
    /// 依次测量的三组输入：参考档案、候选。 / The successive measurements.
    #[serde(default)]
    pub measurements: Vec<HypervolumeMeasurementSpec>,
}

/// 一次测量的输入。 / One measurement's input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HypervolumeMeasurementSpec {
    /// 参考档案坐标。 / The archive coordinates.
    #[serde(default)]
    pub reference: Vec<HypervolumePoint>,
    /// 候选坐标。 / The candidate coordinates.
    #[serde(default)]
    pub candidates: Vec<HypervolumePoint>,
}

/// 一次测量的读数。 / One measurement's reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HypervolumeMeasureRow {
    /// 是否成功。 / Whether it succeeded.
    pub ok: bool,
    /// 失败时的错误文本。 / The refusal text.
    pub error: String,
    /// 增量。 / The increment.
    pub delta: f64,
    /// 是否精确。 / Whether exact.
    pub exact: bool,
    /// 实际样本数。 / The sample count.
    pub samples: u32,
    /// 采样标准误。 / The standard error.
    pub standard_error: f64,
    /// 99% Hoeffding 界。 / The 99% Hoeffding bound.
    pub error_bound: f64,
    /// 实际几何操作数。 / The geometry operations used.
    pub work: u64,
    /// 参考点是否已冻结。 / Whether the reference is frozen.
    pub calibrated: bool,
}

/// 超体积探针结论。 / The hypervolume probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HypervolumeProbeReport {
    /// 每次测量的读数。 / Each measurement's reading.
    pub rows: Vec<HypervolumeMeasureRow>,
    /// 最后一次成功测量的增量。 / The last successful increment.
    pub last_delta: f64,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_calibration_phase_reports_zero_uncertain: bool,
    pub the_calibration_freezes_the_reference: bool,
    pub the_exact_increment_is_nonnegative_and_counted: bool,
    pub the_sampled_increment_carries_uncertainty: bool,
    pub an_empty_increment_is_zero: bool,
    pub bad_coordinates_are_refused: bool,
    pub the_geometry_budget_is_hardcoded: bool,
    pub a_failure_keeps_the_last_measurement: bool,
}

/// 造一份配置。 / Build one configuration.
fn config_of(request: &HypervolumeProbeRequest) -> Result<HypervolumeConfig> {
    let defaults = HypervolumeConfig::default();
    let mode = match request.mode.as_deref() {
        None => HypervolumeMode::Exact,
        Some("Exact") => HypervolumeMode::Exact,
        Some("Sampled") => HypervolumeMode::Sampled,
        Some(_) => return Err(BridgeError::new("InvalidInput", "hypervolume probe failed \
            validation: mode reads Exact or Sampled")),
    };
    Ok(HypervolumeConfig {
        reference: request.reference.clone(),
        mode,
        max_work: request.max_work.unwrap_or(defaults.max_work),
        samples: request.samples.unwrap_or(defaults.samples),
        seed: request.seed.unwrap_or(defaults.seed),
        calibration_observations: request.calibration_observations
            .unwrap_or(defaults.calibration_observations),
    })
}

/// 造一次测量的上下文。 / Build one reward context.
fn context_of(spec: &HypervolumeMeasurementSpec) -> RewardContext {
    RewardContext {
        improved: !spec.candidates.is_empty(),
        contribution: 1.0,
        evaluations: 1,
        action: "probe".to_owned(),
        elapsed_nanos: 0,
        reference: spec.reference.clone(),
        candidates: spec.candidates.clone(),
    }
}

/// 一次读数的行。 / One reading row.
fn row_of(ok: bool, error: String, measurement: Option<&ospf_rust_core::solver::hyper_heuristic::reward::HypervolumeMeasurement>) -> HypervolumeMeasureRow {
    match measurement {
        Some(measurement) => HypervolumeMeasureRow {
            ok, error, delta: measurement.delta, exact: measurement.exact,
            samples: measurement.samples, standard_error: measurement.standard_error,
            error_bound: measurement.error_bound, work: measurement.work,
            calibrated: measurement.calibrated,
        },
        None => HypervolumeMeasureRow { ok, error, delta: 0.0, exact: false, samples: 0,
            standard_error: 0.0, error_bound: 0.0, work: 0, calibrated: false },
    }
}

/// 走一遍超体积探针。非法配置是**错误**。 / Walk the probe. An illegal configuration is an error.
pub fn verify_hypervolume_probe(
    request: &HypervolumeProbeRequest,
) -> Result<HypervolumeProbeReport> {
    if request.measurements.len() > 8 || request.reference.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "hypervolume probe failed validation: at \
            most eight measurements and an eight-dimensional reference are accepted"));
    }
    let findings = canonical_findings();
    let mut reward = HypervolumeDeltaReward::new(config_of(request)?)
        .map_err(|error| BridgeError::new("InvalidInput", error))?;
    let mut rows = Vec::with_capacity(request.measurements.len());
    for spec in &request.measurements {
        // 测量失败是**结果**（失败不写状态），所以这里只记错误文本。 
        // A measurement failure is a **result** (it writes no state), so only the text is recorded.
        match reward.measure(&context_of(spec)) {
            Ok(measurement) => rows.push(row_of(true, String::new(), Some(&measurement))),
            Err(error) => rows.push(row_of(false, error.to_string(), None)),
        }
    }
    let last_delta = reward.last_measurement().map(|measurement| measurement.delta)
        .unwrap_or(0.0);
    Ok(HypervolumeProbeReport {
        rows,
        last_delta,
        the_calibration_phase_reports_zero_uncertain: findings.the_calibration_phase_reports_zero_uncertain,
        the_calibration_freezes_the_reference: findings.the_calibration_freezes_the_reference,
        the_exact_increment_is_nonnegative_and_counted:
            findings.the_exact_increment_is_nonnegative_and_counted,
        the_sampled_increment_carries_uncertainty: findings.the_sampled_increment_carries_uncertainty,
        an_empty_increment_is_zero: findings.an_empty_increment_is_zero,
        bad_coordinates_are_refused: findings.bad_coordinates_are_refused,
        the_geometry_budget_is_hardcoded: findings.the_geometry_budget_is_hardcoded,
        a_failure_keeps_the_last_measurement: findings.a_failure_keeps_the_last_measurement,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_calibration_phase_reports_zero_uncertain: bool,
    the_calibration_freezes_the_reference: bool,
    the_exact_increment_is_nonnegative_and_counted: bool,
    the_sampled_increment_carries_uncertainty: bool,
    an_empty_increment_is_zero: bool,
    bad_coordinates_are_refused: bool,
    the_geometry_budget_is_hardcoded: bool,
    a_failure_keeps_the_last_measurement: bool,
}

/// 造一个奖励策略。 / Build a reward policy.
fn reward(config: HypervolumeConfig) -> HypervolumeDeltaReward {
    HypervolumeDeltaReward::new(config).expect("the fixed configuration is valid")
}

/// 造一次上下文。 / Build one context.
fn points(reference: &[f64], candidates: &[f64]) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    (vec![reference.to_vec()], vec![candidates.to_vec()])
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 校准不足：delta 0 且未冻结。 / Incomplete calibration: zero and unfrozen.
        let the_calibration_phase_reports_zero_uncertain = {
            let mut policy = reward(HypervolumeConfig { calibration_observations: 4,
                ..HypervolumeConfig::default() });
            let (reference, candidates) = points(&[0.0], &[1.0]);
            let first = policy.measure(&RewardContext { reference, candidates, ..context_default() })
                .expect("a first calibration step succeeds");
            first.delta == 0.0 && !first.calibrated && first.samples == 0
        };
        // 2) 凑满校准的那一次冻结参考点。 / The call that fills the calibration freezes the reference.
        let the_calibration_freezes_the_reference = {
            let mut policy = reward(HypervolumeConfig { calibration_observations: 3,
                ..HypervolumeConfig::default() });
            let mut calibrated_at = 0;
            for step in 0..3 {
                let (reference, candidates) = points(&[0.0], &[(step + 1) as f64]);
                let measurement = policy.measure(&RewardContext { reference, candidates,
                    ..context_default() }).expect("calibration succeeds");
                if measurement.calibrated { calibrated_at = step + 1; }
            }
            // 前两步未冻结、第三步冻结且 delta 仍为 0。 
            // The first two steps stay unfrozen and the third freezes with a zero delta.
            calibrated_at == 3
                && policy.last_measurement().is_some_and(|value| value.delta == 0.0
                    && value.calibrated)
        };
        // 3) exact 增量非负且记 work。 / The exact increment is non-negative and counted.
        let the_exact_increment_is_nonnegative_and_counted = {
            let mut policy = reward(HypervolumeConfig {
                reference: vec![10.0, 10.0], mode: HypervolumeMode::Exact,
                ..HypervolumeConfig::default() });
            // 最小化坐标：档案 5×5、候选 0×0 → 增量 = 100 - 25 = 75。 
            // Minimisation coordinates: an archive at five and a candidate at zero gain seventy-five.
            let (reference, candidates) = points(&[5.0, 5.0], &[0.0, 0.0]);
            let measurement = policy.measure(&RewardContext { reference, candidates,
                ..context_default() }).expect("exact measurement succeeds");
            measurement.delta >= 0.0 && measurement.exact && measurement.work > 0
                && (measurement.delta - 75.0).abs() < 1e-9
        };
        // 4) sampled 给 exact = false 与两个误差量。 / The sampled increment carries two uncertainty figures.
        let the_sampled_increment_carries_uncertainty = {
            let mut policy = reward(HypervolumeConfig {
                reference: vec![10.0, 10.0], mode: HypervolumeMode::Sampled, samples: 1024,
                ..HypervolumeConfig::default() });
            // 档案占一角、候选占另一角，采样区域被**部分**覆盖 → 标准误非零。 
            // The archive holds one corner and the candidate the other, so the sampled region is only partly covered.
            let measurement = policy.measure(&RewardContext {
                reference: vec![vec![2.0, 8.0]], candidates: vec![vec![8.0, 2.0]],
                ..context_default() }).expect("sampled measurement succeeds");
            !measurement.exact && measurement.samples == 1024 && measurement.calibrated
                && measurement.standard_error > 0.0 && measurement.error_bound > 0.0
                && measurement.delta >= 0.0
        };
        // 5) 空候选给 0。 / An empty increment is zero.
        let an_empty_increment_is_zero = {
            let mut policy = reward(HypervolumeConfig { reference: vec![10.0],
                ..HypervolumeConfig::default() });
            let (reference, candidates) = points(&[0.0], &[5.0]);
            // 候选更差（最小化下更大）→ 没有增量。 / A worse (larger) candidate adds nothing.
            policy.measure(&RewardContext { reference, candidates, ..context_default() })
                .expect("an empty increment succeeds").delta == 0.0
        };
        // 6) 维度不一致与非有限坐标被拒。 / Inconsistent dimensions and non-finite coordinates are refused.
        let bad_coordinates_are_refused = {
            let mut policy = reward(HypervolumeConfig { reference: vec![10.0, 10.0],
                ..HypervolumeConfig::default() });
            let ragged = policy.measure(&RewardContext {
                reference: vec![vec![0.0, 0.0], vec![1.0]], candidates: Vec::new(),
                ..context_default() });
            let infinite = policy.measure(&RewardContext {
                reference: vec![vec![0.0, 0.0]], candidates: vec![vec![f64::INFINITY, 1.0]],
                ..context_default() });
            ragged.is_err() && infinite.is_err()
        };
        // 7) 几何额度在构造时硬拒。 / The geometry budget is refused at construction.
        let the_geometry_budget_is_hardcoded = {
            HypervolumeDeltaReward::new(HypervolumeConfig { max_work: 0,
                ..HypervolumeConfig::default() }).is_err()
                && HypervolumeDeltaReward::new(HypervolumeConfig { samples: 0,
                    ..HypervolumeConfig::default() }).is_err()
                && HypervolumeDeltaReward::new(HypervolumeConfig {
                    calibration_observations: 0, ..HypervolumeConfig::default() }).is_err()
                && HypervolumeDeltaReward::new(HypervolumeConfig {
                    reference: vec![f64::NAN], ..HypervolumeConfig::default() }).is_err()
        };
        // 8) 失败不覆盖上一次成功。 / A failure keeps the last measurement.
        let a_failure_keeps_the_last_measurement = {
            let mut policy = reward(HypervolumeConfig { reference: vec![10.0],
                ..HypervolumeConfig::default() });
            let (reference, candidates) = points(&[5.0], &[0.0]);
            policy.measure(&RewardContext { reference, candidates, ..context_default() })
                .expect("a sound measurement succeeds");
            let before = policy.last_measurement().map(|value| value.delta);
            let failure = policy.measure(&RewardContext { reference: vec![vec![0.0]],
                candidates: vec![vec![f64::NAN]], ..context_default() });
            failure.is_err() && policy.last_measurement().map(|value| value.delta) == before
        };
        Findings {
            the_calibration_phase_reports_zero_uncertain,
            the_calibration_freezes_the_reference,
            the_exact_increment_is_nonnegative_and_counted,
            the_sampled_increment_carries_uncertainty,
            an_empty_increment_is_zero,
            bad_coordinates_are_refused,
            the_geometry_budget_is_hardcoded,
            a_failure_keeps_the_last_measurement,
        }
    })
}

/// 探针用的默认上下文。 / The default context for the probes.
fn context_default() -> RewardContext {
    RewardContext { improved: false, contribution: 0.0, evaluations: 0, action: String::new(),
        elapsed_nanos: 0, reference: Vec::new(), candidates: Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: HypervolumeProbeRequest) -> HypervolumeProbeReport {
        verify_hypervolume_probe(&request).unwrap()
    }

    fn request() -> HypervolumeProbeRequest {
        HypervolumeProbeRequest { mode: None, reference: vec![10.0, 10.0], max_work: None,
            samples: None, seed: None, calibration_observations: None, measurements: Vec::new() }
    }

    fn measurement(reference: Vec<Vec<f64>>, candidates: Vec<Vec<f64>>)
        -> HypervolumeMeasurementSpec
    {
        HypervolumeMeasurementSpec { reference, candidates }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(HypervolumeProbeRequest {
            measurements: vec![measurement(vec![vec![5.0, 5.0]], vec![vec![0.0, 0.0]])],
            ..request() });
        assert_eq!(report.rows.len(), 1);
        let row = &report.rows[0];
        assert!(row.ok, "{}", row.error);
        assert!(row.exact);
        assert!((row.delta - 75.0).abs() < 1e-9, "{}", row.delta);
        assert!(row.work > 0);
        assert!(row.calibrated);
        assert!((report.last_delta - 75.0).abs() < 1e-9);
        assert!(report.the_calibration_phase_reports_zero_uncertain);
        assert!(report.the_calibration_freezes_the_reference);
        assert!(report.the_exact_increment_is_nonnegative_and_counted);
        assert!(report.the_sampled_increment_carries_uncertainty);
        assert!(report.an_empty_increment_is_zero);
        assert!(report.bad_coordinates_are_refused);
        assert!(report.the_geometry_budget_is_hardcoded);
        assert!(report.a_failure_keeps_the_last_measurement);
    }

    #[test]
    fn the_calibration_phase_reports_zero_then_freezes() {
        let report = ask(HypervolumeProbeRequest {
            reference: Vec::new(), calibration_observations: Some(3),
            measurements: (1..=3).map(|step| measurement(vec![vec![0.0]],
                vec![vec![step as f64]])).collect(), ..request() });
        // 前两步未冻结、delta 0；第三步冻结、delta 仍 0。 
        // The first two stay unfrozen with a zero delta; the third freezes and still reports zero.
        assert_eq!(report.rows[0].delta, 0.0);
        assert!(!report.rows[0].calibrated);
        assert_eq!(report.rows[1].delta, 0.0);
        assert!(!report.rows[1].calibrated);
        assert_eq!(report.rows[2].delta, 0.0);
        assert!(report.rows[2].calibrated);
        assert_eq!(report.last_delta, 0.0);
    }

    #[test]
    fn the_sampled_mode_carries_uncertainty() {
        let report = ask(HypervolumeProbeRequest { mode: Some("Sampled".to_owned()),
            samples: Some(1024),
            measurements: vec![measurement(vec![vec![2.0, 8.0]], vec![vec![8.0, 2.0]])],
            ..request() });
        let row = &report.rows[0];
        assert!(row.ok, "{}", row.error);
        assert!(!row.exact);
        assert_eq!(row.samples, 1024);
        assert!(row.standard_error > 0.0);
        assert!(row.error_bound > 0.0);
        assert!(row.delta >= 0.0);
    }

    #[test]
    fn bad_coordinates_are_reported_as_a_row() {
        let report = ask(HypervolumeProbeRequest {
            measurements: vec![
                measurement(vec![vec![0.0, 0.0], vec![1.0]], Vec::new()),
                measurement(vec![vec![0.0, 0.0]], vec![vec![f64::INFINITY, 1.0]]),
            ], ..request() });
        assert_eq!(report.rows.len(), 2);
        assert!(!report.rows[0].ok);
        assert!(report.rows[0].error.contains("invalid hypervolume coordinate"), "{}",
            report.rows[0].error);
        assert!(!report.rows[1].ok);
    }

    #[test]
    fn bad_configurations_are_refused() {
        assert!(verify_hypervolume_probe(&HypervolumeProbeRequest { mode: Some("Nearly".to_owned()),
            ..request() }).unwrap_err().message.contains("mode reads Exact or Sampled"));
        assert!(verify_hypervolume_probe(&HypervolumeProbeRequest { max_work: Some(0),
            ..request() }).unwrap_err().message.contains("invalid hypervolume parameters"));
        assert!(verify_hypervolume_probe(&HypervolumeProbeRequest { samples: Some(0),
            ..request() }).unwrap_err().message.contains("invalid hypervolume parameters"));
        assert!(verify_hypervolume_probe(&HypervolumeProbeRequest {
            calibration_observations: Some(0), ..request() }).unwrap_err()
            .message.contains("invalid hypervolume parameters"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(HypervolumeProbeRequest {
            measurements: vec![measurement(vec![vec![5.0, 5.0]], vec![vec![0.0, 0.0]])],
            ..request() });
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"lastDelta\""), "{json}");
        assert!(json.contains("\"standardError\""), "{json}");
        assert!(json.contains("\"theCalibrationFreezesTheReference\""), "{json}");
        assert!(!json.contains("last_delta"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<HypervolumeProbeReport>(&copy.to_string()).is_err());
    }
}
