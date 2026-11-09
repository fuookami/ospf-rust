//! 批次 C 第 2 单：奖励策略构造器的纯校验与可恢复状态 / Batch C, delivery 2: reward-policy constructors and restorable state.
//!
//! 三个奖励策略的 `new` 都是**纯校验**，不执行任何业务目标，也不需要求解器：
//! * `TimeNormalizedReward::new(epsilon)` —— 最短计费时长必须**有限且为正**；
//! * `ExtremeValueReward::new(size)` —— 窗口必须**非空且不超过 `i32::MAX`**；
//! * `HypervolumeDeltaReward::new(config)` —— 参考点每个分量必须**有限**、`maxWork` 非零且不超过 `i64::MAX`、
//!   `samples` 非零且不超过 `i32::MAX`、`calibrationObservations` 非零且不超过 `i32::MAX`。
//!
//! 超体积策略还带一份**真实的可恢复状态**（`RecoverableComponent::save`）：里面有定格的参考点、自动校准的
//! `minimum`/`maximum` 与定义指纹 `definition`。这份文本是 TS 侧唯一能拿到的、可用于续跑的状态形状，所以一并返回。
//! 新建实例的 `lastMeasurement` 一定是空的（还没测过），`calibrated` 也一定是 `false`。
//!
//! The three `new` constructors are **pure validation**: no business objective is evaluated and no solver is involved.
//! `TimeNormalizedReward` needs a finite positive minimum duration; `ExtremeValueReward` needs a non-empty window no larger
//! than `i32::MAX`; and `HypervolumeDeltaReward` needs every reference component finite, a non-zero `maxWork` up to
//! `i64::MAX`, and non-zero `samples` / `calibrationObservations` up to `i32::MAX`. The hypervolume policy also carries a
//! **real restorable state** through `RecoverableComponent::save`: the frozen reference, the auto-calibrated minimum and
//! maximum, and the `definition` fingerprint. That text is the only resumable state shape TypeScript can obtain, so it is
//! returned too. A freshly built instance always has an empty `lastMeasurement` (nothing measured yet) and
//! `calibrated == false`.

use ospf_rust_core::solver::hyper_heuristic::components::RecoverableComponent;
use ospf_rust_core::solver::hyper_heuristic::reward::{
    ExtremeValueReward, HypervolumeConfig, HypervolumeDeltaReward, HypervolumeMode, TimeNormalizedReward,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 时间归一化策略输入。 / The time-normalized input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimeNormalizedInput {
    /// 最短计费时长（秒），必须有限且为正。 / The minimum charged duration in seconds; finite and positive.
    pub epsilon_seconds: f64,
}

/// 极值窗口策略输入。 / The extreme-value window input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtremeValueInput {
    /// 窗口容量，必须非零且不超过 `i32::MAX`。 / The window capacity; non-zero and at most `i32::MAX`.
    pub size: u64,
}

/// 超体积策略输入。 / The hypervolume policy input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HypervolumeConfigInput {
    /// 固定劣参考点；空表示自动校准后冻结。 / The fixed worse reference; empty means calibrate then freeze.
    #[serde(default)]
    pub reference: Vec<f64>,
    /// 计算模式：`exact` 或 `sampled`。 / The computation mode: `exact` or `sampled`.
    #[serde(default = "default_mode")]
    pub mode: String,
    /// 每次调用的几何操作额度；非零且不超过 `i64::MAX`。 / The per-call geometry allowance.
    pub max_work: u64,
    /// 采样模式样本数；非零且不超过 `i32::MAX`。 / The sample count; non-zero and at most `i32::MAX`.
    pub samples: u32,
    /// 每次积分重建的共同随机种子。 / The common seed reset for each integration.
    #[serde(default = "default_seed")]
    pub seed: i64,
    /// 自动参考点所需有效候选数；非零且不超过 `i32::MAX`。 / The candidates required for an automatic reference.
    pub calibration_observations: u32,
}

/// 默认模式。 / The default mode.
fn default_mode() -> String { "exact".to_owned() }
/// 默认种子。 / The default seed.
fn default_seed() -> i64 { 1 }

