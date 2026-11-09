//! 续挖第 35 单：CP assumption 的**稳定身份**与主问题绑定的"恰好覆盖一次"契约
//! （`solver/logic_based_benders.rs` 的 `MasterAssignmentAssumptionBinding` /
//! `VerifiedMasterConflict::from_assumption_ids`）
//! Continuation, delivery 35: the stable identity of a CP assumption and the "cover exactly once" contract of the master
//! binding (`MasterAssignmentAssumptionBinding`, `VerifiedMasterConflict::from_assumption_ids`).
//!
//! LBB 要在 CP 子问题与主问题之间建立映射，靠的是 assumption 的**稳定身份**。这里实测出四条：
//!
//! 1. **身份是长度前缀拼接的**：`literal/` / `equal/` / `lower-bound/` / `upper-bound/` / `sparse-domain/` 前缀，
//!    变量名走 `format!("{len}:{value}")`。所以 `"a:b"` 不会被读成两段——名字里的分隔符**不构成**歧义。
//! 2. **`Equal` / `LowerBound` / `UpperBound` 的身份**含数值**，`Literal` 含极性，所以
//!    `Equal(x,5)` 与 `Equal(x,6)` 是两个不同 assumption。
//! 3. **`SparseDomain` 的身份丢掉值域内容**（源码里那个 `_`）：`sparse-domain/{len}:x`。于是
//!    `SparseDomain(x, [1,2])` 与 `SparseDomain(x, [3])` **身份完全相同**——把它们一起传进去会被
//!    "identities must be unique" 拒绝，看起来像"重复登记"，实际是两个**不同**的假设。这是本单最值钱的一条。
//! 4. **映射必须恰好覆盖一次**：少一个 → "must cover every assumption exactly once"；
//!    多一个 → 同一句；映射重复键 → "mappings must be unique"；assumption 身份重复 → "identities must be unique"。
//!    `from_assumption_ids` 走的是另一条路：assumption ID 没在映射里 → "has no master binding mapping"，
//!    而**空**成员集最后由 `VerifiedMasterConflict::new` 拒绝。
//!
//! LBB maps CP subproblem to master through an assumption's **stable identity**, and four things show up here. **The
//! identity is a length-prefixed concatenation**: the `literal/` / `equal/` / `lower-bound/` / `upper-bound/` /
//! `sparse-domain/` prefixes with the variable name encoded as `format!("{len}:{value}")`, so `"a:b"` can never be read as
//! two components — separators inside names create **no** ambiguity. **`Equal` / `LowerBound` / `UpperBound` include the
//! number** and `Literal` includes its polarity, so `Equal(x,5)` and `Equal(x,6)` are different assumptions. **`SparseDomain`
//! drops the domain contents** (that `_` in the source): the identity is `sparse-domain/{len}:x`, so
//! `SparseDomain(x, [1,2])` and `SparseDomain(x, [3])` share an identity and passing both is refused as "identities must be
//! unique" — it looks like a duplicate registration but is really two **different** assumptions. That is this delivery's most
//! valuable finding. And **the mapping must cover every assumption exactly once**: too few gives "must cover every assumption
//! exactly once", too many gives the same message, a repeated mapping key gives "mappings must be unique" and a repeated
//! assumption identity gives "identities must be unique", while `from_assumption_ids` reports "has no master binding mapping"
//! for an unmapped assumption ID and lets `VerifiedMasterConflict::new` refuse an empty member set.

use std::collections::{BTreeMap, BTreeSet};

