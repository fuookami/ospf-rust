//! 续挖第 118 单：**选择策略的七种 kind 只在五个臂里分家，`fixed` 与 `round-robin` 是同一条臂，累计只发生三种
//! kind 上**——`TabularSelectionPolicy::select` 里 `fixed | round-robin` 共用一个臂（游标推进成 `index + 1`），
//! `stagnation` 只在该值**非零且是 limit 的整数倍**时推进游标，`ucb1` 在**学习态**下先选**从未观察过**的动作
//! （且分母的 `total` 是**全局**计数之和、含掩码外动作）；`observe` 在 `learning == false` 时**什么都不学**，
//! 且只对 `q-learning`/`epsilon-greedy`/`ucb1` 三种 kind 累计 `counts`/`sums`；`snapshot().table` **只有
//! `q-learning` 非空**；非法定义（重复动作、未知掩码、越界状态、参数出界、容量超限）一律被拒
//! （`solver/hyper_heuristic/selection.rs` 的 `TabularSelectionPolicy` / `SelectionPolicy` / `PolicySnapshot`）
//! Continuation, delivery 118: **the seven selection kinds split into only five arms, `fixed` and `round-robin` share one,
//! and accumulation happens on three kinds only** — in `TabularSelectionPolicy::select` the `fixed | round-robin` arm shares a
//! cursor advanced to `index + 1`, `stagnation` advances the cursor only when that value is **non-zero and a multiple of the
//! limit**, and `ucb1` in the learning state picks an **unobserved** action first (its `total` is the **global** count sum,
//! including actions outside the mask); `observe` **learns nothing** when `learning == false` and accumulates
//! `counts`/`sums` for `q-learning`/`epsilon-greedy`/`ucb1` only; `snapshot().table` is **non-empty for `q-learning` only**;
//! and illegal definitions (duplicate actions, unknown mask entries, out-of-range states, out-of-range parameters, capacity
//! overflow) are refused (`TabularSelectionPolicy` / `SelectionPolicy` / `PolicySnapshot` in
//! `solver/hyper_heuristic/selection.rs`).
//!
//! 前几单量的是求解值与 lowering 报告，这一单量**超启发式的选择策略本体**。八条实测：
//!
//! 1. **`fixed` 与 `round-robin` 完全同义**（`the_two_rule_kinds_share_one_arm()`）。
//! 2. **规则臂的游标推进成"选中索引 + 1"**（`the_rule_cursor_advances_past_the_pick()`）。
//! 3. **`stagnation` 只在非零且为 limit 整数倍时推进**（`stagnation_advances_only_on_a_nonzero_multiple()`）。
//! 4. **`ucb1` 学习态先选未观察动作**（`ucb1_prefers_unobserved_actions()`）。
//! 5. **`learning == false` 时 `observe` 什么都不学**（`a_non_learning_policy_learns_nothing()`）。
//! 6. **只有三种 kind 累计 `counts`/`sums`**（`only_three_kinds_accumulate()`）。
//! 7. **`snapshot().table` 只有 `q-learning` 非空**（`only_q_learning_exports_a_table()`）。
//! 8. **六类非法定义都被拒**（`six_illegal_definitions_are_refused()`）。
//!
//! Earlier deliveries measured solve values and the lowering report; this one measures **the hyper-heuristic selection policy
//! itself**. **`fixed` and `round-robin` share one arm**, and **the rule cursor advances past the pick**. **Stagnation advances
//! only on a non-zero multiple** of its limit. **`ucb1` prefers unobserved actions**. **A non-learning policy learns
//! nothing**, and **only three kinds accumulate**. **Only `q-learning` exports a table**, and **six illegal definitions are
//! refused**.

