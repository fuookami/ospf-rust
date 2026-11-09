//! 续挖第 70 单：CP 模型 builder 的**提交闸门**与指纹口径（`model/constraint_programming/model.rs` 的
//! `ConstraintProgrammingModel` + `freeze`）
//! Continuation, delivery 70: the **commit gates** of the CP model builder and what its fingerprint covers
//! (`ConstraintProgrammingModel` + `freeze` in `model/constraint_programming/model.rs`).
//!
//! 前面几十单都在读**已经冻结好的** snapshot；这一单量**冻结这一步本身**——builder 收什么、拒什么，以及
//! `freeze()` 造出的指纹到底覆盖了哪些字段。八条实测：
//!
//! 1. **`new` 自带身份**（`a_new_model_already_has_an_identity()`）：`identity_namespace = "model-local"`、
//!    `identity_schema_version = "1.0"`，所以空模型也能冻结；只有**显式置空**才会被拒。
//! 2. **`register_variable` 两道去重**（`register_variable_refuses_two_kinds_of_duplicate()`）：重复稳定 ID 与
//!    重复**本地** ID 都拒，且重复本地 ID 那条只在稳定 ID 不同时才可能触发。
//! 3. **`ConstraintDefinition::new` 把 name 预填成 id**（`a_new_constraint_names_itself_after_its_id()`），
//!    所以"名字为空"这件事在默认路径上不可能发生。
//! 4. **`set_objective` 静默覆盖**（`set_objective_silently_overwrites()`）：没有"第二个目标"这种错误，
//!    只有最后一个；`clear_objective` 之后回到 satisfaction。
//! 5. **`freeze` 重绑本地变量 ID**（`freeze_rebinds_local_ids_to_canonical_indices()`）：快照里的变量 ID 是
//!    `standalone(index)`，**注册顺序被丢弃**，只剩稳定 ID 顺序。
//! 6. **模型名不进指纹**（`the_model_name_is_not_in_the_fingerprint()`）：`compute_fingerprint` 只拼命名空间、
//!    schema 版本、变量、区间、约束、目标、组元数据——**不含 `name`**，所以只差名字的两个模型指纹相同。
//! 7. **`ensure_constraint_group` 是"同 id 不许改绑"**（`a_group_id_may_not_be_rebound()`）：空白名拒、同 id
//!    换名拒、同 id 同名与不同 id 同名都收。
//! 8. **`freeze` 才是引用完整性闸门**（`freeze_is_the_reference_integrity_gate()`）：约束引用未注册变量、
//!    目标引用未注册变量，都在冻结时被拒；`add_constraint` 自己不查引用。
//!
//! Earlier deliveries all read an **already frozen** snapshot; this one measures **the freeze itself** — what the
//! builder accepts, what it refuses, and exactly which fields the resulting fingerprint covers. **`new` already carries an
//! identity** (`model-local` / `1.0`), so an empty model freezes fine and only an explicit blank is refused.
//! **`register_variable` refuses two kinds of duplicate** (stable ID and local ID). **`ConstraintDefinition::new` prefills
//! the name with the ID.** **`set_objective` silently overwrites** — no "second objective" error, just the last one.
//! **`freeze` rebinds local IDs to canonical indices**, discarding registration order. **The model name is not in the
//! fingerprint.** **`ensure_constraint_group` refuses rebinding one ID to another name.** And **`freeze` is the reference
//! integrity gate** — a constraint or objective naming an unregistered variable is refused there, not at insertion.

use std::collections::BTreeMap;

use ospf_rust_core::model::constraint_programming::{
    ConstraintDefinition, ConstraintProgrammingConstraint, ConstraintProgrammingModel,
    ConstraintProgrammingSnapshot, IntegerDomain, IntegerExpression, IntegerObjective,
    IntegerRelation, IntegerVariable,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个变量的声明。 / One variable declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpVariableSpec {
    /// 稳定 ID。 / The stable ID.
    pub id: String,
    /// 区间下界（与 `upper` 一起用）。 / The range lower bound (used with `upper`).
    #[serde(default)]
    pub lower: Option<i64>,
    /// 区间上界。 / The range upper bound.
    #[serde(default)]
    pub upper: Option<i64>,
    /// 稀疏值域（给了就忽略 `lower`/`upper`）。 / A sparse domain (ignores `lower`/`upper`).
    #[serde(default)]
    pub values: Vec<i64>,
    /// 与另一个变量共用一个**本地** ID（量"重复本地 ID"）。 / Reuse another variable's **local** ID.
    #[serde(default)]
    pub share_local_id_with: Option<String>,
}

/// 一个约束的声明。 / One constraint declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpConstraintSpec {
    /// 稳定约束 ID。 / The stable constraint ID.
    pub id: String,
    /// 约束引用的变量 ID。 / The variable ID the constraint references.
    #[serde(default)]
    pub variable: Option<String>,
    /// 关系名（`lessOrEqual` / `greaterOrEqual` / `equal`）。 / The relation name.
    #[serde(default)]
    pub relation: Option<String>,
    /// 右端常数。 / The right-hand constant.
    #[serde(default)]
    pub rhs: Option<i64>,
    /// 组名（空表示不分组）。 / The group name (empty means ungrouped).
    #[serde(default)]
    pub group: Option<String>,
    /// 来源标签。 / The origin label.
    #[serde(default)]
    pub origin: Option<String>,
    /// 显示名（空表示不覆盖）。 / The display name (empty means no override).
    #[serde(default)]
    pub name: Option<String>,
}

