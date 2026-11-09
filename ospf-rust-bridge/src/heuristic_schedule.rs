//! 批次 G 第 1 单：启发式算子的**参数调度曲线**采样（算子本身不绑）/ Batch G, delivery 1: sampling the parameter
//! schedules of heuristic operators (the operators themselves stay unbound).
//!
//! 启发式算子里真正可搬运的一层是**参数演化源**（`operator/evolution.rs` 的 `ParameterSource`）：温度、变异尺度、交叉率
//! 这些标量是按迭代数变化的，而它们的公式是纯的、与模型无关。这一单把这条曲线采出来交给 TS——**不绑任何算子**，
//! 不建种群、不评估适应度。
//!
//! 五种形态，公式都来自领域实现（不是在这里重写）：
//! * `constant`：`value`；
//! * `linear`：`from + (to - from) * clamp(i / limit, 0, 1)`，`limit = 0` 时进度直接取 1（即恒为 `to`）；
//! * `geometric`：`initial * ratio^i`；
//! * `exponential`：`initial * exp(-rate * i)`；
//! * `jittered`：几何调度 + `amplitude * (2 * u - 1)` 的对称抖动，`u` 来自**种子化**熵源。
//!
//! 采样用的是领域自己的 `SeededEntropy`（与 Kotlin 侧同一个 LCG），所以**同一 seed 得到同一条曲线**，TS 侧可以据此
//! 复现或做前后端比对。报告给出首末值、极值与是否全部有限。
//!
//! 采样点数被限制在 1..=4096：这是**信任边界上的限制**——载荷不该能要求十亿个采样点。
//!
//! What is portable inside a heuristic solver really is the **parameter evolution source**: temperature, mutation scale and
//! crossover rate are scalars that vary with the iteration count, and their formulas are pure and model-independent. This
//! delivery samples that curve for TypeScript and **binds no operator**: no population is built and no fitness is evaluated.
//! Five shapes, all using the domain's own formulas: `constant`; `linear` (`from + (to - from) * clamp(i / limit, 0, 1)`,
//! with `limit = 0` collapsing to the endpoint `to`); `geometric` (`initial * ratio^i`); `exponential`
//! (`initial * exp(-rate * i)`); and `jittered` (a geometric schedule plus a symmetric `amplitude * (2u - 1)` jitter whose
//! `u` comes from a **seeded** entropy source). Sampling uses the domain's own `SeededEntropy` (the same LCG as the Kotlin
//! side), so **one seed yields one curve** that TypeScript can reproduce or compare against. The report gives the first and
//! last values, the extremes and whether everything stayed finite. The sample count is capped at 1..=4096 — a **trust-boundary**
//! limit, since a payload must not be able to request a billion samples.

use ospf_rust_core::solver::heuristic::operator::evolution::{
    ConstantSource, ExponentialSource, GeometricSource, JitteredSource, LinearSource, ParameterSource,
};
use ospf_rust_core::solver::heuristic::{Iteration, SeededEntropy};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 曲线采样的请求。 / The curve-sampling request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeuristicScheduleRequest {
    /// 演化源形态：`constant` / `linear` / `geometric` / `exponential` / `jittered`。
    /// The evolution-source shape.
    pub kind: String,
    /// `constant`：常数值。 / `constant`: the value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<f64>,
    /// `linear`：起始值。 / `linear`: the start value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub from: Option<f64>,
    /// `linear`：结束值。 / `linear`: the end value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub to: Option<f64>,
    /// `linear`：迭代上限（`0` 表示恒为 `to`）。 / `linear`: the iteration limit (`0` means always `to`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<u64>,
    /// `geometric` / `exponential`：初始值。 / `geometric` / `exponential`: the initial value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub initial: Option<f64>,
    /// `geometric`：公比。 / `geometric`: the common ratio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ratio: Option<f64>,
    /// `exponential`：衰减率。 / `exponential`: the decay rate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rate: Option<f64>,
    /// `jittered`：抖动幅度（几何基准固定为 `initial` / `ratio`）。 / `jittered`: the jitter amplitude.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub amplitude: Option<f64>,
    /// 抖动熵源的种子（缺省用领域默认种子）。 / The jitter entropy seed (the domain default when omitted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub seed: Option<u64>,
    /// 采样点数（1..=4096）。 / The sample count (1..=4096).
    pub samples: u32,
}

