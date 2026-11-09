//! 列生成批次的证据绑定：空列不是证明，只有完整作用域 + 无改进证据才算。
//! Column-generation batch evidence binding: an empty pool is not a proof; only a full scope plus no-improvement
//! evidence counts.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use ospf_rust_framework::model::column_generation_batch::{
    ColumnGenerationBatch, ColumnGenerationCompletion, ColumnGenerationScope,
};

use crate::{BridgeError, Result};

/// 定价请求完成状态。 / Pricing-request completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LinearColumnCompletion {
    /// 完整搜索。 / Exhaustive search.
    Exhausted,
    /// 计算预算耗尽。 / Computational budget exhausted.
    BudgetLimit,
    /// 协作中断。 / Cooperative interruption.
    Interrupted,
    /// 未认证启发式。 / Uncertified heuristic.
    Heuristic,
}

impl LinearColumnCompletion {
    fn to_domain(self) -> ColumnGenerationCompletion {
        match self {
            Self::Exhausted => ColumnGenerationCompletion::Exhausted,
            Self::BudgetLimit => ColumnGenerationCompletion::BudgetLimit,
            Self::Interrupted => ColumnGenerationCompletion::Interrupted,
            Self::Heuristic => ColumnGenerationCompletion::Heuristic,
        }
    }
}

/// 证据适用范围；标识一致不代替 LP 证书来源验证。 / Evidence scope; matching identities do not validate LP provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearColumnScope {
    /// 本轮唯一请求。 / Unique current request.
    pub request: String,
    /// 覆盖的业务组。 / Covered business groups.
    pub groups: Vec<String>,
    /// 模型版本。 / Model revision.
    pub model: String,
    /// 已验证对偶版本。 / Verified dual revision.
    pub dual: String,
    /// 分支与固定状态版本。 / Branch and fixing revision.
    pub branch: String,
    /// 是否覆盖声明的完整业务域。 / Whether the full declared domain is covered.
    pub global: bool,
    /// 执行器时隙身份。 / Executor-slot identities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub executor_slots: Vec<String>,
}

impl LinearColumnScope {
    fn to_domain(&self) -> Result<ColumnGenerationScope> {
        // 空标识必须在这里拒绝：领域校验器要求每个身份非空，但这里先给出明确的桥接错误。
        // Blank identities are rejected here: the domain validator requires every identity to be non-empty, but this
        // surface reports it as an explicit bridge error.
        for (label, value) in [("request", &self.request), ("model", &self.model), ("dual", &self.dual),
            ("branch", &self.branch)] {
            if value.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", &format!("column scope {label} must not be blank")));
            }
        }
        let mut groups = BTreeSet::new();
        for group in &self.groups {
            if group.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "column scope group identities must not be blank"));
            }
            if !groups.insert(group.clone()) {
                return Err(BridgeError::new("InvalidInput", "column scope repeats a group identity"));
            }
        }
        let mut slots = BTreeSet::new();
        for slot in &self.executor_slots {
            if slot.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "column scope executor-slot identities must not be blank"));
            }
            if !slots.insert(slot.clone()) {
                return Err(BridgeError::new("InvalidInput", "column scope repeats an executor-slot identity"));
            }
        }
        Ok(ColumnGenerationScope { request: self.request.clone(), groups, model: self.model.clone(),
            dual: self.dual.clone(), branch: self.branch.clone(), global: self.global, executor_slots: slots })
    }
}

/// 一次定价批次：候选列数量 + 完成状态 + 声明的证据域 + 独立无改进证据 + 分组子请求。
/// One pricing batch: candidate count, completion, declared scope, independent no-improvement evidence and grouped children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearColumnBatch {
    /// 候选列数量；桥接只关心"有没有"，不搬运列内容。 / Candidate count; the bridge only needs presence, not the column payload.
    pub column_count: u32,
    /// 真实完成状态。 / Actual completion.
    pub completion: LinearColumnCompletion,
    /// 声明的证据域；未声明时为空。 / Declared evidence scope; absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub scope: Option<LinearColumnScope>,
    /// 独立无改进证据；未知时为空。 / Independent no-improvement evidence; absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub no_improving_columns: Option<bool>,
    /// 分组子请求。 / Grouped child requests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<LinearColumnBatch>,
}

/// 把 DTO 展开成领域批次；列内容用占位 `()`，因为判定只依赖数量与证据。
/// Expand the DTO into a domain batch; the column payload is `()`, because the verdict only depends on presence and
/// evidence.
fn to_domain(batch: &LinearColumnBatch, depth: usize) -> Result<ColumnGenerationBatch<()>> {
    if depth > MAX_DEPTH {
        return Err(BridgeError::new("InvalidInput", "column batch nesting exceeds the supported depth"));
    }
    let scope = batch.scope.as_ref().map(LinearColumnScope::to_domain).transpose()?;
    let columns = vec![(); usize::try_from(batch.column_count)
        .map_err(|_| BridgeError::new("InvalidInput", "column count overflow"))?];
    let mut children = Vec::with_capacity(batch.children.len());
    for child in &batch.children {
        children.push(to_domain(child, depth + 1)?);
    }
    Ok(ColumnGenerationBatch { columns, completion: batch.completion.to_domain(), scope,
        no_improving_columns: batch.no_improving_columns, children })
}

/// 嵌套深度上限：防止恶意输入把校验拖成栈溢出。
/// Nesting bound so a hostile payload cannot turn validation into a stack overflow.
const MAX_DEPTH: usize = 32;

