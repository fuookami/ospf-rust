//! 续挖第 89 单：**接受判据的三个陷阱**——`AlwaysAcceptance` 一点也不"无条件"（它只比约束层级）、
//! `BetterOnly` 与 `NotWorse` 的差别**只在平局**、而**平局是由"两个方向都说自己更优"判出来的**
//! （`（Less, Less） ⇒ Equal`，所以一个自相矛盾的比较器会被解释成平局）；`Metropolis` 则先按层级结案、
//! 再用温度门抽 `exp(-Δ/T)`，且 `delta <= 0` 直接接受（**平局也算**）
//! （`solver/heuristic/operator/acceptance.rs` 的 `BetterOnlyAcceptance` / `NotWorseAcceptance` /
//! `AlwaysAcceptance` / `MetropolisAcceptance`）
//! Continuation, delivery 89: the **three traps of the acceptance criteria** — `AlwaysAcceptance` is not unconditional
//! at all (it only compares constraint tiers), `BetterOnly` and `NotWorse` differ **only on ties**, and a **tie is
//! detected by both directions claiming to be better** (a `(Less, Less)` pair becomes `Equal`, so a self-contradictory
//! comparator reads as a tie); `Metropolis` settles on the tier first and then draws `exp(-Δ/T)` through the temperature
//! gate, while `delta <= 0` is accepted outright, **ties included**
//! (`BetterOnlyAcceptance` / `NotWorseAcceptance` / `AlwaysAcceptance` / `MetropolisAcceptance` in
//! `solver/heuristic/operator/acceptance.rs`).
//!
//! 第 87 单量的是配置面，这一单量**决策面**。八条实测：
//!
//! 1. **`BetterOnlyAcceptance` 只在候选严格更优时接受**（`better_only_accepts_only_strict_improvement()`）。
//! 2. **`NotWorseAcceptance` 连平局一起接受**（`not_worse_accepts_ties_as_well()`）：两者的差别**只有平局**。
//! 3. **平局由"两个方向都返回更优"判出**（`a_tie_is_detected_by_both_directions_agreeing()`）：
//!    一个**自相矛盾**的比较器（永远返回 `Less`）读作平局。
//! 4. **`None` 比较也落到平局**（`a_none_comparison_falls_back_to_equal()`）：`NaN` 目标走的是同一条路。
//! 5. **`AlwaysAcceptance` 并不"无条件"**（`always_acceptance_is_not_unconditional()`）：它只比约束层级，
//!    候选层级更差时**拒绝**。本单最值钱的一条。
//! 6. **`Metropolis` 先按层级结案**（`metropolis_settles_on_the_tier_first()`）：层级更好就接受、更差就拒绝，
//!    完全不看目标。
//! 7. **温度守卫排在 `delta <= 0` 之前**（`the_temperature_guard_precedes_the_tie_acceptance()`）：
//!    **温度冻住时连平局都拒绝**，而更优目标在那两层之前就已经返回真。
//! 8. **同 seed 可复现、`Δ` 越大接受越少**（`metropolis_is_reproducible_and_shrinks_with_delta()`）：
//!    温度固定时接受率随 `Δ` 单调下降。
//!
//! Delivery 87 measured the configuration surface; this one measures **the decision surface**. **`BetterOnlyAcceptance`
//! accepts strict improvement only.** **`NotWorseAcceptance` accepts ties too**, so the two differ **only on ties**. **A
//! tie is detected by both directions claiming to be better**, so a self-contradictory comparator reads as a tie. **A
//! `None` comparison falls back to a tie** as well, which is the path a `NaN` objective takes. **`AlwaysAcceptance` is
//! not unconditional** because it only compares constraint tiers and rejects a worse tier — the most valuable finding
//! here. **`Metropolis` settles on the tier first.** **Its temperature guard only bites when the candidate is not strictly
//! better**, since strict improvement returns true earlier. And **the draws are reproducible per seed while the acceptance
//! rate shrinks as `Δ` grows**.

