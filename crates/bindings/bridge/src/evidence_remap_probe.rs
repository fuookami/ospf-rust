//! 续挖第 80 单：后端证据的**回映**——直接成员优先、assumption 路径靠**共享身份**才能命中、
//! `ObjectiveTarget` 与 `Interval` 两条永远匹配不上（`analysis/activation.rs` 的 `remap_backend_evidence` /
//! `source_of_assumption` 与 `analysis/mod.rs` 的 `DiagnosticSource::as_infeasibility_member`）
//! Continuation, delivery 80: the **remapping of backend evidence** — direct members first, the assumption path only
//! hitting through a **shared identity**, and two kinds that can never match
//! (`remap_backend_evidence` / `source_of_assumption` in `analysis/activation.rs` together with
//! `DiagnosticSource::as_infeasibility_member` in `analysis/mod.rs`).
//!
//! 第 78 单量过激活集合自己的账；这一单量**它怎么把后端证据翻译回原始证据**。八条实测：
//!
//! 1. **`ObjectiveTarget` 没有证据成员**（`objective_targets_have_no_evidence_member()`）：
//!    `as_infeasibility_member` 对它返回 `None`，所以它永远回映不了。
//! 2. **`Interval` 成员永远匹配不上**（`the_interval_member_can_never_match()`）：枚举里有这一型，
//!    但没有任何 `DiagnosticSource` 映射到它。
//! 3. **成员丢掉稀疏域的内容**（`as_member_drops_the_sparse_domain_content()`）：`SparseDomain` 只映射成
//!    `Domain(变量名)`，值域内容不参与匹配。
//! 4. **直接成员无需走 assumption 路径**（`direct_members_match_without_the_assumption_path()`）。
//! 5. **assumption 路径靠共享身份命中 `VariableBound`**（`the_assumption_path_reaches_bound_variants()`）：
//!    `source_of_assumption` 从不返回 `VariableBound`，但由于第 78 单那条"两种变体共享一个稳定身份"，
//!    `VariableBound{x, Lower}` 仍能被 `Assumption(lower-bound/...)` 回映。本单最值钱的一条。
//! 6. **未知成员被丢弃**（`unknown_members_are_dropped()`）。
//! 7. **停用记录不参与回映**（`deactivated_sources_are_never_remapped()`）。
//! 8. **共享身份的两条记录不合并**（`shared_identity_records_are_not_deduped()`）：同一个有序列表里出现两次，
//!    回映结果里也出现两次。
//!
//! Delivery 78 measured the activation set's own bookkeeping; this one measures **how it translates backend evidence
//! back into original evidence**. **`ObjectiveTarget` has no evidence member** — `as_infeasibility_member` returns `None`,
//! so it can never be remapped. **The `Interval` member can never match** — the variant exists in the enum yet no
//! `DiagnosticSource` maps to it. **The member drops the sparse-domain content** (`Domain(name)` only). **Direct members
//! match without the assumption path.** **The assumption path reaches `VariableBound` through a shared identity** —
//! `source_of_assumption` never returns a `VariableBound`, yet delivery 78's shared stable identity lets an
//! `Assumption(lower-bound/...)` remap a `VariableBound{x, Lower}`. **Unknown members are dropped.** **Deactivated records
//! never take part.** And **two records with one identity are not deduped**.

use std::collections::BTreeSet;

use ospf_rust_core::analysis::{
    BoundSide, DiagnosticActivation, DiagnosticActivationSet, DiagnosticSource,
    ObjectiveId, ObjectiveTarget,
};
use ospf_rust_core::model::constraint_programming::{
    ConstraintDefinition, ConstraintProgrammingConstraint, ConstraintProgrammingModel,
    ConstraintProgrammingSnapshot, IntegerDomain, IntegerExpression, IntegerRelation,
    IntegerVariable,
};
use ospf_rust_core::solver::{
    InfeasibilityEvidenceMember, StableConstraintId, StableVariableId,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条后端证据成员的声明。 / One backend-evidence member declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceMemberSpec {
    /// 成员种类：`constraint` / `lowerBound` / `upperBound` / `domain` / `interval` / `assumption`。
    /// The member kind.
    pub kind: String,
    /// 成员值（ID 或 assumption 身份）。 / The member value (an ID or an assumption identity).
    pub value: String,
}

/// 一个额外激活来源的声明。 / One extra active-source declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceSourceSpec {
    /// 来源种类：`constraint` / `lowerBound` / `upperBound` / `variableBound` / `sparseDomain` /
    /// `objectiveTarget`。 / The source kind.
    pub kind: String,
    /// 稳定 ID。 / The stable ID.
    #[serde(default)]
    pub id: String,
    /// `variableBound` 的方向：`lower` / `upper`。 / The side for `variableBound`.
    #[serde(default)]
    pub side: Option<String>,
}