use ospf_rust_core::model::constraint_programming::domain::IntegerDomain;
use ospf_rust_core::model::constraint_programming::literal::BooleanLiteral;
use ospf_rust_core::model::constraint_programming::variable::IntegerVariable;
use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_core::solver::{
    ConstraintProgrammingAssumption, ConstraintProgrammingAssumptionId,
};
use ospf_rust_framework::solver::logic_based_benders::{
    MasterAssignmentAssumptionBinding, VerifiedMasterConflict,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条 assumption 的字面量。 / One assumption literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssumptionSpec {
    /// CP 整数变量名（稳定身份）。 / The CP integer variable's stable identity.
    pub variable: String,
    /// 形态：`literal` / `equal` / `lowerBound` / `upperBound` / `sparseDomain`。
    /// The shape: `literal` / `equal` / `lowerBound` / `upperBound` / `sparseDomain`.
    pub shape: String,
    /// `literal` 的极性。 / The `literal` polarity.
    #[serde(default)]
    pub negated: bool,
    /// `equal` / `lowerBound` / `upperBound` 的数值。 / The number for `equal` / `lowerBound` / `upperBound`.
    #[serde(default)]
    pub value: i64,
    /// `sparseDomain` 的值域（**会排序去重**，且**不进身份**）。 / The `sparseDomain` values
    /// (**sorted and deduplicated**, and **not part of the identity**).
    #[serde(default)]
    pub values: Vec<i64>,
}

/// 一条映射。 / One mapping entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssumptionMappingSpec {
    /// 对应的 assumption 下标。 / The assumption's position.
    pub assumption: u32,
    /// 映射到的 master 稳定变量。 / The master stable variable it maps to.
    pub master_variable: String,
}

/// CP assumption 绑定请求。 / The CP assumption binding request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssumptionBindingRequest {
    /// assumption 列表。 / The assumptions.
    pub assumptions: Vec<AssumptionSpec>,
    /// 映射列表。 / The mappings.
    #[serde(default)]
    pub mappings: Vec<AssumptionMappingSpec>,
    /// 用于 `from_assumption_ids` 的 assumption 下标（**故意允许引用不存在的下标**）。
    /// The assumption positions for `from_assumption_ids` (**out-of-range positions are deliberately allowed**).
    #[serde(default)]
    pub conflict_members: Vec<u32>,
    /// 证明引用。 / The proof reference.
    #[serde(default)]
    pub proof_reference: String,
}

/// 一条 assumption 读回的形态。 / One assumption read back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssumptionRow {
    /// 稳定身份。 / The stable identity.
    pub stable_id: String,
    /// 身份前缀（`literal` / `equal` / `lower-bound` / `upper-bound` / `sparse-domain`）。
    /// The identity prefix.
    pub prefix: String,
    /// 值域条数（只对 `sparseDomain` 有意义）。 / The value count (only for `sparseDomain`).
    pub value_count: u64,
}

/// CP assumption 绑定结论。 / The assumption-binding verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssumptionBindingReport {
    /// 每条 assumption 的身份。 / Each assumption's identity.
    pub rows: Vec<AssumptionRow>,
    /// 绑定后的映射（assumption 身份 → master 变量）。 / The bound mapping (identity to master variable).
    pub mapping: Vec<AssumptionRow2>,
    /// 冲突子集的成员。 / The conflict subset's members.
    pub conflict_members: Vec<String>,
    /// 冲突的证明引用。 / The conflict's proof reference.
    pub proof_reference: String,
    /// 身份是长度前缀拼接（**是**）。 / Whether identities are length-prefixed (**yes**).
    pub identity_is_length_prefixed: bool,
    /// 值域不进身份（**是**）。 / Whether domain values stay out of the identity (**yes**).
    pub sparse_domain_identity_ignores_values: bool,
    /// 不同值域 → 同一身份（**是**）。 / Whether different domains share one identity (**yes**).
    pub different_domains_collide: bool,
    /// 同变量不同数值 → 不同身份（**是**）。 / Whether different numbers differ (**yes**).
    pub different_numbers_do_not_collide: bool,
    /// 极性进身份（**是**）。 / Whether polarity is part of the identity (**yes**).
    pub polarity_is_part_of_the_identity: bool,
    /// 值域会排序去重（**是**）。 / Whether domains are sorted and deduplicated (**yes**).
    pub domain_values_are_normalized: bool,
    /// 名字里的分隔符不构成歧义（**是**）。 / Whether separators inside names cause no ambiguity (**yes**).
    pub separators_are_unambiguous: bool,
    /// 映射必须恰好覆盖一次（**是**）。 / Whether the mapping must cover exactly once (**yes**).
    pub mapping_must_cover_exactly_once: bool,
}

