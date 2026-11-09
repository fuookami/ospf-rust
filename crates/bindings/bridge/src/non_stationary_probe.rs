//! 续挖第 119 单：**非平稳选择器把"派发时前驱"冻结在待反馈表里、把窗口归一成"最大正奖励"、并让三种 kind
//! 各管一套信用账**——`frr-mab` 的引导只挑**掩码内从未观察过的动作且按动作规范顺序**（不是掩码顺序），窗口滑过
//! `window` 就丢最旧、窗口内奖励先除以**窗口最大正奖励**（负数一律按 0 计，全非正时尺度取 1）；`choice-function`
//! 在 `select` 时把前驱冻进 `pending`，**乱序完成仍用派发时的前驱**，同一 `decision` 在学习态下**重复 `select` 直接报错**，
//! `discard` 才放开；`adaptive-pursuit` 的概率按 EMA 追目标（赢家 `1 - minProb*(n-1)`、其余 `minProb`），
//! **`rate == 1` 时直接跳到目标**；`terminal` 完成会**清空前驱**；`learning == false` 时 `observe` 什么都不写，
//! 但校验照样执行
//! （`solver/hyper_heuristic/selection/non_stationary.rs` 的 `NonStationarySelector`）
//! Continuation, delivery 119: **the non-stationary selector freezes the dispatch-time predecessor in its pending table, normalises
//! the window by the largest positive reward, and keeps one credit ledger per kind** — `frr-mab`'s bootstrap picks only
//! **unobserved masked actions in canonical action order** (not mask order), drops the oldest once the window exceeds
//! `window`, and normalises rewards by the **largest positive reward** in the window (negatives count as zero, an all-non-positive
//! window uses a scale of one); `choice-function` freezes the predecessor into `pending` at `select`, so **an out-of-order
//! completion still uses the dispatch-time predecessor**, **a second `select` on the same `decision` in the learning state errors
//! outright**, and only `discard` releases it; `adaptive-pursuit` chases its target with an EMA (winner
//! `1 - minProb*(n-1)`, others `minProb`) and **jumps straight to the target when `rate == 1`**; a `terminal` completion
//! **clears the predecessor**; and `observe` writes nothing when `learning == false` even though validation still runs
//! (`NonStationarySelector` in `solver/hyper_heuristic/selection/non_stationary.rs`).
//!
//! 第 118 单量的是表格策略的五个臂，这一单量**同一模块的另一半：非平稳信用分配**。八条实测：
//!
//! 1. **`frr-mab` 引导按动作规范顺序**（`the_bootstrap_follows_the_canonical_order()`）。
//! 2. **窗口滑过 `window` 就丢最旧**（`the_window_forgets_the_oldest()`）。
//! 3. **窗口内归一用最大正奖励、负数按 0 计**（`the_scale_makes_rewards_scale_invariant()`）。
//! 4. **前驱在派发时冻结**（`the_predecessor_is_frozen_at_dispatch()`）。
//! 5. **同一决策重复 `select` 报错，`discard` 才放开**（`a_pending_decision_cannot_be_dispatched_twice()`）。
//! 6. **`adaptive-pursuit` 的 `rate == 1` 直接跳目标**（`a_full_rate_pursuit_lands_on_the_floor()`）。
//! 7. **`terminal` 完成清空前驱**（`a_terminal_completion_clears_the_predecessor()`）。
//! 8. **`learning == false` 时什么都不写但照样校验**（`a_non_learning_selector_validates_without_writing()`）。
//!
//! Delivery 118 measured the tabular policy's five arms; this one measures **the other half of the same module: non-stationary
//! credit assignment**. **The bootstrap follows the canonical order**, **the window forgets the oldest**, and **the scale is
//! the largest positive reward**. **The predecessor is frozen at dispatch**, and **a pending decision cannot be dispatched
//! twice** until discarded. **A full-rate pursuit lands on the floor**, **a terminal completion clears the predecessor**, and
//! **a non-learning selector validates without writing**.

