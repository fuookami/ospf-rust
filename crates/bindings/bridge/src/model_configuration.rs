//! 续挖第 5 单：模型构建阶段与模型配置面（`model/configuration.rs` + `model/status.rs`）
//! Continuation, delivery 5: the model-building stages and the model configuration surface.
//!
//! `model/` 里 6 个模块、23 个未引用类型，其中大半是缓存键与上下文 trait（`RangeCacheKey`、
//! `ValueCacheContextTrait`、`LazyValueCacheContext`、`ValueCacheKind`…）以及模型本体（`BasicModel`、`MetaModel`、
//! `LinearModelState`）——缓存是运行期状态、模型本体是 6000 行构建器，都属真边界。**纯配置与阶段**才是可数据化的：
//!
//! * `FunctionExpansionPolicy` 三型：`Eager`（默认，建模阶段就展开）/ `DeferredNativeFirst` / `Auto`。
//!   关键点是 **`is_deferred()` 定义为 `!is_eager()`**，所以 `Auto` 也算 deferred 而不是第三种中间态。
//! * `BasicModelConfiguration` / `MetaModelConfiguration` 的默认值（实测：`enable_cache = true`、
//!   `lazy_evaluation = true`、`multi_objective = true`、`max_sub_objectives = 100`、`name` 为空）。
//! * `ModelBuildingStage` 七阶段与 `ModelBuildingStatus` 快照。这里有一条**领域不校验**的事实：
//!   `ModelBuildingStatus::new` 接受 `ready > total`、也接受 `total = 0`，两者都会让"进度"失去意义。
//!   桥接不替它拒，而是如实标出 `progressKind`（`empty` / `partial` / `complete` / `overflow`）并提供
//!   `totalIsZero`，让调用方自己决定怎么显示。
//!
//! Of the twenty-three unreferenced types there, most are cache keys and context traits or the model bodies themselves
//! (`BasicModel`, `MetaModel`, the 6000-line builder) — runtime state and constructors, a true boundary. **Pure configuration
//! and stages** are the datafiable part: `FunctionExpansionPolicy` in three forms, where **`is_deferred()` is defined as
//! `!is_eager()`** so `Auto` counts as deferred rather than as a third middle state; the measured defaults of
//! `BasicModelConfiguration` / `MetaModelConfiguration`; and the seven `ModelBuildingStage`s with `ModelBuildingStatus`
//! snapshots. One fact the **domain does not validate**: `ModelBuildingStatus::new` accepts `ready > total` and also
//! `total = 0`, both of which make "progress" meaningless. The bridge does not refuse those — it labels them through
//! `progressKind` (`empty` / `partial` / `complete` / `overflow`) plus `totalIsZero`, leaving the display decision to the
//! caller.

use ospf_rust_core::model::configuration::{
    BasicModelConfiguration, FunctionExpansionPolicy, MetaModelConfiguration,
};
use ospf_rust_core::model::status::{ModelBuildingStage, ModelBuildingStatus};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个构建阶段快照。 / One building-stage snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildingStatusSpec {
    /// 模型名（不能空白）。 / The model name (must not be blank).
    pub model_name: String,
    /// 阶段：`registerTokens` / `registerLinearConstraints` / `registerQuadraticConstraints` /
    /// `registerSymbols` / `flattenLinearModel` / `flattenQuadraticModel` / `buildObjective`。
    /// The stage.
    pub stage: String,
    /// 已完成数量。 / The amount ready.
    pub ready: u32,
    /// 总数量。 / The total amount.
    pub total: u32,
}

/// 配置请求。 / The configuration request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelConfigurationRequest {
    /// 模型名。 / The model name.
    #[serde(default)]
    pub name: String,
    /// 是否启用缓存。 / Whether the cache is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub enable_cache: Option<bool>,
    /// 是否启用延迟求值。 / Whether lazy evaluation is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lazy_evaluation: Option<bool>,
    /// 展开策略：`eager` / `deferredNativeFirst` / `auto`。 / The expansion policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub function_expansion_policy: Option<String>,
    /// 是否允许多目标。 / Whether multiple objectives are allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub multi_objective: Option<bool>,
    /// 最大子目标数量。 / The maximum sub-objective count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_sub_objectives: Option<u32>,
    /// 要复核的阶段快照。 / The stage snapshots to re-check.
    #[serde(default)]
    pub statuses: Vec<BuildingStatusSpec>,
}

