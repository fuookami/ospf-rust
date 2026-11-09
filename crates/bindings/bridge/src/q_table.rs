//! 批次 G 第 2 单：Q 表的编解码与确定性推演（算子本身不绑）/ Batch G, delivery 2: the Q-table codec and a deterministic
//! replay (the operators themselves stay unbound).
//!
//! `operator/rl` 里可搬运的一层是 **Q 表本身**：它是一份「状态 × 动作」的数值矩阵，`snapshot` / `restore` 就是它的编解码，
//! 而 `update` 与动作选择器都是**纯数值函数**。这一单把这条链走通并交出结论——**不建 agent、不接观测者、不跑搜索**。
//!
//! 领域规则逐条保留（桥接一条不重写，全部由 `QTable::restore` / `update` / `best_action` 与两个选择器给出）：
//! * `restore` 要求行数 = 状态数、每行长度 = 动作数、**所有值有限**，否则整份拒绝（原子语义：不部分写入）；
//! * `update` 的时序差分目标是**下一状态**的最大动作价值，**不是当前状态的**——这是实现 Q-learning 时最经典的错；
//! * `terminal = true` 时目标里的下一状态项直接取 0；
//! * `availableActions` 既参与"下一状态取最大"，又被校验为动作下标不越界（掩码不是注释，是规则）；
//! * `alpha` 与 `gamma` 必须落在 `[0, 1]`，`reward` 必须有限，更新结果必须有限（溢出拒绝而不是写进一个 `inf`）；
//! * `best_action` 并列时取**下标最小**者；空表返回 0；
//! * ε-贪心把 `epsilon` **夹到 `[0, 1]`**，探索时**再抽一个独立随机数**决定探索位置（不复用同一次掷骰），
//!   所以 `epsilon = 0` 就是纯贪心。随机性来自**种子化**熵源，于是同一种子的选择序列可复现。
//!
//! 规模有上限：`状态 × 动作 <= 4096`、转移 <= 1024、选择查询 <= 256——都是**信任边界**上的限制。
//!
//! The portable layer inside `operator/rl` is the **Q-table itself**: a numeric state × action matrix whose
//! `snapshot` / `restore` pair is its codec, with `update` and the action selectors being **purely numeric functions**. This
//! delivery walks that chain and reports the verdict — **no agent, no observer, no search**. Every domain rule is preserved
//! and the bridge re-implements none of them: `restore` requires the exact shape and **all values finite** or refuses the
//! whole table (atomic, never partially written); `update`'s temporal-difference target is the maximum over the **next**
//! state, **not the current one** — the classic Q-learning mistake; `terminal = true` drops the next-state term to zero;
//! `availableActions` both masks that maximum and is validated against the action count (the mask is a rule, not a comment);
//! `alpha` and `gamma` must lie in `[0, 1]`, `reward` must be finite and an overflowing update is refused rather than stored as
//! `inf`; `best_action` breaks ties by **lowest index** and returns 0 for an empty table; and ε-greedy **clamps** `epsilon` to
//! `[0, 1]` and draws a **second independent sample** for the exploration pick, so `epsilon = 0` is plain greedy and a seeded
//! entropy source makes a whole selection sequence reproducible. Sizes are capped (`states × actions <= 4096`,
//! transitions <= 1024, selection queries <= 256) as **trust boundaries**.

use ospf_rust_core::solver::heuristic::operator::rl::action_selector::{
    ActionSelector, EpsilonGreedyActionSelector, GreedyActionSelector,
};
use ospf_rust_core::solver::heuristic::operator::rl::q_table::QTable;
use ospf_rust_core::solver::heuristic::SeededEntropy;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// Q 表推演的请求。 / The Q-table replay request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QTableRequest {
    /// 状态数量。 / The number of states.
    pub state_amount: u32,
    /// 动作数量。 / The number of actions.
    pub action_amount: u32,
    /// 初始表（行优先铺平）；缺省表示全 0。 / The initial table (row-major, flattened); omitted means all zeros.
    #[serde(default)]
    pub initial_values: Vec<f64>,
    /// 依次施加的时序差分转移。 / The temporal-difference transitions applied in order.
    #[serde(default)]
    pub transitions: Vec<QTransition>,
    /// 动作选择器（缺省为纯贪心）。 / The action selector (plain greedy when omitted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub selector: Option<QSelector>,
    /// 要用选择器查询的状态（最多 256 个）。 / The states to query with the selector (at most 256).
    #[serde(default)]
    pub selection_states: Vec<u32>,
}