/// 一个约束组声明。 / One constraint-group declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpGroupSpec {
    /// 组 ID。 / The group ID.
    pub id: u64,
    /// 组名。 / The group name.
    pub name: String,
}

/// CP 模型 builder 探针请求。 / The CP model builder probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpModelBuildRequest {
    /// 模型名。 / The model name.
    #[serde(default)]
    pub name: Option<String>,
    /// 身份命名空间（空串表示显式置空）。 / The identity namespace (an empty string blanks it).
    #[serde(default)]
    pub namespace: Option<String>,
    /// 身份 schema 版本（空串表示显式置空）。 / The identity schema version (empty blanks it).
    #[serde(default)]
    pub schema_version: Option<String>,
    /// 变量声明（按注册顺序）。 / The variable declarations, in registration order.
    #[serde(default)]
    pub variables: Vec<CpVariableSpec>,
    /// 约束声明（按注册顺序）。 / The constraint declarations, in registration order.
    #[serde(default)]
    pub constraints: Vec<CpConstraintSpec>,
    /// 目标（`maximize` / `minimize`，空表示不设）。 / The objective (`maximize`/`minimize`; empty means none).
    #[serde(default)]
    pub objective: Option<String>,
    /// 目标的稳定 ID（空表示用默认）。 / The objective's stable ID (empty means the default).
    #[serde(default)]
    pub objective_id: Option<String>,
    /// 目标引用的变量 ID。 / The variable ID the objective references.
    #[serde(default)]
    pub objective_variable: Option<String>,
    /// 是否在设目标后再清掉（量 `clear_objective`）。 / Whether to clear the objective afterwards.
    #[serde(default)]
    pub clear_objective: bool,
    /// 约束组声明。 / The constraint-group declarations.
    #[serde(default)]
    pub groups: Vec<CpGroupSpec>,
}

/// CP 模型 builder 探针结论。 / The CP model builder probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CpModelBuildReport {
    /// 是否冻结成功。 / Whether the freeze succeeded.
    pub accepted: bool,
    /// 被拒时的错误文案。 / The refusal message.
    #[serde(default)]
    pub refusal: Option<String>,
    /// 被拒发生在哪一步（`registerVariable` / `addConstraint` / `group` / `freeze`）。
    /// Which step refused (`registerVariable` / `addConstraint` / `group` / `freeze`).
    #[serde(default)]
    pub refusal_stage: Option<String>,
    /// 冻结后的模型名。 / The frozen model's name.
    #[serde(default)]
    pub name: Option<String>,
    /// 冻结后的命名空间。 / The frozen namespace.
    #[serde(default)]
    pub namespace: Option<String>,
    /// 冻结后的 schema 版本。 / The frozen schema version.
    #[serde(default)]
    pub schema_version: Option<String>,
    /// 快照指纹。 / The snapshot fingerprint.
    #[serde(default)]
    pub fingerprint: Option<String>,
    /// 变量条数与稳定 ID 顺序。 / The variable count and stable-ID order.
    pub variable_count: u32,
    pub variable_ids: Vec<String>,
    /// 快照里的**本地**变量 ID（`group:index`）。 / The snapshot's **local** variable IDs (`group:index`).
    pub variable_local_ids: Vec<String>,
    /// 每个变量的值域文字。 / Each variable's domain text.
    pub variable_domains: Vec<String>,
    /// 约束条数与稳定 ID 顺序。 / The constraint count and stable-ID order.
    pub constraint_count: u32,
    pub constraint_ids: Vec<String>,
    /// 每条约束的显示名。 / Each constraint's display name.
    pub constraint_names: Vec<String>,
    /// 目标的稳定 ID（无则空串）。 / The objective's stable ID (empty when none).
    pub objective_id: String,
    /// 目标方向（`minimum` / `maximum`，无则空串）。 / The objective direction.
    pub objective_category: String,
    /// 组元数据（`id:name` 升序）。 / The group metadata (`id:name`, ascending).
    pub groups: Vec<String>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub a_new_model_already_has_an_identity: bool,
    pub register_variable_refuses_two_kinds_of_duplicate: bool,
    pub a_new_constraint_names_itself_after_its_id: bool,
    pub set_objective_silently_overwrites: bool,
    pub freeze_rebinds_local_ids_to_canonical_indices: bool,
    pub the_model_name_is_not_in_the_fingerprint: bool,
    pub a_group_id_may_not_be_rebound: bool,
    pub freeze_is_the_reference_integrity_gate: bool,
}

/// 关系名 → 领域关系。 / A relation name into the domain relation.
fn relation_of(name: &str) -> Result<IntegerRelation> {
    Ok(match name {
        "lessOrEqual" => IntegerRelation::LessOrEqual,
        "greaterOrEqual" => IntegerRelation::GreaterOrEqual,
        "equal" => IntegerRelation::Equal,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "CP model build failed validation: unknown relation `{other}`"))),
    })
}

