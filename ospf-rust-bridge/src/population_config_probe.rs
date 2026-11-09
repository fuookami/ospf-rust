//! 续挖第 87 单：**种群配置的构造面**——构造器给 `0.1 / 1`、`mutation_rate` 一个字都不校验、
//! `archive_amount` 的 `.max(1)` **只活在 builder 里**（结构体字面量直接绕过它）
//! （`solver/heuristic/composition/population.rs` 的 `PopulationConfig` / `SwarmConfig` / `IslandConfig`）
//! Continuation, delivery 87: the **configuration surface of population search** — the constructor produces
//! `0.1 / 1`, `mutation_rate` validates nothing at all, and the `.max(1)` on `archive_amount` **lives only in the
//! builder** (a struct literal bypasses it)
//! (`PopulationConfig` / `SwarmConfig` / `IslandConfig` in `solver/heuristic/composition/population.rs`).
//!
//! 这一单按"真边界"的约定只绑**纯配置**：算子槽位一律用 crate 自己 `genetic()` 的那套默认组合来填，
//! 不碰任何算子本体。八条实测：
//!
//! 1. **构造器的默认值是 `0.1` 与 `1`**（`the_constructor_defaults_are_point_one_and_one()`）。
//! 2. **`with_mutation_rate` 一点都不校验**（`the_mutation_rate_builder_validates_nothing()`）：负数、`NaN`、
//!    大于 1 全照收。
//! 3. **`with_archive_amount` 把 0 抬成 1**（`the_archive_amount_builder_clamps_zero_to_one()`）：
//!    `amount.max(1)`，所以 **0 是唯一被改写的值**。
//! 4. **`size` 从不被钳制**（`the_size_is_never_clamped()`）：0 照收。
//! 5. **builder 消费并返回同一个配置**（`the_builder_returns_the_same_config()`）：两个字段的链序无关。
//! 6. **结构体字面量直接绕过那个钳制**（`a_struct_literal_bypasses_the_clamp()`）：字段全 `pub`，
//!    于是 `archive_amount: 0` 写得出来——那个 `.max(1)` **不是类型不变量，只是 builder 的规矩**。
//!    本单最值钱的一条。
//! 7. **`SwarmConfig` 的 `fitness_evaluator` 默认 `None`**（`the_swarm_config_starts_without_an_evaluator()`）。
//! 8. **`IslandConfig` 没有构造器**（`the_island_config_has_no_constructor()`）：只能字面量构造，
//!    `migration_period` 一点都不校验（0 照收）。
//!
//! This delivery binds **pure configuration only**, as the "true boundary" convention requires: every operator slot is
//! filled with the crate's own `genetic()` defaults and no operator body is touched. **The constructor defaults are `0.1`
//! and `1`.** **`with_mutation_rate` validates nothing** — negatives, `NaN` and values above one all pass. **The
//! `archive_amount` builder lifts zero to one** via `.max(1)`, so **zero is the only rewritten value**. **`size` is never
//! clamped.** **The builder consumes and returns the same config**, so chain order does not matter. **A struct literal
//! bypasses that clamp** — the fields are all `pub`, so `archive_amount: 0` is writable and the `.max(1)` is **not a type
//! invariant but only a builder habit**; this is the most valuable finding here. **`SwarmConfig` starts without an
//! evaluator.** And **`IslandConfig` has no constructor**, so it can only be built from a literal and `migration_period`
//! validates nothing.

use ospf_rust_core::solver::heuristic::composition::population::{
    IslandConfig, PopulationConfig, SwarmConfig,
};
use ospf_rust_core::solver::heuristic::entropy::SeededEntropy;
use ospf_rust_core::solver::heuristic::operator::initializer::ModelInitializer;
use ospf_rust_core::solver::heuristic::operator::migration::NoMigration;
use ospf_rust_core::solver::heuristic::operator::mutation::GaussianMutation;
use ospf_rust_core::solver::heuristic::operator::stopping::IterationLimit;
use ospf_rust_core::solver::heuristic::operator::survivor::ElitistSurvivor;
use ospf_rust_core::solver::heuristic::operator::topology::GlobalTopology;
use ospf_rust_core::solver::heuristic::operator::update::SinglePointCrossover;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 种群配置探针请求。 / The population-config probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PopulationConfigRequest {
    /// 种群规模（不钳制）。 / The population size, never clamped.
    #[serde(default)]
    pub size: u64,
    /// 变异率（不校验）。 / The mutation rate, never validated.
    #[serde(default)]
    pub mutation_rate: Option<f64>,
    /// 档案容量（builder 会把 0 抬成 1）。 / The archive capacity; the builder lifts zero to one.
    #[serde(default)]
    pub archive_amount: Option<u64>,
    /// 是否走 builder（否则用结构体字面量）。 / Whether to go through the builders (otherwise a struct literal).
    #[serde(default)]
    pub use_builders: Option<bool>,
    /// 粒子数量（`SwarmConfig` 用）。 / The particle amount for `SwarmConfig`.
    #[serde(default)]
    pub swarm_size: Option<u64>,
    /// 迁移周期（`IslandConfig` 用，不校验）。 / The migration period for `IslandConfig`, never validated.
    #[serde(default)]
    pub migration_period: Option<u64>,
}