/// 一次时序差分转移。 / One temporal-difference transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QTransition {
    /// 当前状态。 / The current state.
    pub state: u32,
    /// 采取的动作。 / The action taken.
    pub action: u32,
    /// 即时奖励（必须有限）。 / The immediate reward (must be finite).
    pub reward: f64,
    /// 下一状态。 / The next state.
    pub next_state: u32,
    /// 学习率（必须在 `[0, 1]`）。 / The learning rate (must lie in `[0, 1]`).
    pub alpha: f64,
    /// 折扣因子（必须在 `[0, 1]`）。 / The discount factor (must lie in `[0, 1]`).
    pub gamma: f64,
    /// 是否终止转移（目标里的下一状态项取 0）。 / Whether the transition is terminal (the next-state term becomes zero).
    #[serde(default)]
    pub terminal: bool,
    /// 可动作掩码；参与"下一状态取最大"并被校验不越界。 / The action mask; it masks the next-state maximum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub available_actions: Option<Vec<u32>>,
}

/// 动作选择器。 / The action selector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QSelector {
    /// `greedy` 或 `epsilonGreedy`。 / `greedy` or `epsilonGreedy`.
    pub kind: String,
    /// `epsilonGreedy` 的探索概率（会被夹到 `[0, 1]`）。 / The exploration probability (clamped to `[0, 1]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub epsilon: Option<f64>,
    /// 熵源种子（缺省用领域默认种子）。 / The entropy seed (the domain default when omitted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub seed: Option<u64>,
}

/// 一次转移的结果。 / The outcome of one transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QTransitionOutcome {
    /// 转移序号（从 0 开始）。 / The transition index (0-based).
    pub index: u32,
    /// 更新后的 `Q(s, a)`。 / The updated `Q(s, a)`.
    pub updated_value: f64,
}

/// 一次选择查询的结果。 / The outcome of one selection query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QSelectionOutcome {
    /// 被查询的状态。 / The queried state.
    pub state: u32,
    /// 选中的动作。 / The chosen action.
    pub action: u32,
}

/// Q 表推演的结论。 / The Q-table replay verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QTableReport {
    /// 状态数量。 / The number of states.
    pub state_amount: u32,
    /// 动作数量。 / The number of actions.
    pub action_amount: u32,
    /// 初始表是否由请求给出（否表示全 0）。 / Whether the initial table came from the request.
    pub initial_supplied: bool,
    /// 转移条数。 / The number of transitions.
    pub transition_count: u32,
    /// 逐次转移的结果。 / The per-transition outcomes.
    pub transitions: Vec<QTransitionOutcome>,
    /// 推演结束后的完整表（行优先铺平）。 / The final table (row-major, flattened).
    pub final_values: Vec<f64>,
    /// 每个状态的最优动作（并列取最小下标）。 / Each state's best action (ties by lowest index).
    pub best_actions: Vec<u32>,
    /// 选择器种类（线格式代码）。 / The selector kind (wire code).
    pub selector_kind: String,
    /// 实际使用的种子。 / The seed actually used.
    pub seed: u64,
    /// 逐状态的查询结果。 / The per-state selection outcomes.
    pub selections: Vec<QSelectionOutcome>,
    /// 选择器词表（桥接定义）。 / The selector vocabulary (bridge-defined).
    pub selector_vocabulary: Vec<String>,
}