/// 证据回映探针请求。 / The evidence-remap probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRemapRequest {
    /// 建快照用的变量（`values` 非空即稀疏域）。 / The snapshot variables (non-empty `values` means sparse).
    #[serde(default)]
    pub variables: Vec<EvidenceSourceVariableSpec>,
    /// 是否给快照加一条约束（默认加）。 / Whether the snapshot gets one constraint (default yes).
    #[serde(default)]
    pub with_constraint: Option<bool>,
    /// 是否从快照起手（默认是）。 / Whether to start from the snapshot (default yes).
    #[serde(default)]
    pub from_snapshot: Option<bool>,
    /// 额外激活的来源。 / Extra active sources.
    #[serde(default)]
    pub extra_sources: Vec<EvidenceSourceSpec>,
    /// 额外**停用**的来源。 / Extra **deactivated** sources.
    #[serde(default)]
    pub deactivated_sources: Vec<EvidenceSourceSpec>,
    /// 后端返回的证据成员。 / The backend-reported evidence members.
    #[serde(default)]
    pub members: Vec<EvidenceMemberSpec>,
}

/// 一个变量的声明。 / One variable declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceSourceVariableSpec {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 稀疏取值（非空即稀疏域）。 / The sparse values (non-empty means sparse).
    #[serde(default)]
    pub values: Vec<i64>,
    /// 区间上界（默认 5）。 / The range upper bound (default 5).
    #[serde(default)]
    pub upper: Option<i64>,
}

/// 一个激活来源的读面。 / One active source's readable face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceSourceView {
    /// 来源种类。 / The source kind.
    pub kind: String,
    /// 来源稳定身份。 / The source's stable identity.
    pub stable_id: String,
    /// 映射到的证据成员（`kind:value`；`None` 表示这一型没有成员）。
    /// The mapped evidence member as `kind:value`; absent means this kind has no member.
    #[serde(default)]
    pub member: Option<String>,
    /// 是否被后端证据回映上。 / Whether the backend evidence remaps it.
    pub remapped: bool,
}

/// 证据回映探针结论。 / The evidence-remap probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRemapReport {
    /// 每个激活来源的读面（按列表顺序）。 / Each active source's readable face, in list order.
    pub sources: Vec<EvidenceSourceView>,
    /// `to_assumptions` 产出的身份（按顺序）。 / The identities produced by `to_assumptions`.
    pub assumptions: Vec<String>,
    /// 回映回来的原始证据身份（按顺序，**可能重复**）。 / The remapped identities, in order and **possibly repeated**.
    pub remapped: Vec<String>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub objective_targets_have_no_evidence_member: bool,
    pub the_interval_member_can_never_match: bool,
    pub as_member_drops_the_sparse_domain_content: bool,
    pub direct_members_match_without_the_assumption_path: bool,
    pub the_assumption_path_reaches_bound_variants: bool,
    pub unknown_members_are_dropped: bool,
    pub deactivated_sources_are_never_remapped: bool,
    pub shared_identity_records_are_not_deduped: bool,
}