/// 种群配置探针结论。 / The population-config probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PopulationConfigReport {
    /// 配置里的种群规模。 / The population size held by the config.
    pub size: u32,
    /// 配置里的变异率（非有限值只能以 `None` + `mutation_rate_infinite` 表达）。
    /// The mutation rate held by the config; a non-finite value arrives as `None` plus a flag.
    #[serde(default)]
    pub mutation_rate: Option<f64>,
    /// 变异率是否为非有限值。 / Whether the mutation rate is non-finite.
    pub mutation_rate_infinite: bool,
    /// 配置里的档案容量。 / The archive capacity held by the config.
    pub archive_amount: u32,
    /// 是否走了 builder。 / Whether the builders were used.
    pub used_builders: bool,
    /// `SwarmConfig` 里的粒子数量。 / The particle amount held by the swarm config.
    pub swarm_size: u32,
    /// `SwarmConfig` 是否自带适应度评估算子。 / Whether the swarm config carries a fitness evaluator.
    pub swarm_has_evaluator: bool,
    /// `IslandConfig` 里的迁移周期。 / The migration period held by the island config.
    pub migration_period: u32,
    /// `IslandConfig` 里的岛屿数量。 / The island count held by the island config.
    pub island_count: u32,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_constructor_defaults_are_point_one_and_one: bool,
    pub the_mutation_rate_builder_validates_nothing: bool,
    pub the_archive_amount_builder_clamps_zero_to_one: bool,
    pub the_size_is_never_clamped: bool,
    pub the_builder_returns_the_same_config: bool,
    pub a_struct_literal_bypasses_the_clamp: bool,
    pub the_swarm_config_starts_without_an_evaluator: bool,
    pub the_island_config_has_no_constructor: bool,
}

/// 造一套配置（算子槽位用 crate 自己的默认组合）。 / Build a config set, filling operator slots with the crate defaults.
fn config_of(size: usize, mutation_rate: f64, archive_amount: usize, use_builders: bool)
    -> PopulationConfig<f64>
{
    let base = PopulationConfig::new(size, Box::new(IterationLimit::new(10)),
        Box::new(SeededEntropy::new(7)), Box::new(SinglePointCrossover),
        Box::new(GaussianMutation::default()), Box::new(ElitistSurvivor),
        Box::new(GlobalTopology), Box::new(ModelInitializer));
    if use_builders {
        base.with_mutation_rate(mutation_rate).with_archive_amount(archive_amount)
    } else {
        // 结构体字面量：`archive_amount` 与 `mutation_rate` 原样写入，绕开 builder。 
        // A struct literal writes both fields verbatim and bypasses the builders.
        PopulationConfig { size, mutation_rate, archive_amount, ..base }
    }
}