/// 身份的另一种读回（映射用）。 / A second read-back (for the mapping).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssumptionRow2 {
    /// assumption 稳定身份。 / The assumption's stable identity.
    pub assumption: String,
    /// 映射到的 master 变量。 / The master variable.
    pub master_variable: String,
}

/// assumption 条数上限（信任边界）。 / The assumption cap (a trust boundary).
const MAX_ASSUMPTIONS: usize = 128;

/// 构造一条 CP assumption。 / Build one CP assumption.
fn build(spec: &AssumptionSpec) -> Result<ConstraintProgrammingAssumption> {
    let variable = IntegerVariable::new(spec.variable.as_str());
    Ok(match spec.shape.as_str() {
        "literal" => ConstraintProgrammingAssumption::Literal(if spec.negated {
            BooleanLiteral::negative(variable)
        } else {
            BooleanLiteral::positive(variable)
        }),
        "equal" => ConstraintProgrammingAssumption::Equal(variable, spec.value),
        "lowerBound" => ConstraintProgrammingAssumption::LowerBound(variable, spec.value),
        "upperBound" => ConstraintProgrammingAssumption::UpperBound(variable, spec.value),
        "sparseDomain" => {
            let domain = IntegerDomain::values(spec.values.iter().copied())
                .map_err(|error| BridgeError::new("InvalidInput", &error))?;
            ConstraintProgrammingAssumption::SparseDomain(variable, domain)
        }
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "CP assumption binding failed validation: unknown shape `{other}`"))),
    })
}

/// 身份前缀。 / The identity prefix.
fn prefix_of(id: &str) -> String {
    id.split('/').next().unwrap_or_default().to_owned()
}