/// 值域文字。 / The domain text.
fn domain_text(domain: &IntegerDomain) -> String {
    match domain {
        IntegerDomain::Range { lower, upper } => format!("range[{lower},{upper}]"),
        IntegerDomain::Values(values) => format!(
            "values[{}]", values.iter().map(|value| value.to_string())
                .collect::<Vec<_>>().join(",")),
    }
}

/// 本地变量 ID 文字。 / The local variable ID text.
fn local_id_text(variable: &IntegerVariable) -> String {
    format!("{}:{}", variable.id.group_id, variable.id.index_in_group)
}

/// 冻结结果的可读面。 / The readable face of a freeze.
fn describe(snapshot: &ConstraintProgrammingSnapshot) -> CpModelBuildReport {
    let findings = canonical_findings();
    CpModelBuildReport {
        accepted: true,
        refusal: None,
        refusal_stage: None,
        name: Some(snapshot.name.clone()),
        namespace: Some(snapshot.identity_namespace.clone()),
        schema_version: Some(snapshot.identity_schema_version.clone()),
        fingerprint: Some(snapshot.fingerprint.value.clone()),
        variable_count: u32::try_from(snapshot.variables.len()).unwrap_or(u32::MAX),
        variable_ids: snapshot.variables.iter()
            .map(|entry| entry.variable.stable_id.0.clone()).collect(),
        variable_local_ids: snapshot.variables.iter()
            .map(|entry| local_id_text(&entry.variable)).collect(),
        variable_domains: snapshot.variables.iter()
            .map(|entry| domain_text(&entry.domain)).collect(),
        constraint_count: u32::try_from(snapshot.constraints.len()).unwrap_or(u32::MAX),
        constraint_ids: snapshot.constraints.iter().map(|entry| entry.id.0.clone()).collect(),
        constraint_names: snapshot.constraints.iter().map(|entry| entry.name.clone()).collect(),
        objective_id: snapshot.objective.as_ref().map(|entry| entry.id.clone())
            .unwrap_or_default(),
        objective_category: snapshot.objective.as_ref().map(|entry|
            if entry.category.is_minimum() { "minimum".to_owned() } else { "maximum".to_owned() })
            .unwrap_or_default(),
        groups: snapshot.constraint_groups.iter()
            .map(|(id, name)| format!("{id}:{name}")).collect(),
        a_new_model_already_has_an_identity: findings.a_new_model_already_has_an_identity,
        register_variable_refuses_two_kinds_of_duplicate:
            findings.register_variable_refuses_two_kinds_of_duplicate,
        a_new_constraint_names_itself_after_its_id:
            findings.a_new_constraint_names_itself_after_its_id,
        set_objective_silently_overwrites: findings.set_objective_silently_overwrites,
        freeze_rebinds_local_ids_to_canonical_indices:
            findings.freeze_rebinds_local_ids_to_canonical_indices,
        the_model_name_is_not_in_the_fingerprint: findings.the_model_name_is_not_in_the_fingerprint,
        a_group_id_may_not_be_rebound: findings.a_group_id_may_not_be_rebound,
        freeze_is_the_reference_integrity_gate: findings.freeze_is_the_reference_integrity_gate,
    }
}

/// 一份被拒的结论。 / A refused verdict.
fn refused(stage: &str, refusal: String) -> CpModelBuildReport {
    let findings = canonical_findings();
    CpModelBuildReport {
        accepted: false,
        refusal: Some(refusal),
        refusal_stage: Some(stage.to_owned()),
        name: None, namespace: None, schema_version: None, fingerprint: None,
        variable_count: 0, variable_ids: Vec::new(), variable_local_ids: Vec::new(),
        variable_domains: Vec::new(), constraint_count: 0, constraint_ids: Vec::new(),
        constraint_names: Vec::new(), objective_id: String::new(), objective_category: String::new(),
        groups: Vec::new(),
        a_new_model_already_has_an_identity: findings.a_new_model_already_has_an_identity,
        register_variable_refuses_two_kinds_of_duplicate:
            findings.register_variable_refuses_two_kinds_of_duplicate,
        a_new_constraint_names_itself_after_its_id:
            findings.a_new_constraint_names_itself_after_its_id,
        set_objective_silently_overwrites: findings.set_objective_silently_overwrites,
        freeze_rebinds_local_ids_to_canonical_indices:
            findings.freeze_rebinds_local_ids_to_canonical_indices,
        the_model_name_is_not_in_the_fingerprint: findings.the_model_name_is_not_in_the_fingerprint,
        a_group_id_may_not_be_rebound: findings.a_group_id_may_not_be_rebound,
        freeze_is_the_reference_integrity_gate: findings.freeze_is_the_reference_integrity_gate,
    }
}