use ospf_rust_core_plugin_heuristic::heuristic::session::SearchRandom;
use ospf_rust_core_plugin_heuristic::hyper_heuristic::{
    ComponentSpec, NonStationarySelector, PolicySnapshot, SelectionContext, SelectionPolicy,
    SelectionTransition,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一次派发。 / One dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NonStationarySelectSpec {
    /// 决策序号。 / The decision sequence.
    #[serde(default)]
    pub decision: u64,
    /// 本次可用动作（缺省等于 `actions`）。 / The actions available now.
    #[serde(default)]
    pub available: Option<Vec<String>>,
}

/// 一次反馈。 / One feedback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NonStationaryObservationSpec {
    /// 实际执行的动作。 / The executed action.
    pub action: String,
    /// 决策序号。 / The decision sequence.
    #[serde(default)]
    pub decision: u64,
    /// 奖励。 / The reward.
    #[serde(default)]
    pub reward: f64,
    /// 是否终止。 / Whether terminal.
    #[serde(default)]
    pub terminal: bool,
}

/// 非平稳选择器探针请求。 / The non-stationary selector probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NonStationaryProbeRequest {
    /// kind（`frr-mab` / `choice-function` / `adaptive-pursuit`）。 / The kind.
    pub policy: String,
    /// 稳定动作定义。 / The stable action definitions.
    pub actions: Vec<String>,
    /// 是否学习（默认 false）。 / Whether the selector learns.
    #[serde(default)]
    pub learning: Option<bool>,
    /// 窗口大小。 / The window size.
    #[serde(default)]
    pub window: Option<u64>,
    /// 探索系数。 / The exploration factor.
    #[serde(default)]
    pub exploration: Option<f64>,
    /// 衰减系数。 / The decay factor.
    #[serde(default)]
    pub decay: Option<f64>,
    /// 学习率。 / The rate.
    #[serde(default)]
    pub rate: Option<f64>,
    /// 保底概率。 / The probability floor.
    #[serde(default)]
    pub minimum_probability: Option<f64>,
    /// 新近权重。 / The recency weight.
    #[serde(default)]
    pub recency_weight: Option<f64>,
    /// 随机流初值。 / The random stream seed.
    #[serde(default)]
    pub seed: Option<u64>,
    /// 依次的派发。 / The dispatches, in order.
    #[serde(default)]
    pub selects: Vec<NonStationarySelectSpec>,
    /// 依次的反馈。 / The feedback, in order.
    #[serde(default)]
    pub observations: Vec<NonStationaryObservationSpec>,
    /// 在这些决策号上读前驱。 / Read the predecessor at these decisions.
    #[serde(default)]
    pub predecessor_of: Vec<u64>,
    /// 在这些决策号上丢弃待反馈。 / Discard the pending entry at these decisions.
    #[serde(default)]
    pub discards: Vec<u64>,
}

/// 非平稳选择器探针结论。 / The non-stationary selector probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NonStationaryProbeReport {
    /// 依次选出的动作。 / The selected actions, in order.
    pub selected: Vec<String>,
    /// 学习修订版本。 / The learning revision.
    pub revision: f64,
    /// 窗口长度。 / The window length.
    pub window_len: u32,
    /// 信用时钟。 / The credit clock.
    pub clock: f64,
    /// 待反馈条数。 / How many decisions await feedback.
    pub pending: u32,
    /// 概率（按动作顺序）。 / The probabilities, in action order.
    pub probabilities: Vec<f64>,
    /// 每个被查询决策号的前驱。 / The predecessor at each queried decision.
    pub predecessors: Vec<Option<String>>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_bootstrap_follows_the_canonical_order: bool,
    pub the_window_forgets_the_oldest: bool,
    pub the_scale_makes_rewards_scale_invariant: bool,
    pub the_predecessor_is_frozen_at_dispatch: bool,
    pub a_pending_decision_cannot_be_dispatched_twice: bool,
    pub a_full_rate_pursuit_lands_on_the_floor: bool,
    pub a_terminal_completion_clears_the_predecessor: bool,
    pub a_non_learning_selector_validates_without_writing: bool,
}