use ospf_rust_core::model::callback::Solution;
use ospf_rust_core_plugin_heuristic::heuristic::entropy::{Entropy, SeededEntropy};
use ospf_rust_core_plugin_heuristic::heuristic::individual::Individual;
use ospf_rust_core_plugin_heuristic::heuristic::iteration::Iteration;
use ospf_rust_core_plugin_heuristic::heuristic::model::{HeuristicValue, ObjectiveModel, VariableModel};
use ospf_rust_core_plugin_heuristic::heuristic::operator::acceptance::{
    Acceptance, AlwaysAcceptance, BetterOnlyAcceptance, MetropolisAcceptance, NotWorseAcceptance,
};
use ospf_rust_core_plugin_heuristic::heuristic::operator::evolution::GeometricSource;
use ospf_rust_core::variable::VariableId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::cmp::Ordering;

use crate::{BridgeError, Result};

/// 接受判据探针请求。 / The acceptance-criterion probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptanceRequest {
    /// 当前解的适应度。 / The current individual's fitness.
    pub current_fitness: f64,
    /// 候选解的适应度。 / The candidate's fitness.
    pub candidate_fitness: f64,
    /// 约束层级：`equal` / `candidateBetter` / `candidateWorse`。
    /// The constraint tier: `equal`, `candidateBetter` or `candidateWorse`.
    #[serde(default)]
    pub tier: Option<String>,
    /// 目标比较器行为：`normal` / `alwaysLess` / `alwaysNone`（探针模型的可控缺陷）。
    /// The objective comparator's behaviour: `normal`, `alwaysLess` or `alwaysNone`.
    #[serde(default)]
    pub comparison: Option<String>,
    /// `Metropolis` 的初始温度（按迭代 0 取值）。 / The initial temperature (read at iteration 0).
    #[serde(default)]
    pub temperature: Option<f64>,
    /// `Metropolis` 的抽样次数。 / How many Metropolis draws to take.
    #[serde(default)]
    pub samples: Option<u32>,
    /// 抽样用的随机流种子。 / The random-stream seed for the draws.
    #[serde(default)]
    pub seed: Option<u64>,
}

/// 接受判据探针结论。 / The acceptance-criterion probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptanceReport {
    /// 探针模型给出的个体比较：`less` / `equal` / `greater`。
    /// What the probe model reports for the individuals: `less`, `equal` or `greater`.
    pub ordering: String,
    /// `BetterOnlyAcceptance` 的判定。 / The verdict of `BetterOnlyAcceptance`.
    pub better_only: bool,
    /// `NotWorseAcceptance` 的判定。 / The verdict of `NotWorseAcceptance`.
    pub not_worse: bool,
    /// `AlwaysAcceptance` 的判定。 / The verdict of `AlwaysAcceptance`.
    pub always: bool,
    /// `Metropolis` 的第一次抽样（`samples` 为 0 时为 `null`）。 / The first Metropolis draw, `null` at zero samples.
    #[serde(default)]
    pub metropolis_first: Option<bool>,
    /// `Metropolis` 在 `samples` 次抽样里接受的次数。 / How many of the draws Metropolis accepted.
    pub metropolis_accepted: u32,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub better_only_accepts_only_strict_improvement: bool,
    pub not_worse_accepts_ties_as_well: bool,
    pub a_tie_is_detected_by_both_directions_agreeing: bool,
    pub a_none_comparison_falls_back_to_equal: bool,
    pub always_acceptance_is_not_unconditional: bool,
    pub metropolis_settles_on_the_tier_first: bool,
    pub the_temperature_guard_precedes_the_tie_acceptance: bool,
    pub metropolis_is_reproducible_and_shrinks_with_delta: bool,
}

/// 探针模型：把比较行为做成可调的旋钮，其余能力一律空实现。
/// The probe model turns comparison behaviour into knobs and leaves every other capability empty.
struct ProbeModel {
    /// 目标比较器行为。 / The objective comparator's behaviour.
    comparison: &'static str,
    /// 约束层级。 / The constraint tier.
    tier: Ordering,
}

