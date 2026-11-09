//! 批次 B 第 10 单：criticality profile 的独立构造与校验 / Batch B, delivery 10: standalone criticality profile.
//!
//! 第 9 单已经从多目标报告里把分类规则带过线，这一单补上**独立入口**：给定一组（target, 状态, 阻塞约束）观察，
//! 用领域自己的 `build_criticality_profile` 重新推导 profile，并在调用方给了一份"声称的 profile"时核对二者是否一致。
//!
//! 两条实测语义必须读对：
//! 1. **只有 `unreachable` 的观察参与分类**。一个 `reachable` 观察即使带着阻塞约束 ID，也不会出现在任何类别里
//!    （领域实现先按状态过滤，再计数）。所以"我给了 ID"不等于"它被分类了"。
//! 2. **结构瓶颈要求不可达目标多于一个**（`total > 1 && count == total`）。只有一个不可达目标时，其中的约束
//!    只是局部瓶颈，不会被升格为结构瓶颈。
//!
//! Delivery 9 already brought the classification rules across the wire through the multi-target report; this delivery
//! adds the standalone entry: given (target, status, blocking IDs) observations, recompute the profile with the domain's
//! own `build_criticality_profile`, and cross-check any caller-supplied "claimed" profile against that recomputation. Two
//! measured semantics: **only `unreachable` observations take part in classification** (a `reachable` observation with
//! blocking IDs contributes nothing), and a **structural** bottleneck requires more than one unreachable target.

use ospf_rust_core::analysis::criticality::{
    build_criticality_profile, CriticalityKind, CriticalityObservation, CriticalityProfile,
};
use ospf_rust_core::analysis::ObjectiveTarget;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::analysis_multi_target::BottleneckKindRow;
use crate::{BridgeError, Result};

/// 一条观察输入。 / One observation input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalityObservationInput {
    /// 目标身份。 / The objective identity.
    pub objective_id: String,
    /// `atLeast` 或 `atMost`。 / Either `atLeast` or `atMost`.
    pub relation: String,
    /// 目标值。 / The target value.
    pub value: f64,
    /// 状态：`reachable`/`unreachable`/`unknown`/`unsupported`。 / The status.
    pub status: String,
    /// 阻塞约束的稳定 ID；只有 `unreachable` 观察的 ID 会参与分类。
    /// The blocking constraint stable IDs; only those of `unreachable` observations take part in classification.
    #[serde(default)]
    pub blocking_constraint_ids: Vec<String>,
}

/// 构造与校验请求。 / The build-and-verify request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalityRequest {
    /// 观察列表。 / The observations.
    pub observations: Vec<CriticalityObservationInput>,
    /// 可选：调用方声称的 profile（JSON 文本），必须与重算结果一致。
    /// Optional: a caller-claimed profile (JSON text) that must agree with the recomputation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub profile: Option<String>,
}

/// 一条观察的投影。 / One observation projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalityObservationRow {
    /// 目标身份。 / The objective identity.
    pub objective_id: String,
    /// 方向。 / The direction.
    pub relation: String,
    /// 目标值。 / The target value.
    pub value: f64,
    /// 稳定 target 身份。 / The stable target identity.
    pub stable_id: String,
    /// 状态。 / The status.
    pub status: String,
    /// 阻塞约束稳定 ID。 / The blocking constraint stable IDs.
    pub blocking_constraint_ids: Vec<String>,
    /// 该观察是否参与分类（等价于 `status == unreachable`）。 / Whether it takes part in classification.
    pub contributes_to_classification: bool,
}

/// criticality profile 的构造结论。 / The criticality profile build outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalityProfileReport {
    /// profile schema。 / The profile schema.
    pub schema_version: String,
    /// 观察条数。 / The observation count.
    pub observation_count: u32,
    /// 已证明不可达的目标数（`proven_target_count`）。 / The number of proven-unreachable targets.
    pub proven_target_count: u32,
    /// 被分类的约束数。 / The number of classified constraints.
    pub classified_constraint_count: u32,
    /// 观察明细。 / The observation details.
    pub observations: Vec<CriticalityObservationRow>,
    /// 三类瓶颈（固定顺序，可重叠）。 / The three kinds in a fixed order (they may overlap).
    pub bottlenecks: Vec<BottleneckKindRow>,
}