/// 状态 × 动作的上限：信任边界。 / The state × action cap: a trust boundary.
const MAX_CELLS: u64 = 4096;
/// 转移条数上限：信任边界。 / The transition cap: a trust boundary.
const MAX_TRANSITIONS: usize = 1024;
/// 选择查询上限：信任边界。 / The selection-query cap: a trust boundary.
const MAX_SELECTIONS: usize = 256;

/// 解码初始表、依次施加转移、投影最终表与动作选择。非法输入是**错误**，原因原样带出。
/// Decode the initial table, apply the transitions in order and project the final table and selections.
pub fn verify_q_table(request: &QTableRequest) -> Result<QTableReport> {
    let states = usize::try_from(request.state_amount).map_err(|_| BridgeError::new("InvalidInput",
        "q table failed validation: state count exceeds the platform index range"))?;
    let actions = usize::try_from(request.action_amount).map_err(|_| BridgeError::new("InvalidInput",
        "q table failed validation: action count exceeds the platform index range"))?;
    if states == 0 || actions == 0 {
        return Err(BridgeError::new("InvalidInput",
            "q table failed validation: a table needs at least one state and one action"));
    }
    if (request.state_amount as u64) * (request.action_amount as u64) > MAX_CELLS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "q table failed validation: at most {MAX_CELLS} cells are accepted")));
    }
    if request.transitions.len() > MAX_TRANSITIONS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "q table failed validation: at most {MAX_TRANSITIONS} transitions are accepted")));
    }
    if request.selection_states.len() > MAX_SELECTIONS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "q table failed validation: at most {MAX_SELECTIONS} selection queries are accepted")));
    }
    let mut table = QTable::new(states, actions);
    let initial_supplied = !request.initial_values.is_empty();
    if initial_supplied {
        if request.initial_values.len() != states * actions {
            return Err(BridgeError::new("InvalidInput", &format!(
                "q table failed validation: the initial table needs exactly {} values, got {}",
                states * actions, request.initial_values.len())));
        }
        // `restore` 是领域解码器：形状精确、数值必须有限、原子写入。
        // `restore` is the domain decoder: exact shape, finite values and atomic writes.
        let rows = request.initial_values.chunks(actions).map(<[f64]>::to_vec).collect::<Vec<_>>();
        table.restore(&rows).map_err(|error| BridgeError::new("InvalidInput",
            &format!("q table failed validation: {error}")))?;
    }
    let mut transitions = Vec::with_capacity(request.transitions.len());
    for (index, transition) in request.transitions.iter().enumerate() {
        let available = transition.available_actions.as_ref().map(|mask| mask.iter()
            .map(|action| usize::try_from(*action).map_err(|_| BridgeError::new("InvalidInput",
                "q table failed validation: action index exceeds the platform index range")))
            .collect::<Result<Vec<usize>>>()).transpose()?;
        let updated = table.update(
            usize::try_from(transition.state).unwrap_or(usize::MAX),
            usize::try_from(transition.action).unwrap_or(usize::MAX),
            transition.reward,
            usize::try_from(transition.next_state).unwrap_or(usize::MAX),
            transition.alpha,
            transition.gamma,
            transition.terminal,
            available.as_deref(),
        ).map_err(|error| BridgeError::new("InvalidInput", &format!(
            "q table failed validation: transition {index}: {error}")))?;
        transitions.push(QTransitionOutcome {
            index: u32::try_from(index).unwrap_or(u32::MAX), updated_value: updated });
    }
    let selector = request.selector.as_ref();
    let kind = selector.map_or("greedy", |selector| selector.kind.as_str());
    // 选择器种类在**入口**就校验：没有查询也不该放过一个不存在的选择器——否则"空查询"会变成绕过校验的后门。
    // The kind is validated **up front**: an unknown selector must not pass merely because no query asked for it, otherwise
    // an empty query would become a way to bypass validation.
    if !matches!(kind, "greedy" | "epsilonGreedy") {
        return Err(BridgeError::new("InvalidInput", &format!(
            "q table failed validation: unknown selector `{kind}`")));
    }
    let seed = selector.and_then(|selector| selector.seed).unwrap_or_else(default_seed);
    let mut entropy = SeededEntropy::new(seed);
    let greedy = GreedyActionSelector;
    let epsilon = selector.and_then(|selector| selector.epsilon);
    let epsilon_greedy = EpsilonGreedyActionSelector::new(epsilon.unwrap_or(0.1));
    let mut selections = Vec::with_capacity(request.selection_states.len());
    for state in &request.selection_states {
        let state = usize::try_from(*state).unwrap_or(usize::MAX);
        if state >= states {
            return Err(BridgeError::new("InvalidInput", &format!(
                "q table failed validation: selection state {state} is outside the table")));
        }
        let values = table.values_of(state);
        let action = match kind {
            "greedy" => greedy.select(values, &mut entropy),
            _ => epsilon_greedy.select(values, &mut entropy),
        };
        selections.push(QSelectionOutcome {
            state: u32::try_from(state).unwrap_or(u32::MAX),
            action: u32::try_from(action).unwrap_or(u32::MAX),
        });
    }
    let best_actions = (0..states).map(|state| u32::try_from(table.best_action(state))
        .unwrap_or(u32::MAX)).collect();
    let final_values = table.snapshot().into_iter().flatten().collect();
    Ok(QTableReport {
        state_amount: request.state_amount,
        action_amount: request.action_amount,
        initial_supplied,
        transition_count: u32::try_from(request.transitions.len()).unwrap_or(u32::MAX),
        transitions,
        final_values,
        best_actions,
        selector_kind: kind.to_owned(),
        seed,
        selections,
        selector_vocabulary: vec!["greedy".to_owned(), "epsilonGreedy".to_owned()],
    })
}

