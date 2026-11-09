//! 续挖第 13 单：函数符号使用语境的四个派生判定（`model/intermediate/function_usage.rs`）
//! Continuation, delivery 13: the four derived predicates of the function-usage summary.
//!
//! `FunctionUsageSummary` 是五个布尔标志加四个派生判定。派生判定的公式**互不相同**，而且差别正是全部要点——
//! 把 `in_objective` 当成"结果必须保留"是最容易犯的错：
//!
//! | 派生判定 | 公式 |
//! | --- | --- |
//! | `requires_referenceable_result` | `in_constraint \|\| nested_as_input`（**不含 `in_objective`**） |
//! | `helpers_are_exclusive` | `!externally_referenced`（**与结果列无关**） |
//! | `result_is_consumed` | `in_objective \|\| in_constraint \|\| nested_as_input` |
//! | `forbids_native_write` | `result_is_fixed`（**只看这一个**） |
//!
//! 四条实测关系：
//!
//! 1. **结果只进目标函数时不要求"结果可被引用"**：`in_objective` 单独为真时 `result_is_consumed` 为真而
//!    `requires_referenceable_result` 为**假**——因为只作用于目标的快捷原生接口能替代 `y = f(x)` 关系。全格 32 种组合里，
//!    这样的组合**有 4 种**（`in_objective` 为真、`in_constraint` 与 `nested_as_input` 为假，另两个标志自由）。
//! 2. **`requires_referenceable_result` 蕴含 `result_is_consumed`**，反之不然（上一条就是反例）。
//! 3. **`forbids_native_write` 只由 `result_is_fixed` 决定**：其余四个标志怎么组合都不禁止原生写入；而"结果被固定"
//!    与"结果需要被别的行引用"是**两个不同方向**的约束，可以同时成立（固定列被别的行引用 → 必须回退）。
//! 4. **`helpers_are_exclusive` 与另外四个标志完全独立**：它只说辅助列没被外部引用，不管结果列怎样。
//!
//! Five boolean flags plus four derived predicates, and the formulas **differ** — that difference is the whole point.
//! Treating `in_objective` as "the result must be kept" is the easy mistake. Measured: **an objective-only result does not
//! require a referenceable result** (`result_is_consumed` true while `requires_referenceable_result` is **false**), because an
//! objective-only native shortcut replaces the `y = f(x)` relation — exactly **one** of the 32 flag combinations looks like
//! that (**four** of the 32, since the external and fixed flags are free); **`requires_referenceable_result` implies
//! `result_is_consumed`** but not the other way round; **`forbids_native_write`
//! is decided by `result_is_fixed` alone** (the other four flags never forbid a native write) while "result is fixed" and
//! "result must stay referenceable" are constraints in **different** directions that can hold together; and
//! **`helpers_are_exclusive` is independent of all four**, saying only that no external row touches the helpers.

use ospf_rust_core::model::intermediate::FunctionUsageSummary;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一组标志。 / One set of flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageFlags {
    /// 结果列是否进入目标函数。 / Whether the result column enters the objective.
    #[serde(default)]
    pub in_objective: bool,
    /// 结果列是否进入非本函数的约束行。 / Whether the result enters a row that is not this function's own.
    #[serde(default)]
    pub in_constraint: bool,
    /// 结果列是否被其它中间符号当作输入。 / Whether another symbol consumes the result as input.
    #[serde(default)]
    pub nested_as_input: bool,
    /// 辅助列是否被外部引用。 / Whether a helper column is referenced externally.
    #[serde(default)]
    pub externally_referenced: bool,
    /// 结果列是否已被固定。 / Whether the result column is already fixed.
    #[serde(default)]
    pub result_is_fixed: bool,
}

/// 用例请求。 / The case request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionUsageRequest {
    /// 要看的标志组合。 / The flag combinations to inspect.
    #[serde(default)]
    pub cases: Vec<UsageFlags>,
}

