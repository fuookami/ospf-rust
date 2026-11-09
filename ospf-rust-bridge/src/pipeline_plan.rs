//! 管道列表的可移植部分：**约束组注册序列**。
//! The portable part of the pipeline list: the **constraint-group registration sequence**.
//!
//! `Pipeline::invoke` 是调用方的业务代码（向模型加约束），不可数据化；但 `invoke_all_with_group_registration`
//! 的编排是可移植的：按顺序对每个步骤，若有约束组就先 `ensure_constraint_group`，再 `register`，再 `invoke`。
//! 桥接只绑定其中的**注册序列**——调用方在接上真实管道之前就能看到"哪些组会被注册、按什么顺序、有没有冲突"。
//! `Pipeline::invoke` is caller business code (it adds constraints to a model) and is not data-expressible. The
//! orchestration in `invoke_all_with_group_registration` is: for each step in order, `ensure_constraint_group` when it
//! declares a group, then `register`, then `invoke`. The bridge binds only the **registration sequence** part, so a
//! caller can see which groups will be registered, in what order, and whether there is a conflict — before wiring real
//! pipelines.
//!
//! 一个必须暴露的危险行为：领域实现在 `ensure_constraint_group` 里把**任何** `ConstraintConflict` 都当成
//! "已注册"而吞掉。于是"同一 id、不同 name"会被静默接受，并永久保留**第一个**名字。桥接不会跟着吞，
//! 而是把它作为 `id_name_conflicts` 报出来。
//! One hazardous behaviour that must be surfaced: the domain treats **any** `ConstraintConflict` in
//! `ensure_constraint_group` as "already registered" and swallows it. So "same id, different name" is silently accepted
//! and the **first** name wins forever. The bridge does not swallow it too; it reports it as `id_name_conflicts`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个约束组。 / One constraint group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PipelineGroupSpec {
    /// 唯一标识。 / The unique identifier.
    pub id: u64,
    /// 名称。 / The name.
    pub name: String,
}

/// 一个管道步骤。 / One pipeline step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PipelineStepSpec {
    /// 管道名称。 / The pipeline name.
    pub name: String,
    /// 可选约束组；省略表示该步骤不注册组。 / Optional constraint group; omission means the step registers none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub constraint_group: Option<PipelineGroupSpec>,
}

/// 一个步骤的注册判定。 / The registration verdict of one step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PipelineStepVerdict {
    /// 管道名称。 / The pipeline name.
    pub name: String,
    /// 该步骤是否声明了约束组。 / Whether the step declares a constraint group.
    pub declares_group: bool,
    /// 该步骤是否会真正发起一次注册调用。 / Whether the step actually issues a registration call.
    pub registers_group: bool,
    /// 声明了却在本次之前已被同 id 注册过（领域侧会被吞掉）。
    /// Declared but already registered under the same id earlier (the domain swallows it).
    pub already_registered: bool,
}

/// 管道注册计划。 / The pipeline registration plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PipelinePlanReport {
    /// 每个步骤的判定，保持输入顺序。 / Per-step verdicts in input order.
    pub steps: Vec<PipelineStepVerdict>,
    /// 会真正被注册的约束组，按注册顺序（同 id 只出现一次）。
    /// The groups actually registered, in registration order (each id appears once).
    pub registered_groups: Vec<PipelineGroupSpec>,
    /// 同一 id 被声明成不同名称的分组：`[id, 首次名称, 后续名称]`，按 id 升序。
    /// Groups where one id is declared with different names: `[id, firstName, laterName]`, ascending by id.
    pub id_name_conflicts: Vec<PipelineGroupConflict>,
}

/// 一处「同 id、不同名称」的冲突。 / One "same id, different name" conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PipelineGroupConflict {
    /// 冲突的约束组 id。 / The conflicting group id.
    pub id: u64,
    /// 首次注册时使用的名称（领域侧会保留这个名字）。 / The name used first (the domain keeps this one).
    pub first_name: String,
    /// 后续出现的不同名称，按出现顺序。 / The later, differing names in appearance order.
    pub later_names: Vec<String>,
}