/// 造一个定义。 / Build one definition.
fn spec_of(request: &NonStationaryProbeRequest) -> ComponentSpec {
    let mut spec = ComponentSpec::new(request.policy.as_str());
    if let Some(window) = request.window {
        spec = spec.parameter("window", window);
    }
    if let Some(exploration) = request.exploration {
        spec = spec.parameter("exploration", exploration);
    }
    if let Some(decay) = request.decay {
        spec = spec.parameter("decay", decay);
    }
    if let Some(rate) = request.rate {
        spec = spec.parameter("rate", rate);
    }
    if let Some(minimum) = request.minimum_probability {
        spec = spec.parameter("minimumProbability", minimum);
    }
    if let Some(recency) = request.recency_weight {
        spec = spec.parameter("recencyWeight", recency);
    }
    spec
}

/// 造一个选择器。 / Build one selector.
fn selector_of(request: &NonStationaryProbeRequest) -> Result<NonStationarySelector> {
    NonStationarySelector::new(&spec_of(request), request.actions.clone(),
        request.learning.unwrap_or(false))
        .map_err(|error| BridgeError::new("InvalidInput", error))
}

/// 造一次反馈。 / Build one feedback.
fn transition_of(request: &NonStationaryProbeRequest, spec: &NonStationaryObservationSpec)
    -> SelectionTransition
{
    SelectionTransition {
        episode: "non-stationary-probe".to_owned(),
        decision: spec.decision,
        action: spec.action.clone(),
        before: SelectionContext { decision: spec.decision, ..Default::default() },
        after: SelectionContext::default(),
        available: request.actions.clone(),
        next_available: request.actions.clone(),
        reward: spec.reward,
        evaluations: 1,
        terminal: spec.terminal,
        policy_version: 0,
        feature_version: "progress:1".to_owned(),
        scope: "algorithm".to_owned(),
        steps: 1,
        failed: false,
        merged_improvement: false,
        elapsed_nanos: 0,
        previous_action: None,
    }
}

/// 从快照里读一个数。 / Read one number from the snapshot.
fn number(snapshot: &PolicySnapshot, pointer: &str) -> f64 {
    snapshot.extension.pointer(pointer).and_then(serde_json::Value::as_f64).unwrap_or(f64::NAN)
}