/// 一个组合的结论。 / One combination's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageRow {
    /// 输入标志。 / The input flags.
    pub flags: UsageFlags,
    /// 结果是否需要保持可被引用。 / Whether the result must stay referenceable.
    pub requires_referenceable_result: bool,
    /// 辅助列是否被本函数独占。 / Whether the helpers are exclusive to this function.
    pub helpers_are_exclusive: bool,
    /// 结果是否被任何下游消费者使用。 / Whether any downstream consumer uses the result.
    pub result_is_consumed: bool,
    /// 原生写入是否被禁止。 / Whether a native write is forbidden.
    pub forbids_native_write: bool,
}

/// 语境结论。 / The usage verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionUsageReport {
    /// 请求里的用例数。 / The number of requested cases.
    pub case_count: u32,
    /// 逐条结论（顺序同请求）。 / Each verdict, in request order.
    pub cases: Vec<UsageRow>,
    /// 全格组合数（**32**）。 / The full grid size (**32**).
    pub grid_size: u32,
    /// 全格里"被消费但不要求可被引用"的组合数（实测 **4**）。 / The combos consumed but not referenceable (**4**).
    pub consumed_but_not_referenceable: u32,
    /// 全格里"要求可被引用"的组合数。 / The combos requiring a referenceable result.
    pub referenceable_count: u32,
    /// 全格里"禁止原生写入"的组合数。 / The combos forbidding a native write.
    pub forbidden_count: u32,
    /// 全格里"辅助列独占"的组合数。 / The combos whose helpers are exclusive.
    pub exclusive_count: u32,
    /// 全格里"既要求可被引用又禁止原生写入"的组合数。 / The combos that both require and forbid.
    pub referenceable_and_forbidden: u32,
    /// 全格上"要求可被引用 ⟹ 被消费"是否恒成立（**是**）。 / Whether requiring implies consumed (**yes**).
    pub requires_implies_consumed: bool,
    /// 全格上"禁止原生写入 ⟺ 结果被固定"是否恒成立（**是**）。 / Whether forbidding equals fixed (**yes**).
    pub forbids_equals_fixed: bool,
    /// 全格上"辅助列独占 ⟺ 未被外部引用"是否恒成立（**是**）。 / Whether exclusive equals not-referenced (**yes**).
    pub exclusive_equals_not_referenced: bool,
}

/// 用例上限（信任边界）。 / The case cap (a trust boundary).
const MAX_CASES: usize = 64;

/// 走一遍四个派生判定，并把 32 种组合全部枚举一遍作为证据。非法输入是**错误**。
/// Walk the four derived predicates and enumerate all 32 combinations as evidence.
pub fn verify_function_usage(request: &FunctionUsageRequest) -> Result<FunctionUsageReport> {
    if request.cases.len() > MAX_CASES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "function usage failed validation: at most {MAX_CASES} cases are accepted")));
    }
    let cases = request.cases.iter().map(|flags| evaluate(*flags)).collect::<Vec<_>>();
    // 全格：五个布尔标志的 32 种组合，一个不漏。 / The full grid: all 32 combinations, none skipped.
    let grid = (0u8..32).map(|mask| evaluate(decompose(mask))).collect::<Vec<_>>();
    let mut consumed_but_not_referenceable = 0u32;
    let mut referenceable_count = 0u32;
    let mut forbidden_count = 0u32;
    let mut exclusive_count = 0u32;
    let mut referenceable_and_forbidden = 0u32;
    let mut requires_implies_consumed = true;
    let mut forbids_equals_fixed = true;
    let mut exclusive_equals_not_referenced = true;
    for row in &grid {
        if row.result_is_consumed && !row.requires_referenceable_result {
            consumed_but_not_referenceable = consumed_but_not_referenceable.saturating_add(1);
        }
        if row.requires_referenceable_result {
            referenceable_count = referenceable_count.saturating_add(1);
            if row.forbids_native_write {
                referenceable_and_forbidden = referenceable_and_forbidden.saturating_add(1);
            }
        }
        if row.forbids_native_write {
            forbidden_count = forbidden_count.saturating_add(1);
        }
        if row.helpers_are_exclusive {
            exclusive_count = exclusive_count.saturating_add(1);
        }
        // 三条恒等式：任何一条被打破都说明领域的公式变了。 / Three identities.
        requires_implies_consumed &= !row.requires_referenceable_result || row.result_is_consumed;
        forbids_equals_fixed &= row.forbids_native_write == row.flags.result_is_fixed;
        exclusive_equals_not_referenced &=
            row.helpers_are_exclusive == !row.flags.externally_referenced;
    }
    Ok(FunctionUsageReport {
        case_count: u32::try_from(cases.len()).unwrap_or(u32::MAX),
        cases,
        grid_size: u32::try_from(grid.len()).unwrap_or(u32::MAX),
        consumed_but_not_referenceable,
        referenceable_count,
        forbidden_count,
        exclusive_count,
        referenceable_and_forbidden,
        requires_implies_consumed,
        forbids_equals_fixed,
        exclusive_equals_not_referenced,
    })
}