/// 领域默认种子（与 `SeededEntropy::default` 一致）。 / The domain default seed.
fn default_seed() -> u64 { SeededEntropy::default().state() }

#[cfg(test)]
mod tests {
    use super::*;

    fn request(states: u32, actions: u32) -> QTableRequest {
        QTableRequest { state_amount: states, action_amount: actions, initial_values: Vec::new(),
            transitions: Vec::new(), selector: None, selection_states: Vec::new() }
    }

    fn transition(state: u32, action: u32, reward: f64, next_state: u32, terminal: bool) -> QTransition {
        QTransition { state, action, reward, next_state, alpha: 1.0, gamma: 1.0, terminal,
            available_actions: None }
    }

    #[test]
    fn codec_round_trips_and_best_actions_take_the_lowest_tie() {
        let mut document = request(2, 3);
        document.initial_values = vec![0.0, 3.0, 3.0, -1.0, 0.5, 0.5];
        let report = verify_q_table(&document).unwrap();
        assert!(report.initial_supplied);
        // 解码后再编码必须逐值一致。 / Decoding then encoding must reproduce every value.
        assert_eq!(report.final_values, document.initial_values);
        // 两行的并列都取最小下标：状态 0 在动作 1/2 上并列（都 3.0），状态 1 在动作 1/2 上并列（都 0.5），都取 1。
        // Both rows resolve ties by the lowest index: state 0 ties at actions 1 and 2 (both 3.0) and state 1 ties at
        // actions 1 and 2 (both 0.5), so both answer 1.
        assert_eq!(report.best_actions, vec![1, 1]);
    }