/// 一个阶段快照的结论。 / The verdict for one stage snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildingStatusRow {
    /// 模型名（裁剪后）。 / The model name (trimmed).
    pub model_name: String,
    /// 阶段（线格式代码）。 / The stage (wire code).
    pub stage: String,
    /// 阶段序号（0..6，按领域声明顺序）。 / The stage index in declaration order.
    pub stage_index: u32,
    /// 已完成数量。 / The amount ready.
    pub ready: u32,
    /// 总数量。 / The total amount.
    pub total: u32,
    /// `empty` / `partial` / `complete` / `overflow`（领域不校验 `ready > total`）。
    /// `empty` / `partial` / `complete` / `overflow` (the domain never validates `ready > total`).
    pub progress_kind: String,
    /// 进度；`total = 0` 时为 `undefined`（领域也不拦这个除法）。
    /// The progress, `undefined` when `total = 0` (the domain does not guard that division either).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub progress: Option<f64>,
    /// 总数是否为零（进度无意义）。 / Whether the total is zero, making progress meaningless.
    pub total_is_zero: bool,
}

/// 配置与阶段复核结论。 / The configuration and stage verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelConfigurationReport {
    /// 模型名（裁剪后）。 / The model name (trimmed).
    pub name: String,
    /// 是否启用缓存。 / Whether the cache is enabled.
    pub enable_cache: bool,
    /// 是否启用延迟求值。 / Whether lazy evaluation is enabled.
    pub lazy_evaluation: bool,
    /// 展开策略（线格式代码）。 / The expansion policy (wire code).
    pub function_expansion_policy: String,
    /// 该策略是否在建模阶段展开。 / Whether that policy expands during building.
    pub policy_is_eager: bool,
    /// 该策略是否保留求解器无关结构（**`auto` 也算**）。 / Whether it defers (**`auto` included**).
    pub policy_is_deferred: bool,
    /// 是否允许多目标。 / Whether multiple objectives are allowed.
    pub multi_objective: bool,
    /// 最大子目标数量。 / The maximum sub-objective count.
    pub max_sub_objectives: u32,
    /// 阶段词表（七阶段，按领域顺序）。 / The stage vocabulary in domain order.
    pub stage_vocabulary: Vec<String>,
    /// 逐条快照的结论。 / Each snapshot's verdict.
    pub statuses: Vec<BuildingStatusRow>,
    /// 展开策略词表（桥接定义）。 / The policy vocabulary (bridge-defined).
    pub policy_vocabulary: Vec<String>,
    /// **领域默认值**（照 `Default` 实现实测，不是照文档抄）。 / The **domain defaults**, measured.
    pub defaults: ModelConfigurationDefaults,
}

/// 领域默认值。 / The domain defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelConfigurationDefaults {
    /// 默认模型名。 / The default model name.
    pub name: String,
    /// 默认是否启用缓存。 / The default cache flag.
    pub enable_cache: bool,
    /// 默认是否启用延迟求值。 / The default lazy-evaluation flag.
    pub lazy_evaluation: bool,
    /// 默认展开策略。 / The default expansion policy.
    pub function_expansion_policy: String,
    /// 默认是否允许多目标。 / The default multi-objective flag.
    pub multi_objective: bool,
    /// 默认最大子目标数量。 / The default maximum sub-objective count.
    pub max_sub_objectives: u32,
}

/// 快照上限（信任边界）。 / The snapshot cap (a trust boundary).
const MAX_STATUSES: usize = 4_096;