/// 一个组合的全部结论。 / Every verdict for one combination.
fn evaluate(flags: UsageFlags) -> UsageRow {
    let summary = FunctionUsageSummary { in_objective: flags.in_objective,
        in_constraint: flags.in_constraint, nested_as_input: flags.nested_as_input,
        externally_referenced: flags.externally_referenced, result_is_fixed: flags.result_is_fixed };
    UsageRow {
        flags,
        requires_referenceable_result: summary.requires_referenceable_result(),
        helpers_are_exclusive: summary.helpers_are_exclusive(),
        result_is_consumed: summary.result_is_consumed(),
        forbids_native_write: summary.forbids_native_write(),
    }
}

/// 位掩码 → 五个标志。 / A bit mask into the five flags.
fn decompose(mask: u8) -> UsageFlags {
    UsageFlags {
        in_objective: mask & 1 != 0,
        in_constraint: mask & 2 != 0,
        nested_as_input: mask & 4 != 0,
        externally_referenced: mask & 8 != 0,
        result_is_fixed: mask & 16 != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(objective: bool, constraint: bool, nested: bool, external: bool, fixed: bool)
        -> UsageFlags
    {
        UsageFlags { in_objective: objective, in_constraint: constraint,
            nested_as_input: nested, externally_referenced: external, result_is_fixed: fixed }
    }

    fn request(cases: Vec<UsageFlags>) -> FunctionUsageRequest {
        FunctionUsageRequest { cases }
    }

    #[test]
    fn objective_only_is_consumed_but_not_referenceable() {
        // 结果只进目标：**被消费**，但**不要求可被引用**——快捷原生接口能替代 `y = f(x)`。
        // An objective-only result is **consumed** but **not** required to stay referenceable.
        let report = verify_function_usage(&request(vec![flags(true, false, false, false, false)]))
            .unwrap();
        let row = report.cases[0];
        assert!(row.result_is_consumed);
        assert!(!row.requires_referenceable_result);
        assert!(row.helpers_are_exclusive);
        assert!(!row.forbids_native_write);
        // 全格里这种组合有 4 种：`in_objective` 真、`in_constraint` 与 `nested_as_input` 假，另两个标志自由。
        // Four combinations: objective true, constraint and nested false, the other two flags free.
        assert_eq!(report.consumed_but_not_referenceable, 4);
        assert_eq!(report.grid_size, 32);
    }

    #[test]
    fn constraint_or_nested_requires_a_referenceable_result() {
        for (constraint, nested) in [(true, false), (false, true), (true, true)] {
            let report = verify_function_usage(&request(vec![flags(false, constraint, nested,
                false, false)])).unwrap();
            let row = report.cases[0];
            assert!(row.requires_referenceable_result, "{constraint}/{nested}");
            assert!(row.result_is_consumed, "{constraint}/{nested}");
            assert!(!row.forbids_native_write, "{constraint}/{nested}");
        }
        // 要求可被引用 ⟹ 被消费，全格恒成立。 / Requiring implies consumed across the whole grid.
        let report = verify_function_usage(&request(Vec::new())).unwrap();
        assert!(report.requires_implies_consumed);
        // 24 种组合要求可被引用：`in_constraint || nested` 占 4 种里的 3 种，另三个标志自由（3 × 8）。
        // Twenty-four combinations require it: three of four for the pair, times eight for the rest.
        assert_eq!(report.referenceable_count, 24);
    }

    #[test]
    fn forbidding_native_write_follows_only_the_fixed_flag() {
        let report = verify_function_usage(&request(Vec::new())).unwrap();
        assert!(report.forbids_equals_fixed);
        // 固定 → 禁止，16 种组合。 / Fixed → forbidden, sixteen combinations.
        assert_eq!(report.forbidden_count, 16);
        // 既要求可被引用又禁止原生写入：3（约束/嵌套的三种）× 2（目标）× 2（外部）= **12**。
        // 我手算时漏了"另两个标志自由"这一层，先写成 6，被实测打回。
        // Both at once: 3 × 2 × 2 = **12** — I hand-computed 6 by dropping the free flags.
        assert_eq!(report.referenceable_and_forbidden, 12);
        // 结果被固定但**没有**被消费、也不是可被引用的：仍禁止原生写入。
        // Fixed but neither consumed nor referenceable: still forbidden.
        let report = verify_function_usage(&request(vec![flags(false, false, false, false, true)]))
            .unwrap();
        let row = report.cases[0];
        assert!(row.forbids_native_write);
        assert!(!row.result_is_consumed);
        assert!(!row.requires_referenceable_result);
        // 反过来说：要求可被引用**不**禁止原生写入，只要求保留结果列。
        // Conversely, requiring a referenceable result does not forbid a native write.
        let report = verify_function_usage(&request(vec![flags(false, true, false, false, false)]))
            .unwrap();
        assert!(report.cases[0].requires_referenceable_result);
        assert!(!report.cases[0].forbids_native_write);
    }

    #[test]
    fn helpers_are_exclusive_regardless_of_the_result_column() {
        let report = verify_function_usage(&request(Vec::new())).unwrap();
        assert!(report.exclusive_equals_not_referenced);
        // 辅助列独占只看 `externally_referenced`，所以 32 种里 16 种。 / Sixteen of thirty-two.
        assert_eq!(report.exclusive_count, 16);
        // 结果列被消费也不影响辅助列的独占性。 / A consumed result does not change helper exclusivity.
        let report = verify_function_usage(&request(vec![flags(true, true, true, false, true)]))
            .unwrap();
        let row = report.cases[0];
        assert!(row.helpers_are_exclusive);
        assert!(row.requires_referenceable_result);
        assert!(row.forbids_native_write);
        let report = verify_function_usage(&request(vec![flags(true, true, true, true, true)]))
            .unwrap();
        assert!(!report.cases[0].helpers_are_exclusive);
    }

    #[test]
    fn bad_input_is_refused_and_all_zero_is_the_quiet_case() {
        // 全零：什么都没用、什么都没固定。 / All false: nothing used, nothing fixed.
        let report = verify_function_usage(&request(vec![flags(false, false, false, false, false)]))
            .unwrap();
        let row = report.cases[0];
        assert!(!row.result_is_consumed);
        assert!(!row.requires_referenceable_result);
        assert!(row.helpers_are_exclusive);
        assert!(!row.forbids_native_write);
        // 空请求合法，报告仍然给出全格结论。 / A bare request is legal and still reports the grid.
        let report = verify_function_usage(&request(Vec::new())).unwrap();
        assert_eq!(report.case_count, 0);
        assert!(report.cases.is_empty());
        assert_eq!(report.grid_size, 32);
        // 上限。 / The cap.
        let many = request(vec![flags(false, false, false, false, false); MAX_CASES + 1]);
        assert!(verify_function_usage(&many).unwrap_err().message.contains("at most"));
    }
}