    #[test]
    fn updates_use_the_next_state_and_respect_terminal_and_masks() {
        // 下一状态的最大值参与目标，不是当前状态的：状态 0/动作 0 从 0 更新到 `reward + gamma * max(next)`。
        // The next state's maximum enters the target, not the current state's.
        let mut document = request(2, 2);
        document.initial_values = vec![9.0, 9.0, 1.0, 4.0];
        document.transitions = vec![transition(0, 0, 0.0, 1, false)];
        let report = verify_q_table(&document).unwrap();
        assert_eq!(report.transitions[0].updated_value, 4.0);
        // 终止转移把下一状态项归零。 / A terminal transition zeroes the next-state term.
        let mut terminal = request(2, 2);
        terminal.initial_values = vec![9.0, 9.0, 1.0, 4.0];
        terminal.transitions = vec![transition(0, 0, 2.0, 1, true)];
        assert_eq!(verify_q_table(&terminal).unwrap().transitions[0].updated_value, 2.0);
        // 掩码参与取最大：只允许动作 0 时，下一状态的最大值是 1 而不是 4。
        // The mask participates in the maximum: allowing only action 0 gives 1 instead of 4.
        let mut masked = request(2, 2);
        masked.initial_values = vec![9.0, 9.0, 1.0, 4.0];
        let mut masked_transition = transition(0, 0, 0.0, 1, false);
        masked_transition.available_actions = Some(vec![0]);
        masked.transitions = vec![masked_transition];
        assert_eq!(verify_q_table(&masked).unwrap().transitions[0].updated_value, 1.0);
        // 半程学习率：`Q += alpha * (target - Q)`。 / A partial learning rate.
        let mut partial = request(2, 2);
        partial.initial_values = vec![0.0, 0.0, 1.0, 4.0];
        let mut partial_transition = transition(0, 0, 0.0, 1, false);
        partial_transition.alpha = 0.5;
        partial.transitions = vec![partial_transition];
        assert_eq!(verify_q_table(&partial).unwrap().transitions[0].updated_value, 2.0);
        // 多次转移按顺序累加。 / Several transitions accumulate in order.
        let mut twice = request(1, 1);
        let mut first = transition(0, 0, 1.0, 0, true);
        first.alpha = 0.5;
        let mut second = transition(0, 0, 3.0, 0, true);
        second.alpha = 0.5;
        twice.transitions = vec![first, second];
        let report = verify_q_table(&twice).unwrap();
        assert_eq!(report.transitions.len(), 2);
        assert!((report.transitions[0].updated_value - 0.5).abs() < 1e-12);
        assert!((report.transitions[1].updated_value - 1.75).abs() < 1e-12);
    }

    #[test]
    fn selection_is_greedy_by_default_and_seeded_when_exploring() {
        let mut document = request(1, 4);
        document.initial_values = vec![0.1, 0.2, 0.3, 0.4];
        document.selection_states = vec![0];
        let report = verify_q_table(&document).unwrap();
        assert_eq!(report.selector_kind, "greedy");
        assert_eq!(report.selections[0].action, 3);
        assert_eq!(report.selector_vocabulary, vec!["greedy".to_owned(), "epsilonGreedy".to_owned()]);
        // ε = 0 等价于贪心。 / ε = 0 is plain greedy.
        let mut greedy = document.clone();
        greedy.selector = Some(QSelector { kind: "epsilonGreedy".to_owned(), epsilon: Some(0.0), seed: Some(5) });
        let report = verify_q_table(&greedy).unwrap();
        assert_eq!(report.selections[0].action, 3);
        assert_eq!(report.seed, 5);
        // ε = 1 时同一种子给出可复现序列，且始终在范围内。 / ε = 1 with one seed is reproducible and in range.
        let mut explore = document.clone();
        explore.selection_states = vec![0; 16];
        explore.selector = Some(QSelector { kind: "epsilonGreedy".to_owned(), epsilon: Some(1.0), seed: Some(9) });
        let first = verify_q_table(&explore).unwrap();
        let again = verify_q_table(&explore).unwrap();
        assert_eq!(first.selections, again.selections);
        assert!(first.selections.iter().all(|selection| selection.action < 4));
        // 换种子改变序列。 / A different seed changes the sequence.
        explore.selector = Some(QSelector { kind: "epsilonGreedy".to_owned(), epsilon: Some(1.0), seed: Some(10) });
        assert_ne!(first.selections, verify_q_table(&explore).unwrap().selections);
    }

