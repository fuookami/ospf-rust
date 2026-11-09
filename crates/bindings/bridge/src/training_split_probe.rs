//! 续挖第 120 单：**训练切分的校验有三条互不重叠的枪口，而"同一个问题"是按 `ComponentSpec` 的结构相等判的**——
//! `TrainingSplit::validate` 一次性查三件事：**ID 必须非空且跨三个集合全局唯一**、**训练集非空**（验证集与测试集
//! 为空**是允许的**）、**同一问题定义不得跨集合出现**（同一集合内却允许重复）；由于 `problem` 用
//! `ComponentSpec` 的结构相等来比，**只差 `version` 或只差一个参数就算不同问题**（因此不构成泄漏）；
//! 且同一 id 又跨集合时，**先命中的是 ID 判重**（错误文本是 ID 那条，不是泄漏那条）
//! （`solver/hyper_heuristic/learning.rs` 的 `TrainingSplit` / `TrainingCase`）
//! Continuation, delivery 120: **the training split validates three non-overlapping things, and "the same problem" is decided by
//! `ComponentSpec`'s structural equality** — `TrainingSplit::validate` checks three things at once: **the IDs must be non-empty
//! and globally unique across all three splits**, **the training split must be non-empty** (an empty validation or test split
//! **is allowed**), and **one problem definition must not appear in two splits** (repetition inside one split is fine); because
//! `problem` compares by `ComponentSpec`'s structural equality, **differing only in `version` or in one parameter counts as a
//! different problem** and is therefore not a leak; and when the same id also crosses splits, **the ID check fires first**, so the
//! message is the ID one rather than the leak one (`TrainingSplit` / `TrainingCase` in `solver/hyper_heuristic/learning.rs`).
//!
//! 第 119 单量的是非平稳选择器，这一单量**训练切分的校验口径**。八条实测：
//!
//! 1. **空 ID 被拒**（`an_empty_id_is_refused()`）。
//! 2. **同一集合内重复 ID 被拒**（`a_repeated_id_inside_one_split_is_refused()`）。
//! 3. **跨集合重复 ID 被拒**（`a_repeated_id_across_splits_is_refused()`）。
//! 4. **训练集为空被拒，验证/测试为空被接受**（`an_empty_training_split_is_refused()`）。
//! 5. **同一问题跨集合被拒**（`a_problem_leaking_across_splits_is_refused()`）。
//! 6. **同一集合内同一问题重复被接受**（`a_repeated_problem_inside_one_split_is_accepted()`）。
//! 7. **`ComponentSpec` 的结构相等参与判定**（`the_problem_equality_is_structural()`）：只差 `version` 或只差一个参数
//!    就算不同问题，因此不算泄漏。
//! 8. **ID 判重先于泄漏判定**（`the_id_check_fires_before_the_leak_check()`）。
//!
//! Delivery 119 measured the non-stationary selector; this one measures **the training split's validation contract**.
//! **An empty id is refused**, **a repeated id inside one split is refused**, and **a repeated id across splits is refused**.
//! **An empty training split is refused** while an empty validation or test split is accepted. **A leaking problem is refused**,
//! **a repeated problem inside one split is accepted**, and **the problem equality is structural**, so a differing version or a
//! single differing parameter makes it a different problem. **The id check fires before the leak check.**

use ospf_rust_core::solver::hyper_heuristic::learning::{TrainingCase, TrainingSplit};
use ospf_rust_core::solver::hyper_heuristic::config::ComponentSpec;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个训练实例的描述。 / The description of one training instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainingCaseSpec {
    /// 稳定实例 ID。 / The stable instance id.
    pub id: String,
    /// 问题定义的类型名。 / The problem definition's type name.
    pub problem: String,
    /// 问题定义版本（默认 1）。 / The definition version.
    #[serde(default)]
    pub version: Option<u32>,
    /// 显式参数（字符串键值，值按 JSON 数字或文本读）。 / The explicit parameters.
    #[serde(default)]
    pub parameters: Vec<TrainingParameterSpec>,
}

/// 一个显式参数。 / One explicit parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainingParameterSpec {
    /// 参数名。 / The parameter name.
    pub key: String,
    /// 参数值（按数字读；读不成数字就按文本）。 / The value, read as a number when possible.
    pub value: String,
}

/// 训练切分探针请求。 / The training split probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainingSplitProbeRequest {
    /// 允许学习的实例。 / The instances eligible for learning.
    #[serde(default)]
    pub training: Vec<TrainingCaseSpec>,
    /// 冻结验证实例。 / The frozen validation instances.
    #[serde(default)]
    pub validation: Vec<TrainingCaseSpec>,
    /// 冻结测试实例。 / The frozen test instances.
    #[serde(default)]
    pub test: Vec<TrainingCaseSpec>,
}