/// 复核模型配置与构建阶段快照。非法输入是**错误**。
/// Re-check the model configuration and the building-stage snapshots. Illegal input is an **error**.
pub fn verify_model_configuration(request: &ModelConfigurationRequest)
    -> Result<ModelConfigurationReport>
{
    if request.statuses.len() > MAX_STATUSES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "model configuration failed validation: at most {MAX_STATUSES} status snapshots are accepted")));
    }
    // 从**领域默认值**出发，再叠请求里的覆盖项。 / Start from the **domain defaults** and layer the overrides on top.
    let mut basic = BasicModelConfiguration::default();
    basic.name = request.name.trim().to_owned();
    if let Some(flag) = request.enable_cache { basic.enable_cache = flag; }
    if let Some(flag) = request.lazy_evaluation { basic.lazy_evaluation = flag; }
    if let Some(policy) = &request.function_expansion_policy {
        basic.function_expansion_policy = parse_policy(policy)?;
    }
    let mut meta = MetaModelConfiguration { basic, ..MetaModelConfiguration::default() };
    if let Some(flag) = request.multi_objective { meta.multi_objective = flag; }
    if let Some(count) = request.max_sub_objectives {
        meta.max_sub_objectives = usize::try_from(count).map_err(|_| BridgeError::new(
            "InvalidInput", "model configuration failed validation: the objective cap overflowed"))?;
    }
    let policy = meta.basic.function_expansion_policy;
    let mut statuses = Vec::with_capacity(request.statuses.len());
    for spec in &request.statuses {
        statuses.push(check_status(spec)?);
    }
    let defaults = {
        let basic = BasicModelConfiguration::default();
        let meta = MetaModelConfiguration::default();
        ModelConfigurationDefaults {
            name: basic.name.clone(),
            enable_cache: basic.enable_cache,
            lazy_evaluation: basic.lazy_evaluation,
            function_expansion_policy: policy_code(basic.function_expansion_policy).to_owned(),
            multi_objective: meta.multi_objective,
            max_sub_objectives: u32::try_from(meta.max_sub_objectives).unwrap_or(u32::MAX),
        }
    };
    Ok(ModelConfigurationReport {
        name: meta.basic.name.clone(),
        enable_cache: meta.basic.enable_cache,
        lazy_evaluation: meta.basic.lazy_evaluation,
        function_expansion_policy: policy_code(policy).to_owned(),
        // `is_deferred()` 就是 `!is_eager()`，所以 `auto` 落在 deferred 一侧。
        // `is_deferred()` is exactly `!is_eager()`, so `auto` falls on the deferred side.
        policy_is_eager: policy.is_eager(),
        policy_is_deferred: policy.is_deferred(),
        multi_objective: meta.multi_objective,
        max_sub_objectives: u32::try_from(meta.max_sub_objectives).unwrap_or(u32::MAX),
        stage_vocabulary: STAGES.iter().map(|(code, _)| (*code).to_owned()).collect(),
        statuses,
        policy_vocabulary: vec!["eager".to_owned(), "deferredNativeFirst".to_owned(), "auto".to_owned()],
        defaults,
    })
}

/// 七个阶段，按领域声明顺序。 / The seven stages in domain declaration order.
const STAGES: [(&str, ModelBuildingStage); 7] = [
    ("registerTokens", ModelBuildingStage::RegisterTokens),
    ("registerLinearConstraints", ModelBuildingStage::RegisterLinearConstraints),
    ("registerQuadraticConstraints", ModelBuildingStage::RegisterQuadraticConstraints),
    ("registerSymbols", ModelBuildingStage::RegisterSymbols),
    ("flattenLinearModel", ModelBuildingStage::FlattenLinearModel),
    ("flattenQuadraticModel", ModelBuildingStage::FlattenQuadraticModel),
    ("buildObjective", ModelBuildingStage::BuildObjective),
];