/// 证据结论：两个判定分开给出，绝不合并成单个布尔。
/// Evidence verdict: the two checks are reported separately and never merged into one boolean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearColumnCertificate {
    /// 批次是否在声明作用域内被完整搜索。 / Whether the batch was exhaustively searched within its declared scope.
    pub certifies_exhaustive: bool,
    /// 是否连同无改进证据一起成立。 / Whether it also holds together with the no-improvement evidence.
    pub certifies_no_improving_columns: bool,
    /// 被复核的声明作用域。 / The declared scope that was rechecked.
    pub scope: LinearColumnScope,
}

/// 对照声明作用域复核一个批次。 / Recheck one batch against a declared scope.
pub fn verify_column_batch(batch: &LinearColumnBatch, expected: &LinearColumnScope) -> Result<LinearColumnCertificate> {
    let domain = to_domain(batch, 0)?;
    let expected_domain = expected.to_domain()?;
    // 领域判定自带全部不变量（完成状态、global、标识非空、子请求覆盖笛卡尔域），这里只如实转述。
    // The domain verdict carries every invariant (completion, `global`, non-blank identities, child coverage of the
    // Cartesian domain); this surface only relays it faithfully.
    let certifies_exhaustive = domain.certifies(&expected_domain);
    let certifies_no_improving_columns = domain.certifies_no_improving_columns(&expected_domain);
    // 无改进证据成立时必须同时完整搜索，否则就是自相矛盾的声明。
    // No-improvement evidence implies exhaustive search; the opposite would be a self-contradictory claim.
    if certifies_no_improving_columns && !certifies_exhaustive {
        return Err(BridgeError::new("InvalidReport",
            "a no-improvement certificate cannot hold without an exhaustive certificate"));
    }
    Ok(LinearColumnCertificate { certifies_exhaustive, certifies_no_improving_columns, scope: expected.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> LinearColumnScope {
        LinearColumnScope { request: "round-1".into(), groups: vec!["a".into(), "b".into()], model: "model-1".into(),
            dual: "dual-1".into(), branch: "root".into(), global: true,
            executor_slots: vec!["a:0".into(), "b:0".into()] }
    }

    fn exhausted() -> LinearColumnBatch {
        LinearColumnBatch { column_count: 0, completion: LinearColumnCompletion::Exhausted, scope: Some(scope()),
            no_improving_columns: Some(true), children: Vec::new() }
    }

    #[test]
    fn empty_heuristic_batches_never_certify_and_blank_scopes_are_refused() {
        // 启发式空列不是证明。 / An empty heuristic pool is not a proof.
        let heuristic = LinearColumnBatch { completion: LinearColumnCompletion::Heuristic, scope: None,
            no_improving_columns: None, ..exhausted() };
        let certificate = verify_column_batch(&heuristic, &scope()).unwrap();
        assert!(!certificate.certifies_exhaustive);
        assert!(!certificate.certifies_no_improving_columns);
        // 完整搜索但缺无改进证据：完整搜索成立，无改进不成立。
        // Exhaustive but without no-improvement evidence: the first holds, the second does not.
        let no_evidence = LinearColumnBatch { no_improving_columns: None, ..exhausted() };
        let certificate = verify_column_batch(&no_evidence, &scope()).unwrap();
        assert!(certificate.certifies_exhaustive);
        assert!(!certificate.certifies_no_improving_columns);
        // 非全局作用域不可能成立。 / A non-global scope can never hold.
        let mut partial = scope();
        partial.global = false;
        let certificate = verify_column_batch(&exhausted(), &partial).unwrap();
        assert!(!certificate.certifies_exhaustive);
        // 空标识必须被拒。 / Blank identities must be refused.
        let mut blank = scope();
        blank.request = "  ".into();
        assert!(verify_column_batch(&exhausted(), &blank).is_err());
        let mut repeated = scope();
        repeated.groups.push("a".into());
        assert!(verify_column_batch(&exhausted(), &repeated).is_err());
    }

    #[test]
    fn child_coverage_is_checked_on_the_cartesian_domain() {
        // 子请求分别覆盖两轴，但没有覆盖组合：不能成立。
        // Children cover each axis separately without covering the combinations: this cannot hold.
        let mut parent = LinearColumnBatch { children: Vec::new(), ..exhausted() };
        for group in ["a", "b"] {
            let mut child_scope = scope();
            child_scope.request = format!("round-1:{group}");
            child_scope.groups = vec![group.to_owned()];
            child_scope.executor_slots = vec![format!("{group}:0")];
            parent.children.push(LinearColumnBatch { scope: Some(child_scope), ..exhausted() });
        }
        assert!(!verify_column_batch(&parent, &scope()).unwrap().certifies_no_improving_columns);
        // 每个子请求覆盖全部时隙后成立。 / It holds once each child covers every slot.
        for child in &mut parent.children {
            if let Some(child_scope) = child.scope.as_mut() {
                child_scope.executor_slots = scope().executor_slots;
            }
        }
        assert!(verify_column_batch(&parent, &scope()).unwrap().certifies_no_improving_columns);
        // 一个子请求对偶过期后整批不再成立。 / One stale child dual breaks the whole batch.
        if let Some(child_scope) = parent.children[1].scope.as_mut() {
            child_scope.dual = "stale".into();
        }
        assert!(!verify_column_batch(&parent, &scope()).unwrap().certifies_exhaustive);
    }

    #[test]
    fn nesting_is_bounded() {
        fn nest(depth: usize) -> LinearColumnBatch {
            let mut batch = LinearColumnBatch { column_count: 0, completion: LinearColumnCompletion::Heuristic,
                scope: None, no_improving_columns: None, children: Vec::new() };
            for _ in 0..depth {
                let inner = batch.clone();
                batch = LinearColumnBatch { children: vec![inner], ..batch };
            }
            batch
        }
        assert!(verify_column_batch(&nest(MAX_DEPTH + 2), &scope()).is_err());
    }
}