/// 训练切分探针结论。 / The training split probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainingSplitProbeReport {
    /// 校验是否通过。 / Whether validation passed.
    pub valid: bool,
    /// 失败时的错误文本。 / The error text when it fails.
    pub error: String,
    /// 三个集合的实例总数。 / The total instance count.
    pub instances: u32,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub an_empty_id_is_refused: bool,
    pub a_repeated_id_inside_one_split_is_refused: bool,
    pub a_repeated_id_across_splits_is_refused: bool,
    pub an_empty_training_split_is_refused: bool,
    pub a_problem_leaking_across_splits_is_refused: bool,
    pub a_repeated_problem_inside_one_split_is_accepted: bool,
    pub the_problem_equality_is_structural: bool,
    pub the_id_check_fires_before_the_leak_check: bool,
}

/// 造一个实例。 / Build one instance.
fn case_of(spec: &TrainingCaseSpec) -> TrainingCase {
    let mut problem = ComponentSpec::new(spec.problem.as_str());
    if let Some(version) = spec.version {
        problem.version = version;
    }
    for parameter in &spec.parameters {
        // 数字能读就读成数字，否则按文本；这条差异本身就是"参数参与相等"。 
        // Readable numbers stay numbers, everything else is text; that difference is exactly what participates in equality.
        match parameter.value.parse::<i64>() {
            Ok(number) => problem = problem.parameter(parameter.key.as_str(), number),
            Err(_) => problem = problem.parameter(parameter.key.as_str(), parameter.value.as_str()),
        }
    }
    TrainingCase { id: spec.id.clone(), problem }
}

/// 造一个切分。 / Build one split.
fn split_of(request: &TrainingSplitProbeRequest) -> TrainingSplit {
    TrainingSplit {
        training: request.training.iter().map(case_of).collect(),
        validation: request.validation.iter().map(case_of).collect(),
        test: request.test.iter().map(case_of).collect(),
    }
}