/// 一个快照的结论。 / One snapshot's verdict.
fn check_status(spec: &BuildingStatusSpec) -> Result<BuildingStatusRow> {
    if spec.model_name.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "model configuration failed validation: status model names must not be blank"));
    }
    let index = STAGES.iter().position(|(code, _)| *code == spec.stage).ok_or_else(|| {
        BridgeError::new("InvalidInput", &format!(
            "model configuration failed validation: unknown building stage `{}`", spec.stage))
    })?;
    // 阶段走**领域构造器**，桥接只做标记。 / The stage goes through the **domain constructor**; the bridge labels it.
    let status = ModelBuildingStatus::new(spec.model_name.trim(), STAGES[index].1,
        usize::try_from(spec.ready).unwrap_or(usize::MAX),
        usize::try_from(spec.total).unwrap_or(usize::MAX));
    let (kind, progress) = if status.total == 0 {
        // 领域不拦 `total = 0`；这里如实标 `empty` 并给 `undefined` 进度，不凭空给 0。
        // The domain never guards `total = 0`; label it `empty` with an `undefined` progress rather than inventing zero.
        ("empty", None)
    } else if status.ready > status.total {
        // 领域也不拦 `ready > total`。 / The domain does not guard `ready > total` either.
        ("overflow", Some(status.ready as f64 / status.total as f64))
    } else if status.ready == status.total {
        ("complete", Some(1.0))
    } else {
        ("partial", Some(status.ready as f64 / status.total as f64))
    };
    Ok(BuildingStatusRow {
        model_name: status.model_name.clone(),
        stage: STAGES[index].0.to_owned(),
        stage_index: u32::try_from(index).unwrap_or(u32::MAX),
        ready: u32::try_from(status.ready).unwrap_or(u32::MAX),
        total: u32::try_from(status.total).unwrap_or(u32::MAX),
        progress_kind: kind.to_owned(),
        progress,
        total_is_zero: status.total == 0,
    })
}

/// 解析展开策略。 / Parse the expansion policy.
fn parse_policy(text: &str) -> Result<FunctionExpansionPolicy> {
    match text {
        "eager" => Ok(FunctionExpansionPolicy::Eager),
        "deferredNativeFirst" => Ok(FunctionExpansionPolicy::DeferredNativeFirst),
        "auto" => Ok(FunctionExpansionPolicy::Auto),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "model configuration failed validation: unknown function expansion policy `{other}`"))),
    }
}