/// 一个采样点。 / One sample point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeuristicScheduleSample {
    /// 迭代序号（从 0 开始）。 / The iteration index (0-based).
    pub iteration: u64,
    /// 该迭代的参数值。 / The parameter value at that iteration.
    pub value: f64,
}

/// 曲线采样的结论。 / The curve-sampling verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeuristicScheduleReport {
    /// 演化源形态（线格式代码）。 / The shape (wire code).
    pub kind: String,
    /// 实际使用的种子（缺省时是领域默认种子，抖动之外也可能被读取到）。
    /// The seed actually used (the domain default when omitted; it may be read even outside jitter).
    pub seed: u64,
    /// 采样点数。 / The sample count.
    pub sample_count: u32,
    /// 采样点。 / The sample points.
    pub samples: Vec<HeuristicScheduleSample>,
    /// 首个采样值。 / The first sampled value.
    pub first_value: f64,
    /// 最后一个采样值。 / The last sampled value.
    pub last_value: f64,
    /// 最小采样值。 / The smallest sampled value.
    pub min_value: f64,
    /// 最大采样值。 / The largest sampled value.
    pub max_value: f64,
    /// 是否所有采样值都有限。 / Whether every sampled value is finite.
    pub all_finite: bool,
    /// 形态词表（桥接定义）。 / The shape vocabulary (bridge-defined).
    pub kind_vocabulary: Vec<String>,
}

/// 采样点数上限：信任边界，载荷不该能要求十亿个点。
/// The sample cap: a trust boundary, since a payload must not be able to request a billion points.
const MAX_SAMPLES: u32 = 4096;

/// 采样一条参数曲线。非法参数是**错误**，原因原样带出。
/// Sample one parameter curve. An illegal parameter is an **error** carrying the reason.
pub fn sample_heuristic_schedule(request: &HeuristicScheduleRequest) -> Result<HeuristicScheduleReport> {
    if request.samples == 0 || request.samples > MAX_SAMPLES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "heuristic schedule failed validation: sample count must be between 1 and {MAX_SAMPLES}")));
    }
    // 非有限数值在 JSON 里会变成 `null`，只能在 DTO 边界拦。
    // Non-finite numbers become `null` in JSON, so they can only be refused at the DTO boundary.
    for (label, value) in [("value", request.value), ("from", request.from), ("to", request.to),
        ("initial", request.initial), ("ratio", request.ratio), ("rate", request.rate),
        ("amplitude", request.amplitude)]
    {
        if value.is_some_and(|value| !value.is_finite()) {
            return Err(BridgeError::new("InvalidInput", &format!(
                "heuristic schedule failed validation: `{label}` must be finite")));
        }
    }
    let seed = request.seed.unwrap_or_else(default_seed);
    let mut entropy = SeededEntropy::new(seed);
    let mut source = build_source(request)?;
    let mut samples = Vec::with_capacity(usize::try_from(request.samples).unwrap_or(0));
    let mut min_value = f64::INFINITY;
    let mut max_value = f64::NEG_INFINITY;
    let mut all_finite = true;
    for index in 0..u64::from(request.samples) {
        let mut iteration = Iteration::with_counts(index, 0);
        let value = source.value(&mut iteration, &mut entropy);
        all_finite &= value.is_finite();
        if value.is_finite() {
            min_value = min_value.min(value);
            max_value = max_value.max(value);
        }
        samples.push(HeuristicScheduleSample { iteration: index, value });
    }
    let first_value = samples.first().map_or(f64::NAN, |sample| sample.value);
    let last_value = samples.last().map_or(f64::NAN, |sample| sample.value);
    if !all_finite {
        // 非有限采样值无法给出极值，直接拒绝而不是报一个假区间。
        // A non-finite sample leaves no usable extremes, so the request is refused rather than given a fake range.
        return Err(BridgeError::new("InvalidInput", &format!(
            "heuristic schedule failed validation: the `{}` curve leaves the finite range", request.kind)));
    }
    Ok(HeuristicScheduleReport {
        kind: request.kind.clone(),
        seed,
        sample_count: request.samples,
        samples,
        first_value,
        last_value,
        min_value,
        max_value,
        all_finite,
        kind_vocabulary: vec!["constant".to_owned(), "linear".to_owned(), "geometric".to_owned(),
            "exponential".to_owned(), "jittered".to_owned()],
    })
}