/// 走一遍 CP 模型 builder 探针。非法输入是**错误**，被拒的模型是**结论**。
/// Walk the CP model builder probe: bad input is an error, a refused model is a verdict.
pub fn verify_cp_model_build(request: &CpModelBuildRequest) -> Result<CpModelBuildReport> {
    if request.variables.len() > 16 || request.constraints.len() > 16 || request.groups.len() > 16 {
        return Err(BridgeError::new("InvalidInput",
            "CP model build failed validation: at most 16 entries are accepted"));
    }
    if request.variables.iter().any(|spec| spec.id.trim().is_empty()) {
        return Err(BridgeError::new("InvalidInput",
            "CP model build failed validation: stable variable IDs must not be blank"));
    }
    if request.constraints.iter().any(|spec| spec.id.trim().is_empty()) {
        return Err(BridgeError::new("InvalidInput",
            "CP model build failed validation: stable constraint IDs must not be blank"));
    }
    let mut model = ConstraintProgrammingModel::new(
        request.name.clone().unwrap_or_else(|| "cp-model-build".to_owned()));
    if let Some(namespace) = &request.namespace {
        model = model.with_identity_metadata(namespace.clone(),
            request.schema_version.clone().unwrap_or_else(|| "1.0".to_owned()));
    } else if let Some(schema_version) = &request.schema_version {
        model = model.with_identity_metadata("model-local".to_owned(), schema_version.clone());
    }

    // 变量：本地 ID 可以显式复用（`share_local_id_with`），用来触发第二道去重。
    let mut variables = BTreeMap::<String, IntegerVariable>::new();
    for spec in &request.variables {
        let variable = match spec.share_local_id_with.as_deref() {
            Some(shared) => {
                let base = variables.get(shared).ok_or_else(|| BridgeError::new("InvalidInput",
                    &format!("CP model build failed validation: no variable `{shared}` to share")))?;
                IntegerVariable::with_ids(base.id.clone(), spec.id.clone())
            }
            None => IntegerVariable::new(spec.id.clone()),
        };
        let domain = if spec.values.is_empty() {
            IntegerDomain::range(spec.lower.unwrap_or(0), spec.upper.unwrap_or(4))
        } else {
            IntegerDomain::values(spec.values.iter().copied())
        };
        let domain = match domain {
            Ok(domain) => domain,
            Err(error) => return Ok(refused("registerVariable", error.to_string())),
        };
        if let Err(error) = model.register_variable(variable.clone(), domain) {
            return Ok(refused("registerVariable", error.to_string()));
        }
        variables.insert(spec.id.clone(), variable);
    }

    // 约束：只引用已注册变量；引用不存在的变量留到 `freeze` 去拒（量"插入不查引用"）。
    for spec in &request.constraints {
        let variable = match spec.variable.as_deref() {
            Some(id) => variables.get(id).cloned()
                .unwrap_or_else(|| IntegerVariable::new(id.to_owned())),
            None => IntegerVariable::new(spec.id.clone()),
        };
        let relation = match relation_of(spec.relation.as_deref().unwrap_or("lessOrEqual")) {
            Ok(relation) => relation,
            Err(error) => return Err(error),
        };
        let mut definition = ConstraintDefinition::new(spec.id.clone(),
            ConstraintProgrammingConstraint::integer(IntegerExpression::variable(variable),
                relation, spec.rhs.unwrap_or(0)));
        if let Some(group) = &spec.group {
            definition = definition.with_group(group.clone());
        }
        if let Some(origin) = &spec.origin {
            definition = definition.with_origin(origin.clone());
        }
        if let Some(name) = &spec.name {
            definition = definition.with_name(name.clone());
        }
        if let Err(error) = model.add_constraint(definition) {
            return Ok(refused("addConstraint", error.to_string()));
        }
    }

    // 组：同 id 同名可重复、同 id 换名被拒。
    for spec in &request.groups {
        if let Err(error) = model.ensure_constraint_group(spec.id, spec.name.clone()) {
            return Ok(refused("group", error.to_string()));
        }
    }

    if let Some(direction) = &request.objective {
        // 目标必须引用**已注册**的那个变量实例（同一稳定 ID 换个本地 ID 会被 `freeze` 拒）。
        // The objective must reference the **registered** instance; the same stable ID under a fresh local ID is
        // refused by `freeze`.
        let id = request.objective_variable.clone().or_else(|| request.variables.first()
            .map(|spec| spec.id.clone())).unwrap_or_else(|| "x".to_owned());
        let variable = variables.get(&id).cloned()
            .unwrap_or_else(|| IntegerVariable::new(id.clone()));
        let expression = IntegerExpression::variable(variable);
        let id = request.objective_id.clone().unwrap_or_default();
        let objective = match direction.as_str() {
            "maximize" => if id.is_empty() { IntegerObjective::maximize(expression) }
                else { IntegerObjective::maximize_with_id(id, expression) },
            "minimize" => if id.is_empty() { IntegerObjective::minimize(expression) }
                else { IntegerObjective::minimize_with_id(id, expression) },
            other => return Err(BridgeError::new("InvalidInput", &format!(
                "CP model build failed validation: unknown objective direction `{other}`"))),
        };
        model.set_objective(objective);
        if request.clear_objective {
            model.clear_objective();
        }
    }

    match model.freeze() {
        Ok(snapshot) => Ok(describe(&snapshot)),
        Err(error) => Ok(refused("freeze", error.to_string())),
    }
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    a_new_model_already_has_an_identity: bool,
    register_variable_refuses_two_kinds_of_duplicate: bool,
    a_new_constraint_names_itself_after_its_id: bool,
    set_objective_silently_overwrites: bool,
    freeze_rebinds_local_ids_to_canonical_indices: bool,
    the_model_name_is_not_in_the_fingerprint: bool,
    a_group_id_may_not_be_rebound: bool,
    freeze_is_the_reference_integrity_gate: bool,
}