use ospf_rust_core_plugin_heuristic::heuristic::session::SearchRandom;
use ospf_rust_core_plugin_heuristic::hyper_heuristic::{
    ComponentSpec, PolicySnapshot, SelectionContext, SelectionPolicy, SelectionTransition,
    TabularSelectionPolicy,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一次选择请求。 / One selection step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionStepSpec {
    /// 离散状态。 / The discrete state.
    #[serde(default)]
    pub state: u64,
    /// 连续停滞次数。 / Consecutive stagnant outcomes.
    #[serde(default)]
    pub stagnation: u64,
    /// 本次可用动作。 / The actions available now.
    #[serde(default)]
    pub available: Vec<String>,
}

/// 一次反馈归因。 / One attribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionObservationSpec {
    /// 实际执行的动作。 / The executed action.
    pub action: String,
    /// 奖励。 / The reward.
    #[serde(default)]
    pub reward: f64,
    /// 派发时状态。 / The dispatch state.
    #[serde(default)]
    pub before: u64,
    /// 完成时状态。 / The completion state.
    #[serde(default)]
    pub after: u64,
    /// 派发时可用动作（缺省等于 `actions`）。 / The available actions at dispatch.
    #[serde(default)]
    pub available: Option<Vec<String>>,
    /// 后继可用动作（缺省为空）。 / The successor's available actions.
    #[serde(default)]
    pub next_available: Option<Vec<String>>,
    /// 是否终止。 / Whether terminal.
    #[serde(default)]
    pub terminal: bool,
}

/// 选择策略探针请求。 / The selection policy probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionProbeRequest {
    /// 策略 kind（`fixed` / `round-robin` / `stagnation` / `random` / `epsilon-greedy` / `ucb1` / `q-learning`）。
    /// The policy kind.
    pub policy: String,
    /// 稳定动作定义。 / The stable action definitions.
    pub actions: Vec<String>,
    /// 是否学习（默认 false）。 / Whether the policy learns.
    #[serde(default)]
    pub learning: Option<bool>,
    /// 探索率。 / The exploration rate.
    #[serde(default)]
    pub epsilon: Option<f64>,
    /// 学习率。 / The learning rate.
    #[serde(default)]
    pub alpha: Option<f64>,
    /// 折扣因子。 / The discount factor.
    #[serde(default)]
    pub gamma: Option<f64>,
    /// 状态数（默认 64）。 / The state count.
    #[serde(default)]
    pub states: Option<u64>,
    /// 停滞阈值（默认 10）。 / The stagnation limit.
    #[serde(default)]
    pub stagnation_limit: Option<u64>,
    /// 随机流初值。 / The random stream seed.
    #[serde(default)]
    pub seed: Option<u64>,
    /// 依次的选择。 / The selections, in order.
    #[serde(default)]
    pub selects: Vec<SelectionStepSpec>,
    /// 依次的反馈归因。 / The attributions, in order.
    #[serde(default)]
    pub observations: Vec<SelectionObservationSpec>,
}

/// 选择策略探针结论。 / The selection policy probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionProbeReport {
    /// 依次选出的动作。 / The selected actions, in order.
    pub selected: Vec<String>,
    /// 归因后的学习修订版本。 / The learning revision after the attributions.
    pub revision: u64,
    /// 每个动作的观察次数。 / The observation count per action.
    pub counts: Vec<u64>,
    /// 每个动作的累计奖励。 / The accumulated reward per action.
    pub sums: Vec<f64>,
    /// 导出的 Q 表行数。 / The exported Q table's row count.
    pub table_rows: u32,
    /// 快照里的 kind 回读。 / The kind read back from the snapshot.
    pub snapshot_kind: String,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_two_rule_kinds_share_one_arm: bool,
    pub the_rule_cursor_advances_past_the_pick: bool,
    pub stagnation_advances_only_on_a_nonzero_multiple: bool,
    pub ucb1_prefers_unobserved_actions: bool,
    pub a_non_learning_policy_learns_nothing: bool,
    pub only_three_kinds_accumulate: bool,
    pub only_q_learning_exports_a_table: bool,
    pub six_illegal_definitions_are_refused: bool,
}

/// 造一个组件定义。 / Build one component definition.
fn spec_of(request: &SelectionProbeRequest) -> ComponentSpec {
    let mut spec = ComponentSpec::new(request.policy.as_str());
    if let Some(epsilon) = request.epsilon {
        spec = spec.parameter("epsilon", epsilon);
    }
    if let Some(alpha) = request.alpha {
        spec = spec.parameter("alpha", alpha);
    }
    if let Some(gamma) = request.gamma {
        spec = spec.parameter("gamma", gamma);
    }
    if let Some(states) = request.states {
        spec = spec.parameter("states", states);
    }
    if let Some(limit) = request.stagnation_limit {
        spec = spec.parameter("stagnationLimit", limit);
    }
    spec
}