/// 构造 criticality profile，并在给出"声称的 profile"时核对一致性。非法输入是**错误**，原因原样带出。
/// Build the profile and cross-check a claimed one when supplied. An illegal input is an **error** carrying the reason
/// verbatim.
pub fn verify_criticality_profile(request: &CriticalityRequest) -> Result<CriticalityProfileReport> {
    let observations = request
        .observations
        .iter()
        .map(observation)
        .collect::<Result<Vec<CriticalityObservation>>>()?;
    // 走领域的构造器：分类规则（局部/持久/结构）只在领域里实现一次，桥接不重写。
    // Use the domain constructor: the classification rules live in exactly one place, and the bridge does not re-write
    // them.
    let profile = build_criticality_profile(observations).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("criticality profile failed validation: {error}"))
    })?;
    if let Some(claimed) = request.profile.as_deref() {
        if claimed.len() > 8 * 1024 * 1024 {
            return Err(BridgeError::new("InvalidInput", "the claimed criticality profile exceeds 8 MiB"));
        }
        let claimed: CriticalityProfile = serde_json::from_str(claimed).map_err(|error| {
            BridgeError::new("InvalidInput", &format!("claimed criticality profile is not decodable: {error}"))
        })?;
        claimed.validate().map_err(|error| {
            BridgeError::new("InvalidInput",
                &format!("claimed criticality profile failed validation: {error}"))
        })?;
        // 逐项核对：观察、分类、schema 都必须与重算结果完全一致，否则"声称的结论"与"能证明的结论"不同。
        // Compare every part: the observations, classifications and schema must match the recomputation exactly, otherwise
        // the claimed conclusion differs from the provable one.
        if claimed.observations != profile.observations {
            return Err(BridgeError::new("InvalidInput",
                "claimed criticality profile observations disagree with the recomputation"));
        }
        if claimed.classifications != profile.classifications {
            return Err(BridgeError::new("InvalidInput",
                "claimed criticality profile classifications disagree with the recomputation"));
        }
        if claimed.schema_version != profile.schema_version {
            return Err(BridgeError::new("InvalidInput",
                "claimed criticality profile schema disagrees with the recomputation"));
        }
    }
    Ok(CriticalityProfileReport {
        schema_version: profile.schema_version.clone(),
        observation_count: u32::try_from(profile.observations.len()).unwrap_or(u32::MAX),
        proven_target_count: u32::try_from(profile.proven_target_count()).unwrap_or(u32::MAX),
        classified_constraint_count: u32::try_from(profile.classifications.len()).unwrap_or(u32::MAX),
        observations: profile.observations.iter().map(|observation| CriticalityObservationRow {
            objective_id: observation.target.objective_id().as_str().to_owned(),
            relation: format!("{:?}", observation.target.relation()),
            value: observation.target.value(),
            stable_id: observation.target.stable_id(),
            status: status_name(observation.status).to_owned(),
            blocking_constraint_ids: observation.blocking_constraint_ids.iter()
                .map(|id| id.0.clone()).collect(),
            contributes_to_classification: observation.status == ospf_rust_core::analysis::AnalysisStatus::Unreachable,
        }).collect(),
        bottlenecks: [CriticalityKind::LocalBottleneck, CriticalityKind::PersistentBottleneck,
            CriticalityKind::StructuralBottleneck]
            .into_iter()
            .map(|kind| BottleneckKindRow {
                kind: kind_name(kind),
                constraint_ids: profile.constraints(kind).into_iter().map(|id| id.0).collect(),
            })
            .collect(),
    })
}

/// 把一条输入转成领域观察（target 走真实构造器）。 / Convert one input into a domain observation.
fn observation(input: &CriticalityObservationInput) -> Result<CriticalityObservation> {
    let target = match input.relation.as_str() {
        "atLeast" => ObjectiveTarget::at_least(input.objective_id.as_str(), input.value),
        "atMost" => ObjectiveTarget::at_most(input.objective_id.as_str(), input.value),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "criticality observation failed validation: unsupported relation `{other}` (expected `atLeast` or `atMost`)"))),
    }
    .map_err(|error| {
        BridgeError::new("InvalidInput", &format!("criticality observation target failed validation: {error}"))
    })?;
    let status = match input.status.as_str() {
        "reachable" => ospf_rust_core::analysis::AnalysisStatus::Reachable,
        "unreachable" => ospf_rust_core::analysis::AnalysisStatus::Unreachable,
        "unknown" => ospf_rust_core::analysis::AnalysisStatus::Unknown,
        "unsupported" => ospf_rust_core::analysis::AnalysisStatus::Unsupported,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "criticality observation failed validation: unknown status `{other}`"))),
    };
    Ok(CriticalityObservation {
        target,
        status,
        blocking_constraint_ids: input.blocking_constraint_ids.iter()
            .map(|id| ospf_rust_core::solver::report::StableConstraintId(id.clone()))
            .collect(),
    })
}