    #[test]
    fn illegal_inputs_are_refused() {
        // 维度为零 / 单元格过多。 / Zero dimensions and too many cells.
        assert!(verify_q_table(&request(0, 1)).unwrap_err().message.contains("at least one state"));
        assert!(verify_q_table(&request(1, 0)).unwrap_err().message.contains("at least one state"));
        assert!(verify_q_table(&request(65, 64)).unwrap_err().message.contains("at most 4096 cells"));
        // 初始表长度不符 / 含非有限值：`restore` 整份拒绝。
        // A wrong initial length and a non-finite value: `restore` refuses the whole table.
        let mut wrong = request(2, 2);
        wrong.initial_values = vec![0.0, 1.0, 2.0];
        assert!(verify_q_table(&wrong).unwrap_err().message.contains("exactly 4 values"));
        let mut infinite = request(2, 2);
        infinite.initial_values = vec![0.0, 1.0, 2.0, f64::INFINITY];
        assert!(verify_q_table(&infinite).unwrap_err().message.contains("shape or values"));
        // 越界状态 / 动作 / 下一状态。 / Out-of-range state, action and next state.
        for broken in [transition(9, 0, 0.0, 0, false), transition(0, 9, 0.0, 0, false),
            transition(0, 0, 0.0, 9, false)]
        {
            let mut document = request(2, 2);
            document.transitions = vec![broken];
            let error = verify_q_table(&document).unwrap_err();
            assert!(error.message.contains("out of range"), "{}", error.message);
        }
        // 学习率与折扣因子必须落在 `[0, 1]`。 / alpha and gamma must lie in `[0, 1]`.
        let mut bad_alpha = request(2, 2);
        let mut sample = transition(0, 0, 0.0, 1, false);
        sample.alpha = 1.5;
        bad_alpha.transitions = vec![sample];
        assert!(verify_q_table(&bad_alpha).unwrap_err().message.contains("alpha"));
        let mut bad_gamma = request(2, 2);
        let mut sample = transition(0, 0, 0.0, 1, false);
        sample.gamma = -0.5;
        bad_gamma.transitions = vec![sample];
        assert!(verify_q_table(&bad_gamma).unwrap_err().message.contains("gamma"));
        // 非有限奖励 / 掩码越界。 / A non-finite reward and an out-of-range mask.
        let mut bad_reward = request(2, 2);
        bad_reward.transitions = vec![transition(0, 0, f64::NAN, 1, false)];
        assert!(verify_q_table(&bad_reward).unwrap_err().message.contains("reward"));
        let mut bad_mask = request(2, 2);
        let mut sample = transition(0, 0, 0.0, 1, false);
        sample.available_actions = Some(vec![7]);
        bad_mask.transitions = vec![sample];
        assert!(verify_q_table(&bad_mask).unwrap_err().message.contains("action mask"));
        // 选择状态越界 / 未知选择器。 / An out-of-range selection state and an unknown selector.
        let mut outside = request(2, 2);
        outside.selection_states = vec![5];
        assert!(verify_q_table(&outside).unwrap_err().message.contains("outside the table"));
        let mut unknown = request(2, 2);
        unknown.selector = Some(QSelector { kind: "softmax".to_owned(), epsilon: None, seed: None });
        unknown.selection_states = vec![0];
        assert!(verify_q_table(&unknown).unwrap_err().message.contains("unknown selector"));
        // 规模上限。 / The size caps.
        let mut many = request(2, 2);
        many.transitions = vec![transition(0, 0, 0.0, 1, false); MAX_TRANSITIONS + 1];
        assert!(verify_q_table(&many).unwrap_err().message.contains("at most 1024 transitions"));
        let mut queries = request(2, 2);
        queries.selection_states = vec![0; MAX_SELECTIONS + 1];
        assert!(verify_q_table(&queries).unwrap_err().message.contains("at most 256 selection"));
    }
}