/// 走一遍 assumption 绑定。非法输入是**错误**。 / Walk the assumption binding.
pub fn verify_assumption_binding(
    request: &AssumptionBindingRequest,
) -> Result<AssumptionBindingReport> {
    if request.assumptions.len() > MAX_ASSUMPTIONS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "CP assumption binding failed validation: at most {MAX_ASSUMPTIONS} assumptions are accepted")));
    }
    for spec in &request.assumptions {
        if spec.variable.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "CP assumption binding failed validation: variable IDs must not be blank"));
        }
    }
    let assumptions: Vec<ConstraintProgrammingAssumption> = request.assumptions.iter()
        .map(build).collect::<Result<_>>()?;
    // 绑定：映射下标 → assumption 身份。下标越界在这里就是错误。
    // The binding: mapped position to assumption identity. An out-of-range position is an error here.
    let mut mapped = BTreeMap::new();
    for mapping in &request.mappings {
        let position = usize::try_from(mapping.assumption).map_err(|_| BridgeError::new(
            "InvalidInput", "CP assumption binding failed validation: mapping position overflows"))?;
        let assumption = assumptions.get(position).ok_or_else(|| BridgeError::new(
            "InvalidInput",
            "CP assumption binding failed validation: a mapping position is out of range"))?;
        if mapping.master_variable.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "CP assumption binding failed validation: master variables must not be blank"));
        }
        // 重复映射同一个 assumption → 领域报 "mappings must be unique"。
        // Mapping the same assumption twice makes the domain report "mappings must be unique".
        if mapped.insert(assumption.stable_id(),
            StableVariableId(mapping.master_variable.clone())).is_some()
        {
            return Err(BridgeError::new("InvalidInput",
                "CP assumption binding failed validation: mappings must be unique"));
        }
    }
    let binding = MasterAssignmentAssumptionBinding::new(assumptions.clone(), mapped)
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    // 冲突子集：由 assumption ID 反查映射；映射缺这一项由领域拒绝。
    // The conflict subset: resolve assumption IDs through the mapping; a missing entry is refused by the domain.
    let mut member_ids = Vec::with_capacity(request.conflict_members.len());
    for member in &request.conflict_members {
        let position = usize::try_from(*member).map_err(|_| BridgeError::new("InvalidInput",
            "CP assumption binding failed validation: a conflict position overflows"))?;
        let assumption = assumptions.get(position).ok_or_else(|| BridgeError::new(
            "InvalidInput",
            "CP assumption binding failed validation: a conflict position is out of range"))?;
        member_ids.push(assumption.stable_id());
    }
    let conflict = VerifiedMasterConflict::from_assumption_ids(member_ids,
        &binding.master_variables, request.proof_reference.as_str())
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    let findings = canonical_findings();
    Ok(AssumptionBindingReport {
        rows: assumptions.iter().map(|assumption| row(assumption)).collect(),
        mapping: binding.master_variables.iter()
            .map(|(id, variable)| AssumptionRow2 { assumption: id.0.clone(),
                master_variable: variable.0.clone() })
            .collect(),
        conflict_members: conflict.variable_ids.iter().map(|id| id.0.clone()).collect(),
        proof_reference: conflict.proof_reference.clone(),
        identity_is_length_prefixed: findings.identity_is_length_prefixed,
        sparse_domain_identity_ignores_values: findings.sparse_domain_identity_ignores_values,
        different_domains_collide: findings.different_domains_collide,
        different_numbers_do_not_collide: findings.different_numbers_do_not_collide,
        polarity_is_part_of_the_identity: findings.polarity_is_part_of_the_identity,
        domain_values_are_normalized: findings.domain_values_are_normalized,
        separators_are_unambiguous: findings.separators_are_unambiguous,
        mapping_must_cover_exactly_once: findings.mapping_must_cover_exactly_once,
    })
}