/// 按形态构造领域演化源。 / Build the domain evolution source for the requested shape.
fn build_source(request: &HeuristicScheduleRequest) -> Result<Box<dyn ParameterSource>> {
    let required = |value: Option<f64>, label: &str| -> Result<f64> {
        value.ok_or_else(|| BridgeError::new("InvalidInput", &format!(
            "heuristic schedule failed validation: `{}` requires `{label}`", request.kind)))
    };
    Ok(match request.kind.as_str() {
        "constant" => Box::new(ConstantSource { value: required(request.value, "value")? }),
        "linear" => Box::new(LinearSource::new(required(request.from, "from")?,
            required(request.to, "to")?, request.limit.unwrap_or(0))),
        "geometric" => Box::new(GeometricSource::new(required(request.initial, "initial")?,
            required(request.ratio, "ratio")?)),
        "exponential" => Box::new(ExponentialSource::new(required(request.initial, "initial")?,
            required(request.rate, "rate")?)),
        // 抖动源以几何调度为基准：这是领域 `MetropolisAcceptance::default()` 用的同一族形态。
        // The jittered source uses a geometric base, the same family `MetropolisAcceptance::default()` uses.
        "jittered" => Box::new(JitteredSource::new(
            Box::new(GeometricSource::new(required(request.initial, "initial")?,
                required(request.ratio, "ratio")?)),
            required(request.amplitude, "amplitude")?)),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "heuristic schedule failed validation: unknown shape `{other}`"))),
    })
}

/// 领域默认种子（与 `SeededEntropy::default` 一致）。 / The domain default seed.
fn default_seed() -> u64 { SeededEntropy::default().state() }

#[cfg(test)]
mod tests {
    use super::*;

    fn request(kind: &str, samples: u32) -> HeuristicScheduleRequest {
        HeuristicScheduleRequest { kind: kind.to_owned(), value: None, from: None, to: None,
            limit: None, initial: None, ratio: None, rate: None, amplitude: None, seed: None,
            samples }
    }