/// 走一遍训练切分探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_training_split_probe(
    request: &TrainingSplitProbeRequest,
) -> Result<TrainingSplitProbeReport> {
    let total = request.training.len() + request.validation.len() + request.test.len();
    if total > 128 {
        return Err(BridgeError::new("InvalidInput", "training split probe failed validation: \
            at most one hundred and twenty-eight instances are accepted"));
    }
    if request.training.iter().chain(&request.validation).chain(&request.test)
        .any(|spec| spec.problem.trim().is_empty())
    {
        return Err(BridgeError::new("InvalidInput", "training split probe failed validation: \
            a problem definition must not be blank"));
    }
    let findings = canonical_findings();
    // 校验结论是**结果**而不是桥接失败：`valid = false` 与错误文本一起回读。 
    // A validation verdict is a **result**, not a bridge failure: `valid = false` comes back with the text.
    let (valid, error) = match split_of(request).validate() {
        Ok(()) => (true, String::new()),
        Err(error) => (false, error.to_string()),
    };
    Ok(TrainingSplitProbeReport {
        valid,
        error,
        instances: u32::try_from(total).unwrap_or(u32::MAX),
        an_empty_id_is_refused: findings.an_empty_id_is_refused,
        a_repeated_id_inside_one_split_is_refused:
            findings.a_repeated_id_inside_one_split_is_refused,
        a_repeated_id_across_splits_is_refused: findings.a_repeated_id_across_splits_is_refused,
        an_empty_training_split_is_refused: findings.an_empty_training_split_is_refused,
        a_problem_leaking_across_splits_is_refused:
            findings.a_problem_leaking_across_splits_is_refused,
        a_repeated_problem_inside_one_split_is_accepted:
            findings.a_repeated_problem_inside_one_split_is_accepted,
        the_problem_equality_is_structural: findings.the_problem_equality_is_structural,
        the_id_check_fires_before_the_leak_check: findings.the_id_check_fires_before_the_leak_check,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    an_empty_id_is_refused: bool,
    a_repeated_id_inside_one_split_is_refused: bool,
    a_repeated_id_across_splits_is_refused: bool,
    an_empty_training_split_is_refused: bool,
    a_problem_leaking_across_splits_is_refused: bool,
    a_repeated_problem_inside_one_split_is_accepted: bool,
    the_problem_equality_is_structural: bool,
    the_id_check_fires_before_the_leak_check: bool,
}

/// 造一个实例（内部用）。 / Build one instance (internal).
fn case(id: &str, problem: &str) -> TrainingCase {
    TrainingCase { id: id.to_owned(), problem: ComponentSpec::new(problem) }
}

/// 造一个只含训练集的切分。 / Build a split with a training part only.
fn training_only(cases: Vec<TrainingCase>) -> TrainingSplit {
    TrainingSplit { training: cases, validation: Vec::new(), test: Vec::new() }
}

/// 只回错误文本；通过则给空串。 / The error text, empty when the split passes.
fn verdict(split: &TrainingSplit) -> String {
    split.validate().err().map(|error| error.to_string()).unwrap_or_default()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) 空 ID 被拒。 / An empty id is refused.
        let an_empty_id_is_refused = {
            let blank = training_only(vec![case("", "p1")]);
            verdict(&blank).contains("duplicate or empty training instance ID")
                && verdict(&training_only(vec![case("x", "p1")])).is_empty()
        };
        // 2) 同一集合内重复 ID 被拒。 / A repeated id inside one split is refused.
        let a_repeated_id_inside_one_split_is_refused = {
            let doubled = training_only(vec![case("x", "p1"), case("x", "p2")]);
            verdict(&doubled).contains("duplicate or empty training instance ID")
        };
        // 3) 跨集合重复 ID 被拒。 / A repeated id across splits is refused.
        let a_repeated_id_across_splits_is_refused = {
            let split = TrainingSplit {
                training: vec![case("x", "p1")],
                validation: vec![case("x", "p2")],
                test: Vec::new(),
            };
            verdict(&split).contains("duplicate or empty training instance ID")
        };
        // 4) 训练集为空被拒，验证/测试为空被接受。 
        // An empty training split is refused; empty validation and test parts are accepted.
        let an_empty_training_split_is_refused = {
            let empty = TrainingSplit::default();
            verdict(&empty).contains("training instances are required")
                && verdict(&training_only(vec![case("x", "p1")])).is_empty()
        };
        // 5) 同一问题跨集合被拒。 / A problem leaking across splits is refused.
        let a_problem_leaking_across_splits_is_refused = {
            let split = TrainingSplit {
                training: vec![case("x", "p1")],
                validation: vec![case("y", "p1")],
                test: Vec::new(),
            };
            verdict(&split).contains("problem instance leaks across training splits")
        };
        // 6) 同一集合内同一问题重复被接受。 
        // A repeated problem inside one split is accepted.
        let a_repeated_problem_inside_one_split_is_accepted = {
            let split = training_only(vec![case("x", "p1"), case("y", "p1")]);
            verdict(&split).is_empty()
        };
        // 7) 问题相等是结构相等：只差 version 或只差一个参数都算不同问题。 
        // The problem equality is structural: a differing version or a single differing parameter is a different problem.
        let the_problem_equality_is_structural = {
            let versioned = TrainingSplit {
                training: vec![TrainingCase { id: "x".to_owned(),
                    problem: ComponentSpec::new("p1") }],
                validation: vec![TrainingCase { id: "y".to_owned(),
                    problem: ComponentSpec { version: 2, ..ComponentSpec::new("p1") } }],
                test: Vec::new(),
            };
            let parameterized = TrainingSplit {
                training: vec![TrainingCase { id: "x".to_owned(),
                    problem: ComponentSpec::new("p1") }],
                validation: vec![TrainingCase { id: "y".to_owned(),
                    problem: ComponentSpec::new("p1").parameter("seed", 7_i64) }],
                test: Vec::new(),
            };
            // 两种"只差一点"都不构成泄漏。 / Neither near-miss counts as a leak.
            verdict(&versioned).is_empty() && verdict(&parameterized).is_empty()
                // 而完全相同的定义一定泄漏。 / An identical definition definitely leaks.
                && !verdict(&TrainingSplit {
                    training: vec![case("x", "p1")],
                    validation: vec![case("y", "p1")],
                    test: Vec::new(),
                }).is_empty()
        };
        // 8) ID 判重先于泄漏判定。 / The id check fires before the leak check.
        let the_id_check_fires_before_the_leak_check = {
            // 同一个 id、同一个问题跨集合：两条规则都该触发，先报的是 ID。 
            // The same id and the same problem across splits trip both rules; the ID one reports first.
            let both = TrainingSplit {
                training: vec![case("x", "p1")],
                validation: vec![case("x", "p1")],
                test: Vec::new(),
            };
            let text = verdict(&both);
            text.contains("duplicate or empty training instance ID")
                && !text.contains("leaks across")
        };
        Findings {
            an_empty_id_is_refused,
            a_repeated_id_inside_one_split_is_refused,
            a_repeated_id_across_splits_is_refused,
            an_empty_training_split_is_refused,
            a_problem_leaking_across_splits_is_refused,
            a_repeated_problem_inside_one_split_is_accepted,
            the_problem_equality_is_structural,
            the_id_check_fires_before_the_leak_check,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: TrainingSplitProbeRequest) -> TrainingSplitProbeReport {
        verify_training_split_probe(&request).unwrap()
    }

    fn spec(id: &str, problem: &str) -> TrainingCaseSpec {
        TrainingCaseSpec { id: id.to_owned(), problem: problem.to_owned(), version: None,
            parameters: Vec::new() }
    }

    fn request() -> TrainingSplitProbeRequest {
        TrainingSplitProbeRequest { training: vec![spec("x", "p1")],
            validation: vec![spec("y", "p2")], test: Vec::new() }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request());
        assert!(report.valid, "{}", report.error);
        assert_eq!(report.error, "");
        assert_eq!(report.instances, 2);
        assert!(report.an_empty_id_is_refused);
        assert!(report.a_repeated_id_inside_one_split_is_refused);
        assert!(report.a_repeated_id_across_splits_is_refused);
        assert!(report.an_empty_training_split_is_refused);
        assert!(report.a_problem_leaking_across_splits_is_refused);
        assert!(report.a_repeated_problem_inside_one_split_is_accepted);
        assert!(report.the_problem_equality_is_structural);
        assert!(report.the_id_check_fires_before_the_leak_check);
    }

    #[test]
    fn the_id_rules_report_their_own_message() {
        let blank = ask(TrainingSplitProbeRequest { training: vec![spec("", "p1")],
            ..request() });
        assert!(!blank.valid);
        assert!(blank.error.contains("duplicate or empty training instance ID"), "{}", blank.error);
        let doubled = ask(TrainingSplitProbeRequest {
            training: vec![spec("x", "p1"), spec("x", "p2")], ..request() });
        assert!(doubled.error.contains("duplicate or empty training instance ID"));
        // 跨集合重复 ID 同样报 ID 那条。 / A cross-split repeat reports the id message too.
        let crossed = ask(TrainingSplitProbeRequest {
            training: vec![spec("x", "p1")], validation: vec![spec("x", "p2")],
            test: Vec::new() });
        assert!(crossed.error.contains("duplicate or empty training instance ID"));
    }

    #[test]
    fn an_empty_training_split_is_refused_but_empty_validation_is_fine() {
        let empty = ask(TrainingSplitProbeRequest { training: Vec::new(),
            validation: vec![spec("y", "p2")], test: Vec::new() });
        assert!(!empty.valid);
        assert!(empty.error.contains("training instances are required"), "{}", empty.error);
        let no_validation = ask(TrainingSplitProbeRequest { training: vec![spec("x", "p1")],
            validation: Vec::new(), test: Vec::new() });
        assert!(no_validation.valid, "{}", no_validation.error);
    }

    #[test]
    fn the_leak_rule_uses_structural_equality() {
        // 相同定义跨集合 → 泄漏。 / An identical definition across splits leaks.
        let leak = ask(TrainingSplitProbeRequest { training: vec![spec("x", "p1")],
            validation: vec![spec("y", "p1")], test: Vec::new() });
        assert!(!leak.valid);
        assert!(leak.error.contains("leaks across training splits"), "{}", leak.error);
        // 只差版本 → 不同问题，不算泄漏。 / Only the version differs, so it is a different problem.
        let versioned = ask(TrainingSplitProbeRequest {
            training: vec![spec("x", "p1")],
            validation: vec![TrainingCaseSpec { version: Some(2), ..spec("y", "p1") }],
            test: Vec::new() });
        assert!(versioned.valid, "{}", versioned.error);
        // 只差一个参数 → 也不算泄漏。 / One differing parameter is not a leak either.
        let parameterized = ask(TrainingSplitProbeRequest {
            training: vec![spec("x", "p1")],
            validation: vec![TrainingCaseSpec {
                parameters: vec![TrainingParameterSpec { key: "seed".to_owned(),
                    value: "7".to_owned() }], ..spec("y", "p1") }],
            test: Vec::new() });
        assert!(parameterized.valid, "{}", parameterized.error);
        // 同一集合内同一问题重复 → 接受。 / A repeated problem inside one split is accepted.
        let twice = ask(TrainingSplitProbeRequest {
            training: vec![spec("x", "p1"), spec("z", "p1")], validation: Vec::new(),
            test: Vec::new() });
        assert!(twice.valid, "{}", twice.error);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_training_split_probe(&TrainingSplitProbeRequest {
            training: vec![spec("x", "  ")], ..request() }).unwrap_err()
            .message.contains("must not be blank"));
        assert!(verify_training_split_probe(&TrainingSplitProbeRequest {
            training: (0..129).map(|index| spec(&format!("i{index}"), "p1")).collect(),
            ..request() }).unwrap_err().message.contains("one hundred and twenty-eight"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"instances\""), "{json}");
        assert!(json.contains("\"theProblemEqualityIsStructural\""), "{json}");
        assert!(!json.contains("an_empty_id"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<TrainingSplitProbeReport>(&copy.to_string()).is_err());
    }
}