/// 造快照。 / Build the snapshot.
fn snapshot_of(request: &EvidenceRemapRequest) -> Result<ConstraintProgrammingSnapshot> {
    let mut model = ConstraintProgrammingModel::new("evidence-remap");
    let mut first = None;
    for spec in &request.variables {
        if spec.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "evidence remap failed validation: variable IDs must not be blank"));
        }
        let domain = if spec.values.is_empty() {
            IntegerDomain::range(0, spec.upper.unwrap_or(5))
        } else {
            IntegerDomain::values(spec.values.iter().copied())
        }
        .map_err(|error| BridgeError::new("InvalidInput", &format!(
            "evidence remap failed validation: {error}")))?;
        let variable = IntegerVariable::new(spec.id.as_str());
        model.register_variable(variable.clone(), domain).map_err(|error| BridgeError::new(
            "InvalidInput", &format!("evidence remap failed validation: {error}")))?;
        first.get_or_insert(variable);
    }
    if request.with_constraint.unwrap_or(true) {
        let expression = match &first {
            Some(variable) => IntegerExpression::variable(variable.clone()),
            None => IntegerExpression::constant(0),
        };
        model.add_constraint(ConstraintDefinition::new("c0",
            ConstraintProgrammingConstraint::integer(expression, IntegerRelation::LessOrEqual, 5)))
            .map_err(|error| BridgeError::new("InvalidInput", &format!(
                "evidence remap failed validation: {error}")))?;
    }
    model.freeze().map_err(|error| BridgeError::new("InvalidInput", &format!(
        "evidence remap failed validation: {error}")))
}

/// 造一个变量边界引用。 / Build a variable-bound reference.
fn bound_ref(variable: &str, side: Option<&str>) -> Result<ospf_rust_core::analysis::VariableBoundRef> {
    let side = match side {
        Some("lower") | None => BoundSide::Lower,
        Some("upper") => BoundSide::Upper,
        Some(other) => return Err(BridgeError::new("InvalidInput", &format!(
            "evidence remap failed validation: unknown bound side `{other}`"))),
    };
    Ok(ospf_rust_core::analysis::VariableBoundRef {
        variable_id: StableVariableId(variable.to_owned()), side })
}

/// 造一个来源。 / Build a source.
fn source_of(spec: &EvidenceSourceSpec) -> Result<DiagnosticSource> {
    Ok(match spec.kind.as_str() {
        "constraint" => DiagnosticSource::Constraint { id: StableConstraintId(spec.id.clone()) },
        "lowerBound" => DiagnosticSource::VariableLowerBound {
            variable_id: StableVariableId(spec.id.clone()) },
        "upperBound" => DiagnosticSource::VariableUpperBound {
            variable_id: StableVariableId(spec.id.clone()) },
        "variableBound" => DiagnosticSource::VariableBound {
            bound: bound_ref(&spec.id, spec.side.as_deref())? },
        "sparseDomain" => DiagnosticSource::SparseDomain {
            variable_id: StableVariableId(spec.id.clone()) },
        "objectiveTarget" => DiagnosticSource::ObjectiveTarget {
            target: ObjectiveTarget::AtLeast { objective_id: ObjectiveId::new(spec.id.clone()),
                value: 1.0 } },
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "evidence remap failed validation: unknown source kind `{other}`"))),
    })
}

/// 造一个证据成员。 / Build an evidence member.
fn member_of(spec: &EvidenceMemberSpec) -> Result<InfeasibilityEvidenceMember> {
    Ok(match spec.kind.as_str() {
        "constraint" => InfeasibilityEvidenceMember::Constraint(spec.value.clone()),
        "lowerBound" => InfeasibilityEvidenceMember::LowerBound(spec.value.clone()),
        "upperBound" => InfeasibilityEvidenceMember::UpperBound(spec.value.clone()),
        "domain" => InfeasibilityEvidenceMember::Domain(spec.value.clone()),
        "interval" => InfeasibilityEvidenceMember::Interval(spec.value.clone()),
        "assumption" => InfeasibilityEvidenceMember::Assumption(spec.value.clone()),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "evidence remap failed validation: unknown member kind `{other}`"))),
    })
}

/// 把成员渲染成 `kind:value`。 / Render a member as `kind:value`.
fn member_text(member: &InfeasibilityEvidenceMember) -> String {
    match member {
        InfeasibilityEvidenceMember::Constraint(value) => format!("constraint:{value}"),
        InfeasibilityEvidenceMember::LowerBound(value) => format!("lowerBound:{value}"),
        InfeasibilityEvidenceMember::UpperBound(value) => format!("upperBound:{value}"),
        InfeasibilityEvidenceMember::Domain(value) => format!("domain:{value}"),
        InfeasibilityEvidenceMember::Interval(value) => format!("interval:{value}"),
        InfeasibilityEvidenceMember::Assumption(value) => format!("assumption:{value}"),
    }
}