    #[test]
    fn closed_form_curves_match_the_domain_formulas() {
        // constant：恒等。 / constant: constant.
        let mut constant = request("constant", 3);
        constant.value = Some(2.5);
        let report = curve(&constant);
        assert_eq!(report.first_value, 2.5);
        assert_eq!(report.last_value, 2.5);
        assert!(report.all_finite);

        // linear：`from + (to - from) * clamp(i / limit, 0, 1)`。
        let mut linear = request("linear", 5);
        linear.from = Some(0.0);
        linear.to = Some(1.0);
        linear.limit = Some(4);
        let report = curve(&linear);
        let values: Vec<f64> = report.samples.iter().map(|sample| sample.value).collect();
        assert_eq!(values, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
        assert_eq!(report.min_value, 0.0);
        assert_eq!(report.max_value, 1.0);

        // 进度被夹到 1：超过上限的迭代停在上限值。 / Progress clamps to one: later iterations hold the endpoint.
        let mut overrun = request("linear", 6);
        overrun.from = Some(0.0);
        overrun.to = Some(1.0);
        overrun.limit = Some(2);
        let report = curve(&overrun);
        let values: Vec<f64> = report.samples.iter().map(|sample| sample.value).collect();
        assert_eq!(values, vec![0.0, 0.5, 1.0, 1.0, 1.0, 1.0]);

        // `limit = 0` 时进度直接取 1，也就是恒为 `to`。 / `limit = 0` collapses to the endpoint.
        let mut collapsed = request("linear", 3);
        collapsed.from = Some(9.0);
        collapsed.to = Some(4.0);
        collapsed.limit = Some(0);
        let report = curve(&collapsed);
        assert!(report.samples.iter().all(|sample| sample.value == 4.0));

        // geometric：`initial * ratio^i`。 / geometric: `initial * ratio^i`.
        let mut geometric = request("geometric", 3);
        geometric.initial = Some(100.0);
        geometric.ratio = Some(0.5);
        let report = curve(&geometric);
        let values: Vec<f64> = report.samples.iter().map(|sample| sample.value).collect();
        assert_eq!(values, vec![100.0, 50.0, 25.0]);

        // exponential：`initial * exp(-rate * i)`。 / exponential: `initial * exp(-rate * i)`.
        let mut exponential = request("exponential", 3);
        exponential.initial = Some(1.0);
        exponential.rate = Some(1.0);
        let report = curve(&exponential);
        let values: Vec<f64> = report.samples.iter().map(|sample| sample.value).collect();
        assert!((values[0] - 1.0).abs() < 1e-12);
        assert!((values[1] - (-1.0f64).exp()).abs() < 1e-12);
        assert!((values[2] - (-2.0f64).exp()).abs() < 1e-12);
    }

    #[test]
    fn jitter_is_seeded_and_reproducible() {
        let mut jittered = request("jittered", 8);
        jittered.initial = Some(10.0);
        jittered.ratio = Some(1.0);
        jittered.amplitude = Some(0.5);
        jittered.seed = Some(7);
        let first = curve(&jittered);
        let again = curve(&jittered);
        // 同一种子 → 同一条曲线。 / One seed yields one curve.
        assert_eq!(first.samples, again.samples);
        assert_eq!(first.seed, 7);
        // 抖动确实偏离基准 10，且落在 ±amplitude 之内。 / The jitter leaves the base value but stays inside the amplitude.
        assert!(first.samples.iter().any(|sample| sample.value != 10.0));
        assert!(first.samples.iter().all(|sample| (sample.value - 10.0).abs() <= 0.5 + 1e-12));
        // 换种子 → 换曲线。 / A different seed yields a different curve.
        jittered.seed = Some(8);
        let other = curve(&jittered);
        assert_ne!(first.samples, other.samples);
        // 缺省种子是领域默认值。 / The omitted seed is the domain default.
        jittered.seed = None;
        let defaulted = curve(&jittered);
        assert_eq!(defaulted.seed, 20_240_521);
    }

    #[test]
    fn illegal_parameters_are_refused() {
        // 采样点数必须在 1..=4096。 / The sample count must be within 1..=4096.
        let mut zero = request("constant", 0);
        zero.value = Some(1.0);
        assert!(sample_heuristic_schedule(&zero).unwrap_err().message.contains("between 1 and 4096"));
        let too_many = request("constant", MAX_SAMPLES + 1);
        assert!(sample_heuristic_schedule(&too_many).unwrap_err().message.contains("between 1 and 4096"));
        // 缺参数 / 非有限参数。 / A missing or non-finite parameter.
        assert!(sample_heuristic_schedule(&request("constant", 2)).unwrap_err()
            .message.contains("requires `value`"));
        let mut infinite = request("geometric", 2);
        infinite.initial = Some(f64::INFINITY);
        infinite.ratio = Some(0.5);
        assert!(sample_heuristic_schedule(&infinite).unwrap_err().message.contains("must be finite"));
        // 未知形态。 / An unknown shape.
        assert!(sample_heuristic_schedule(&request("simulated", 2)).unwrap_err()
            .message.contains("unknown shape"));
        // 曲线越出有限范围：拒绝而不是报假极值。 / A curve leaving the finite range is refused, not given a fake range.
        let mut overflow = request("geometric", 4096);
        overflow.initial = Some(1e300);
        overflow.ratio = Some(1e10);
        let error = sample_heuristic_schedule(&overflow).unwrap_err();
        assert!(error.message.contains("leaves the finite range"), "{}", error.message);
    }

    /// 测试别名：合法的输入直接取结论。 / Test alias: a legal input unwraps the verdict.
    fn curve(request: &HeuristicScheduleRequest) -> HeuristicScheduleReport {
        sample_heuristic_schedule(request).expect("legal heuristic schedule")
    }
}
