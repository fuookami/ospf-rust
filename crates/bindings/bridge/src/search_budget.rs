//! 续挖第 21 单：评估预算的嵌套计费契约与随机流的可恢复性
//! （`solver/heuristic/session/budget.rs` 的 `SearchBudget` / `SearchSlice` / `SearchRandom`）
//! Continuation, delivery 21: the nested accounting contract of the evaluation budget, and the
//! random stream's resumability.
//!
//! 这段代码的作用是"子任务不能超支外层配额"，但它的实现里有三条规定**不在字面上**：
//!
//! 1. **只有成功才消费**：`SearchSlice::acquire_shared` 先做本地 CAS 把 `used` 加一，再向父片段（或无父时向全局）
//!    申请；**父级拒绝时会把本次 `used` 减回去**。所以"本地额度够、但父级不够"不会留下脏计数。
//! 2. **`steps` 不参与准入判断**：`acquire` 只看 `evaluations` 与时间；`steps` 只是给 runner 读的额度，
//!    拿 `steps = 0` 的片段照样能领取评估。把"步数上限"当成"评估上限"会读错。
//! 3. **`denied` 一旦置位就永久为真**（只 `store(true)`，从不清零）。它的用途是**阻止残缺代提交**，
//!    所以"某一代被拒"之后即使额度后来又有余量，这个标记也不会自己消失。
//!
//! 4. **孙片段的领取会消耗父片段的本地额度**，所以**父片段本地额度耗尽后，孙片段即使全局还有余量也会被拒**——
//!    嵌套是"配额守恒"，不只是"记账"。
//!
//! 另外 `remaining()` = **min(本地剩余, 父级剩余)**，所以报告的"剩余"永远是从严的那个；`SearchRandom` 是
//! SplitMix64，**每个状态都可以直接当快照存下来**，`split(id)` 派生稳定子流**不消费父流**——这正是会话恢复
//! 不会改变随机序列的原因（LCG 做不到）。注意 `SearchRandom` **没有 `FromState` 入口**，只能 `new(state)`
//! 再抽取，所以"从快照恢复"在 API 层面就是"用那个 state 重新 `new`"。
//!
//! The code exists so a child task cannot overspend its parent's allowance, and three of its rules are not in the
//! prose. **Only successful claims are consumed**: `acquire_shared` first bumps the local `used` through a CAS and then
//! asks the parent (or the global budget when there is no parent), **rolling the local `used` back when the parent
//! refuses**. **`steps` does not gate admission**: `acquire` looks only at `evaluations` and time, so a slice with
//! `steps = 0` still claims evaluations — reading the step cap as the evaluation cap is wrong. And **`denied` is sticky**:
//! only ever `store(true)`d, because its purpose is to **block partial-generation commits**, so once a generation is denied
//! the flag does not clear even if allowance reappears. `remaining()` is **min(local remaining, parent remaining)**.
//! `SearchRandom` is SplitMix64 where **every state is directly snapshottable**, and `split(id)` derives a stable child
//! stream **without consuming the parent** — the reason resuming a session does not change the random stream. Note there is
//! **no `FromState` entry point**: resuming is literally `new(state)` on the saved state.

use std::sync::Arc;

use ospf_rust_core_plugin_heuristic::heuristic::session::budget::{
    SearchBudget, SearchRandom, SearchSlice,
};
use ospf_rust_math::distribution::Entropy;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 预算请求。 / The budget request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchBudgetRequest {
    /// 全局上限。 / The global limit.
    pub limit: u64,
    /// 起始已消费额度。 / The starting consumed allowance.
    #[serde(default)]
    pub used: u64,
    /// 起始目标调用次数。 / The starting objective call count.
    #[serde(default)]
    pub objective_calls: u64,
    /// 子片段评估额度。 / The child slice's evaluation allowance.
    pub evaluations: u64,
    /// 子片段步数额度（**不参与准入**）。 / The child's step allowance (**does not gate admission**).
    #[serde(default)]
    pub steps: u64,
    /// 孙片段评估额度。 / The grandchild slice's evaluation allowance.
    #[serde(default)]
    pub grandchild_evaluations: u64,
    /// 在子片段上申请多少次。 / How many claims to make on the child.
    pub claims: u32,
    /// 在孙片段上申请多少次。 / How many claims to make on the grandchild.
    #[serde(default)]
    pub grandchild_claims: u32,
    /// 随机流初始状态。 / The random stream's initial state.
    pub random_state: u64,
    /// 抽取多少个单位值。 / How many unit values to draw.
    pub random_draws: u32,
    /// 派生子流用的标识。 / The identifier used to derive a child stream.
    pub split_id: String,
}