/// 起手集合。 / The starting set.
fn base_set(request: &EvidenceRemapRequest,
    snapshot: &ConstraintProgrammingSnapshot) -> Result<DiagnosticActivationSet> {
    let mut set = if request.from_snapshot.unwrap_or(true) {
        DiagnosticActivationSet::from_snapshot(snapshot)
    } else {
        DiagnosticActivationSet::new()
    };
    for spec in &request.extra_sources {
        set.insert(DiagnosticActivation::activate(source_of(spec)?));
    }
    for spec in &request.deactivated_sources {
        set.insert(DiagnosticActivation::deactivate(source_of(spec)?));
    }
    Ok(set)
}

/// 后端证据集合。 / The backend evidence set.
fn members_of(request: &EvidenceRemapRequest) -> Result<BTreeSet<InfeasibilityEvidenceMember>> {
    let mut members = BTreeSet::new();
    for spec in &request.members {
        if spec.value.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "evidence remap failed validation: member values must not be blank"));
        }
        members.insert(member_of(spec)?);
    }
    Ok(members)
}

/// 只做回映，不进公开入口（内部探针用）。 / Remap only, without the public entry (for internal probes).
fn remap_only(snapshot: &ConstraintProgrammingSnapshot, set: &DiagnosticActivationSet,
    members: &BTreeSet<InfeasibilityEvidenceMember>) -> Vec<String> {
    set.remap_backend_evidence(snapshot, members).into_iter()
        .map(|source| source.stable_id()).collect()
}