/// 瓶颈类别名，走领域的 serde 表示。 / The bottleneck kind name through the domain's serde representation.
fn kind_name(kind: CriticalityKind) -> String {
    serde_json::to_value(kind).ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{kind:?}"))
}

/// 分析状态名。 / The analysis status name.
fn status_name(status: ospf_rust_core::analysis::AnalysisStatus) -> &'static str {
    use ospf_rust_core::analysis::AnalysisStatus;
    match status {
        AnalysisStatus::Reachable => "reachable",
        AnalysisStatus::Unreachable => "unreachable",
        AnalysisStatus::Unknown => "unknown",
        AnalysisStatus::Unsupported => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation_input(id: &str, status: &str, ids: &[&str]) -> CriticalityObservationInput {
        CriticalityObservationInput {
            objective_id: id.to_owned(),
            relation: "atLeast".to_owned(),
            value: 10.0,
            status: status.to_owned(),
            blocking_constraint_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    fn request(observations: Vec<CriticalityObservationInput>) -> CriticalityRequest {
        CriticalityRequest { observations, profile: None }
    }

    #[test]
    fn classification_counts_only_unreachable_observations() {
        let report = verify_criticality_profile(&request(vec![
            observation_input("throughput", "unreachable", &["capacity", "shift"]),
            observation_input("cost", "unreachable", &["capacity"]),
            // 这条带着阻塞 ID，但状态是 `reachable`：**不参与分类**。
            // This one carries blocking IDs but is `reachable`: it does **not** take part in classification.
            observation_input("latency", "reachable", &["ghost"]),
        ])).unwrap();
        assert_eq!(report.observation_count, 3);
        // 只有两条不可达观察算"已证明"。 / Only the two unreachable observations count as proven.
        assert_eq!(report.proven_target_count, 2);
        // `ghost` 不出现：它来自一条不参与分类的观察。 / `ghost` never appears: it came from a non-classifying observation.
        assert_eq!(report.classified_constraint_count, 2);
        assert_eq!(report.observations[2].contributes_to_classification, false);
        assert_eq!(report.observations[2].blocking_constraint_ids, vec!["ghost".to_owned()]);
        assert_eq!(report.bottlenecks[0].kind, "LOCAL_BOTTLENECK");
        assert_eq!(report.bottlenecks[0].constraint_ids, vec!["shift".to_owned()]);
        assert_eq!(report.bottlenecks[1].kind, "PERSISTENT_BOTTLENECK");
        assert_eq!(report.bottlenecks[1].constraint_ids, vec!["capacity".to_owned()]);
        assert_eq!(report.bottlenecks[2].kind, "STRUCTURAL_BOTTLENECK");
        assert_eq!(report.bottlenecks[2].constraint_ids, vec!["capacity".to_owned()]);
    }

    #[test]
    fn structural_requires_more_than_one_unreachable_target() {
        // 只有一个不可达目标时，其中的约束只算局部瓶颈——不会被升格成结构瓶颈。
        // With a single unreachable target its constraints are only local; none is upgraded to structural.
        let report = verify_criticality_profile(&request(vec![
            observation_input("throughput", "unreachable", &["capacity"]),
        ])).unwrap();
        assert_eq!(report.proven_target_count, 1);
        assert_eq!(report.bottlenecks[0].constraint_ids, vec!["capacity".to_owned()]);
        assert_eq!(report.bottlenecks[1].constraint_ids, Vec::<String>::new());
        assert_eq!(report.bottlenecks[2].constraint_ids, Vec::<String>::new());

        // 没有任何不可达目标：分类为空，`proven_target_count = 0`，而且这不是错误。
        // No unreachable target at all: empty classification and zero proven targets, and that is not an error.
        let report = verify_criticality_profile(&request(vec![
            observation_input("throughput", "unknown", &["capacity"]),
        ])).unwrap();
        assert_eq!(report.proven_target_count, 0);
        assert_eq!(report.classified_constraint_count, 0);
        assert!(report.bottlenecks.iter().all(|row| row.constraint_ids.is_empty()));
    }

    #[test]
    fn claimed_profile_must_match_the_recomputation() {
        let observations = vec![
            observation_input("throughput", "unreachable", &["capacity", "shift"]),
            observation_input("cost", "unreachable", &["capacity"]),
        ];
        // 先拿到真实的 profile 文本（由领域构造器序列化而来）。
        // Obtain the real profile text first (serialized from the domain constructor).
        let real = {
            let built = build_criticality_profile(observations.iter()
                .map(observation).collect::<Result<Vec<_>>>().unwrap()).unwrap();
            serde_json::to_string(&built).unwrap()
        };
        let mut matched = request(observations.clone());
        matched.profile = Some(real.clone());
        let report = verify_criticality_profile(&matched).unwrap();
        assert_eq!(report.observation_count, 2);

        // 篡改分类：声称的 profile 与重算不一致。 / A tampered classification: claimed and recomputed disagree.
        let mut tampered: serde_json::Value = serde_json::from_str(&real).unwrap();
        let classifications = tampered.get_mut("classifications").unwrap();
        classifications["shift"] = serde_json::json!(["LOCAL_BOTTLENECK", "STRUCTURAL_BOTTLENECK"]);
        let mut mismatched = request(observations.clone());
        mismatched.profile = Some(serde_json::to_string(&tampered).unwrap());
        let error = verify_criticality_profile(&mismatched).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        // 实测：声称的 profile 自己就自相矛盾，所以先被它自己的校验挡住。
        // Measured: the claimed profile is itself self-contradictory, so its own validation fires first.
        assert!(error.message.contains("claimed criticality profile failed validation"), "{}", error.message);

        // 声称的 profile 少了第三条观察：与重算不一致（观察层面）。
        // A claimed profile missing an observation: disagreement at the observation level.
        let mut fewer = request(vec![
            observation_input("throughput", "unreachable", &["capacity"]),
            observation_input("cost", "unreachable", &["capacity"]),
        ]);
        fewer.profile = Some(real.clone());
        let error = verify_criticality_profile(&fewer).unwrap_err();
        assert!(error.message.contains("observations disagree with the recomputation"), "{}", error.message);
    }

    #[test]
    fn illegal_requests_are_refused_with_the_domain_reason() {
        // 重复 target：领域拒绝。 / A duplicate target: refused by the domain.
        let error = verify_criticality_profile(&request(vec![
            observation_input("throughput", "unreachable", &["capacity"]),
            observation_input("throughput", "unreachable", &["shift"]),
        ])).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("duplicate targets"), "{}", error.message);

        // 空白约束 ID：领域拒绝。 / A blank constraint ID: refused by the domain.
        let error = verify_criticality_profile(&request(vec![
            observation_input("throughput", "unreachable", &["  "]),
        ])).unwrap_err();
        assert!(error.message.contains("must not be blank"), "{}", error.message);

        // 空白目标身份 / 未知方向 / 未知状态。 / A blank objective identity, an unknown relation and an unknown status.
        let error = verify_criticality_profile(&request(vec![CriticalityObservationInput {
            objective_id: "  ".to_owned(), relation: "atLeast".to_owned(), value: 1.0,
            status: "unreachable".to_owned(), blocking_constraint_ids: Vec::new() }])).unwrap_err();
        assert!(error.message.contains("must not be blank"), "{}", error.message);
        let mut unknown_relation = observation_input("throughput", "unreachable", &[]);
        unknown_relation.relation = "greater".to_owned();
        let error = verify_criticality_profile(&request(vec![unknown_relation])).unwrap_err();
        assert!(error.message.contains("unsupported relation"), "{}", error.message);
        let mut unknown_status = observation_input("throughput", "unreachable", &[]);
        unknown_status.status = "solved".to_owned();
        let error = verify_criticality_profile(&request(vec![unknown_status])).unwrap_err();
        assert!(error.message.contains("unknown status"), "{}", error.message);

        // 空观察列表是合法的（profile 为空），不是错误。
        // An empty observation list is legal (an empty profile) and not an error.
        let report = verify_criticality_profile(&request(Vec::new())).unwrap();
        assert_eq!(report.observation_count, 0);
        assert_eq!(report.proven_target_count, 0);

        // 声称的 profile 不是 JSON。 / A claimed profile that is not JSON.
        let mut bad_claim = request(Vec::new());
        bad_claim.profile = Some("not json".to_owned());
        assert_eq!(verify_criticality_profile(&bad_claim).unwrap_err().code, "InvalidInput");
    }
}