/// 一个片段的结论。 / One slice's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SliceVerdict {
    /// 评估额度。 / The evaluation allowance.
    pub evaluations: u64,
    /// 步数额度。 / The step allowance.
    pub steps: u64,
    /// 已用请求。 / Requests consumed.
    pub used: u64,
    /// 剩余（含父级）。 / Remaining, parent included.
    pub remaining: u64,
    /// 是否被拒过。 / Whether admission ever failed.
    pub denied: bool,
    /// 实际获批的申请次数。 / How many claims were actually granted.
    pub granted: u32,
}

/// 随机流结论。 / The random stream's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RandomVerdict {
    /// 起始状态。 / The starting state.
    pub state_before: u64,
    /// 抽取到的单位值。 / The drawn unit values.
    pub draws: Vec<f64>,
    /// 抽取后的状态。 / The state after drawing.
    pub state_after: u64,
    /// 快照状态（最后一次抽取之前）。 / The snapshot state, taken before the last draw.
    pub snapshot_state: u64,
    /// 从快照恢复后的抽取。 / The draw after resuming from the snapshot.
    pub resumed_draw: f64,
    /// 恢复后的抽取应与最后一次抽取一致。 / The resumed draw should equal the last draw.
    pub resume_matches: bool,
    /// `split` 派生出的子流起始状态。 / The derived child stream's starting state.
    pub split_state: u64,
    /// `split` 是否改变了父流状态（**否**）。 / Whether `split` moved the parent's state (**no**).
    pub split_moves_parent: bool,
    /// 两次同样初始化的流给出同样序列（**是**）。 / Whether identically seeded streams agree (**yes**).
    pub identical_seeds_agree: bool,
    /// 单位值都落在 `[0, 1)`（**是**）。 / Whether every draw lands in `[0, 1)` (**yes**).
    pub draws_in_unit_interval: bool,
}

/// 预算结论。 / The budget verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchBudgetReport {
    /// 全局上限。 / The global limit.
    pub limit: u64,
    /// 全局起始已用。 / The global's starting usage.
    pub used_before: u64,
    /// 全局最终已用。 / The global's final usage.
    pub used_after: u64,
    /// 全局最终剩余。 / The global's final remainder.
    pub remaining_after: u64,
    /// 目标调用次数（**不随领取变化**）。 / The objective call count (**unchanged by claims**).
    pub objective_calls: u64,
    /// 子片段。 / The child slice.
    pub child: SliceVerdict,
    /// 孙片段。 / The grandchild slice.
    pub grandchild: SliceVerdict,
    /// 随机流。 / The random stream.
    pub random: RandomVerdict,
    /// 全局在额度用尽后停发（**是**）。 / Whether the global stops at its limit (**yes**).
    pub global_stops_at_limit: bool,
    /// `steps` 不参与准入（**是**）。 / Whether `steps` fails to gate admission (**yes**).
    pub steps_do_not_gate_admission: bool,
    /// 孙片段的消耗同时计入子片段与全局（**是**）。 / Whether grandchild claims charge both (**yes**).
    pub child_charges_parent: bool,
    /// 父级拒绝时本地计数回退（**是**）。 / Whether a parent refusal rolls the local count back (**yes**).
    pub parent_refusal_rolls_back: bool,
    /// `denied` 一旦置位就不再清零（**是**）。 / Whether `denied` is sticky (**yes**).
    pub denied_is_sticky: bool,
    /// 剩余额取本地与父级的较小值（**是**）。 / Whether the remainder is the stricter of the two (**yes**).
    pub remaining_is_minimum: bool,
}

/// 上限（信任边界）。 / The caps (a trust boundary).
const MAX_CLAIMS: u32 = 4_096;
const MAX_DRAWS: u32 = 4_096;