/// 走一遍配置探针。非法输入是**错误**。 / Walk the config probe.
pub fn verify_population_config(request: &PopulationConfigRequest)
    -> Result<PopulationConfigReport>
{
    let size = usize::try_from(request.size).map_err(|_| BridgeError::new("InvalidInput",
        "population config failed validation: population size overflow"))?;
    let archive_amount = usize::try_from(request.archive_amount.unwrap_or(1))
        .map_err(|_| BridgeError::new("InvalidInput",
            "population config failed validation: archive capacity overflow"))?;
    let swarm_size = usize::try_from(request.swarm_size.unwrap_or(1))
        .map_err(|_| BridgeError::new("InvalidInput",
            "population config failed validation: swarm size overflow"))?;
    let migration_period = request.migration_period.unwrap_or(0);
    // 四个数值全部压在百万以内，跨层就走 `u32`，精确无争议。 
    // All four numbers stay below one million, so they cross as `u32` and remain exact.
    if size > 1_000_000 || archive_amount > 1_000_000 || swarm_size > 1_000_000
        || migration_period > 1_000_000
    {
        return Err(BridgeError::new("InvalidInput",
            "population config failed validation: sizes must stay below one million"));
    }
    let findings = canonical_findings();
    let mutation_rate = request.mutation_rate.unwrap_or(0.1);
    // 请求侧的 `NaN` 过不来 JSON；能过来的只有有限值，非有限语义由内部探针量出。
    // A `NaN` request cannot cross JSON, so only finite values arrive here and the non-finite semantics come from the
    // internal probes.
    if !mutation_rate.is_finite() {
        return Err(BridgeError::new("InvalidInput", "population config failed validation: \
            a non-finite mutation rate cannot cross the boundary"));
    }
    let used_builders = request.use_builders.unwrap_or(true);
    let config = config_of(size, mutation_rate, archive_amount, used_builders);
    let swarm = SwarmConfig::<f64>::new(swarm_size, Box::new(IterationLimit::new(10)),
        Box::new(SeededEntropy::new(7)), Box::new(ModelInitializer));
    let island = IslandConfig::<f64> { islands: Vec::new(),
        migration_period,
        migration: Box::new(NoMigration), stopping: Box::new(IterationLimit::new(10)),
        entropy: Box::new(SeededEntropy::new(7)) };
    Ok(PopulationConfigReport {
        size: u32::try_from(config.size).unwrap_or(u32::MAX),
        mutation_rate: config.mutation_rate.is_finite().then_some(config.mutation_rate),
        mutation_rate_infinite: !config.mutation_rate.is_finite(),
        archive_amount: u32::try_from(config.archive_amount).unwrap_or(u32::MAX),
        used_builders,
        swarm_size: u32::try_from(swarm.size).unwrap_or(u32::MAX),
        swarm_has_evaluator: swarm.fitness_evaluator.is_some(),
        migration_period: u32::try_from(island.migration_period).unwrap_or(u32::MAX),
        island_count: u32::try_from(island.islands.len()).unwrap_or(u32::MAX),
        the_constructor_defaults_are_point_one_and_one:
            findings.the_constructor_defaults_are_point_one_and_one,
        the_mutation_rate_builder_validates_nothing:
            findings.the_mutation_rate_builder_validates_nothing,
        the_archive_amount_builder_clamps_zero_to_one:
            findings.the_archive_amount_builder_clamps_zero_to_one,
        the_size_is_never_clamped: findings.the_size_is_never_clamped,
        the_builder_returns_the_same_config: findings.the_builder_returns_the_same_config,
        a_struct_literal_bypasses_the_clamp: findings.a_struct_literal_bypasses_the_clamp,
        the_swarm_config_starts_without_an_evaluator:
            findings.the_swarm_config_starts_without_an_evaluator,
        the_island_config_has_no_constructor: findings.the_island_config_has_no_constructor,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_constructor_defaults_are_point_one_and_one: bool,
    the_mutation_rate_builder_validates_nothing: bool,
    the_archive_amount_builder_clamps_zero_to_one: bool,
    the_size_is_never_clamped: bool,
    the_builder_returns_the_same_config: bool,
    a_struct_literal_bypasses_the_clamp: bool,
    the_swarm_config_starts_without_an_evaluator: bool,
    the_island_config_has_no_constructor: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 构造器的默认值是 0.1 与 1。 / The constructor defaults are 0.1 and 1.
    let the_constructor_defaults_are_point_one_and_one = {
        let config = config_of(3, 0.1, 1, true);
        // 直接看 `new` 出来的那三个字段。 / Look at the three fields straight out of `new`.
        let fresh = PopulationConfig::<f64>::new(3, Box::new(IterationLimit::new(10)),
            Box::new(SeededEntropy::new(7)), Box::new(SinglePointCrossover),
            Box::new(GaussianMutation::default()), Box::new(ElitistSurvivor),
            Box::new(GlobalTopology), Box::new(ModelInitializer));
        config.mutation_rate == 0.1 && config.archive_amount == 1
            && fresh.mutation_rate == 0.1 && fresh.archive_amount == 1
    };
    // 2) `with_mutation_rate` 一点都不校验。 / `with_mutation_rate` validates nothing.
    let the_mutation_rate_builder_validates_nothing = {
        // 负数、`NaN`、大于 1 全照收。 / Negatives, `NaN` and values above one all pass.
        let negative = config_of(3, -5.0, 1, true).mutation_rate == -5.0;
        let nan = config_of(3, f64::NAN, 1, true).mutation_rate.is_nan();
        let above = config_of(3, 7.5, 1, true).mutation_rate == 7.5;
        let infinite = config_of(3, f64::INFINITY, 1, true).mutation_rate.is_infinite();
        negative && nan && above && infinite
    };
    // 3) `with_archive_amount` 把 0 抬成 1。 / The archive builder lifts zero to one.
    let the_archive_amount_builder_clamps_zero_to_one = {
        // 0 是**唯一**被改写的值：其余原样保留。 
        // Zero is the **only** rewritten value; every other value passes through.
        config_of(3, 0.1, 0, true).archive_amount == 1
            && config_of(3, 0.1, 1, true).archive_amount == 1
            && config_of(3, 0.1, 5, true).archive_amount == 5
            && [0_usize, 2, 9, 100].iter().all(|amount| {
                let clamped = config_of(3, 0.1, *amount, true).archive_amount;
                clamped == (*amount).max(1)
            })
    };
    // 4) `size` 从不被钳制。 / `size` is never clamped.
    let the_size_is_never_clamped = {
        config_of(0, 0.1, 1, true).size == 0
            && config_of(0, 0.1, 1, false).size == 0
            && config_of(9_999, 0.1, 1, true).size == 9_999
    };
    // 5) builder 消费并返回同一个配置。 / The builder consumes and returns the same config.
    let the_builder_returns_the_same_config = {
        // 两个字段的链序无关。 / Chain order does not matter for the two fields.
        let forward = config_of(4, 0.3, 2, true);
        let backward = PopulationConfig::<f64>::new(4, Box::new(IterationLimit::new(10)),
            Box::new(SeededEntropy::new(7)), Box::new(SinglePointCrossover),
            Box::new(GaussianMutation::default()), Box::new(ElitistSurvivor),
            Box::new(GlobalTopology), Box::new(ModelInitializer))
            .with_archive_amount(2).with_mutation_rate(0.3);
        forward.mutation_rate == backward.mutation_rate
            && forward.archive_amount == backward.archive_amount
            && forward.size == backward.size
    };
    // 6) 结构体字面量直接绕过那个钳制。 / A struct literal bypasses the clamp.
    let a_struct_literal_bypasses_the_clamp = {
        // 同一份 `archive_amount: 0` 走 builder 变 1、走字面量仍是 0。 
        // The same `archive_amount: 0` becomes 1 through the builder and stays 0 through a literal.
        config_of(3, 0.1, 0, true).archive_amount == 1
            && config_of(3, 0.1, 0, false).archive_amount == 0
    };
    // 7) `SwarmConfig` 的 `fitness_evaluator` 默认 `None`。 / The swarm config starts without an evaluator.
    let the_swarm_config_starts_without_an_evaluator = {
        let swarm = SwarmConfig::<f64>::new(5, Box::new(IterationLimit::new(10)),
            Box::new(SeededEntropy::new(7)), Box::new(ModelInitializer));
        swarm.fitness_evaluator.is_none() && swarm.size == 5
    };
    // 8) `IslandConfig` 没有构造器。 / The island config has no constructor.
    let the_island_config_has_no_constructor = {
        // 只能字面量构造，`migration_period: 0` 照收（没有任何校验路径）。 
        // It can only be built from a literal, and a zero migration period passes because no validation path exists.
        let island = IslandConfig::<f64> { islands: Vec::new(), migration_period: 0,
            migration: Box::new(NoMigration), stopping: Box::new(IterationLimit::new(10)),
            entropy: Box::new(SeededEntropy::new(7)) };
        island.migration_period == 0 && island.islands.is_empty()
            && { let other = IslandConfig::<f64> { islands: Vec::new(),
                migration_period: u64::MAX, migration: Box::new(NoMigration),
                stopping: Box::new(IterationLimit::new(10)),
                entropy: Box::new(SeededEntropy::new(7)) };
                other.migration_period == u64::MAX }
    };
    Findings {
        the_constructor_defaults_are_point_one_and_one,
        the_mutation_rate_builder_validates_nothing,
        the_archive_amount_builder_clamps_zero_to_one,
        the_size_is_never_clamped,
        the_builder_returns_the_same_config,
        a_struct_literal_bypasses_the_clamp,
        the_swarm_config_starts_without_an_evaluator,
        the_island_config_has_no_constructor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> PopulationConfigRequest {
        PopulationConfigRequest { size: 4, mutation_rate: Some(0.1), archive_amount: Some(1),
            use_builders: Some(true), swarm_size: Some(5), migration_period: Some(3) }
    }

    #[test]
    fn the_defaults_survive_the_constructor() {
        let report = verify_population_config(&PopulationConfigRequest {
            mutation_rate: None, archive_amount: None, ..request() }).unwrap();
        assert_eq!(report.mutation_rate, Some(0.1));
        assert_eq!(report.archive_amount, 1);
        assert_eq!(report.size, 4);
        assert!(report.used_builders);
        assert_eq!(report.swarm_size, 5);
        assert!(!report.swarm_has_evaluator);
        assert_eq!(report.migration_period, 3);
        assert_eq!(report.island_count, 0);
        assert!(report.the_constructor_defaults_are_point_one_and_one);
        assert!(report.the_mutation_rate_builder_validates_nothing);
        assert!(report.the_archive_amount_builder_clamps_zero_to_one);
        assert!(report.the_size_is_never_clamped);
        assert!(report.the_builder_returns_the_same_config);
        assert!(report.a_struct_literal_bypasses_the_clamp);
        assert!(report.the_swarm_config_starts_without_an_evaluator);
        assert!(report.the_island_config_has_no_constructor);
    }

    #[test]
    fn the_archive_builder_lifts_zero_but_a_literal_does_not() {
        // 走 builder：0 → 1。 / Through the builder zero becomes one.
        let built = verify_population_config(&PopulationConfigRequest {
            archive_amount: Some(0), use_builders: Some(true), ..request() }).unwrap();
        assert_eq!(built.archive_amount, 1);
        // 走字面量：0 原样保留。 / Through a literal zero stays zero.
        let literal = verify_population_config(&PopulationConfigRequest {
            archive_amount: Some(0), use_builders: Some(false), ..request() }).unwrap();
        assert_eq!(literal.archive_amount, 0);
        assert!(!literal.used_builders);
        // 非零值两条路一样。 / A non-zero value is identical on both paths.
        let five = verify_population_config(&PopulationConfigRequest { archive_amount: Some(5),
            use_builders: Some(false), ..request() }).unwrap();
        assert_eq!(five.archive_amount, 5);
    }

    #[test]
    fn the_mutation_rate_is_stored_verbatim() {
        // 越界的变异率照收（builder 不校验）。 / An out-of-range rate is stored verbatim; the builder never validates.
        let negative = verify_population_config(&PopulationConfigRequest {
            mutation_rate: Some(-5.0), ..request() }).unwrap();
        assert_eq!(negative.mutation_rate, Some(-5.0));
        let above = verify_population_config(&PopulationConfigRequest {
            mutation_rate: Some(7.5), ..request() }).unwrap();
        assert_eq!(above.mutation_rate, Some(7.5));
        assert!(!negative.mutation_rate_infinite);
    }

    #[test]
    fn the_size_is_never_clamped() {
        let zero = verify_population_config(&PopulationConfigRequest { size: 0, ..request() })
            .unwrap();
        assert_eq!(zero.size, 0);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_population_config(&PopulationConfigRequest { size: 2_000_000,
            ..request() }).unwrap_err().message.contains("below one million"));
        assert!(verify_population_config(&PopulationConfigRequest {
            mutation_rate: Some(f64::NAN), ..request() }).unwrap_err()
            .message.contains("non-finite mutation rate"));
        assert!(verify_population_config(&PopulationConfigRequest {
            mutation_rate: Some(f64::INFINITY), ..request() }).unwrap_err()
            .message.contains("non-finite mutation rate"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_population_config(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"mutationRateInfinite\""), "{json}");
        assert!(json.contains("\"archiveAmount\""), "{json}");
        assert!(json.contains("\"aStructLiteralBypassesTheClamp\""), "{json}");
        assert!(!json.contains("mutation_rate_infinite"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<PopulationConfigReport>(&copy.to_string()).is_err());
    }
}