/// 策略的线格式代码。 / The wire code of a policy.
fn policy_code(policy: FunctionExpansionPolicy) -> &'static str {
    match policy {
        FunctionExpansionPolicy::Eager => "eager",
        FunctionExpansionPolicy::DeferredNativeFirst => "deferredNativeFirst",
        FunctionExpansionPolicy::Auto => "auto",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(model: &str, stage: &str, ready: u32, total: u32) -> BuildingStatusSpec {
        BuildingStatusSpec { model_name: model.to_owned(), stage: stage.to_owned(), ready, total }
    }

    fn request() -> ModelConfigurationRequest {
        ModelConfigurationRequest { name: "  plan-1  ".to_owned(), enable_cache: None,
            lazy_evaluation: None, function_expansion_policy: None, multi_objective: None,
            max_sub_objectives: None, statuses: Vec::new() }
    }

    #[test]
    fn defaults_are_measured_from_the_domain_not_documented() {
        let report = verify_model_configuration(&request()).unwrap();
        // 领域默认值实测。 / The measured domain defaults.
        assert_eq!(report.defaults.name, "");
        assert!(report.defaults.enable_cache);
        assert!(report.defaults.lazy_evaluation);
        assert_eq!(report.defaults.function_expansion_policy, "eager");
        assert!(report.defaults.multi_objective);
        assert_eq!(report.defaults.max_sub_objectives, 100);
        // 不给覆盖项时，结论就是默认值（模型名被裁空白）。 / Without overrides the verdict is the defaults.
        assert_eq!(report.name, "plan-1");
        assert!(report.enable_cache);
        assert_eq!(report.function_expansion_policy, "eager");
        assert_eq!(report.max_sub_objectives, 100);
        assert_eq!(report.stage_vocabulary.len(), 7);
        assert_eq!(report.stage_vocabulary[0], "registerTokens");
        assert_eq!(report.stage_vocabulary[6], "buildObjective");
        assert_eq!(report.policy_vocabulary.len(), 3);
    }

    #[test]
    fn auto_is_deferred_because_deferred_is_defined_as_not_eager() {
        // 三种策略：只有 `eager` 是 eager；`auto` 与 `deferredNativeFirst` 都在 deferred 一侧。
        // Three policies: only `eager` is eager; both others sit on the deferred side, `auto` included.
        for (policy, eager, deferred) in [("eager", true, false), ("deferredNativeFirst", false, true),
            ("auto", false, true)]
        {
            let mut document = request();
            document.function_expansion_policy = Some(policy.to_owned());
            let report = verify_model_configuration(&document).unwrap();
            assert_eq!(report.function_expansion_policy, policy);
            assert_eq!(report.policy_is_eager, eager, "{policy}");
            assert_eq!(report.policy_is_deferred, deferred, "{policy}");
            // 两者恒互为反。 / The two are always negations of each other.
            assert_ne!(report.policy_is_eager, report.policy_is_deferred);
        }
        // 覆盖项确实生效，而不是被默认值吃掉。 / The overrides really take effect.
        let mut document = request();
        document.enable_cache = Some(false);
        document.lazy_evaluation = Some(false);
        document.multi_objective = Some(false);
        document.max_sub_objectives = Some(7);
        let report = verify_model_configuration(&document).unwrap();
        assert!(!report.enable_cache);
        assert!(!report.lazy_evaluation);
        assert!(!report.multi_objective);
        assert_eq!(report.max_sub_objectives, 7);
    }

    #[test]
    fn progress_labels_the_two_things_the_domain_never_validates() {
        let mut document = request();
        document.statuses = vec![
            status("plan", "registerTokens", 0, 0),
            status("plan", "registerLinearConstraints", 3, 10),
            status("plan", "flattenLinearModel", 5, 5),
            status("plan", "buildObjective", 9, 4),
        ];
        let report = verify_model_configuration(&document).unwrap();
        assert_eq!(report.statuses.len(), 4);
        // `total = 0`：进度无意义，标 `empty` 给 `undefined`，不凭空给 0。
        // `total = 0`: progress is meaningless, labelled `empty` with `undefined` rather than an invented zero.
        assert_eq!(report.statuses[0].progress_kind, "empty");
        assert_eq!(report.statuses[0].progress, None);
        assert!(report.statuses[0].total_is_zero);
        assert_eq!(report.statuses[0].stage_index, 0);
        // 部分完成。 / Partial.
        assert_eq!(report.statuses[1].progress_kind, "partial");
        assert_eq!(report.statuses[1].progress, Some(0.3));
        assert_eq!(report.statuses[1].stage_index, 1);
        // 完成。 / Complete.
        assert_eq!(report.statuses[2].progress_kind, "complete");
        assert_eq!(report.statuses[2].progress, Some(1.0));
        // **`ready > total` 领域不校验**：如实标 `overflow`，进度照算（2.25），不夹到 1。
        // **The domain never validates `ready > total`**: labelled `overflow` with the raw ratio 2.25, not clamped to 1.
        assert_eq!(report.statuses[3].progress_kind, "overflow");
        assert_eq!(report.statuses[3].progress, Some(2.25));
        assert_eq!(report.statuses[3].stage_index, 6);
        assert_eq!(report.statuses[3].stage, "buildObjective");
        // 模型名被裁空白（走领域构造器）。 / The name is trimmed through the domain constructor.
        assert_eq!(report.statuses[1].model_name, "plan");
    }

    #[test]
    fn bad_input_is_refused() {
        // 未知阶段 / 未知策略 / 空白模型名。 / An unknown stage, an unknown policy, a blank model name.
        let mut bad_stage = request();
        bad_stage.statuses = vec![status("plan", "compile", 1, 2)];
        assert!(verify_model_configuration(&bad_stage).unwrap_err().message.contains("unknown building stage"));
        let mut bad_policy = request();
        bad_policy.function_expansion_policy = Some("lazy".to_owned());
        assert!(verify_model_configuration(&bad_policy).unwrap_err().message
            .contains("unknown function expansion policy"));
        let mut blank_name = request();
        blank_name.statuses = vec![status("   ", "registerTokens", 1, 2)];
        assert!(verify_model_configuration(&blank_name).unwrap_err().message
            .contains("status model names must not be blank"));
        let mut too_many = request();
        too_many.statuses = (0..=MAX_STATUSES)
            .map(|index| status(&format!("m{index}"), "registerTokens", 0, 1)).collect();
        assert!(verify_model_configuration(&too_many).unwrap_err().message.contains("at most"));
        // 空请求（只给名字）是合法的。 / A bare request with just a name is legal.
        let report = verify_model_configuration(&ModelConfigurationRequest { name: "solo".to_owned(),
            enable_cache: None, lazy_evaluation: None, function_expansion_policy: None,
            multi_objective: None, max_sub_objectives: None, statuses: Vec::new() }).unwrap();
        assert_eq!(report.name, "solo");
        assert!(report.statuses.is_empty());
    }
}