/// 奖励策略校验请求：三个策略都可省略，但至少要给一个。 / The request; every policy is optional but at least one is required.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RewardPolicyRequest {
    /// 时间归一化策略。 / The time-normalized policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub time_normalized: Option<TimeNormalizedInput>,
    /// 极值窗口策略。 / The extreme-value window policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub extreme_value: Option<ExtremeValueInput>,
    /// 超体积策略。 / The hypervolume policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hypervolume: Option<HypervolumeConfigInput>,
}

/// 超体积策略的回显与状态。 / The hypervolume echo and state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HypervolumeRow {
    /// 参考点（空表示自动校准）。 / The reference (empty means automatic calibration).
    pub reference: Vec<f64>,
    /// 计算模式（`Exact` / `Sampled`）。 / The computation mode.
    pub mode: String,
    /// 几何操作额度。 / The geometry allowance.
    pub max_work: u64,
    /// 样本数。 / The sample count.
    pub samples: u32,
    /// 共同随机种子。 / The common seed.
    pub seed: i64,
    /// 自动参考点所需候选数。 / The candidates required for an automatic reference.
    pub calibration_observations: u32,
    /// 可恢复状态文本（`RecoverableComponent::save` 的真实输出）。
    /// The restorable state text (the real `RecoverableComponent::save` output).
    pub state_json: String,
    /// 是否已有测量结论；新建实例恒为 `false`。 / Whether a measurement exists; always `false` for a fresh instance.
    pub has_measurement: bool,
}

/// 奖励策略校验结论。 / The reward-policy verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RewardPolicyReport {
    /// 已校验的策略个数。 / How many policies were validated.
    pub validated_count: u32,
    /// 时间归一化的最短计费时长回显。 / The echoed minimum duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub time_normalized_epsilon_seconds: Option<f64>,
    /// 极值窗口容量回显。 / The echoed window capacity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub extreme_value_size: Option<u64>,
    /// 超体积策略回显与状态。 / The hypervolume echo and state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hypervolume: Option<HypervolumeRow>,
}

/// 校验奖励策略构造参数。非法输入是**错误**，原因原样带出（并指明是哪个策略）。
/// Validate reward-policy constructor parameters. An illegal input is an **error** carrying the reason verbatim (and
/// naming the policy).
pub fn verify_reward_policy(request: &RewardPolicyRequest) -> Result<RewardPolicyReport> {
    if request.time_normalized.is_none() && request.extreme_value.is_none() && request.hypervolume.is_none() {
        return Err(BridgeError::new("InvalidInput",
            "reward verification requires at least one reward policy"));
    }
    let mut validated = 0u32;
    let mut epsilon = None;
    if let Some(input) = request.time_normalized.as_ref() {
        // 领域的构造器：只在 `epsilon` 有限且为正时成功。
        // The domain constructor: it only succeeds for a finite positive `epsilon`.
        TimeNormalizedReward::new(input.epsilon_seconds).map_err(|error| {
            BridgeError::new("InvalidInput", &format!("time-normalized reward failed validation: {error}"))
        })?;
        epsilon = Some(input.epsilon_seconds);
        validated += 1;
    }
    let mut size = None;
    if let Some(input) = request.extreme_value.as_ref() {
        let window = usize::try_from(input.size).map_err(|_| BridgeError::new("InvalidInput",
            "extreme-value window failed validation: window exceeds the platform index range"))?;
        ExtremeValueReward::new(window).map_err(|error| {
            BridgeError::new("InvalidInput", &format!("extreme-value reward failed validation: {error}"))
        })?;
        size = Some(input.size);
        validated += 1;
    }
    let mut hypervolume = None;
    if let Some(input) = request.hypervolume.as_ref() {
        let mode = match input.mode.as_str() {
            "exact" => HypervolumeMode::Exact,
            "sampled" => HypervolumeMode::Sampled,
            other => return Err(BridgeError::new("InvalidInput", &format!(
                "hypervolume reward failed validation: unsupported mode `{other}` (expected exact or sampled)"))),
        };
        let config = HypervolumeConfig {
            reference: input.reference.clone(),
            mode,
            max_work: input.max_work,
            samples: input.samples,
            seed: input.seed,
            calibration_observations: input.calibration_observations,
        };
        let reward = HypervolumeDeltaReward::new(config).map_err(|error| {
            BridgeError::new("InvalidInput", &format!("hypervolume reward failed validation: {error}"))
        })?;
        // `save()` 是可恢复状态的真实形状：参考点、自动校准区间与 `definition` 指纹都在里面。
        // `save()` is the real restorable state shape: the reference, the auto-calibrated envelope and the `definition`
        // fingerprint all live in it.
        let state_json = reward.save().map_err(|error| {
            BridgeError::new("CodecFailure", &format!("hypervolume state could not be encoded: {error}"))
        })?;
        hypervolume = Some(HypervolumeRow {
            reference: input.reference.clone(),
            mode: mode_name(mode),
            max_work: input.max_work,
            samples: input.samples,
            seed: input.seed,
            calibration_observations: input.calibration_observations,
            state_json: serde_json::to_string(&state_json).map_err(|error| {
                BridgeError::new("CodecFailure", &format!("hypervolume state text could not be encoded: {error}"))
            })?,
            has_measurement: reward.last_measurement().is_some(),
        });
        validated += 1;
    }
    Ok(RewardPolicyReport {
        validated_count: validated,
        time_normalized_epsilon_seconds: epsilon,
        extreme_value_size: size,
        hypervolume,
    })
}