/// 造一个策略。 / Build one policy.
fn policy_of(request: &SelectionProbeRequest) -> Result<TabularSelectionPolicy> {
    TabularSelectionPolicy::new(&spec_of(request), request.actions.clone(),
        request.learning.unwrap_or(false))
        .map_err(|error| BridgeError::new("InvalidInput", error))
}

/// 造一次归因。 / Build one attribution.
fn transition_of(request: &SelectionProbeRequest, spec: &SelectionObservationSpec)
    -> SelectionTransition
{
    let context = |state: u64| SelectionContext { state: state as usize, features: Vec::new(),
        decision: 0, stagnation: 0 };
    SelectionTransition {
        episode: "selection-probe".to_owned(),
        decision: 0,
        action: spec.action.clone(),
        before: context(spec.before),
        after: context(spec.after),
        available: spec.available.clone().unwrap_or_else(|| request.actions.clone()),
        next_available: spec.next_available.clone().unwrap_or_default(),
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

/// 走一遍选择策略探针。非法定义是**错误**。 / Walk the probe. An illegal definition is an error.
pub fn verify_selection_probe(
    request: &SelectionProbeRequest,
) -> Result<SelectionProbeReport> {
    if request.actions.len() > 32 || request.selects.len() > 64 || request.observations.len() > 64 {
        return Err(BridgeError::new("InvalidInput", "selection probe failed validation: at \
            most thirty-two actions, sixty-four selections and sixty-four observations are accepted"));
    }
    let findings = canonical_findings();
    let mut policy = policy_of(request)?;
    let mut random = SearchRandom::new(request.seed.unwrap_or(0x5eed));
    let mut selected = Vec::with_capacity(request.selects.len());
    for step in &request.selects {
        let context = SelectionContext { state: step.state as usize, features: Vec::new(),
            decision: 0, stagnation: step.stagnation };
        let available = if step.available.is_empty() {
            request.actions.clone()
        } else {
            step.available.clone()
        };
        let picked = policy.select(&context, &available, &mut random)
            .map_err(|error| BridgeError::new("InvalidInput", error))?;
        selected.push(picked);
    }
    for observation in &request.observations {
        policy.observe(&transition_of(request, observation))
            .map_err(|error| BridgeError::new("InvalidInput", error))?;
    }
    let snapshot: PolicySnapshot = policy.snapshot()
        .map_err(|error| BridgeError::new("InvalidInput", error))?;
    Ok(SelectionProbeReport {
        selected,
        revision: policy.revision(),
        counts: snapshot.counts.clone(),
        sums: snapshot.sums.clone(),
        table_rows: u32::try_from(snapshot.table.len()).unwrap_or(u32::MAX),
        snapshot_kind: snapshot.r#type,
        the_two_rule_kinds_share_one_arm: findings.the_two_rule_kinds_share_one_arm,
        the_rule_cursor_advances_past_the_pick: findings.the_rule_cursor_advances_past_the_pick,
        stagnation_advances_only_on_a_nonzero_multiple:
            findings.stagnation_advances_only_on_a_nonzero_multiple,
        ucb1_prefers_unobserved_actions: findings.ucb1_prefers_unobserved_actions,
        a_non_learning_policy_learns_nothing: findings.a_non_learning_policy_learns_nothing,
        only_three_kinds_accumulate: findings.only_three_kinds_accumulate,
        only_q_learning_exports_a_table: findings.only_q_learning_exports_a_table,
        six_illegal_definitions_are_refused: findings.six_illegal_definitions_are_refused,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_two_rule_kinds_share_one_arm: bool,
    the_rule_cursor_advances_past_the_pick: bool,
    stagnation_advances_only_on_a_nonzero_multiple: bool,
    ucb1_prefers_unobserved_actions: bool,
    a_non_learning_policy_learns_nothing: bool,
    only_three_kinds_accumulate: bool,
    only_q_learning_exports_a_table: bool,
    six_illegal_definitions_are_refused: bool,
}

/// 三个动作的固定定义。 / The fixed three-action definition.
fn actions() -> Vec<String> {
    vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
}

/// 造一个策略（内部用）。 / Build a policy (internal).
fn policy(kind: &str, learning: bool) -> TabularSelectionPolicy {
    TabularSelectionPolicy::new(&ComponentSpec::new(kind), actions(), learning)
        .expect("the fixed definition is valid")
}

/// 按固定序列选。 / Select along a fixed sequence.
fn select_all(kind: &str, stagnations: &[u64], available: &[String]) -> Vec<String> {
    let mut policy = policy(kind, false);
    let mut random = SearchRandom::new(0x5eed);
    stagnations.iter().map(|stagnation| {
        let context = SelectionContext { state: 0, features: Vec::new(), decision: 0,
            stagnation: *stagnation };
        policy.select(&context, available, &mut random).expect("the fixed mask is valid")
    }).collect()
}

/// 造一次归因（内部用）。 / Build one attribution (internal).
fn transition(action: &str, reward: f64, available: &[String], next: &[String])
    -> SelectionTransition
{
    let context = SelectionContext::default();
    SelectionTransition {
        episode: "probe".to_owned(), decision: 0, action: action.to_owned(),
        before: context.clone(), after: context, available: available.to_vec(),
        next_available: next.to_vec(), reward, evaluations: 1, terminal: false,
        policy_version: 0, feature_version: "progress:1".to_owned(),
        scope: "algorithm".to_owned(), steps: 1, failed: false, merged_improvement: false,
        elapsed_nanos: 0, previous_action: None,
    }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        let all = actions();
        // 1) fixed 与 round-robin 同一条臂。 / `fixed` and `round-robin` share one arm.
        let the_two_rule_kinds_share_one_arm = {
            let zeros = vec![0_u64; 4];
            select_all("fixed", &zeros, &all) == select_all("round-robin", &zeros, &all)
        };
        // 2) 规则臂的游标推进成"选中索引 + 1"。 
        // The rule arm advances its cursor past the picked index.
        let the_rule_cursor_advances_past_the_pick = {
            // 掩码只有 `c`：游标停在 c 之后，下一轮再从 c 开始。 
            // The mask holds `c` only, so the cursor parks past it and the next round starts at `c`.
            let only_c = vec!["c".to_owned()];
            let picks = select_all("round-robin", &vec![0_u64; 3], &only_c);
            picks == vec!["c".to_owned(), "c".to_owned(), "c".to_owned()]
                // 全掩码下三次选择各往前走一步。 / With the full mask three picks walk forward.
                && select_all("round-robin", &vec![0_u64; 3], &all)
                    == vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
        };
        // 3) stagnation 只在非零整数倍推进。 / Stagnation advances only on a non-zero multiple.
        let stagnation_advances_only_on_a_nonzero_multiple = {
            // 阈值 10：stagnation = 0/5/9 不推进，10/20 推进。 
            // Limit ten: stagnation 0/5/9 does not advance, 10/20 does.
            let mut limited = TabularSelectionPolicy::new(&ComponentSpec::new("stagnation")
                .parameter("stagnationLimit", 10_u64), actions(), false)
                .expect("valid definition");
            let mut random = SearchRandom::new(0x5eed);
            let mut picks = Vec::new();
            for stagnation in [0_u64, 5, 9, 10, 20] {
                let context = SelectionContext { state: 0, features: Vec::new(), decision: 0,
                    stagnation };
                picks.push(limited.select(&context, &all, &mut random).expect("valid mask"));
            }
            // 0/5/9 全给 a；10 与 20 各推进一次游标。 
            // Zero, five and nine all yield `a`; ten and twenty each advance the cursor once.
            picks == vec!["a".to_owned(), "a".to_owned(), "a".to_owned(), "b".to_owned(),
                "c".to_owned()]
        };
        // 4) ucb1 学习态先选未观察动作。 / `ucb1` in the learning state prefers unobserved actions.
        let ucb1_prefers_unobserved_actions = {
            let mut learning = policy("ucb1", true);
            let mut random = SearchRandom::new(0x5eed);
            let context = SelectionContext::default();
            let first = learning.select(&context, &all, &mut random).expect("valid mask");
            // 观察 a 之后，未观察的 b 仍优先于 a。 
            // After observing `a`, the unobserved `b` still outranks `a`.
            learning.observe(&transition(first.as_str(), 1.0, &all, &all)).expect("valid");
            let second = learning.select(&context, &all, &mut random).expect("valid mask");
            first == "a" && second == "b"
        };
        // 5) 非学习策略什么都不学。 / A non-learning policy learns nothing.
        let a_non_learning_policy_learns_nothing = {
            let mut idle = policy("q-learning", false);
            let before = idle.revision();
            idle.observe(&transition("a", 1.0, &all, &all)).expect("valid");
            let snapshot = idle.snapshot().expect("valid");
            idle.revision() == before && snapshot.counts.iter().all(|count| *count == 0)
                && snapshot.sums.iter().all(|sum| *sum == 0.0)
        };
        // 6) 只有三种 kind 累计。 / Only three kinds accumulate.
        let only_three_kinds_accumulate = {
            let accumulates = |kind: &str| {
                let mut policy = policy(kind, true);
                policy.observe(&transition("a", 2.0, &all, &all)).expect("valid");
                let snapshot = policy.snapshot().expect("valid");
                snapshot.counts[0] == 1 && policy.revision() == 1
            };
            accumulates("q-learning") && accumulates("epsilon-greedy") && accumulates("ucb1")
                && !accumulates("fixed") && !accumulates("round-robin")
                && !accumulates("stagnation") && !accumulates("random")
        };
        // 7) 只有 q-learning 导出表。 / Only `q-learning` exports a table.
        let only_q_learning_exports_a_table = {
            let table_of = |kind: &str| {
                let mut policy = policy(kind, true);
                policy.observe(&transition("a", 1.0, &all, &all)).expect("valid");
                policy.snapshot().expect("valid").table.len()
            };
            table_of("q-learning") > 0
                && table_of("epsilon-greedy") == 0 && table_of("ucb1") == 0
                && table_of("fixed") == 0
        };
        // 8) 六类非法定义被拒。 / Six illegal definitions are refused.
        let six_illegal_definitions_are_refused = {
            let refused = |spec: ComponentSpec, actions: Vec<String>| {
                TabularSelectionPolicy::new(&spec, actions, false).is_err()
            };
            // 未知 kind。 / An unknown kind.
            refused(ComponentSpec::new("nearly"), actions())
                // 重复动作。 / Duplicate actions.
                && refused(ComponentSpec::new("fixed"),
                    vec!["a".to_owned(), "a".to_owned()])
                // 空动作表。 / An empty action list.
                && refused(ComponentSpec::new("fixed"), Vec::new())
                // epsilon 出界。 / An out-of-range epsilon.
                && refused(ComponentSpec::new("epsilon-greedy").parameter("epsilon", 1.5),
                    actions())
                // states 为零。 / A zero state count.
                && refused(ComponentSpec::new("fixed").parameter("states", 0_u64), actions())
                // 容量超限。 / The capacity bound.
                && refused(ComponentSpec::new("fixed").parameter("states", 20_000_000_u64),
                    actions())
        };
        Findings {
            the_two_rule_kinds_share_one_arm,
            the_rule_cursor_advances_past_the_pick,
            stagnation_advances_only_on_a_nonzero_multiple,
            ucb1_prefers_unobserved_actions,
            a_non_learning_policy_learns_nothing,
            only_three_kinds_accumulate,
            only_q_learning_exports_a_table,
            six_illegal_definitions_are_refused,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: SelectionProbeRequest) -> SelectionProbeReport {
        verify_selection_probe(&request).unwrap()
    }

    fn request(policy: &str) -> SelectionProbeRequest {
        SelectionProbeRequest { policy: policy.to_owned(), actions: actions(), learning: None,
            epsilon: None, alpha: None, gamma: None, states: None, stagnation_limit: None,
            seed: None, selects: Vec::new(), observations: Vec::new() }
    }

    fn observation(action: &str, reward: f64) -> SelectionObservationSpec {
        SelectionObservationSpec { action: action.to_owned(), reward, before: 0, after: 0,
            available: None, next_available: None, terminal: false }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request("fixed"));
        assert_eq!(report.snapshot_kind, "fixed");
        assert_eq!(report.table_rows, 0);
        assert_eq!(report.counts, vec![0, 0, 0]);
        assert!(report.the_two_rule_kinds_share_one_arm);
        assert!(report.the_rule_cursor_advances_past_the_pick);
        assert!(report.stagnation_advances_only_on_a_nonzero_multiple);
        assert!(report.ucb1_prefers_unobserved_actions);
        assert!(report.a_non_learning_policy_learns_nothing);
        assert!(report.only_three_kinds_accumulate);
        assert!(report.only_q_learning_exports_a_table);
        assert!(report.six_illegal_definitions_are_refused);
    }

    #[test]
    fn the_rule_arm_walks_the_mask_in_order() {
        let picks = ask(SelectionProbeRequest {
            selects: (0..3).map(|_| SelectionStepSpec { state: 0, stagnation: 0,
                available: Vec::new() }).collect(), ..request("round-robin") });
        assert_eq!(picks.selected, vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        // 掩码只有 c：每轮都选它。 / The mask holds `c` only, so every round picks it.
        let frozen = ask(SelectionProbeRequest {
            selects: (0..2).map(|_| SelectionStepSpec { state: 0, stagnation: 0,
                available: vec!["c".to_owned()] }).collect(), ..request("fixed") });
        assert_eq!(frozen.selected, vec!["c".to_owned(), "c".to_owned()]);
    }

    #[test]
    fn learning_accumulates_only_for_the_three_kinds() {
        let learned = ask(SelectionProbeRequest {
            learning: Some(true), observations: vec![observation("a", 2.0),
                observation("b", -1.0)], ..request("epsilon-greedy") });
        assert_eq!(learned.revision, 2);
        assert_eq!(learned.counts, vec![1, 1, 0]);
        assert_eq!(learned.sums, vec![2.0, -1.0, 0.0]);
        // fixed 只推进游标，不学习。 / `fixed` only moves a cursor; it never learns.
        let idle = ask(SelectionProbeRequest {
            learning: Some(true), observations: vec![observation("a", 2.0)], ..request("fixed") });
        assert_eq!(idle.revision, 0);
        assert_eq!(idle.counts, vec![0, 0, 0]);
    }

    #[test]
    fn only_q_learning_exports_a_table() {
        let q = ask(SelectionProbeRequest { learning: Some(true),
            observations: vec![observation("a", 1.0)], ..request("q-learning") });
        assert!(q.table_rows > 0);
        let ucb = ask(SelectionProbeRequest { learning: Some(true),
            observations: vec![observation("a", 1.0)], ..request("ucb1") });
        assert_eq!(ucb.table_rows, 0);
        assert_eq!(ucb.counts, vec![1, 0, 0]);
    }

    #[test]
    fn bad_definitions_are_refused() {
        let refused = |policy: &str, actions: Vec<String>| {
            verify_selection_probe(&SelectionProbeRequest { policy: policy.to_owned(), actions,
                ..request("fixed") }).unwrap_err().message.clone()
        };
        assert!(refused("nearly", actions()).contains("invalid selector"));
        assert!(refused("fixed", vec!["a".to_owned(), "a".to_owned()])
            .contains("invalid selector"));
        assert!(refused("fixed", Vec::new()).contains("invalid selector"));
        assert!(verify_selection_probe(&SelectionProbeRequest { epsilon: Some(1.5),
            ..request("epsilon-greedy") }).unwrap_err().message.contains("invalid learning"));
        assert!(verify_selection_probe(&SelectionProbeRequest { states: Some(0),
            ..request("fixed") }).unwrap_err().message.contains("invalid learning"));
        assert!(verify_selection_probe(&SelectionProbeRequest { states: Some(20_000_000),
            ..request("fixed") }).unwrap_err().message.contains("exceeds supported capacity"));
        // 掩码里有未知动作、状态越界都拒。 / An unknown mask entry and an out-of-range state are refused.
        assert!(verify_selection_probe(&SelectionProbeRequest {
            selects: vec![SelectionStepSpec { state: 0, stagnation: 0,
                available: vec!["zzz".to_owned()] }], ..request("fixed") }).unwrap_err()
            .message.contains("unknown action"));
        assert!(verify_selection_probe(&SelectionProbeRequest {
            selects: vec![SelectionStepSpec { state: 999, stagnation: 0, available: Vec::new() }],
            ..request("fixed") }).unwrap_err().message.contains("invalid state"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request("fixed"));
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"snapshotKind\""), "{json}");
        assert!(json.contains("\"tableRows\""), "{json}");
        assert!(json.contains("\"ucb1PrefersUnobservedActions\""), "{json}");
        assert!(!json.contains("snapshot_kind"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<SelectionProbeReport>(&copy.to_string()).is_err());
    }
}