/// 走一遍证据回映探针。非法输入是**错误**。 / Walk the evidence-remap probe.
pub fn verify_evidence_remap(request: &EvidenceRemapRequest) -> Result<EvidenceRemapReport> {
    if request.members.len() > 32 {
        return Err(BridgeError::new("InvalidInput",
            "evidence remap failed validation: at most 32 members are accepted"));
    }
    let findings = canonical_findings();
    let snapshot = snapshot_of(request)?;
    let set = base_set(request, &snapshot)?;
    let members = members_of(request)?;
    let remapped = remap_only(&snapshot, &set, &members);
    let sources = set.active_sources().into_iter().map(|source| {
        let stable_id = source.stable_id();
        EvidenceSourceView {
            kind: source.kind().to_owned(),
            member: source.as_infeasibility_member().as_ref().map(member_text),
            remapped: remapped.contains(&stable_id),
            stable_id,
        }
    }).collect::<Vec<_>>();
    Ok(EvidenceRemapReport {
        sources,
        assumptions: set.to_assumptions(&snapshot).into_iter()
            .map(|assumption| assumption.stable_id().0).collect::<Vec<_>>(),
        remapped,
        objective_targets_have_no_evidence_member: findings.objective_targets_have_no_evidence_member,
        the_interval_member_can_never_match: findings.the_interval_member_can_never_match,
        as_member_drops_the_sparse_domain_content:
            findings.as_member_drops_the_sparse_domain_content,
        direct_members_match_without_the_assumption_path:
            findings.direct_members_match_without_the_assumption_path,
        the_assumption_path_reaches_bound_variants:
            findings.the_assumption_path_reaches_bound_variants,
        unknown_members_are_dropped: findings.unknown_members_are_dropped,
        deactivated_sources_are_never_remapped: findings.deactivated_sources_are_never_remapped,
        shared_identity_records_are_not_deduped:
            findings.shared_identity_records_are_not_deduped,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    objective_targets_have_no_evidence_member: bool,
    the_interval_member_can_never_match: bool,
    as_member_drops_the_sparse_domain_content: bool,
    direct_members_match_without_the_assumption_path: bool,
    the_assumption_path_reaches_bound_variants: bool,
    unknown_members_are_dropped: bool,
    deactivated_sources_are_never_remapped: bool,
    shared_identity_records_are_not_deduped: bool,
}

/// 造一个最小快照（`x` 区间 `[0,5]`、`y` 稀疏 `{1,4,9}`、一条约束 `c0`）。
/// Build a minimal snapshot (`x` over `[0,5]`, `y` sparse `{1,4,9}`, one constraint `c0`).
fn probe_snapshot() -> ConstraintProgrammingSnapshot {
    let mut model = ConstraintProgrammingModel::new("evidence-probe");
    let x = IntegerVariable::new("x");
    let y = IntegerVariable::new("y");
    model.register_variable(x.clone(), IntegerDomain::range(0, 5).expect("domain"))
        .expect("register");
    model.register_variable(y, IntegerDomain::values([1, 4, 9]).expect("domain"))
        .expect("register");
    model.add_constraint(ConstraintDefinition::new("c0",
        ConstraintProgrammingConstraint::integer(IntegerExpression::variable(x),
            IntegerRelation::LessOrEqual, 5))).expect("constraint");
    model.freeze().expect("freeze")
}

/// 造一个成员声明。 / Build a member declaration.
fn member(kind: &str, value: &str) -> InfeasibilityEvidenceMember {
    match kind {
        "constraint" => InfeasibilityEvidenceMember::Constraint(value.to_owned()),
        "lowerBound" => InfeasibilityEvidenceMember::LowerBound(value.to_owned()),
        "upperBound" => InfeasibilityEvidenceMember::UpperBound(value.to_owned()),
        "domain" => InfeasibilityEvidenceMember::Domain(value.to_owned()),
        _ => InfeasibilityEvidenceMember::Assumption(value.to_owned()),
    }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let snapshot = probe_snapshot();
    // 1) `ObjectiveTarget` 没有证据成员。 / `ObjectiveTarget` has no evidence member.
    let objective_targets_have_no_evidence_member = {
        let target = DiagnosticSource::ObjectiveTarget {
            target: ObjectiveTarget::AtLeast { objective_id: ObjectiveId::new("obj"), value: 1.0 } };
        // 六型里只有它是 `None`。 / It is the only `None` among the six kinds.
        target.as_infeasibility_member().is_none()
            && DiagnosticSource::Constraint { id: StableConstraintId("c0".to_owned()) }
                .as_infeasibility_member().is_some()
            && DiagnosticSource::VariableLowerBound { variable_id: StableVariableId("x".to_owned()) }
                .as_infeasibility_member().is_some()
            && DiagnosticSource::VariableUpperBound { variable_id: StableVariableId("x".to_owned()) }
                .as_infeasibility_member().is_some()
            && DiagnosticSource::VariableBound {
                bound: bound_ref("x", Some("lower")).expect("bound") }
                .as_infeasibility_member().is_some()
            && DiagnosticSource::SparseDomain { variable_id: StableVariableId("x".to_owned()) }
                .as_infeasibility_member().is_some()
    };
    // 2) `Interval` 成员永远匹配不上。 / The `Interval` member can never match.
    let the_interval_member_can_never_match = {
        let mut set = DiagnosticActivationSet::from_snapshot(&snapshot);
        set.insert(DiagnosticActivation::activate(DiagnosticSource::VariableLowerBound {
            variable_id: StableVariableId("ivA".to_owned()) }));
        // 即便成员值与变量名一致，`Interval` 也不是任何来源映射出来的型。 
        // Even with a matching value, `Interval` is not a kind any source maps to.
        let interval_only = BTreeSet::from([
            InfeasibilityEvidenceMember::Interval("ivA".to_owned())]);
        remap_only(&snapshot, &set, &interval_only).is_empty()
            // 对照：同名换成 `LowerBound` 立刻命中。 / Control: the same name as `LowerBound` hits at once.
            && !remap_only(&snapshot, &set,
                &BTreeSet::from([member("lowerBound", "ivA")])).is_empty()
    };
    // 3) 成员丢掉稀疏域内容。 / The member drops the sparse-domain content.
    let as_member_drops_the_sparse_domain_content = {
        let source = DiagnosticSource::SparseDomain {
            variable_id: StableVariableId("y".to_owned()) };
        // `Domain(y)`——值域 `{1,4,9}` 不参与。 / `Domain(y)` — the `{1,4,9}` values never take part.
        source.as_infeasibility_member() == Some(InfeasibilityEvidenceMember::Domain("y".to_owned()))
            && member_text(&source.as_infeasibility_member().expect("member")) == "domain:y"
    };
    // 4) 直接成员无需 assumption 路径。 / Direct members need no assumption path.
    let direct_members_match_without_the_assumption_path = {
        let set = DiagnosticActivationSet::from_snapshot(&snapshot);
        let direct = remap_only(&snapshot, &set,
            &BTreeSet::from([member("constraint", "c0")]));
        // 一条约束在快照里出现一次，回映一次。 / One constraint appears once in the snapshot and remaps once.
        direct == vec!["constraint:c0".to_owned()]
    };
    // 5) assumption 路径能命中 `VariableBound`。 / The assumption path reaches `VariableBound`.
    let the_assumption_path_reaches_bound_variants = {
        let mut set = DiagnosticActivationSet::new();
        // 只放 `VariableBound`（`source_of_assumption` 从不返回它）。 
        // Only `VariableBound`, which `source_of_assumption` never returns.
        set.insert(DiagnosticActivation::activate(DiagnosticSource::VariableBound {
            bound: bound_ref("x", Some("lower")).expect("bound") }));
        let assumptions = set.to_assumptions(&snapshot);
        let identity = assumptions.first().map(|assumption| assumption.stable_id().0)
            .unwrap_or_default();
        // 共享身份让这条路走得通。 / The shared identity makes this path work.
        identity.starts_with("lower-bound/")
            && remap_only(&snapshot, &set,
                &BTreeSet::from([InfeasibilityEvidenceMember::Assumption(identity.clone())]))
                == vec!["variable:x:lower".to_owned()]
            // 而直接成员是 `lowerBound:x`（另一种渲染）。 
            // A direct member would be `lowerBound:x`, a different rendering.
            && remap_only(&snapshot, &set, &BTreeSet::from([member("lowerBound", "x")]))
                == vec!["variable:x:lower".to_owned()]
    };
    // 6) 未知成员被丢弃。 / Unknown members are dropped.
    let unknown_members_are_dropped = {
        let set = DiagnosticActivationSet::from_snapshot(&snapshot);
        remap_only(&snapshot, &set,
            &BTreeSet::from([member("constraint", "ghost"), member("assumption", "ghost")]))
            .is_empty()
    };
    // 7) 停用记录不参与回映。 / Deactivated records never take part.
    let deactivated_sources_are_never_remapped = {
        let mut set = DiagnosticActivationSet::from_snapshot(&snapshot);
        set.deactivate_source(DiagnosticSource::Constraint {
            id: StableConstraintId("c0".to_owned()) }).expect("deactivate");
        remap_only(&snapshot, &set, &BTreeSet::from([member("constraint", "c0")])).is_empty()
    };
    // 8) 共享身份的两条记录不合并。 / Two records with one identity are not deduped.
    let shared_identity_records_are_not_deduped = {
        let mut set = DiagnosticActivationSet::new();
        set.insert(DiagnosticActivation::activate(DiagnosticSource::VariableLowerBound {
            variable_id: StableVariableId("x".to_owned()) }));
        set.insert(DiagnosticActivation::activate(DiagnosticSource::VariableBound {
            bound: bound_ref("x", Some("lower")).expect("bound") }));
        // 两条记录在同一份列表里，回映结果也出现两次。 
        // Both records sit in the same list and the remapped result carries two entries.
        let remapped = remap_only(&snapshot, &set, &BTreeSet::from([member("lowerBound", "x")]));
        remapped.len() == 2 && remapped.iter().all(|id| id == "variable:x:lower")
            && set.len() == 2
    };
    Findings {
        objective_targets_have_no_evidence_member,
        the_interval_member_can_never_match,
        as_member_drops_the_sparse_domain_content,
        direct_members_match_without_the_assumption_path,
        the_assumption_path_reaches_bound_variants,
        unknown_members_are_dropped,
        deactivated_sources_are_never_remapped,
        shared_identity_records_are_not_deduped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variable(id: &str) -> EvidenceSourceVariableSpec {
        EvidenceSourceVariableSpec { id: id.to_owned(), values: Vec::new(), upper: None }
    }

    fn spec(kind: &str, id: &str) -> EvidenceSourceSpec {
        EvidenceSourceSpec { kind: kind.to_owned(), id: id.to_owned(), side: None }
    }

    fn member_spec(kind: &str, value: &str) -> EvidenceMemberSpec {
        EvidenceMemberSpec { kind: kind.to_owned(), value: value.to_owned() }
    }

    fn request() -> EvidenceRemapRequest {
        EvidenceRemapRequest { variables: vec![variable("x")], with_constraint: Some(true),
            from_snapshot: Some(false), extra_sources: Vec::new(), deactivated_sources: Vec::new(),
            members: Vec::new() }
    }

    #[test]
    fn an_empty_evidence_set_remaps_nothing() {
        let report = verify_evidence_remap(&request()).unwrap();
        assert!(report.sources.is_empty());
        assert!(report.remapped.is_empty());
        assert!(report.assumptions.is_empty());
        assert!(report.objective_targets_have_no_evidence_member);
        assert!(report.the_interval_member_can_never_match);
        assert!(report.as_member_drops_the_sparse_domain_content);
        assert!(report.direct_members_match_without_the_assumption_path);
        assert!(report.the_assumption_path_reaches_bound_variants);
        assert!(report.unknown_members_are_dropped);
        assert!(report.deactivated_sources_are_never_remapped);
        assert!(report.shared_identity_records_are_not_deduped);
    }

    #[test]
    fn the_snapshot_surface_maps_every_kind_but_the_target() {
        let report = verify_evidence_remap(&EvidenceRemapRequest {
            variables: vec![variable("x"), EvidenceSourceVariableSpec { id: "y".to_owned(),
                values: vec![1, 4, 9], upper: None }], with_constraint: Some(true),
            from_snapshot: Some(true), extra_sources: vec![spec("objectiveTarget", "obj")],
            deactivated_sources: Vec::new(), members: Vec::new() }).unwrap();
        // 六条快照记录 + 一条目标。 / Six snapshot records plus one target.
        assert_eq!(report.sources.len(), 7);
        let target = report.sources.last().unwrap();
        assert_eq!(target.kind, "objective-target");
        assert_eq!(target.member, None);
        // 其余都有成员。 / Every other record has one.
        assert!(report.sources[..6].iter().all(|view| view.member.is_some()));
        let sparse = &report.sources[5];
        assert_eq!(sparse.kind, "sparse-domain");
        assert_eq!(sparse.member.as_deref(), Some("domain:y"));
        // assumption 只来自边界与稀疏域。 / Assumptions come only from bounds and the sparse domain.
        assert_eq!(report.assumptions.len(), 5);
    }

    #[test]
    fn a_direct_member_remaps_the_constraint() {
        let report = verify_evidence_remap(&EvidenceRemapRequest {
            from_snapshot: Some(true), with_constraint: Some(true),
            members: vec![member_spec("constraint", "c0")], ..request() }).unwrap();
        assert_eq!(report.remapped, vec!["constraint:c0".to_owned()]);
        assert!(report.sources.iter().find(|view| view.kind == "constraint")
            .unwrap().remapped);
    }

    #[test]
    fn the_assumption_path_reaches_a_bound_variant() {
        // 只放 `VariableBound`：`source_of_assumption` 从不返回它，但共享身份让它命中。 
        // Only `VariableBound`: `source_of_assumption` never returns it, yet the shared identity lets it hit.
        let with_assumption = verify_evidence_remap(&EvidenceRemapRequest {
            extra_sources: vec![EvidenceSourceSpec { kind: "variableBound".to_owned(),
                id: "x".to_owned(), side: Some("lower".to_owned()) }],
            members: vec![member_spec("assumption", "lower-bound/1:x/0")], ..request() }).unwrap();
        assert_eq!(with_assumption.remapped, vec!["variable:x:lower".to_owned()]);
        // 同一条来源也能被直接成员命中。 / The same source also hits through a direct member.
        let direct = verify_evidence_remap(&EvidenceRemapRequest {
            extra_sources: vec![EvidenceSourceSpec { kind: "variableBound".to_owned(),
                id: "x".to_owned(), side: Some("lower".to_owned()) }],
            members: vec![member_spec("lowerBound", "x")], ..request() }).unwrap();
        assert_eq!(direct.remapped, vec!["variable:x:lower".to_owned()]);
        // 身份拼错就不命中。 / A wrong identity misses.
        let wrong = verify_evidence_remap(&EvidenceRemapRequest {
            extra_sources: vec![EvidenceSourceSpec { kind: "variableBound".to_owned(),
                id: "x".to_owned(), side: Some("lower".to_owned()) }],
            members: vec![member_spec("assumption", "lower-bound/1:x/9")], ..request() }).unwrap();
        assert!(wrong.remapped.is_empty());
    }

    #[test]
    fn shared_identity_records_remap_twice() {
        let report = verify_evidence_remap(&EvidenceRemapRequest {
            extra_sources: vec![spec("lowerBound", "x"), EvidenceSourceSpec {
                kind: "variableBound".to_owned(), id: "x".to_owned(),
                side: Some("lower".to_owned()) }],
            members: vec![member_spec("lowerBound", "x")], ..request() }).unwrap();
        // 同一份列表里两条记录、回映结果也两条——不合并。 
        // Two records in one list and two remapped results — no dedup.
        assert_eq!(report.sources.len(), 2);
        assert_eq!(report.remapped.len(), 2);
        assert!(report.remapped.iter().all(|id| id == "variable:x:lower"));
    }

    #[test]
    fn an_interval_member_and_an_unknown_member_both_miss() {
        let report = verify_evidence_remap(&EvidenceRemapRequest {
            extra_sources: vec![spec("lowerBound", "ivA")],
            members: vec![member_spec("interval", "ivA"), member_spec("constraint", "ghost"),
                member_spec("assumption", "ghost")], ..request() }).unwrap();
        assert!(report.remapped.is_empty());
        assert!(!report.sources[0].remapped);
    }

    #[test]
    fn a_deactivated_source_never_remaps() {
        let report = verify_evidence_remap(&EvidenceRemapRequest {
            deactivated_sources: vec![spec("constraint", "c0")],
            members: vec![member_spec("constraint", "c0")], ..request() }).unwrap();
        // 名单里根本没有它——`active_sources` 先过滤状态。 
        // It is not even listed: `active_sources` filters on state first.
        assert!(report.sources.is_empty());
        assert!(report.remapped.is_empty());
    }

    #[test]
    fn a_sparse_domain_remaps_by_name_only() {
        let report = verify_evidence_remap(&EvidenceRemapRequest {
            variables: vec![EvidenceSourceVariableSpec { id: "y".to_owned(),
                values: vec![1, 4, 9], upper: None }], from_snapshot: Some(false),
            extra_sources: vec![spec("sparseDomain", "y")],
            members: vec![member_spec("domain", "y")], ..request() }).unwrap();
        assert_eq!(report.remapped, vec!["variable:y:domain".to_owned()]);
        // 加值域内容就命中不了。 / Adding the domain content misses.
        let with_content = verify_evidence_remap(&EvidenceRemapRequest {
            variables: vec![EvidenceSourceVariableSpec { id: "y".to_owned(),
                values: vec![1, 4, 9], upper: None }], from_snapshot: Some(false),
            extra_sources: vec![spec("sparseDomain", "y")],
            members: vec![member_spec("domain", "y:[1,4,9]")], ..request() }).unwrap();
        assert!(with_content.remapped.is_empty());
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_evidence_remap(&EvidenceRemapRequest {
            extra_sources: vec![spec("nope", "x")], ..request() }).unwrap_err()
            .message.contains("unknown source kind"));
        assert!(verify_evidence_remap(&EvidenceRemapRequest {
            extra_sources: vec![EvidenceSourceSpec { kind: "variableBound".to_owned(),
                id: "x".to_owned(), side: Some("middle".to_owned()) }], ..request() })
            .unwrap_err().message.contains("unknown bound side"));
        assert!(verify_evidence_remap(&EvidenceRemapRequest {
            members: vec![member_spec("nope", "x")], ..request() }).unwrap_err()
            .message.contains("unknown member kind"));
        assert!(verify_evidence_remap(&EvidenceRemapRequest {
            members: vec![member_spec("constraint", "  ")], ..request() }).unwrap_err()
            .message.contains("member values must not be blank"));
        assert!(verify_evidence_remap(&EvidenceRemapRequest {
            variables: vec![variable("  ")], ..request() }).unwrap_err()
            .message.contains("variable IDs must not be blank"));
        assert!(verify_evidence_remap(&EvidenceRemapRequest {
            members: (0..33).map(|index| member_spec("constraint", &format!("c{index}")))
                .collect(), ..request() }).unwrap_err().message.contains("at most 32 members"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_evidence_remap(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"remapped\""), "{json}");
        assert!(json.contains("\"objectiveTargetsHaveNoEvidenceMember\""), "{json}");
        assert!(json.contains("\"sharedIdentityRecordsAreNotDeduped\""), "{json}");
        assert!(!json.contains("objective_targets"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<EvidenceRemapReport>(&copy.to_string()).is_err());
    }
}