/// 模式名。 / The mode name.
fn mode_name(mode: HypervolumeMode) -> String {
    match mode {
        HypervolumeMode::Exact => "Exact".to_owned(),
        HypervolumeMode::Sampled => "Sampled".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RewardPolicyRequest {
        RewardPolicyRequest { time_normalized: None, extreme_value: None, hypervolume: None }
    }

    #[test]
    fn all_three_policies_validate_and_echo_their_parameters() {
        let report = verify_reward_policy(&RewardPolicyRequest {
            time_normalized: Some(TimeNormalizedInput { epsilon_seconds: 0.001 }),
            extreme_value: Some(ExtremeValueInput { size: 32 }),
            hypervolume: Some(HypervolumeConfigInput {
                reference: vec![10.0, 10.0], mode: "sampled".to_owned(), max_work: 1_000_000,
                samples: 16384, seed: 7, calibration_observations: 32,
            }),
        }).unwrap();
        assert_eq!(report.validated_count, 3);
        assert_eq!(report.time_normalized_epsilon_seconds, Some(0.001));
        assert_eq!(report.extreme_value_size, Some(32));
        let hypervolume = report.hypervolume.unwrap();
        assert_eq!(hypervolume.reference, vec![10.0, 10.0]);
        assert_eq!(hypervolume.mode, "Sampled");
        assert_eq!(hypervolume.max_work, 1_000_000);
        assert_eq!(hypervolume.samples, 16384);
        assert_eq!(hypervolume.seed, 7);
        // 新建实例没测过：恒为 `false`。 / A fresh instance has not measured: always `false`.
        assert!(!hypervolume.has_measurement);
        // 状态文本是真实的 JSON（是一个字符串化的 JSON 对象）。
        // The state text is real JSON (a JSON object encoded as a string).
        let state: serde_json::Value = serde_json::from_str(&hypervolume.state_json).unwrap();
        assert_eq!(state["version"], 1);
        assert_eq!(state["reference"], serde_json::json!([10.0, 10.0]));
        assert_eq!(state["observations"], 0);
        assert!(state["measurement"].is_null());
        // `definition` 指纹包含参考点与全部数值参数。 / The `definition` fingerprint covers the reference and every number.
        let definition = state["definition"].as_str().unwrap();
        assert!(definition.contains("10.0, 10.0"), "{definition}");
        assert!(definition.contains("Sampled"), "{definition}");
    }

    #[test]
    fn each_policy_refuses_its_own_illegal_parameters() {
        // 最短计费时长：零、负数、非有限都拒绝。 / The minimum duration: zero, negative and non-finite are refused.
        for epsilon in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let error = verify_reward_policy(&RewardPolicyRequest {
                time_normalized: Some(TimeNormalizedInput { epsilon_seconds: epsilon }),
                extreme_value: None, hypervolume: None,
            }).unwrap_err();
            assert_eq!(error.code, "InvalidInput");
            assert!(error.message.contains("time-normalized reward failed validation"), "{}", error.message);
            // 领域原文是 `invalid minimum reward duration`。 / The domain message is `invalid minimum reward duration`.
            assert!(error.message.contains("minimum reward duration"), "{}", error.message);
        }

        // 极值窗口：零与超过 `i32::MAX` 都拒绝。 / The window: zero and above `i32::MAX` are refused.
        for size in [0u64, i32::MAX as u64 + 1] {
            let error = verify_reward_policy(&RewardPolicyRequest {
                time_normalized: None, extreme_value: Some(ExtremeValueInput { size }), hypervolume: None,
            }).unwrap_err();
            assert!(error.message.contains("extreme-value reward failed validation"), "{}", error.message);
            assert!(error.message.contains("extreme-value window"), "{}", error.message);
        }
        // 边界值 `i32::MAX` 本身合法。 / The boundary value `i32::MAX` itself is legal.
        assert_eq!(verify_reward_policy(&RewardPolicyRequest {
            time_normalized: None, extreme_value: Some(ExtremeValueInput { size: i32::MAX as u64 }),
            hypervolume: None,
        }).unwrap().extreme_value_size, Some(i32::MAX as u64));

        // 超体积：参考点非有限、额度为零、样本为零、校准观测为零都拒绝。
        // Hypervolume: a non-finite reference, a zero allowance, zero samples and zero calibration candidates are refused.
        let legal = HypervolumeConfigInput {
            reference: vec![1.0], mode: "exact".to_owned(), max_work: 100,
            samples: 1, seed: 1, calibration_observations: 1,
        };
        let cases = [
            (HypervolumeConfigInput { reference: vec![f64::INFINITY], ..legal.clone() }, "hypervolume"),
            (HypervolumeConfigInput { reference: vec![f64::NAN], ..legal.clone() }, "hypervolume"),
            (HypervolumeConfigInput { max_work: 0, ..legal.clone() }, "hypervolume"),
            (HypervolumeConfigInput { samples: 0, ..legal.clone() }, "hypervolume"),
            (HypervolumeConfigInput { calibration_observations: 0, ..legal.clone() }, "hypervolume"),
        ];
        for (input, label) in cases {
            let error = verify_reward_policy(&RewardPolicyRequest {
                time_normalized: None, extreme_value: None, hypervolume: Some(input),
            }).unwrap_err();
            assert_eq!(error.code, "InvalidInput");
            assert!(error.message.contains("hypervolume reward failed validation"), "{label}: {}", error.message);
            assert!(error.message.contains("invalid hypervolume parameters"), "{label}: {}", error.message);
        }
        // 未知模式 / 一个策略都没给。 / An unknown mode and no policy at all.
        let error = verify_reward_policy(&RewardPolicyRequest {
            time_normalized: None, extreme_value: None,
            hypervolume: Some(HypervolumeConfigInput { mode: "approximate".to_owned(), ..legal.clone() }),
        }).unwrap_err();
        assert!(error.message.contains("unsupported mode"), "{}", error.message);
        let error = verify_reward_policy(&request()).unwrap_err();
        assert!(error.message.contains("at least one reward policy"), "{}", error.message);

        // 只有超体积时 `mode` 省略走默认 `exact`，`seed` 省略走默认 `1`。
        // With hypervolume only, an omitted `mode` defaults to `exact` and an omitted `seed` to `1`.
        let report = verify_reward_policy(&RewardPolicyRequest {
            time_normalized: None, extreme_value: None,
            hypervolume: Some(HypervolumeConfigInput {
                reference: Vec::new(), mode: default_mode(), max_work: 10, samples: 2,
                seed: default_seed(), calibration_observations: 4,
            }),
        }).unwrap();
        let hypervolume = report.hypervolume.unwrap();
        assert_eq!(hypervolume.mode, "Exact");
        assert_eq!(hypervolume.seed, 1);
        // 空参考点：状态里 `calibrated` 为 `false`，自动校准尚未发生。
        // An empty reference: the state has no calibration yet.
        let state: serde_json::Value = serde_json::from_str(&hypervolume.state_json).unwrap();
        assert_eq!(state["reference"], serde_json::json!([]));
        assert_eq!(state["minimum"], serde_json::json!([]));
        assert_eq!(state["maximum"], serde_json::json!([]));
    }
}