impl VariableModel<f64> for ProbeModel {
    fn num_variables(&self) -> usize { 0 }
    fn variable_order(&self) -> Vec<VariableId> { Vec::new() }
    fn lower_bound(&self, _index: usize) -> Option<f64> { None }
    fn upper_bound(&self, _index: usize) -> Option<f64> { None }
    fn coerce_in(&self, _index: usize, value: f64) -> f64 { value }
    fn initial_solutions(&self, _amount: usize) -> Vec<Solution<f64>> { Vec::new() }
    fn constraint_satisfied(&self, _solution: &Solution<f64>) -> Option<bool> { Some(true) }
}

impl ObjectiveModel<f64, f64> for ProbeModel {
    fn default_objective(&self) -> Option<f64> { None }
    fn objective(&self, _solution: &Solution<f64>) -> Option<f64> { None }

    fn compare_objective(&self, lhs: &f64, rhs: &f64) -> Option<Ordering> {
        match self.comparison {
            // 自相矛盾：永远说自己更优。 / Self-contradictory: it always claims to be better.
            "always_less" => Some(Ordering::Less),
            // 永不表态：`NaN` 走的正是这条路。 / Never commits, which is exactly the `NaN` path.
            "always_none" => None,
            _ => lhs.partial_cmp(rhs),
        }
    }

    fn compare_constraint_tiers(&self, _lhs: &Solution<f64>, _rhs: &Solution<f64>) -> Ordering {
        self.tier
    }
}

/// 造一对个体（解内容为空，比较只看适应度）。 / Build a pair of individuals; the empty solutions are never read.
fn pair(model: &ProbeModel, current: f64, candidate: f64) -> (Individual<f64, f64>,
    Individual<f64, f64>)
{
    let _ = model;
    (Individual::new(Solution::new(), current), Individual::new(Solution::new(), candidate))
}

/// 三路判定 + `Metropolis` 抽样。 / The three verdicts plus the Metropolis draws.
fn measure(model: &ProbeModel, current: f64, candidate: f64, temperature: f64, samples: u32,
    seed: u64) -> (String, bool, bool, bool, Option<bool>, u32)
{
    let (current_individual, candidate_individual) = pair(model, current, candidate);
    let iteration = Iteration::new();
    let mut entropy = SeededEntropy::new(seed);
    let ordering = match model.compare_individuals(&candidate_individual, &current_individual) {
        Ordering::Less => "less",
        Ordering::Greater => "greater",
        Ordering::Equal => "equal",
    };
    let mut better_only = BetterOnlyAcceptance;
    let mut not_worse = NotWorseAcceptance;
    let mut always = AlwaysAcceptance;
    let mut metropolis = MetropolisAcceptance::new(Box::new(GeometricSource::new(temperature,
        0.95)));
    let better = better_only.accept(model, &iteration, &current_individual, &candidate_individual,
        &mut entropy);
    let not_worse_verdict = not_worse.accept(model, &iteration, &current_individual,
        &candidate_individual, &mut entropy);
    let always_verdict = always.accept(model, &iteration, &current_individual, &candidate_individual,
        &mut entropy);
    let mut first = None;
    let mut accepted = 0;
    for index in 0..samples {
        let verdict = metropolis.accept(model, &iteration, &current_individual,
            &candidate_individual, &mut entropy);
        if index == 0 { first = Some(verdict); }
        if verdict { accepted += 1; }
    }
    (ordering.to_owned(), better, not_worse_verdict, always_verdict, first, accepted)
}