/// 一份最小可冻结模型，连同它的 `x`。 / A minimal freezable model together with its `x`.
fn minimal_model() -> (ConstraintProgrammingModel, IntegerVariable) {
    let x = IntegerVariable::new("x");
    let mut model = ConstraintProgrammingModel::new("probe");
    model.register_variable(x.clone(), IntegerDomain::range(0, 4).expect("domain"))
        .expect("variable");
    model.add_constraint(ConstraintDefinition::new("capacity",
        ConstraintProgrammingConstraint::integer(IntegerExpression::variable(x.clone()),
            IntegerRelation::LessOrEqual, 4))).expect("constraint");
    (model, x)
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) `new` 自带身份。 / `new` already carries an identity.
    let a_new_model_already_has_an_identity = {
        let empty = ConstraintProgrammingModel::new("empty");
        let frozen = empty.freeze().is_ok();
        let blank = ConstraintProgrammingModel::new("blank")
            .with_identity_metadata("  ", "1.0").freeze().is_err();
        let blank_schema = ConstraintProgrammingModel::new("blank-schema")
            .with_identity_metadata("model-local", "  ").freeze().is_err();
        frozen && blank && blank_schema
    };
    // 2) 两道去重。 / Two kinds of duplicate.
    let register_variable_refuses_two_kinds_of_duplicate = {
        let mut model = ConstraintProgrammingModel::new("dedupe");
        let first = IntegerVariable::new("x");
        model.register_variable(first.clone(), IntegerDomain::range(0, 4).expect("domain"))
            .expect("first");
        let duplicate_stable = model.register_variable(IntegerVariable::new("x"),
            IntegerDomain::range(0, 4).expect("domain"));
        // 同一本地 ID、不同稳定 ID。 / Same local ID, different stable ID.
        let duplicate_local = model.register_variable(
            IntegerVariable::with_ids(first.id.clone(), "y"),
            IntegerDomain::range(0, 4).expect("domain"));
        duplicate_stable.is_err_and(|error| error.to_string().contains("duplicate stable variable ID"))
            && duplicate_local.is_err_and(|error| error.to_string().contains("duplicate local variable ID"))
    };
    // 3) `ConstraintDefinition::new` 用 id 预填 name。 / `new` prefills the name with the ID.
    let a_new_constraint_names_itself_after_its_id = {
        let definition = ConstraintDefinition::new("capacity",
            ConstraintProgrammingConstraint::integer(
                IntegerExpression::variable(IntegerVariable::new("x")),
                IntegerRelation::LessOrEqual, 4));
        definition.name == "capacity" && definition.name == definition.id.0
    };
    // 4) `set_objective` 静默覆盖。 / `set_objective` silently overwrites.
    let set_objective_silently_overwrites = {
        let (mut model, x) = minimal_model();
        model.set_objective(IntegerObjective::maximize_with_id("first",
            IntegerExpression::variable(x.clone())));
        model.set_objective(IntegerObjective::minimize_with_id("second",
            IntegerExpression::variable(x)));
        let last_wins = model.freeze().expect("snapshot").objective
            .is_some_and(|objective| objective.id == "second" && objective.category.is_minimum());
        let mut cleared = model;
        cleared.clear_objective();
        let clears = cleared.freeze().expect("snapshot").objective.is_none();
        last_wins && clears
    };
    // 5) `freeze` 重绑本地 ID。 / `freeze` rebinds local IDs.
    let freeze_rebinds_local_ids_to_canonical_indices = {
        // 反序注册，看快照里的本地 ID 是否按**稳定 ID 顺序**重排成 `0:0`、`1:0`。
        // Register in reverse order and see whether the local IDs become `0:0`, `1:0` in stable-ID order.
        let mut model = ConstraintProgrammingModel::new("rebind");
        for id in ["b", "a"] {
            model.register_variable(IntegerVariable::new(id),
                IntegerDomain::range(0, 4).expect("domain")).expect("variable");
        }
        let snapshot = model.freeze().expect("snapshot");
        snapshot.variables.iter().map(|entry| entry.variable.stable_id.0.clone())
            .collect::<Vec<_>>() == vec!["a".to_owned(), "b".to_owned()]
            && snapshot.variables.iter().map(|entry| local_id_text(&entry.variable))
                .collect::<Vec<_>>() == vec!["0:0".to_owned(), "1:0".to_owned()]
    };
    // 6) 模型名不进指纹。 / The model name is not in the fingerprint.
    let the_model_name_is_not_in_the_fingerprint = {
        let left = ConstraintProgrammingModel::new("alpha").freeze().expect("snapshot");
        let right = ConstraintProgrammingModel::new("beta").freeze().expect("snapshot");
        // 名字确实不同，但指纹相同。 / The names differ; the fingerprints do not.
        left.name != right.name && left.fingerprint == right.fingerprint
    };
    // 7) 组 id 不许改绑。 / A group ID may not be rebound.
    let a_group_id_may_not_be_rebound = {
        let mut model = ConstraintProgrammingModel::new("groups");
        let same_twice = model.ensure_constraint_group(1, "capacity").is_ok()
            && model.ensure_constraint_group(1, "capacity").is_ok();
        let other_id_same_name = model.ensure_constraint_group(2, "capacity").is_ok();
        let rebound = model.ensure_constraint_group(1, "renamed").is_err();
        let blank = model.ensure_constraint_group(3, "  ").is_err();
        same_twice && other_id_same_name && rebound && blank
    };
    // 8) `freeze` 是引用完整性闸门。 / `freeze` is the reference integrity gate.
    let freeze_is_the_reference_integrity_gate = {
        // 插入时不查引用：`add_constraint` 收下一个引用未注册变量的约束。
        // Insertion does not check references: `add_constraint` accepts a constraint on an unregistered variable.
        let ghost = IntegerVariable::new("ghost");
        let mut dangling = ConstraintProgrammingModel::new("dangling");
        dangling.add_constraint(ConstraintDefinition::new("dangling",
            ConstraintProgrammingConstraint::integer(
                IntegerExpression::variable(ghost.clone()),
                IntegerRelation::LessOrEqual, 1))).expect("insertion accepts it");
        let constraint_refused = dangling.freeze().is_err();
        let (mut objective, _) = minimal_model();
        objective.set_objective(IntegerObjective::maximize(
            IntegerExpression::variable(IntegerVariable::new("ghost"))));
        let objective_refused = objective.freeze().is_err();
        // 同一份模型，把变量补上就能冻结——说明拒绝确实来自引用检查。
        // Adding the missing variable makes the same model freezable, so the refusal really is the reference check.
        let mut fixed = dangling;
        fixed.register_variable(ghost, IntegerDomain::range(0, 4).expect("domain"))
            .expect("variable");
        let fixed_ok = fixed.freeze().is_ok();
        constraint_refused && objective_refused && fixed_ok
    };
    Findings {
        a_new_model_already_has_an_identity,
        register_variable_refuses_two_kinds_of_duplicate,
        a_new_constraint_names_itself_after_its_id,
        set_objective_silently_overwrites,
        freeze_rebinds_local_ids_to_canonical_indices,
        the_model_name_is_not_in_the_fingerprint,
        a_group_id_may_not_be_rebound,
        freeze_is_the_reference_integrity_gate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CpModelBuildRequest {
        CpModelBuildRequest {
            name: Some("probe".to_owned()), namespace: None, schema_version: None,
            variables: vec![CpVariableSpec { id: "x".to_owned(), lower: Some(0), upper: Some(4),
                values: Vec::new(), share_local_id_with: None }],
            constraints: vec![CpConstraintSpec { id: "capacity".to_owned(),
                variable: Some("x".to_owned()), relation: Some("lessOrEqual".to_owned()),
                rhs: Some(4), group: None, origin: None, name: None }],
            objective: None, objective_id: None, objective_variable: None,
            clear_objective: false, groups: Vec::new() }
    }

    #[test]
    fn a_minimal_model_freezes_with_its_own_identity() {
        let report = verify_cp_model_build(&request()).unwrap();
        assert!(report.accepted, "{:?}", report.refusal);
        assert_eq!(report.name.as_deref(), Some("probe"));
        // `new` 自带身份。 / `new` already carries an identity.
        assert_eq!(report.namespace.as_deref(), Some("model-local"));
        assert_eq!(report.schema_version.as_deref(), Some("1.0"));
        assert_eq!(report.variable_count, 1);
        assert_eq!(report.variable_ids, vec!["x".to_owned()]);
        assert_eq!(report.variable_local_ids, vec!["0:0".to_owned()]);
        assert_eq!(report.variable_domains, vec!["range[0,4]".to_owned()]);
        assert_eq!(report.constraint_count, 1);
        // `ConstraintDefinition::new` 用 id 预填 name。 / The name is prefilled from the ID.
        assert_eq!(report.constraint_names, vec!["capacity".to_owned()]);
        assert_eq!(report.objective_id, "");
        assert_eq!(report.objective_category, "");
        assert!(report.fingerprint.is_some());
        assert!(report.a_new_model_already_has_an_identity);
        assert!(report.register_variable_refuses_two_kinds_of_duplicate);
        assert!(report.a_new_constraint_names_itself_after_its_id);
        assert!(report.set_objective_silently_overwrites);
        assert!(report.freeze_rebinds_local_ids_to_canonical_indices);
        assert!(report.the_model_name_is_not_in_the_fingerprint);
        assert!(report.a_group_id_may_not_be_rebound);
        assert!(report.freeze_is_the_reference_integrity_gate);
    }

    #[test]
    fn a_blank_identity_is_refused_at_freeze() {
        let report = verify_cp_model_build(&CpModelBuildRequest {
            namespace: Some("  ".to_owned()), ..request() }).unwrap();
        assert!(!report.accepted);
        assert_eq!(report.refusal_stage.as_deref(), Some("freeze"));
        assert!(report.refusal.as_deref().unwrap().contains("namespace must not be blank"));
        let schema = verify_cp_model_build(&CpModelBuildRequest {
            schema_version: Some("  ".to_owned()), ..request() }).unwrap();
        assert!(!schema.accepted);
        assert!(schema.refusal.as_deref().unwrap().contains("schema version must not be blank"));
    }

    #[test]
    fn a_duplicate_local_id_is_refused_at_registration() {
        let report = verify_cp_model_build(&CpModelBuildRequest {
            variables: vec![
                CpVariableSpec { id: "x".to_owned(), lower: Some(0), upper: Some(4),
                    values: Vec::new(), share_local_id_with: None },
                CpVariableSpec { id: "y".to_owned(), lower: Some(0), upper: Some(4),
                    values: Vec::new(), share_local_id_with: Some("x".to_owned()) }],
            constraints: Vec::new(), ..request() }).unwrap();
        assert!(!report.accepted);
        assert_eq!(report.refusal_stage.as_deref(), Some("registerVariable"));
        assert!(report.refusal.as_deref().unwrap().contains("duplicate local variable ID"));
        // 同一稳定 ID 两次也是注册阶段拒。 / The same stable ID twice is refused at registration too.
        let stable = verify_cp_model_build(&CpModelBuildRequest {
            variables: vec![
                CpVariableSpec { id: "x".to_owned(), lower: Some(0), upper: Some(4),
                    values: Vec::new(), share_local_id_with: None },
                CpVariableSpec { id: "x".to_owned(), lower: Some(0), upper: Some(4),
                    values: Vec::new(), share_local_id_with: None }],
            constraints: Vec::new(), ..request() }).unwrap();
        assert!(!stable.accepted);
        assert!(stable.refusal.as_deref().unwrap().contains("duplicate stable variable ID"));
    }

    #[test]
    fn freeze_rebinds_local_ids_in_stable_order() {
        let report = verify_cp_model_build(&CpModelBuildRequest {
            variables: vec![
                CpVariableSpec { id: "b".to_owned(), lower: Some(0), upper: Some(4),
                    values: Vec::new(), share_local_id_with: None },
                CpVariableSpec { id: "a".to_owned(), lower: Some(0), upper: Some(4),
                    values: Vec::new(), share_local_id_with: None }],
            constraints: Vec::new(), ..request() }).unwrap();
        assert!(report.accepted);
        assert_eq!(report.variable_ids, vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(report.variable_local_ids, vec!["0:0".to_owned(), "1:0".to_owned()]);
    }

    #[test]
    fn a_sparse_domain_is_read_as_values() {
        let report = verify_cp_model_build(&CpModelBuildRequest {
            variables: vec![CpVariableSpec { id: "x".to_owned(), lower: None, upper: None,
                values: vec![1, 3, 7], share_local_id_with: None }],
            constraints: Vec::new(), ..request() }).unwrap();
        assert!(report.accepted);
        assert_eq!(report.variable_domains, vec!["values[1,3,7]".to_owned()]);
    }

    #[test]
    fn the_last_objective_wins_and_can_be_cleared() {
        let report = verify_cp_model_build(&CpModelBuildRequest {
            objective: Some("maximize".to_owned()), objective_id: Some("profit".to_owned()),
            objective_variable: Some("x".to_owned()), ..request() }).unwrap();
        assert!(report.accepted);
        assert_eq!(report.objective_id, "profit");
        assert_eq!(report.objective_category, "maximum");
        let cleared = verify_cp_model_build(&CpModelBuildRequest {
            objective: Some("maximize".to_owned()), clear_objective: true, ..request() }).unwrap();
        assert!(cleared.accepted);
        assert_eq!(cleared.objective_id, "");
        // 默认 ID 来自领域常量。 / The default ID comes from the domain constant.
        let default_id = verify_cp_model_build(&CpModelBuildRequest {
            objective: Some("minimize".to_owned()), ..request() }).unwrap();
        assert!(default_id.accepted, "{:?}", default_id.refusal);
        assert!(!default_id.objective_id.is_empty(), "{:?}", default_id);
        assert_eq!(default_id.objective_category, "minimum");
    }

    #[test]
    fn a_group_id_may_not_be_rebound() {
        let report = verify_cp_model_build(&CpModelBuildRequest {
            groups: vec![CpGroupSpec { id: 1, name: "capacity".to_owned() },
                CpGroupSpec { id: 1, name: "renamed".to_owned() }],
            ..request() }).unwrap();
        assert!(!report.accepted);
        assert_eq!(report.refusal_stage.as_deref(), Some("group"));
        assert!(report.refusal.as_deref().unwrap().contains("already bound to a different name"));
        let blank = verify_cp_model_build(&CpModelBuildRequest {
            groups: vec![CpGroupSpec { id: 1, name: "  ".to_owned() }], ..request() }).unwrap();
        assert!(!blank.accepted);
        assert!(blank.refusal.as_deref().unwrap().contains("must not be blank"));
        let ok = verify_cp_model_build(&CpModelBuildRequest {
            groups: vec![CpGroupSpec { id: 2, name: "capacity".to_owned() },
                CpGroupSpec { id: 1, name: "capacity".to_owned() }], ..request() }).unwrap();
        assert!(ok.accepted);
        assert_eq!(ok.groups, vec!["1:capacity".to_owned(), "2:capacity".to_owned()]);
    }

    #[test]
    fn freeze_is_the_reference_integrity_gate() {
        // 约束引用未注册变量：插入不查，冻结才拒。 / A dangling constraint passes insertion and dies at freeze.
        let report = verify_cp_model_build(&CpModelBuildRequest {
            constraints: vec![CpConstraintSpec { id: "dangling".to_owned(),
                variable: Some("ghost".to_owned()), relation: Some("lessOrEqual".to_owned()),
                rhs: Some(1), group: None, origin: None, name: None }],
            variables: Vec::new(), ..request() }).unwrap();
        assert!(!report.accepted);
        assert_eq!(report.refusal_stage.as_deref(), Some("freeze"));
        // 目标引用未注册变量。 / An objective naming an unregistered variable.
        let objective = verify_cp_model_build(&CpModelBuildRequest {
            objective: Some("maximize".to_owned()),
            objective_variable: Some("ghost".to_owned()), ..request() }).unwrap();
        assert!(!objective.accepted);
        assert_eq!(objective.refusal_stage.as_deref(), Some("freeze"));
        // 把变量补上就能冻结。 / Adding the variable makes it freezable.
        let fixed = verify_cp_model_build(&CpModelBuildRequest {
            variables: vec![CpVariableSpec { id: "ghost".to_owned(), lower: Some(0),
                upper: Some(4), values: Vec::new(), share_local_id_with: None }],
            constraints: vec![CpConstraintSpec { id: "dangling".to_owned(),
                variable: Some("ghost".to_owned()), relation: Some("lessOrEqual".to_owned()),
                rhs: Some(1), group: None, origin: None, name: None }],
            ..request() }).unwrap();
        assert!(fixed.accepted, "{:?}", fixed.refusal);
    }

    #[test]
    fn a_constraint_can_carry_a_group_origin_and_name() {
        let report = verify_cp_model_build(&CpModelBuildRequest {
            constraints: vec![CpConstraintSpec { id: "capacity".to_owned(),
                variable: Some("x".to_owned()), relation: Some("equal".to_owned()), rhs: Some(2),
                group: Some("capacity".to_owned()), origin: Some("shift".to_owned()),
                name: Some("Shift capacity".to_owned()) }],
            ..request() }).unwrap();
        assert!(report.accepted);
        assert_eq!(report.constraint_names, vec!["Shift capacity".to_owned()]);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_cp_model_build(&CpModelBuildRequest {
            variables: vec![CpVariableSpec { id: "  ".to_owned(), lower: None, upper: None,
                values: Vec::new(), share_local_id_with: None }], ..request() }).unwrap_err()
            .message.contains("stable variable IDs must not be blank"));
        assert!(verify_cp_model_build(&CpModelBuildRequest {
            constraints: vec![CpConstraintSpec { id: "  ".to_owned(), variable: None,
                relation: None, rhs: None, group: None, origin: None, name: None }],
            ..request() }).unwrap_err().message.contains("stable constraint IDs must not be blank"));
        assert!(verify_cp_model_build(&CpModelBuildRequest {
            constraints: vec![CpConstraintSpec { id: "c".to_owned(), variable: None,
                relation: Some("nope".to_owned()), rhs: None, group: None, origin: None,
                name: None }], ..request() }).unwrap_err().message.contains("unknown relation"));
        assert!(verify_cp_model_build(&CpModelBuildRequest {
            objective: Some("nope".to_owned()), ..request() }).unwrap_err().message
            .contains("unknown objective direction"));
        assert!(verify_cp_model_build(&CpModelBuildRequest {
            variables: vec![CpVariableSpec { id: "x".to_owned(), lower: None, upper: None,
                values: Vec::new(), share_local_id_with: Some("missing".to_owned()) }],
            ..request() }).unwrap_err().message.contains("no variable `missing` to share"));
        assert!(verify_cp_model_build(&CpModelBuildRequest {
            variables: (0..17).map(|index| CpVariableSpec { id: format!("x{index}"),
                lower: Some(0), upper: Some(1), values: Vec::new(),
                share_local_id_with: None }).collect(), ..request() }).unwrap_err().message
            .contains("at most 16 entries"));
        // 反向值域在注册阶段拒（领域校验）。 / A reversed domain is refused at registration.
        let reversed = verify_cp_model_build(&CpModelBuildRequest {
            variables: vec![CpVariableSpec { id: "x".to_owned(), lower: Some(4), upper: Some(0),
                values: Vec::new(), share_local_id_with: None }], ..request() }).unwrap();
        assert!(!reversed.accepted);
        assert_eq!(reversed.refusal_stage.as_deref(), Some("registerVariable"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_cp_model_build(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"variableCount\""), "{json}");
        assert!(json.contains("\"variableLocalIds\""), "{json}");
        assert!(json.contains("\"theModelNameIsNotInTheFingerprint\""), "{json}");
        assert!(json.contains("\"refusalStage\""), "{json}");
        assert!(!json.contains("variable_count"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<CpModelBuildReport>(&copy.to_string()).is_err());
    }
}