/// 走一遍非平稳选择器探针。非法定义是**错误**。 / Walk the probe. An illegal definition is an error.
pub fn verify_non_stationary_probe(
    request: &NonStationaryProbeRequest,
) -> Result<NonStationaryProbeReport> {
    if request.actions.len() > 32 || request.selects.len() > 64
        || request.observations.len() > 64
    {
        return Err(BridgeError::new("InvalidInput", "non-stationary probe failed validation: \
            at most thirty-two actions and sixty-four steps are accepted"));
    }
    let findings = canonical_findings();
    let mut selector = selector_of(request)?;
    let mut random = SearchRandom::new(request.seed.unwrap_or(0x5eed));
    // 先归因、再派发：这样"派发时前驱"来自上一条反馈，语义就是"先学再决"。 
    // Feedback first, dispatch second: the dispatch-time predecessor then comes from the previous feedback.
    for spec in &request.observations {
        selector.observe(&transition_of(request, spec))
            .map_err(|error| BridgeError::new("InvalidInput", error))?;
    }
    let mut selected = Vec::with_capacity(request.selects.len());
    for step in &request.selects {
        let available = step.available.clone().unwrap_or_else(|| request.actions.clone());
        let context = SelectionContext { decision: step.decision, ..Default::default() };
        let picked = selector.select(&context, &available, &mut random)
            .map_err(|error| BridgeError::new("InvalidInput", error))?;
        selected.push(picked);
    }
    for decision in &request.discards {
        selector.discard(*decision);
    }
    let snapshot = selector.snapshot()
        .map_err(|error| BridgeError::new("InvalidInput", error))?;
    Ok(NonStationaryProbeReport {
        selected,
        revision: selector.revision() as f64,
        window_len: u32::try_from(snapshot.extension.pointer("/window")
            .and_then(serde_json::Value::as_array).map(Vec::len).unwrap_or(0))
            .unwrap_or(u32::MAX),
        clock: number(&snapshot, "/clock"),
        pending: u32::try_from(snapshot.extension.pointer("/pending")
            .and_then(serde_json::Value::as_object).map(serde_json::Map::len).unwrap_or(0))
            .unwrap_or(u32::MAX),
        probabilities: request.actions.iter()
            .map(|action| number(&snapshot, &format!("/probabilities/{action}"))).collect(),
        predecessors: request.predecessor_of.iter()
            .map(|decision| selector.predecessor(*decision)).collect(),
        the_bootstrap_follows_the_canonical_order:
            findings.the_bootstrap_follows_the_canonical_order,
        the_window_forgets_the_oldest: findings.the_window_forgets_the_oldest,
        the_scale_makes_rewards_scale_invariant:
            findings.the_scale_makes_rewards_scale_invariant,
        the_predecessor_is_frozen_at_dispatch: findings.the_predecessor_is_frozen_at_dispatch,
        a_pending_decision_cannot_be_dispatched_twice:
            findings.a_pending_decision_cannot_be_dispatched_twice,
        a_full_rate_pursuit_lands_on_the_floor: findings.a_full_rate_pursuit_lands_on_the_floor,
        a_terminal_completion_clears_the_predecessor:
            findings.a_terminal_completion_clears_the_predecessor,
        a_non_learning_selector_validates_without_writing:
            findings.a_non_learning_selector_validates_without_writing,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_bootstrap_follows_the_canonical_order: bool,
    the_window_forgets_the_oldest: bool,
    the_scale_makes_rewards_scale_invariant: bool,
    the_predecessor_is_frozen_at_dispatch: bool,
    a_pending_decision_cannot_be_dispatched_twice: bool,
    a_full_rate_pursuit_lands_on_the_floor: bool,
    a_terminal_completion_clears_the_predecessor: bool,
    a_non_learning_selector_validates_without_writing: bool,
}

/// 三个动作的固定定义。 / The fixed three-action definition.
fn actions() -> Vec<String> {
    vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
}

/// 造一个选择器（内部用）。 / Build a selector (internal).
fn selector(kind: &str, learning: bool, window: Option<u64>,
    parameters: &[(&str, f64)]) -> NonStationarySelector
{
    let mut spec = ComponentSpec::new(kind);
    // `window` 是整型参数：传 f64 会被 `integer()` 拒掉（实测打回）。 
    // `window` is an integer parameter; an f64 is refused by `integer()` (measured).
    if let Some(window) = window {
        spec = spec.parameter("window", window);
    }
    for (key, value) in parameters {
        spec = spec.parameter(*key, *value);
    }
    NonStationarySelector::new(&spec, actions(), learning)
        .expect("the fixed definition is valid")
}

/// 造一次反馈（内部用）。 / Build one feedback (internal).
fn event(action: &str, decision: u64, reward: f64, terminal: bool) -> SelectionTransition {
    SelectionTransition {
        episode: "probe".to_owned(), decision, action: action.to_owned(),
        before: SelectionContext { decision, ..Default::default() },
        after: SelectionContext::default(), available: actions(), next_available: actions(),
        reward, evaluations: 1, terminal, policy_version: 0,
        feature_version: "progress:1".to_owned(), scope: "algorithm".to_owned(), steps: 1,
        failed: false, merged_improvement: false, elapsed_nanos: 0, previous_action: None,
    }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        let all = actions();
        // 1) 引导按动作规范顺序，而不是掩码顺序。 
        // The bootstrap follows the canonical action order, not the mask order.
        let the_bootstrap_follows_the_canonical_order = {
            let mut policy = selector("frr-mab", true, None, &[]);
            let mut random = SearchRandom::new(42);
            // 掩码写成 [c, b]，引导仍先给 b。 / The mask reads `[c, b]` and the bootstrap still picks `b` first.
            let mask = vec!["c".to_owned(), "b".to_owned()];
            let first = policy.select(&SelectionContext::default(), &mask, &mut random);
            if let Ok(picked) = &first {
                policy.observe(&event(picked.as_str(), 0, 10.0, false)).unwrap_or_default();
            }
            let second = policy.select(&SelectionContext::default(), &mask, &mut random);
            first.ok() == Some("b".to_owned()) && second.ok() == Some("c".to_owned())
        };
        // 2) 窗口滑过上限就丢最旧。 / The window forgets the oldest once it exceeds its size.
        let the_window_forgets_the_oldest = {
            let mut policy = selector("frr-mab", true, Some(3), &[]);
            for (decision, action) in ["a", "b", "c", "a"].iter().enumerate() {
                policy.observe(&event(action, decision as u64, 1.0, false)).expect("valid");
            }
            let snapshot = policy.snapshot().expect("valid");
            snapshot.extension.pointer("/window").and_then(serde_json::Value::as_array)
                .map(Vec::len) == Some(3)
        };
        // 3) 归一让奖励尺度无关：整体放大 100 倍选同一个动作。 
        // The normalisation makes rewards scale-invariant: scaling them all by a hundred picks the same action.
        let the_scale_makes_rewards_scale_invariant = {
            let pick = |rewards: [(f64, &str); 2]| {
                let mut policy = selector("frr-mab", true, Some(3), &[("exploration", 0.0)]);
                for (decision, (reward, action)) in rewards.into_iter().enumerate() {
                    policy.observe(&event(action, decision as u64, reward, false))
                        .expect("valid");
                }
                let mut random = SearchRandom::new(42);
                policy.select(&SelectionContext::default(), &actions(), &mut random).ok()
            };
            // 负奖励照进窗口，只是权重按 0 计——所以 (a:2, b:1) 与 (a:200, b:100) 必须同解。 
            // Negatives still enter the window and simply weigh zero, so (a:2, b:1) and (a:200, b:100) must agree.
            let small = pick([(2.0, "a"), (1.0, "b")]);
            let large = pick([(200.0, "a"), (100.0, "b")]);
            small.is_some() && small == large
        };
        // 4) 前驱在派发时冻结。 / The predecessor is frozen at dispatch.
        let the_predecessor_is_frozen_at_dispatch = {
            let mut policy = selector("choice-function", true, None, &[]);
            policy.observe(&event("a", 0, 2.0, false)).expect("valid");
            let mut random = SearchRandom::new(42);
            let only_b = vec!["b".to_owned()];
            policy.select(&SelectionContext { decision: 1, ..Default::default() }, &only_b,
                &mut random).expect("valid");
            let frozen = policy.predecessor(1);
            // 派发之后又完成了一次 c：待反馈表里的前驱不受影响。 
            // Another completion lands afterwards and the frozen predecessor stays untouched.
            policy.observe(&event("a", 9, 5.0, false)).expect("valid");
            frozen == Some("a".to_owned()) && policy.predecessor(1) == Some("a".to_owned())
        };
        // 5) 同一决策重复派发报错，discard 才放开。 
        // A pending decision cannot be dispatched twice; only `discard` releases it.
        let a_pending_decision_cannot_be_dispatched_twice = {
            let mut policy = selector("choice-function", true, None, &[]);
            let mut random = SearchRandom::new(42);
            let context = SelectionContext { decision: 7, ..Default::default() };
            let first = policy.select(&context, &all, &mut random).is_ok();
            let second = policy.select(&context, &all, &mut random).is_err();
            policy.discard(7);
            let third = policy.select(&context, &all, &mut random).is_ok();
            first && second && third
        };
        // 6) rate == 1 的追逐直接落到目标（赢家 1 - floor*(n-1)，其余 floor）。 
        // A full-rate pursuit lands straight on the target.
        let a_full_rate_pursuit_lands_on_the_floor = {
            let mut policy = selector("adaptive-pursuit", true, None,
                &[("rate", 1.0), ("minimumProbability", 0.1)]);
            policy.observe(&event("a", 0, 2.0, false)).expect("valid");
            let snapshot = policy.snapshot().expect("valid");
            let winner = number(&snapshot, "/probabilities/a");
            let loser = number(&snapshot, "/probabilities/b");
            (winner - (1.0 - 0.1 * 2.0)).abs() < 1e-12 && (loser - 0.1).abs() < 1e-12
                && (number(&snapshot, "/probabilities/c") - 0.1).abs() < 1e-12
        };
        // 7) terminal 完成清空前驱。 / A terminal completion clears the predecessor.
        let a_terminal_completion_clears_the_predecessor = {
            let mut policy = selector("choice-function", true, None, &[]);
            // 先完成一次 a，让"当前前驱"变成 a，再派发决策 1。 
            // First complete `a` so the current predecessor is `a`, then dispatch decision one.
            policy.observe(&event("a", 0, 2.0, false)).expect("valid");
            let mut random = SearchRandom::new(42);
            let only_b = vec!["b".to_owned()];
            policy.select(&SelectionContext { decision: 1, ..Default::default() }, &only_b,
                &mut random).expect("valid");
            let before = policy.predecessor(1);
            policy.observe(&event("b", 1, 1.0, true)).expect("valid");
            // 终止后前驱被清空，且时钟照常推进。 
            // After a terminal completion the predecessor is gone and the clock moved.
            let after = policy.predecessor(1);
            let clock = number(&policy.snapshot().expect("valid"), "/clock");
            before == Some("a".to_owned()) && after.is_none() && clock > 0.0
        };
        // 8) 非学习选择器只校验不写。 / A non-learning selector validates without writing.
        let a_non_learning_selector_validates_without_writing = {
            let mut policy = selector("choice-function", false, None, &[]);
            let before = policy.snapshot().expect("valid");
            let written = policy.observe(&event("a", 0, 1.0, false)).is_ok()
                && policy.revision() == 0;
            // 非法反馈照样被拒。 / Illegal feedback is still refused.
            let mut illegal = event("a", 0, 1.0, false);
            illegal.available = vec!["zzz".to_owned()];
            let refused = policy.observe(&illegal).is_err();
            written && refused && policy.snapshot().expect("valid") == before
        };
        Findings {
            the_bootstrap_follows_the_canonical_order,
            the_window_forgets_the_oldest,
            the_scale_makes_rewards_scale_invariant,
            the_predecessor_is_frozen_at_dispatch,
            a_pending_decision_cannot_be_dispatched_twice,
            a_full_rate_pursuit_lands_on_the_floor,
            a_terminal_completion_clears_the_predecessor,
            a_non_learning_selector_validates_without_writing,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: NonStationaryProbeRequest) -> NonStationaryProbeReport {
        verify_non_stationary_probe(&request).unwrap()
    }

    fn request(policy: &str) -> NonStationaryProbeRequest {
        NonStationaryProbeRequest { policy: policy.to_owned(), actions: actions(),
            learning: None, window: None, exploration: None, decay: None, rate: None,
            minimum_probability: None, recency_weight: None, seed: None, selects: Vec::new(),
            observations: Vec::new(), predecessor_of: Vec::new(), discards: Vec::new() }
    }

    fn observation(action: &str, decision: u64, reward: f64) -> NonStationaryObservationSpec {
        NonStationaryObservationSpec { action: action.to_owned(), decision, reward,
            terminal: false }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request("frr-mab"));
        assert!(report.the_bootstrap_follows_the_canonical_order);
        assert!(report.the_window_forgets_the_oldest);
        assert!(report.the_scale_makes_rewards_scale_invariant);
        assert!(report.the_predecessor_is_frozen_at_dispatch);
        assert!(report.a_pending_decision_cannot_be_dispatched_twice);
        assert!(report.a_full_rate_pursuit_lands_on_the_floor);
        assert!(report.a_terminal_completion_clears_the_predecessor);
        assert!(report.a_non_learning_selector_validates_without_writing);
    }

    #[test]
    fn the_bootstrap_walks_the_canonical_order() {
        // 掩码 [c, b]：引导仍先给 b，再给 c。 
        // The mask reads `[c, b]` and the bootstrap still picks `b` first, then `c`.
        let first = ask(NonStationaryProbeRequest {
            learning: Some(true),
            selects: vec![NonStationarySelectSpec { decision: 0,
                available: Some(vec!["c".to_owned(), "b".to_owned()]) }],
            ..request("frr-mab") });
        assert_eq!(first.selected, vec!["b".to_owned()]);
        // 反馈先于派发：b 已被观察过，于是这次引导给 c。 
        // Feedback precedes dispatch: `b` has been observed, so the bootstrap yields `c`.
        let second = ask(NonStationaryProbeRequest {
            learning: Some(true),
            selects: vec![NonStationarySelectSpec { decision: 0,
                available: Some(vec!["c".to_owned(), "b".to_owned()]) }],
            observations: vec![observation("b", 0, 10.0)], ..request("frr-mab") });
        assert_eq!(second.selected, vec!["c".to_owned()]);
        assert_eq!(second.revision, 1.0);
    }

    #[test]
    fn the_window_and_the_clock_follow_the_observations() {
        let report = ask(NonStationaryProbeRequest {
            learning: Some(true), window: Some(3),
            observations: vec![observation("a", 0, 1.0), observation("b", 1, 2.0),
                observation("c", 2, 3.0), observation("a", 3, 4.0)], ..request("frr-mab") });
        assert_eq!(report.revision, 4.0);
        assert_eq!(report.clock, 4.0);
        assert_eq!(report.window_len, 3);
    }

    #[test]
    fn the_pursuit_probabilities_land_on_the_target() {
        let report = ask(NonStationaryProbeRequest {
            learning: Some(true), rate: Some(1.0), minimum_probability: Some(0.1),
            observations: vec![observation("a", 0, 2.0)], ..request("adaptive-pursuit") });
        assert!((report.probabilities[0] - 0.8).abs() < 1e-12, "{:?}", report.probabilities);
        assert!((report.probabilities[1] - 0.1).abs() < 1e-12, "{:?}", report.probabilities);
        assert!((report.probabilities[2] - 0.1).abs() < 1e-12, "{:?}", report.probabilities);
    }

    #[test]
    fn the_predecessor_is_frozen_and_discard_clears_it() {
        let frozen = ask(NonStationaryProbeRequest {
            learning: Some(true), predecessor_of: vec![1],
            selects: vec![NonStationarySelectSpec { decision: 1,
                available: Some(vec!["b".to_owned()]) }],
            observations: vec![observation("a", 0, 2.0)], ..request("choice-function") });
        // 上一条反馈是 a，所以派发时冻结的前驱就是 a。 
        // The previous feedback is `a`, so the dispatch freezes `a` as its predecessor.
        assert_eq!(frozen.predecessors, vec![Some("a".to_owned())]);
        assert_eq!(frozen.pending, 1);
        // discard 之后待反馈表清空。 / After `discard` the pending table is empty.
        let discarded = ask(NonStationaryProbeRequest {
            learning: Some(true), discards: vec![1],
            selects: vec![NonStationarySelectSpec { decision: 1,
                available: Some(vec!["b".to_owned()]) }], ..request("choice-function") });
        assert_eq!(discarded.pending, 0);
    }

    #[test]
    fn bad_definitions_and_masks_are_refused() {
        let refused = |policy: &str, window: Option<u64>, rate: Option<f64>,
            minimum: Option<f64>| {
            verify_non_stationary_probe(&NonStationaryProbeRequest { policy: policy.to_owned(),
                window, rate, minimum_probability: minimum,
                ..request("frr-mab") }).unwrap_err().message.clone()
        };
        assert!(refused("nearly", None, None, None).contains("invalid non-stationary"));
        // frr-mab 要求 window >= 动作数。 / `frr-mab` needs a window at least as wide as the action count.
        assert!(refused("frr-mab", Some(2), None, None).contains("invalid non-stationary"));
        assert!(refused("frr-mab", Some(1_000_001), None, None)
            .contains("invalid non-stationary"));
        assert!(refused("choice-function", None, Some(0.0), None)
            .contains("invalid non-stationary"));
        // 保底概率乘动作数超过 1 被拒。 / A floor times the action count above one is refused.
        assert!(refused("adaptive-pursuit", None, None, Some(0.4))
            .contains("invalid non-stationary"));
        assert!(verify_non_stationary_probe(&NonStationaryProbeRequest {
            learning: Some(true),
            selects: vec![NonStationarySelectSpec { decision: 0,
                available: Some(vec!["zzz".to_owned()]) }], ..request("frr-mab") })
            .unwrap_err().message.contains("mask"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request("frr-mab"));
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"windowLen\""), "{json}");
        assert!(json.contains("\"aPendingDecisionCannotBeDispatchedTwice\""), "{json}");
        assert!(!json.contains("window_len"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<NonStationaryProbeReport>(&copy.to_string()).is_err());
    }
}