/// 走一遍接受判据探针。非法输入是**错误**。 / Walk the acceptance probe.
pub fn verify_acceptance(request: &AcceptanceRequest) -> Result<AcceptanceReport> {
    if !request.current_fitness.is_finite() || !request.candidate_fitness.is_finite() {
        return Err(BridgeError::new("InvalidInput", "acceptance probe failed validation: \
            non-finite fitness cannot cross the boundary"));
    }
    let tier = match request.tier.as_deref().unwrap_or("equal") {
        "equal" => Ordering::Equal,
        "candidate_better" | "candidateBetter" => Ordering::Less,
        "candidate_worse" | "candidateWorse" => Ordering::Greater,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "acceptance probe failed validation: unknown tier `{other}`"))),
    };
    let comparison: &'static str = match request.comparison.as_deref().unwrap_or("normal") {
        "normal" => "normal",
        "always_less" | "alwaysLess" => "always_less",
        "always_none" | "alwaysNone" => "always_none",
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "acceptance probe failed validation: unknown comparison `{other}`"))),
    };
    let samples = request.samples.unwrap_or(0);
    if samples > 10_000 {
        return Err(BridgeError::new("InvalidInput",
            "acceptance probe failed validation: at most 10000 samples are accepted"));
    }
    let temperature = request.temperature.unwrap_or(100.0);
    if !temperature.is_finite() || temperature < 0.0 {
        return Err(BridgeError::new("InvalidInput", "acceptance probe failed validation: \
            the temperature must be finite and nonnegative"));
    }
    let model = ProbeModel { comparison, tier };
    let findings = canonical_findings();
    let (ordering, better_only, not_worse, always, metropolis_first, metropolis_accepted) =
        measure(&model, request.current_fitness, request.candidate_fitness, temperature, samples,
            request.seed.unwrap_or(0));
    Ok(AcceptanceReport {
        ordering,
        better_only,
        not_worse,
        always,
        metropolis_first,
        metropolis_accepted,
        better_only_accepts_only_strict_improvement:
            findings.better_only_accepts_only_strict_improvement,
        not_worse_accepts_ties_as_well: findings.not_worse_accepts_ties_as_well,
        a_tie_is_detected_by_both_directions_agreeing:
            findings.a_tie_is_detected_by_both_directions_agreeing,
        a_none_comparison_falls_back_to_equal: findings.a_none_comparison_falls_back_to_equal,
        always_acceptance_is_not_unconditional: findings.always_acceptance_is_not_unconditional,
        metropolis_settles_on_the_tier_first: findings.metropolis_settles_on_the_tier_first,
        the_temperature_guard_precedes_the_tie_acceptance:
            findings.the_temperature_guard_precedes_the_tie_acceptance,
        metropolis_is_reproducible_and_shrinks_with_delta:
            findings.metropolis_is_reproducible_and_shrinks_with_delta,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    better_only_accepts_only_strict_improvement: bool,
    not_worse_accepts_ties_as_well: bool,
    a_tie_is_detected_by_both_directions_agreeing: bool,
    a_none_comparison_falls_back_to_equal: bool,
    always_acceptance_is_not_unconditional: bool,
    metropolis_settles_on_the_tier_first: bool,
    the_temperature_guard_precedes_the_tie_acceptance: bool,
    metropolis_is_reproducible_and_shrinks_with_delta: bool,
}

/// 造一个正常比较器 + 给定层级的模型。 / A normal comparator with the given tier.
fn normal(tier: Ordering) -> ProbeModel {
    ProbeModel { comparison: "normal", tier }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) `BetterOnlyAcceptance` 只在严格更优时接受。 / BetterOnly accepts strict improvement only.
        let better_only_accepts_only_strict_improvement = {
            let model = normal(Ordering::Equal);
            // 更优 → 接受；平局与更差 → 拒绝。 / Better accepts; a tie and worse reject.
            measure(&model, 5.0, 3.0, 100.0, 0, 0).1
                && !measure(&model, 3.0, 3.0, 100.0, 0, 0).1
                && !measure(&model, 3.0, 5.0, 100.0, 0, 0).1
        };
        // 2) `NotWorseAcceptance` 连平局一起接受。 / NotWorse accepts ties too.
        let not_worse_accepts_ties_as_well = {
            let model = normal(Ordering::Equal);
            // 两种判据的差别**只**在平局。 / The two differ **only** on the tie.
            measure(&model, 5.0, 3.0, 100.0, 0, 0).2
                && measure(&model, 3.0, 3.0, 100.0, 0, 0).2
                && !measure(&model, 3.0, 5.0, 100.0, 0, 0).2
                // 平局处正是分家点。 / The tie is exactly where they split.
                && !measure(&model, 3.0, 3.0, 100.0, 0, 0).1
        };
        // 3) 平局由"两个方向都返回更优"判出。 / A tie comes from both directions claiming to be better.
        let a_tie_is_detected_by_both_directions_agreeing = {
            // 自相矛盾的比较器：候选与当前都说自己更优 → 读作平局。 
            // A self-contradictory comparator where both claim to be better reads as a tie.
            let model = ProbeModel { comparison: "always_less", tier: Ordering::Equal };
            let (ordering, better, not_worse, _, _, _) = measure(&model, 3.0, 9.0, 100.0, 0, 0);
            // 目标明明差了 6，却被当成平局。 / The objectives differ by six yet it reads as a tie.
            ordering == "equal" && !better && not_worse
        };
        // 4) `None` 比较也落到平局。 / A `None` comparison falls back to a tie.
        let a_none_comparison_falls_back_to_equal = {
            // `NaN` 目标走的正是这条路：`partial_cmp` 两侧都是 `None`。 
            // A `NaN` objective takes exactly this path: `partial_cmp` is `None` both ways.
            let model = ProbeModel { comparison: "always_none", tier: Ordering::Equal };
            let (ordering, better, not_worse, _, _, _) = measure(&model, 3.0, 9.0, 100.0, 0, 0);
            ordering == "equal" && !better && not_worse
        };
        // 5) `AlwaysAcceptance` 并不"无条件"。 / `AlwaysAcceptance` is not unconditional.
        let always_acceptance_is_not_unconditional = {
            // 层级更差时，哪怕目标更好也拒绝。 / With a worse tier it rejects even a better objective.
            let worse = normal(Ordering::Greater);
            !measure(&worse, 5.0, 3.0, 100.0, 0, 0).3
                // 层级相同/更好则接受。 / An equal or better tier is accepted.
                && measure(&normal(Ordering::Equal), 5.0, 3.0, 100.0, 0, 0).3
                && measure(&normal(Ordering::Less), 3.0, 5.0, 100.0, 0, 0).3
        };
        // 6) `Metropolis` 先按层级结案。 / Metropolis settles on the tier first.
        let metropolis_settles_on_the_tier_first = {
            // 层级更差 → 拒绝（目标更好也没用）——但注意 `samples = 1` 才看得到这一票。 
            // A worse tier rejects even a better objective; a single sample exposes that vote.
            let worse = measure(&normal(Ordering::Greater), 5.0, 3.0, 100.0, 1, 0);
            // 层级更好 → 接受（目标更差也行）。 / A better tier accepts even a worse objective.
            let better = measure(&normal(Ordering::Less), 9.0, 1.0, 100.0, 1, 0);
            worse.4 == Some(false) && better.4 == Some(true)
        };
        // 7) 温度守卫排在 `delta <= 0` 之前。 / The temperature guard precedes the tie acceptance.
        let the_temperature_guard_precedes_the_tie_acceptance = {
            // 温度 0：更差的对拒绝、更优的对在温度之前就返回真；而**平局**要看温度是否还活着。 
            // At temperature zero a worse pair is rejected and an improving pair returns true before the temperature is
            // read, while a **tie** hinges on whether the temperature is still alive.
            let worse = measure(&normal(Ordering::Equal), 5.0, 9.0, 0.0, 1, 0);
            let improving = measure(&normal(Ordering::Equal), 9.0, 5.0, 0.0, 1, 0);
            let frozen_tie = measure(&normal(Ordering::Equal), 5.0, 5.0, 0.0, 1, 0);
            let live_tie = measure(&normal(Ordering::Equal), 5.0, 5.0, 100.0, 1, 0);
            worse.4 == Some(false) && improving.4 == Some(true)
                // 温度冻住 → 平局被拒；温度还活着 → 平局被接受。 
                // A frozen temperature rejects the tie while a live one accepts it.
                && frozen_tie.4 == Some(false) && live_tie.4 == Some(true)
        };
        // 8) 同 seed 可复现、`Δ` 越大接受越少。 / Reproducible per seed, fewer accepts as delta grows.
        let metropolis_is_reproducible_and_shrinks_with_delta = {
            let model = normal(Ordering::Equal);
            // 温度 10、Δ = 5 与 Δ = 25：后者接受率明显更低。 
            // At temperature ten, a delta of five accepts much more often than a delta of twenty-five.
            let near = measure(&model, 10.0, 15.0, 10.0, 200, 42).5;
            let far = measure(&model, 10.0, 35.0, 10.0, 200, 42).5;
            // 同一 seed 两趟一致。 / Two runs from one seed agree.
            let repeat = measure(&model, 10.0, 15.0, 10.0, 200, 42).5;
            repeat == near && far < near
        };
        Findings {
            better_only_accepts_only_strict_improvement,
            not_worse_accepts_ties_as_well,
            a_tie_is_detected_by_both_directions_agreeing,
            a_none_comparison_falls_back_to_equal,
            always_acceptance_is_not_unconditional,
            metropolis_settles_on_the_tier_first,
            the_temperature_guard_precedes_the_tie_acceptance,
            metropolis_is_reproducible_and_shrinks_with_delta,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AcceptanceRequest {
        AcceptanceRequest { current_fitness: 3.0, candidate_fitness: 5.0, tier: None,
            comparison: None, temperature: Some(100.0), samples: Some(0), seed: Some(0) }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_pair() {
        let report = verify_acceptance(&request()).unwrap();
        assert!(report.better_only_accepts_only_strict_improvement);
        assert!(report.not_worse_accepts_ties_as_well);
        assert!(report.a_tie_is_detected_by_both_directions_agreeing);
        assert!(report.a_none_comparison_falls_back_to_equal);
        assert!(report.always_acceptance_is_not_unconditional);
        assert!(report.metropolis_settles_on_the_tier_first);
        assert!(report.the_temperature_guard_precedes_the_tie_acceptance);
        assert!(report.metropolis_is_reproducible_and_shrinks_with_delta);
        // 候选更差：两种判据都拒绝，`Always` 层级相同仍接受。 
        // A worse candidate is rejected by both, while `Always` still accepts at an equal tier.
        assert_eq!(report.ordering, "greater");
        assert!(!report.better_only);
        assert!(!report.not_worse);
        assert!(report.always);
        assert_eq!(report.metropolis_first, None);
        assert_eq!(report.metropolis_accepted, 0);
    }

    #[test]
    fn a_better_candidate_is_accepted_and_a_tie_only_by_not_worse() {
        let better = verify_acceptance(&AcceptanceRequest { current_fitness: 5.0,
            candidate_fitness: 3.0, ..request() }).unwrap();
        assert_eq!(better.ordering, "less");
        assert!(better.better_only);
        assert!(better.not_worse);
        let tied = verify_acceptance(&AcceptanceRequest { current_fitness: 3.0,
            candidate_fitness: 3.0, ..request() }).unwrap();
        assert_eq!(tied.ordering, "equal");
        // 平局处正是两种判据的分家点。 / The tie is exactly where the two criteria split.
        assert!(!tied.better_only);
        assert!(tied.not_worse);
    }

    #[test]
    fn the_always_criterion_rejects_a_worse_constraint_tier() {
        // 目标更好但层级更差 → `AlwaysAcceptance` 拒绝。 
        // A better objective on a worse tier is rejected by `AlwaysAcceptance`.
        let worse = verify_acceptance(&AcceptanceRequest { current_fitness: 5.0,
            candidate_fitness: 3.0, tier: Some("candidateWorse".to_owned()), ..request() })
            .unwrap();
        assert_eq!(worse.ordering, "less");
        assert!(worse.better_only);
        assert!(!worse.always);
        // 层级更好 → 哪怕目标更差也接受。 / A better tier is accepted even with a worse objective.
        let better_tier = verify_acceptance(&AcceptanceRequest { current_fitness: 3.0,
            candidate_fitness: 5.0, tier: Some("candidateBetter".to_owned()), ..request() })
            .unwrap();
        assert!(!better_tier.better_only);
        assert!(better_tier.always);
        assert_eq!(better_tier.metropolis_first, None);
    }

    #[test]
    fn a_self_contradictory_comparator_reads_as_a_tie() {
        let report = verify_acceptance(&AcceptanceRequest { current_fitness: 3.0,
            candidate_fitness: 9.0, comparison: Some("alwaysLess".to_owned()), ..request() })
            .unwrap();
        // 目标差 6，却被读成平局。 / The objectives differ by six yet it reads as a tie.
        assert_eq!(report.ordering, "equal");
        assert!(!report.better_only);
        assert!(report.not_worse);
    }

    #[test]
    fn a_never_committing_comparator_reads_as_a_tie_too() {
        let report = verify_acceptance(&AcceptanceRequest { current_fitness: 3.0,
            candidate_fitness: 9.0, comparison: Some("alwaysNone".to_owned()), ..request() })
            .unwrap();
        assert_eq!(report.ordering, "equal");
        assert!(report.not_worse);
    }

    #[test]
    fn metropolis_draws_are_reproducible_and_temperature_bounded() {
        let plain = AcceptanceRequest { current_fitness: 5.0, candidate_fitness: 9.0,
            tier: Some("equal".to_owned()), comparison: None, temperature: Some(10.0),
            samples: Some(200), seed: Some(42) };
        let first = verify_acceptance(&plain).unwrap();
        let second = verify_acceptance(&plain).unwrap();
        assert_eq!(first.metropolis_accepted, second.metropolis_accepted);
        assert!(first.metropolis_accepted > 0 && first.metropolis_accepted < 200,
            "{first:?}");
        // 温度 0：一律拒绝。 / At temperature zero every draw is rejected.
        let frozen = verify_acceptance(&AcceptanceRequest { temperature: Some(0.0),
            ..plain.clone() }).unwrap();
        assert_eq!(frozen.metropolis_accepted, 0);
        assert_eq!(frozen.metropolis_first, Some(false));
        // 更优的对在温度 0 也接受（温度之前就返回真）。 / An improving pair accepts even at temperature zero.
        let improving = verify_acceptance(&AcceptanceRequest { current_fitness: 9.0,
            candidate_fitness: 5.0, ..plain }).unwrap();
        assert_eq!(improving.metropolis_accepted, 200);
        assert_eq!(improving.metropolis_first, Some(true));
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_acceptance(&AcceptanceRequest { current_fitness: f64::INFINITY,
            ..request() }).unwrap_err().message.contains("non-finite fitness"));
        assert!(verify_acceptance(&AcceptanceRequest { tier: Some("nope".to_owned()),
            ..request() }).unwrap_err().message.contains("unknown tier"));
        assert!(verify_acceptance(&AcceptanceRequest { comparison: Some("nope".to_owned()),
            ..request() }).unwrap_err().message.contains("unknown comparison"));
        assert!(verify_acceptance(&AcceptanceRequest { samples: Some(20_000), ..request() })
            .unwrap_err().message.contains("at most 10000 samples"));
        assert!(verify_acceptance(&AcceptanceRequest { temperature: Some(-1.0), ..request() })
            .unwrap_err().message.contains("finite and nonnegative"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_acceptance(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"betterOnly\""), "{json}");
        assert!(json.contains("\"metropolisAccepted\""), "{json}");
        assert!(json.contains("\"alwaysAcceptanceIsNotUnconditional\""), "{json}");
        assert!(!json.contains("metropolis_accepted"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<AcceptanceReport>(&copy.to_string()).is_err());
    }
}