/// 计算管道列表的约束组注册计划。 / Compute the constraint-group registration plan of a pipeline list.
pub fn plan_pipeline_registration(steps: &[PipelineStepSpec]) -> Result<PipelinePlanReport> {
    let mut verdicts = Vec::with_capacity(steps.len());
    let mut registered: Vec<PipelineGroupSpec> = Vec::new();
    let mut first_name_by_id: std::collections::BTreeMap<u64, String> = std::collections::BTreeMap::new();
    let mut conflicts: std::collections::BTreeMap<u64, Vec<String>> = std::collections::BTreeMap::new();
    for step in steps {
        if step.name.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "pipeline step names must not be blank"));
        }
        let mut registers = false;
        let mut already = false;
        if let Some(group) = &step.constraint_group {
            if group.name.trim().is_empty() {
                return Err(BridgeError::new("InvalidInput", "pipeline constraint group names must not be blank"));
            }
            match first_name_by_id.get(&group.id) {
                // 领域实现会把冲突当"已注册"吞掉，所以这次不会真正建立新的组。
                // The domain treats the conflict as "already registered", so no new group is actually created.
                Some(first) => {
                    already = true;
                    if *first != group.name {
                        conflicts.entry(group.id).or_default().push(group.name.clone());
                    }
                }
                None => {
                    first_name_by_id.insert(group.id, group.name.clone());
                    registered.push(group.clone());
                    registers = true;
                }
            }
        }
        verdicts.push(PipelineStepVerdict { name: step.name.clone(),
            declares_group: step.constraint_group.is_some(), registers_group: registers,
            already_registered: already });
    }
    let id_name_conflicts = conflicts.into_iter()
        .map(|(id, later_names)| PipelineGroupConflict {
            first_name: first_name_by_id.get(&id).cloned().unwrap_or_default(),
            id,
            later_names,
        })
        .collect();
    Ok(PipelinePlanReport { steps: verdicts, registered_groups: registered, id_name_conflicts })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::error::Result as CoreResult;
    use ospf_rust_core::model::mechanism::ConstraintGroup;
    use ospf_rust_framework::model::pipeline::{
        ConstraintGroupRegistrar, Pipeline, PipelineListWithGroupRegistration,
    };
    use std::sync::{Arc, Mutex};

    /// 记录模型：把收到的每一次 `ensure_constraint_group` 都记下来，并复述真实循环的吞冲突行为。
    /// Recording model: log every `ensure_constraint_group` call and reproduce the real loop's conflict swallowing.
    #[derive(Default)]
    struct RecordingModel {
        ensured: Arc<Mutex<Vec<(u64, String)>>>,
        invoked: Arc<Mutex<Vec<String>>>,
    }

    impl ConstraintGroupRegistrar for RecordingModel {
        fn ensure_constraint_group(&mut self, group: &ConstraintGroup) -> CoreResult<()> {
            self.ensured.lock().unwrap().push((group.id, group.name.clone()));
            Ok(())
        }
    }

    struct Step {
        name: String,
        group: Option<ConstraintGroup>,
        invoked: Arc<Mutex<Vec<String>>>,
    }

    impl Pipeline<RecordingModel> for Step {
        fn name(&self) -> &str { &self.name }
        fn constraint_group(&self) -> Option<&ConstraintGroup> { self.group.as_ref() }
        fn invoke(&self, _model: &RecordingModel) -> CoreResult<()> {
            self.invoked.lock().unwrap().push(self.name.clone());
            Ok(())
        }
    }

    /// 跑一遍真实循环，返回它实际发起的注册调用与执行顺序。
    /// Run the real loop once and return the registration calls and invocation order it actually made.
    fn run_real_loop(steps: &[PipelineStepSpec]) -> (Vec<(u64, String)>, Vec<String>) {
        let model = RecordingModel::default();
        let ensured = model.ensured.clone();
        let invoked = model.invoked.clone();
        let pipelines: Vec<Arc<dyn Pipeline<RecordingModel>>> = steps.iter().map(|step| {
            Arc::new(Step { name: step.name.clone(),
                group: step.constraint_group.as_ref().map(|group| ConstraintGroup::new(group.id, &group.name)),
                invoked: invoked.clone() }) as Arc<dyn Pipeline<RecordingModel>>
        }).collect();
        let mut model = model;
        // 领域实现会把任何 ConstraintConflict 吞掉；RecordingModel 不做去重，所以这里看到的正是"每一步都调用"。
        // The domain swallows any ConstraintConflict; RecordingModel does not deduplicate, so what is observed here is
        // exactly "one call per step".
        pipelines.invoke_all_with_group_registration(&mut model).unwrap();
        (ensured.lock().unwrap().clone(), invoked.lock().unwrap().clone())
    }

    #[test]
    fn the_plan_agrees_with_the_real_pipeline_loop() {
        let steps = vec![
            step("first", Some((1, "loads"))),
            step("no-group", None),
            step("second", Some((1, "loads"))),
            step("third", Some((2, "capacity"))),
        ];
        let (ensured, invoked) = run_real_loop(&steps);
        let report = plan_pipeline_registration(&steps).unwrap();
        // 真实循环对每个声明了组的步骤都调用一次注册（去重靠领域吞冲突），顺序与步骤顺序一致。
        // The real loop calls registration once per step that declares a group (dedup happens by swallowing conflicts
        // in the domain), in step order.
        assert_eq!(ensured, vec![(1, "loads".to_owned()), (1, "loads".to_owned()), (2, "capacity".to_owned())]);
        // 桥接报的"真正建立起来的组"就是这条序列按 id 去重后的结果。
        // The bridge's "groups actually created" is that sequence deduplicated by id.
        assert_eq!(report.registered_groups.len(), 2);
        assert_eq!(report.registered_groups[0].id, 1);
        assert_eq!(report.registered_groups[1].id, 2);
        // 每个步骤都会被 invoke，包括没有组的。 / Every step is invoked, including those without a group.
        assert_eq!(invoked, vec!["first".to_owned(), "no-group".to_owned(), "second".to_owned(), "third".to_owned()]);
        assert_eq!(report.steps.len(), invoked.len());
    }

    #[test]
    fn the_real_loop_really_swallows_same_id_different_name() {
        let steps = vec![step("first", Some((7, "loads"))), step("second", Some((7, "weight")))];
        let (ensured, _) = run_real_loop(&steps);
        // 领域实现每次都会调用一次，且把第二次当作"已注册"吞掉——桥接必须把这件事报成冲突。
        // The domain calls once per step and swallows the second as "already registered"; the bridge must report it.
        assert_eq!(ensured, vec![(7, "loads".to_owned()), (7, "weight".to_owned())]);
        let report = plan_pipeline_registration(&steps).unwrap();
        assert_eq!(report.registered_groups.len(), 1);
        assert_eq!(report.id_name_conflicts[0].first_name, "loads");
        assert_eq!(report.id_name_conflicts[0].later_names, vec!["weight".to_owned()]);
    }

    fn step(name: &str, group: Option<(u64, &str)>) -> PipelineStepSpec {
        PipelineStepSpec { name: name.into(),
            constraint_group: group.map(|(id, name)| PipelineGroupSpec { id, name: name.into() }) }
    }

    #[test]
    fn registration_order_matches_the_real_loop() {
        // 真实循环：按顺序对每一步先确保组、再 register、再 invoke。
        // The real loop, in order, ensures the group, then registers, then invokes for each step.
        let report = plan_pipeline_registration(&[
            step("first", Some((1, "loads"))),
            step("no-group", None),
            step("second", Some((1, "loads"))),
            step("third", Some((2, "capacity"))),
        ]).unwrap();
        assert_eq!(report.registered_groups,
            vec![PipelineGroupSpec { id: 1, name: "loads".into() },
                PipelineGroupSpec { id: 2, name: "capacity".into() }]);
        // 第二次声明同 id 同名称：领域侧会被吞掉，不产生新组。
        // The second declaration with the same id and name is swallowed by the domain and creates no new group.
        assert!(!report.steps[2].registers_group);
        assert!(report.steps[2].already_registered);
        assert!(report.steps[2].declares_group);
        assert!(!report.steps[1].declares_group);
        assert!(!report.steps[1].registers_group);
        assert!(report.id_name_conflicts.is_empty());
    }

    #[test]
    fn same_id_with_a_different_name_is_reported_not_swallowed() {
        // 领域实现会把"同 id、不同名"当 ConstraintConflict 吞掉，并永久保留第一个名字。
        // The domain swallows "same id, different name" as a ConstraintConflict and keeps the first name forever.
        let report = plan_pipeline_registration(&[
            step("first", Some((7, "loads"))),
            step("second", Some((7, "weight"))),
            step("third", Some((7, "volume"))),
        ]).unwrap();
        // 只注册一次，名字是第一个。 / Registered once, under the first name.
        assert_eq!(report.registered_groups,
            vec![PipelineGroupSpec { id: 7, name: "loads".into() }]);
        // 冲突必须被报出来，而不是跟着吞。 / The conflict is reported instead of being swallowed along with it.
        assert_eq!(report.id_name_conflicts.len(), 1);
        assert_eq!(report.id_name_conflicts[0].id, 7);
        assert_eq!(report.id_name_conflicts[0].first_name, "loads");
        assert_eq!(report.id_name_conflicts[0].later_names, vec!["weight".to_owned(), "volume".to_owned()]);
        // 空步骤列表是合法的。 / An empty step list is legal.
        let empty = plan_pipeline_registration(&[]).unwrap();
        assert!(empty.steps.is_empty());
        assert!(empty.registered_groups.is_empty());
        // 非法输入被拒。 / Malformed input is refused.
        assert!(plan_pipeline_registration(&[step(" ", None)]).is_err());
        assert!(plan_pipeline_registration(&[step("a", Some((1, " ")))]).is_err());
    }
}