/// 走一遍预算与随机流。非法输入是**错误**。 / Walk the budget and the random stream.
pub fn verify_search_budget(request: &SearchBudgetRequest) -> Result<SearchBudgetReport> {
    if request.claims > MAX_CLAIMS || request.grandchild_claims > MAX_CLAIMS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "search budget failed validation: at most {MAX_CLAIMS} claims are accepted")));
    }
    if request.random_draws > MAX_DRAWS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "search budget failed validation: at most {MAX_DRAWS} draws are accepted")));
    }
    if request.used > request.limit {
        return Err(BridgeError::new("InvalidInput", &format!(
            "search budget failed validation: the starting usage {} exceeds the limit {}",
            request.used, request.limit)));
    }
    // 一份全局预算，子片段挂在它下面，孙片段挂在子片段下面。 
    // One global budget, the child under it, the grandchild under the child.
    let global = Arc::new(SearchBudget::with_counts(request.limit, request.used,
        request.objective_calls));
    let used_before = global.used();
    let objective_calls = global.objective_calls();
    let child = Arc::new(SearchSlice::new(Arc::clone(&global), request.evaluations,
        request.steps));
    let mut granted = 0;
    for _ in 0..request.claims {
        if child.acquire_shared() {
            granted += 1;
        }
    }
    let grandchild = SearchSlice::child(&child, request.grandchild_evaluations, request.steps);
    let mut grandchild_granted = 0;
    for _ in 0..request.grandchild_claims {
        if grandchild.acquire_shared() {
            grandchild_granted += 1;
        }
    }
    // 这条是**规则本身**的性质，用独立探针验证：孙片段领一次必须同时抬高子片段与全局的已用。
    // 直接用本请求的数字会被"孙片段一次都没获批"的场景打成 false（第 19 单踩过同一类错）。
    // This is a property of the **rule**, checked with an isolated probe: one grandchild claim must raise both the child's
    // and the global's usage. Reusing this request's numbers would flip it to false whenever the grandchild is refused.
    let charge_global = Arc::new(SearchBudget::new(10));
    let charge_child = Arc::new(SearchSlice::new(Arc::clone(&charge_global), 5, 1));
    let charge_grandchild = SearchSlice::child(&charge_child, 3, 1);
    let charged = charge_grandchild.acquire_shared();
    let child_charges_parent = charged && charge_grandchild.used() == 1
        && charge_child.used() == 1 && charge_global.used() == 1;
    let used_after = global.used();
    let remaining_after = global.remaining();
    // 全局停发用**独立**的一份预算探测：真全局要留给 `used_after` / 各片段 `remaining` 读数，抽干它会污染结论。
    // The stop-at-limit probe uses a **separate** budget: draining the real one would pollute the reported usage and
    // remainders.
    let probe_budget = SearchBudget::with_counts(request.limit, request.used, 0);
    for _ in 0..used_after.saturating_sub(used_before) {
        probe_budget.acquire();
    }
    let expected_grants = request.limit.saturating_sub(probe_budget.used());
    let mut drained = 0u64;
    while probe_budget.acquire() {
        drained += 1;
        // ponytail: 防御性上界，理论上不可达；`acquire` 在 `used == limit` 时就返回 false。
        if drained > expected_grants {
            break;
        }
    }
    let global_stops_at_limit = drained == expected_grants
        && probe_budget.used() == request.limit && probe_budget.remaining() == 0
        && !probe_budget.acquire();
    // `steps = 0` 的片段照样能领两次评估 → `steps` 不参与准入。
    let probe_global = Arc::new(SearchBudget::new(8));
    let probe = SearchSlice::new(Arc::clone(&probe_global), 2, 0);
    let steps_do_not_gate_admission = probe.acquire_shared() && probe.acquire_shared()
        && !probe.acquire_shared() && probe.used() == 2;
    // 父级拒绝时回退：本地 5、全局只剩 1；第 2 次本地先加一，被全局拒后减回。
    let tight = Arc::new(SearchBudget::new(1));
    let tight_slice = SearchSlice::new(Arc::clone(&tight), 5, 1);
    let first = tight_slice.acquire_shared();
    let second = tight_slice.acquire_shared();
    let parent_refusal_rolls_back = first && !second && tight_slice.used() == 1
        && tight_slice.denied() && tight.used() == 1;
    let denied_is_sticky = !tight_slice.acquire_shared() && tight_slice.denied();
    // 剩余取较小值：本地 100、全局只剩 3 → 剩余必须恰为 3。
    let cap = Arc::new(SearchBudget::new(3));
    let wide = SearchSlice::new(Arc::clone(&cap), 100, 1);
    let remaining_is_minimum = wide.remaining() == 3;
    // 随机流：同种子同序列、快照可恢复、`split` 不消费父流。
    let mut stream = SearchRandom::new(request.random_state);
    let state_before = stream.state();
    let mut draws = Vec::with_capacity(request.random_draws as usize);
    let mut snapshot_state = state_before;
    for _ in 0..request.random_draws {
        snapshot_state = stream.state();
        draws.push(stream.next_unit());
    }
    let state_after = stream.state();
    let draws_in_unit_interval = draws.iter().all(|value| (0.0..1.0).contains(value));
    let mut resumed = SearchRandom::new(snapshot_state);
    let resumed_draw = resumed.next_unit();
    let resume_matches = draws.last().is_some_and(|last| *last == resumed_draw);
    let mut splitter = SearchRandom::new(state_before);
    let split_state = splitter.split(&request.split_id).state();
    let split_moves_parent = splitter.state() != state_before;
    let mut twin_a = SearchRandom::new(request.random_state);
    let mut twin_b = SearchRandom::new(request.random_state);
    let identical_seeds_agree = twin_a.next_unit() == twin_b.next_unit()
        && twin_a.state() == twin_b.state();
    Ok(SearchBudgetReport {
        limit: request.limit,
        used_before,
        used_after,
        remaining_after,
        objective_calls,
        child: SliceVerdict { evaluations: child.evaluations(), steps: child.steps(),
            used: child.used(), remaining: child.remaining(), denied: child.denied(), granted },
        grandchild: SliceVerdict { evaluations: grandchild.evaluations(),
            steps: grandchild.steps(), used: grandchild.used(), remaining: grandchild.remaining(),
            denied: grandchild.denied(), granted: grandchild_granted },
        random: RandomVerdict { state_before, draws, state_after, snapshot_state, resumed_draw,
            resume_matches, split_state, split_moves_parent, identical_seeds_agree,
            draws_in_unit_interval },
        global_stops_at_limit,
        steps_do_not_gate_admission,
        child_charges_parent,
        parent_refusal_rolls_back,
        denied_is_sticky,
        remaining_is_minimum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(limit: u64, used: u64, evaluations: u64, claims: u32) -> SearchBudgetRequest {
        SearchBudgetRequest { limit, used, objective_calls: 0, evaluations, steps: 1,
            grandchild_evaluations: 0, claims, grandchild_claims: 0, random_state: 7,
            random_draws: 4, split_id: "seed".to_owned() }
    }

    #[test]
    fn the_global_never_grants_more_than_its_limit() {
        let report = verify_search_budget(&request(3, 0, 10, 10)).unwrap();
        assert!(report.global_stops_at_limit);
        assert_eq!(report.used_after, 3);
        assert_eq!(report.remaining_after, 0);
        // 起始已用时，余量按差额算。报告里的 `used_after` 只反映**实际申请**，不含抽干探测。
        // With a starting usage the remainder is the difference; `used_after` reflects **actual claims only**, not the
        // drain probe.
        let report = verify_search_budget(&request(5, 2, 10, 1)).unwrap();
        assert!(report.global_stops_at_limit);
        assert_eq!(report.used_before, 2);
        assert_eq!(report.used_after, 3);
        assert_eq!(report.remaining_after, 2);
    }

    #[test]
    fn steps_do_not_gate_admission() {
        let report = verify_search_budget(&request(100, 0, 3, 10)).unwrap();
        assert!(report.steps_do_not_gate_admission);
        // 子片段 `steps = 1`，但获批 3 次 → 步数不是准入条件。 / `steps = 1` yet three claims are granted.
        assert_eq!(report.child.steps, 1);
        assert_eq!(report.child.granted, 3);
        assert_eq!(report.child.used, 3);
        assert!(report.child.denied);
    }

    #[test]
    fn a_parent_refusal_rolls_the_local_count_back() {
        // 本地额度 5、全局只剩 1：第 2 次申请本地先加一，被全局拒后减回 1。
        // Local 5, global 1: the second claim bumps locally then rolls back to 1.
        let report = verify_search_budget(&request(1, 0, 5, 2)).unwrap();
        assert!(report.parent_refusal_rolls_back);
        assert_eq!(report.child.used, 1);
        assert_eq!(report.child.granted, 1);
        assert!(report.child.denied);
        // `denied` 粘性：再申请仍是 true。 / `denied` is sticky.
        assert!(report.denied_is_sticky);
    }

    #[test]
    fn the_remainder_is_the_stricter_of_local_and_parent() {
        let report = verify_search_budget(&request(3, 0, 100, 0)).unwrap();
        assert!(report.remaining_is_minimum);
        assert_eq!(report.child.remaining, 3);
        assert_eq!(report.child.evaluations, 100);
    }

    #[test]
    fn a_grandchild_charges_the_child_and_the_global() {
        let mut document = request(100, 0, 10, 0);
        document.grandchild_evaluations = 4;
        document.grandchild_claims = 2;
        let report = verify_search_budget(&document).unwrap();
        assert_eq!(report.grandchild.granted, 2);
        assert_eq!(report.grandchild.used, 2);
        // 孙片段的领取同时记到子片段与全局。 / Grandchild claims charge both the child and the global.
        assert_eq!(report.child.used, 2);
        assert_eq!(report.used_after, 2);
        assert!(report.child_charges_parent);
        // 目标调用次数不随领取变化。 / The objective call count is untouched.
        assert_eq!(report.objective_calls, 0);
    }

    #[test]
    fn the_random_stream_is_reproducible_and_resumable() {
        let report = verify_search_budget(&request(10, 0, 10, 1)).unwrap();
        let random = &report.random;
        assert_eq!(random.draws.len(), 4);
        assert!(random.draws_in_unit_interval);
        assert!(random.identical_seeds_agree);
        // 每个状态都能当快照：从最后一次抽取之前的状态恢复，必须复现最后一次。
        // Every state is a snapshot: resuming from before the last draw reproduces it.
        assert!(random.resume_matches);
        assert_eq!(random.resumed_draw, random.draws[3]);
        assert_ne!(random.state_before, random.state_after);
        // `split` 派生稳定子流但**不消费**父流。 / `split` derives without consuming the parent.
        assert!(!random.split_moves_parent);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut document = request(10, 0, 10, 1);
        document.used = 11;
        assert!(verify_search_budget(&document).unwrap_err().message.contains("exceeds the limit"));
        let many = request(10, 0, 10, MAX_CLAIMS + 1);
        assert!(verify_search_budget(&many).unwrap_err().message.contains("at most"));
        let mut many_draws = request(10, 0, 10, 1);
        many_draws.random_draws = MAX_DRAWS + 1;
        assert!(verify_search_budget(&many_draws).unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_search_budget(&request(10, 0, 10, 1)).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"usedBefore\""), "{json}");
        assert!(json.contains("\"deniedIsSticky\""), "{json}");
        assert!(json.contains("\"resumeMatches\""), "{json}");
        assert!(!json.contains("used_before"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<SearchBudgetReport>(&copy.to_string()).is_err());
    }

    #[test]
    fn an_exhausted_parent_blocks_its_grandchild_even_with_global_room() {
        // 子片段本地额度 2 被自身领满，全局还有余量，但孙片段仍被拒 —— 配额守恒。
        // The child's own local allowance (2) is used up while the global still has room, yet the grandchild is refused:
        // quota is conserved.
        let mut document = request(50, 0, 2, 2);
        document.grandchild_evaluations = 5;
        document.grandchild_claims = 3;
        let report = verify_search_budget(&document).unwrap();
        assert_eq!(report.child.used, 2);
        assert!(report.child.denied);
        assert_eq!(report.grandchild.granted, 0);
        assert_eq!(report.grandchild.used, 0);
        assert!(report.grandchild.denied);
        // 全局只被扣了子片段那两次。 / The global only paid for the child's two claims.
        assert_eq!(report.used_after, 2);
        assert!(report.remaining_after > 0);
    }
}