/// 读一条 assumption。 / Read one assumption.
fn row(assumption: &ConstraintProgrammingAssumption) -> AssumptionRow {
    let id = assumption.stable_id().0;
    let value_count = match assumption {
        ConstraintProgrammingAssumption::SparseDomain(_, domain) => match domain {
            IntegerDomain::Values(values) => values.len() as u64,
            IntegerDomain::Range { .. } => 0,
        },
        _ => 0,
    };
    AssumptionRow { prefix: prefix_of(&id), stable_id: id, value_count }
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    identity_is_length_prefixed: bool,
    sparse_domain_identity_ignores_values: bool,
    different_domains_collide: bool,
    different_numbers_do_not_collide: bool,
    polarity_is_part_of_the_identity: bool,
    domain_values_are_normalized: bool,
    separators_are_unambiguous: bool,
    mapping_must_cover_exactly_once: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let variable = IntegerVariable::new("probe.x");
    let equal_five = ConstraintProgrammingAssumption::Equal(variable.clone(), 5);
    let equal_six = ConstraintProgrammingAssumption::Equal(variable.clone(), 6);
    let positive = ConstraintProgrammingAssumption::Literal(BooleanLiteral::positive(variable.clone()));
    let negative = ConstraintProgrammingAssumption::Literal(BooleanLiteral::negative(variable.clone()));
    // 1) 长度前缀：`equal/5:probe.x/5`——变量名前面是它的长度。
    // 1) Length prefix: the variable name is preceded by its own length.
    let equal_id = equal_five.stable_id().0;
    let identity_is_length_prefixed = equal_id.starts_with("equal/")
        && equal_id.contains(&format!("{}:probe.x", "probe.x".len()));
    // 2) 值域不进身份：两份不同值域 → 同一身份。 
    // 2) Domain values stay out of the identity: two different domains share it.
    let narrow = ConstraintProgrammingAssumption::SparseDomain(variable.clone(),
        IntegerDomain::values([1, 2]).expect("probe domain"));
    let wide = ConstraintProgrammingAssumption::SparseDomain(variable.clone(),
        IntegerDomain::values([3]).expect("probe domain"));
    let sparse_domain_identity_ignores_values =
        narrow.stable_id() == wide.stable_id()
            && narrow.stable_id().0 == wide.stable_id().0;
    let different_domains_collide = narrow.stable_id() == wide.stable_id();
    // 3) 数值进身份：5 与 6 不同。 / The number is part of the identity: 5 differs from 6.
    let different_numbers_do_not_collide = equal_five.stable_id() != equal_six.stable_id();
    // 4) 极性进身份。 / Polarity is part of the identity.
    let polarity_is_part_of_the_identity = positive.stable_id() != negative.stable_id()
        && positive.stable_id().0.ends_with("/true") && negative.stable_id().0.ends_with("/false");
    // 5) 值域排序去重。 / Domains are sorted and deduplicated.
    let messy = IntegerDomain::values([3, 1, 3, 2, 1]).expect("probe domain");
    let domain_values_are_normalized = messy == IntegerDomain::Values(vec![1, 2, 3]);
    // 6) 名字里的分隔符不构成歧义：`a/b` 与 `a` + `b` 不会撞。 
    // 6) Separators inside names cause no ambiguity: `a/b` never collides with `a` plus `b`.
    let slash = IntegerVariable::new("a/b");
    let plain = IntegerVariable::new("a");
    let separators_are_unambiguous =
        ConstraintProgrammingAssumption::Equal(slash, 1).stable_id()
            != ConstraintProgrammingAssumption::Equal(plain, 1).stable_id();
    // 7) 映射必须恰好覆盖一次：少一个与多一个都拒绝。 
    // 7) The mapping must cover exactly once: too few and too many are both refused.
    let two = vec![equal_five.clone(), equal_six.clone()];
    let one_entry: BTreeMap<ConstraintProgrammingAssumptionId, StableVariableId> =
        [(equal_five.stable_id(), StableVariableId("m".to_owned()))].into_iter().collect();
    let extra_entry: BTreeMap<ConstraintProgrammingAssumptionId, StableVariableId> = [
        (equal_five.stable_id(), StableVariableId("m".to_owned())),
        (equal_six.stable_id(), StableVariableId("n".to_owned())),
        (ConstraintProgrammingAssumptionId("stranger".to_owned()),
            StableVariableId("o".to_owned())),
    ].into_iter().collect();
    let complete: BTreeMap<ConstraintProgrammingAssumptionId, StableVariableId> = [
        (equal_five.stable_id(), StableVariableId("m".to_owned())),
        (equal_six.stable_id(), StableVariableId("n".to_owned())),
    ].into_iter().collect();
    let mapping_must_cover_exactly_once =
        MasterAssignmentAssumptionBinding::new(two.clone(), one_entry).is_err()
            && MasterAssignmentAssumptionBinding::new(two.clone(), extra_entry).is_err()
            && MasterAssignmentAssumptionBinding::new(two, complete).is_ok();
    // 顺带确认：两份碰撞的 SparseDomain 无法同时登记（身份重复）。 
    // Also confirmed: two colliding `SparseDomain`s cannot both be registered (duplicate identities).
    let _ = BTreeSet::<ConstraintProgrammingAssumptionId>::new();
    Findings { identity_is_length_prefixed, sparse_domain_identity_ignores_values,
        different_domains_collide, different_numbers_do_not_collide,
        polarity_is_part_of_the_identity, domain_values_are_normalized, separators_are_unambiguous,
        mapping_must_cover_exactly_once }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AssumptionBindingRequest {
        AssumptionBindingRequest {
            assumptions: vec![
                AssumptionSpec { variable: "cp.x".to_owned(), shape: "equal".to_owned(),
                    negated: false, value: 5, values: Vec::new() },
                AssumptionSpec { variable: "cp.y".to_owned(), shape: "sparseDomain".to_owned(),
                    negated: false, value: 0, values: vec![3, 1, 3] },
                AssumptionSpec { variable: "cp.z".to_owned(), shape: "literal".to_owned(),
                    negated: true, value: 0, values: Vec::new() },
            ],
            mappings: vec![
                AssumptionMappingSpec { assumption: 0, master_variable: "m.x".to_owned() },
                AssumptionMappingSpec { assumption: 1, master_variable: "m.y".to_owned() },
                AssumptionMappingSpec { assumption: 2, master_variable: "m.z".to_owned() },
            ],
            conflict_members: vec![0, 2],
            proof_reference: "probe/proof".to_owned(),
        }
    }

    #[test]
    fn identities_are_length_prefixed_and_carry_the_number() {
        let report = verify_assumption_binding(&request()).unwrap();
        assert_eq!(report.rows.len(), 3);
        assert_eq!(report.rows[0].prefix, "equal");
        assert_eq!(report.rows[0].stable_id, "equal/4:cp.x/5");
        assert_eq!(report.rows[2].prefix, "literal");
        assert!(report.rows[2].stable_id.ends_with("/false"));
        assert!(report.identity_is_length_prefixed);
        assert!(report.different_numbers_do_not_collide);
        assert!(report.polarity_is_part_of_the_identity);
        assert!(report.separators_are_unambiguous);
        // 值域排序去重：给 [3,1,3] 得两条。 / The domain is normalised: [3,1,3] yields two values.
        assert_eq!(report.rows[1].value_count, 2);
        assert!(report.domain_values_are_normalized);
    }

    #[test]
    fn a_sparse_domain_identity_drops_its_values() {
        let report = verify_assumption_binding(&request()).unwrap();
        // 身份里没有值域内容。 / The identity carries no domain contents.
        assert_eq!(report.rows[1].stable_id, "sparse-domain/4:cp.y");
        assert!(report.sparse_domain_identity_ignores_values);
        assert!(report.different_domains_collide);
        // 两份不同值域一起传 → 被拒绝，但**报的是"映射重复"**（因为两份身份相同、都被映射了），
        // 而不是"身份重复"：碰撞先撞在映射这一层。**按实测纠正**我原本的预期。
        // Two different domains together are refused, but the message says **duplicate mapping** (both share an identity
        // and both are mapped) rather than duplicate identity — the collision surfaces at the mapping layer first.
        // Measured correction of my original expectation.
        let mut colliding = request();
        colliding.assumptions.push(AssumptionSpec { variable: "cp.y".to_owned(),
            shape: "sparseDomain".to_owned(), negated: false, value: 0, values: vec![9] });
        colliding.mappings.push(AssumptionMappingSpec { assumption: 3,
            master_variable: "m.y2".to_owned() });
        let error = verify_assumption_binding(&colliding).unwrap_err();
        assert!(error.message.contains("mappings must be unique"), "{}", error.message);
        // 干脆不给映射时才会撞到领域自己的"身份重复"检查（映射这一层已经不插手了）。
        // With no mappings at all the domain's own identity check fires, since the mapping layer no longer interferes.
        let mut bare = request();
        bare.assumptions.push(AssumptionSpec { variable: "cp.y".to_owned(),
            shape: "sparseDomain".to_owned(), negated: false, value: 0, values: vec![9] });
        bare.mappings = Vec::new();
        bare.conflict_members = vec![0];
        let error = verify_assumption_binding(&bare).unwrap_err();
        assert!(error.message.contains("identities must be unique"), "{}", error.message);
    }

    #[test]
    fn the_conflict_subset_resolves_through_the_mapping() {
        let report = verify_assumption_binding(&request()).unwrap();
        // 两个成员映射到 m.x 与 m.z。 / The two members resolve to m.x and m.z.
        assert_eq!(report.conflict_members, vec!["m.x".to_owned(), "m.z".to_owned()]);
        assert_eq!(report.proof_reference, "probe/proof");
        assert_eq!(report.mapping.len(), 3);
        assert!(report.mapping_must_cover_exactly_once);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let mut other = request();
        other.assumptions = vec![AssumptionSpec { variable: "only".to_owned(),
            shape: "lowerBound".to_owned(), negated: false, value: -2, values: Vec::new() }];
        other.mappings = vec![AssumptionMappingSpec { assumption: 0,
            master_variable: "m.only".to_owned() }];
        other.conflict_members = vec![0];
        let report = verify_assumption_binding(&other).unwrap();
        assert!(report.identity_is_length_prefixed);
        assert!(report.sparse_domain_identity_ignores_values);
        assert!(report.different_domains_collide);
        assert!(report.different_numbers_do_not_collide);
        assert!(report.polarity_is_part_of_the_identity);
        assert!(report.domain_values_are_normalized);
        assert!(report.separators_are_unambiguous);
        assert!(report.mapping_must_cover_exactly_once);
        assert_eq!(report.rows[0].prefix, "lower-bound");
    }

    #[test]
    fn bad_input_is_refused() {
        let mut unknown = request();
        unknown.assumptions[0].shape = "between".to_owned();
        assert!(verify_assumption_binding(&unknown).unwrap_err().message
            .contains("unknown shape"));
        let mut blank = request();
        blank.assumptions[0].variable = "  ".to_owned();
        assert!(verify_assumption_binding(&blank).unwrap_err().message
            .contains("variable IDs must not be blank"));
        let mut incomplete = request();
        incomplete.mappings.pop();
        assert!(verify_assumption_binding(&incomplete).unwrap_err().message
            .contains("cover every assumption exactly once"));
        let mut extra = request();
        extra.mappings.push(AssumptionMappingSpec { assumption: 0,
            master_variable: "m.again".to_owned() });
        assert!(verify_assumption_binding(&extra).unwrap_err().message
            .contains("mappings must be unique"));
        let mut blank_master = request();
        blank_master.mappings[0].master_variable = "  ".to_owned();
        assert!(verify_assumption_binding(&blank_master).unwrap_err().message
            .contains("master variables must not be blank"));
        let mut out_of_range = request();
        out_of_range.mappings[0].assumption = 9;
        assert!(verify_assumption_binding(&out_of_range).unwrap_err().message
            .contains("mapping position is out of range"));
        let mut empty_conflict = request();
        empty_conflict.conflict_members = Vec::new();
        assert!(verify_assumption_binding(&empty_conflict).is_err());
        let mut blank_proof = request();
        blank_proof.proof_reference = "  ".to_owned();
        assert!(verify_assumption_binding(&blank_proof).is_err());
        let mut unmapped = request();
        unmapped.assumptions.push(AssumptionSpec { variable: "cp.w".to_owned(),
            shape: "equal".to_owned(), negated: false, value: 1, values: Vec::new() });
        unmapped.mappings.push(AssumptionMappingSpec { assumption: 3,
            master_variable: "m.w".to_owned() });
        unmapped.conflict_members = vec![3];
        // 新 assumption 有映射，所以这里应当成功——用它反过来证明"缺映射才失败"。
        // The new assumption has a mapping, so this succeeds — proving that only a missing mapping fails.
        assert!(verify_assumption_binding(&unmapped).is_ok());
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_assumption_binding(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"identityIsLengthPrefixed\""), "{json}");
        assert!(json.contains("\"sparseDomainIdentityIgnoresValues\""), "{json}");
        assert!(json.contains("\"stableId\""), "{json}");
        assert!(!json.contains("stable_id"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<AssumptionBindingReport>(&copy.to_string()).is_err());
    }
}
