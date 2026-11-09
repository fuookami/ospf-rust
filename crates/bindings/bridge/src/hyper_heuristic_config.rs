//! 批次 C 第 1 单：超启发式配置的三格式往返与结构校验 / Batch C, delivery 1: three-format config round trip and validation.
//!
//! `ConfigurationFormats::{decode_format,encode_format}` + `HyperHeuristicConfig::validate()` 是纯文本能力，不需要任何
//! 求解器或注册表：给定一份 JSON / YAML / TOML 配置文本，严格解码（拒绝重复键、拒绝未知字段、拒绝非有限数）、
//! 校验结构，再给出规范化 JSON。
//!
//! 结构校验规则（全部实测）：
//! * `version` 必须为 1；算法列表非空；算法 `id` 非空、首字符字母数字、其余仅 `[A-Za-z0-9._-]`、且**全局唯一**
//!   （含子会话）；`size > 0`；
//! * `children` 与 `constraints` **互斥**（有子会话就不能再声明叶会话的约束策略）；
//! * 预算必须**至少有一个**全局停止上限（`evaluations`/`decisions`/`steps`/`timeMillis`），且片段额度必须为正；
//! * `parallelism`、`archiveCapacity`、`workingCapacity` 必须为正；
//! * `everyDecisions > 0` 时**必须**启用持久化**并**给出 `checkpointPath`。
//!
//! `ConfigurationFormats` plus `HyperHeuristicConfig::validate()` are pure text capabilities needing neither a solver nor a
//! registry: a JSON / YAML / TOML config text is strictly decoded (duplicate keys, unknown fields and non-finite numbers are
//! refused), structurally validated, and normalized to JSON. The structural rules are all measured: version must be 1 with a
//! non-empty algorithm list; ids must be non-empty, start alphanumeric, use only `[A-Za-z0-9._-]` and be globally unique
//! (children included); `size` must be positive; `children` and `constraints` are **mutually exclusive**; the budget needs
//! **at least one** global stop limit with positive slice allowances; parallelism and both capacities must be positive; and
//! `everyDecisions > 0` **requires** persistence enabled **and** a checkpoint path.

use ospf_rust_core_plugin_heuristic::hyper_heuristic::config::{
    AlgorithmConfig, BudgetConfig, ConfigurationFormat, ConfigurationFormats, ExecutionConfig,
    HyperHeuristicConfig, LearningConfig, MemoryConfig, PersistenceConfig,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 配置文本请求。 / The configuration-text request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HyperHeuristicConfigRequest {
    /// 配置文本。 / The configuration text.
    pub text: String,
    /// 文本格式：`json` / `yaml` / `toml`。 / The text format.
    #[serde(default = "default_format")]
    pub format: String,
}

/// 默认格式。 / The default format.
fn default_format() -> String { "json".to_owned() }

/// 一个算法实例的投影。 / One algorithm instance projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlgorithmRow {
    /// 稳定实例标识。 / The stable instance identifier.
    pub id: String,
    /// 原生算法引用。 / The native algorithm reference.
    pub algorithm: String,
    /// 种群规模。 / The population size.
    pub size: u32,
    /// 更新/变异/邻域算子池条数。 / The update, mutation and neighborhood pool sizes.
    pub update_count: u32,
    pub mutation_count: u32,
    pub neighborhood_count: u32,
    /// 参数源池的键数。 / The number of parameter-source keys.
    pub parameter_pool_count: u32,
    /// 槽位选择策略。 / The slot selection policy.
    pub selection: String,
    /// 子会话数（有子会话时本实例是组合子会话）。 / The child count (non-zero means a composite session).
    pub child_count: u32,
    /// 是否声明了叶会话约束策略。 / Whether a leaf constraint policy is declared.
    pub has_constraints: bool,
    /// 初始种子来源。 / The initial seed source.
    pub initialization: String,
    /// 导入模式。 / The import mode.
    pub import_mode: String,
}

/// 预算投影。 / The budget projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BudgetRow {
    /// 全局评估上限。 / The global evaluation limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub evaluations: Option<u64>,
    /// 全局决策上限。 / The global decision limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub decisions: Option<u64>,
    /// 全局步数上限。 / The global step limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub steps: Option<u64>,
    /// 全局时间上限（毫秒）。 / The global time limit in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub time_millis: Option<u64>,
    /// 单片段评估额度。 / The per-slice evaluation allowance.
    pub slice_evaluations: u64,
    /// 单片段步数额度。 / The per-slice step allowance.
    pub slice_steps: u64,
    /// 预算分配器。 / The budget allocator.
    pub allocator: String,
    /// 有几个全局停止上限（`>= 1` 才合法）。 / How many global stop limits exist (at least one is required).
    pub global_limit_count: u32,
}

/// 执行投影。 / The execution projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionRow {
    /// 执行模式（`Serial`/`Synchronous`/`Asynchronous`）。 / The execution mode.
    pub mode: String,
    /// 最大并发会话数。 / The maximum concurrent sessions.
    pub parallelism: u32,
    /// 根随机种子。 / The root random seed.
    pub seed: i64,
}

/// 记忆投影。 / The memory projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryRow {
    /// 有效档案容量。 / The valid archive capacity.
    pub archive_capacity: u32,
    /// 已接受工作集容量。 / The accepted working-set capacity.
    pub working_capacity: u32,
    /// 接受策略。 / The acceptance policy.
    pub acceptance: String,
    /// 交换策略。 / The exchange policy.
    pub exchange: String,
}

/// 学习投影。 / The learning projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearningRow {
    /// 学习模式（`Train`/`Continue`/`Frozen`）。 / The learning mode.
    pub mode: String,
    /// 特征提取器。 / The feature extractor.
    pub features: String,
    /// 奖励函数。 / The reward policy.
    pub reward: String,
    /// 是否给出轨迹输出位置。 / Whether a trajectory destination is given.
    pub has_trajectory_path: bool,
}

/// 持久化投影。 / The persistence projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistenceRow {
    /// 是否启用。 / Whether it is enabled.
    pub enabled: bool,
    /// 是否给出自动检查点位置。 / Whether an automatic checkpoint destination is given.
    pub has_checkpoint_path: bool,
    /// 自动保存间隔（零为手动）。 / The automatic interval (zero for manual).
    pub every_decisions: u64,
}

/// 配置校验结论。 / The configuration verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HyperHeuristicConfigReport {
    /// 配置版本。 / The configuration version.
    pub version: u32,
    /// 问题工厂引用。 / The problem-factory reference.
    pub problem: String,
    /// 顶层算法选择策略。 / The top-level algorithm selection policy.
    pub selection: String,
    /// 算法实例数（含子会话按递归计）。 / The algorithm-instance count (children counted recursively).
    pub algorithm_count: u32,
    /// 顶层算法投影。 / The top-level algorithm projections.
    pub algorithms: Vec<AlgorithmRow>,
    /// 预算投影。 / The budget projection.
    pub budget: BudgetRow,
    /// 执行投影。 / The execution projection.
    pub execution: ExecutionRow,
    /// 记忆投影。 / The memory projection.
    pub memory: MemoryRow,
    /// 学习投影。 / The learning projection.
    pub learning: LearningRow,
    /// 持久化投影。 / The persistence projection.
    pub persistence: PersistenceRow,
    /// 规范化 JSON（由领域自己的 `encode_format(Json)` 产出）。
    /// The normalized JSON, produced by the domain's own `encode_format(Json)`.
    pub canonical_json: String,
}

/// 解码、校验并规范化一份超启发式配置文本。非法输入是**错误**，原因原样带出。
/// Decode, validate and normalize a hyper-heuristic config text. An illegal input is an **error** carrying the reason
/// verbatim.
pub fn verify_hyper_heuristic_config(request: &HyperHeuristicConfigRequest) -> Result<HyperHeuristicConfigReport> {
    let format = match request.format.as_str() {
        "json" => ConfigurationFormat::Json,
        "yaml" => ConfigurationFormat::Yaml,
        "toml" => ConfigurationFormat::Toml,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "configuration failed validation: unsupported format `{other}` (expected json, yaml or toml)"))),
    };
    // 领域自己的入口：严格解码（重复键/未知字段/非有限数一律拒绝）+ 结构校验。
    // The domain's own entry: strict decoding (duplicate keys, unknown fields and non-finite numbers are all refused) plus
    // structural validation.
    let config = ConfigurationFormats::decode_format(&request.text, format).map_err(|error| {
        BridgeError::new("InvalidInput", &format!("configuration failed validation: {error}"))
    })?;
    // 规范化输出也走领域：这里不重新实现 pretty-print 或字段顺序。
    // The normalized output also comes from the domain: no re-implementation of pretty-printing or field order here.
    let canonical_json = ConfigurationFormats::encode_format(&config, ConfigurationFormat::Json).map_err(|error| {
        BridgeError::new("CodecFailure", &format!("configuration could not be encoded: {error}"))
    })?;
    Ok(HyperHeuristicConfigReport {
        version: config.version,
        problem: config.problem.r#type.clone(),
        selection: config.selection.r#type.clone(),
        algorithm_count: count_algorithms(&config.algorithms),
        algorithms: config.algorithms.iter().map(algorithm_row).collect(),
        budget: BudgetRow {
            evaluations: config.budget.evaluations,
            decisions: config.budget.decisions,
            steps: config.budget.steps,
            time_millis: config.budget.time_millis,
            slice_evaluations: config.budget.slice_evaluations,
            slice_steps: config.budget.slice_steps,
            allocator: config.budget.allocator.r#type.clone(),
            global_limit_count: u32::try_from([
                config.budget.evaluations, config.budget.decisions,
                config.budget.steps, config.budget.time_millis,
            ].iter().filter(|limit| limit.is_some()).count()).unwrap_or(u32::MAX),
        },
        execution: ExecutionRow {
            mode: serde_name(config.execution.mode),
            parallelism: u32::try_from(config.execution.parallelism).unwrap_or(u32::MAX),
            seed: config.execution.seed,
        },
        memory: MemoryRow {
            archive_capacity: u32::try_from(config.memory.archive_capacity).unwrap_or(u32::MAX),
            working_capacity: u32::try_from(config.memory.working_capacity).unwrap_or(u32::MAX),
            acceptance: config.memory.acceptance.r#type.clone(),
            exchange: config.memory.exchange.r#type.clone(),
        },
        learning: LearningRow {
            mode: serde_name(config.learning.mode),
            features: config.learning.features.r#type.clone(),
            reward: config.learning.reward.r#type.clone(),
            has_trajectory_path: config.learning.trajectory_path.is_some(),
        },
        persistence: PersistenceRow {
            enabled: config.persistence.enabled,
            has_checkpoint_path: config.persistence.checkpoint_path.is_some(),
            every_decisions: config.persistence.every_decisions,
        },
        canonical_json,
    })
}

/// 递归统计算法实例数（含子会话）。 / Count algorithm instances recursively, children included.
fn count_algorithms(algorithms: &[AlgorithmConfig]) -> u32 {
    algorithms.iter().fold(0u32, |total, algorithm| {
        total
            .saturating_add(1)
            .saturating_add(count_algorithms(&algorithm.children))
    })
}

/// 把一个算法实例投影成扁平行。 / Project one algorithm instance into a flat row.
fn algorithm_row(algorithm: &AlgorithmConfig) -> AlgorithmRow {
    AlgorithmRow {
        id: algorithm.id.clone(),
        algorithm: algorithm.algorithm.r#type.clone(),
        size: u32::try_from(algorithm.size).unwrap_or(u32::MAX),
        update_count: u32::try_from(algorithm.updates.len()).unwrap_or(u32::MAX),
        mutation_count: u32::try_from(algorithm.mutations.len()).unwrap_or(u32::MAX),
        neighborhood_count: u32::try_from(algorithm.neighborhoods.len()).unwrap_or(u32::MAX),
        parameter_pool_count: u32::try_from(algorithm.parameters.len()).unwrap_or(u32::MAX),
        selection: algorithm.selection.r#type.clone(),
        child_count: u32::try_from(algorithm.children.len()).unwrap_or(u32::MAX),
        has_constraints: algorithm.constraints.is_some(),
        // 这四个枚举没有 `rename_all`，所以线格式就是变体名本身——用 serde 取值，而不是猜字符串。
        // These four enums carry no `rename_all`, so the wire form is the variant name itself; read it through serde
        // rather than guessing a string.
        initialization: serde_name(algorithm.initialization),
        import_mode: serde_name(algorithm.import_mode),
    }
}

/// 取一个枚举的 serde 表示。 / Read an enum's serde representation.
fn serde_name<T: Serialize>(value: T) -> String {
    serde_json::to_value(value).ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一份最小的合法 JSON 配置文本（手写文本在这里是**被测对象**，不是载荷）。
    /// A minimal legal JSON config text (the hand-written text is the **subject under test** here, not a payload).
    const MINIMAL: &str = r#"{
        "problem": {"type": "bpp", "version": 1},
        "algorithms": [{"id": "a1", "algorithm": {"type": "simulated-annealing", "version": 1}}],
        "budget": {"evaluations": 1000}
    }"#;

    fn request(text: &str, format: &str) -> HyperHeuristicConfigRequest {
        HyperHeuristicConfigRequest { text: text.to_owned(), format: format.to_owned() }
    }

    #[test]
    fn minimal_config_is_normalized_with_domain_defaults() {
        let report = verify_hyper_heuristic_config(&request(MINIMAL, "json")).unwrap();
        assert_eq!(report.version, 1);
        assert_eq!(report.problem, "bpp");
        assert_eq!(report.selection, "round-robin");
        assert_eq!(report.algorithm_count, 1);
        assert_eq!(report.algorithms[0].id, "a1");
        assert_eq!(report.algorithms[0].algorithm, "simulated-annealing");
        // 默认种群规模 20、默认选择策略 round-robin、默认初始化 `Model`、默认导入 `Native`。
        // Default population 20, default selection round-robin, default initialization `Model` and default import `Native`.
        assert_eq!(report.algorithms[0].size, 20);
        assert_eq!(report.algorithms[0].selection, "round-robin");
        assert_eq!(report.algorithms[0].initialization, "Model");
        assert_eq!(report.algorithms[0].import_mode, "Native");
        assert_eq!(report.algorithms[0].child_count, 0);
        assert!(!report.algorithms[0].has_constraints);
        // 预算默认值：片段额度 256 / 1，分配器 `fixed`，只有一个全局上限。
        // Budget defaults: slice allowances 256 / 1, allocator `fixed` and exactly one global limit.
        assert_eq!(report.budget.evaluations, Some(1000));
        assert_eq!(report.budget.slice_evaluations, 256);
        assert_eq!(report.budget.slice_steps, 1);
        assert_eq!(report.budget.allocator, "fixed");
        assert_eq!(report.budget.global_limit_count, 1);
        // 执行 / 记忆 / 学习 / 持久化的默认值。
        // Execution, memory, learning and persistence defaults.
        assert_eq!(report.execution.mode, "Synchronous");
        assert_eq!(report.execution.parallelism, 1);
        assert_eq!(report.execution.seed, 1);
        assert_eq!(report.memory.archive_capacity, 100);
        assert_eq!(report.memory.working_capacity, 20);
        assert_eq!(report.memory.acceptance, "not-worse");
        assert_eq!(report.memory.exchange, "none");
        assert_eq!(report.learning.mode, "Train");
        assert_eq!(report.learning.features, "progress");
        assert_eq!(report.learning.reward, "improvement");
        assert!(!report.learning.has_trajectory_path);
        assert!(!report.persistence.enabled);
        assert!(!report.persistence.has_checkpoint_path);
        assert_eq!(report.persistence.every_decisions, 0);

        // 规范化 JSON 必须能被再次解码（往返稳定），并且与摘要一致。
        // The normalized JSON must decode again (stable round trip) and agree with the summary.
        let again = verify_hyper_heuristic_config(&request(&report.canonical_json, "json")).unwrap();
        assert_eq!(again.canonical_json, report.canonical_json, "normalization must be idempotent");
        assert_eq!(again.budget.evaluations, report.budget.evaluations);
    }

    #[test]
    fn yaml_and_toml_texts_decode_into_the_same_model() {
        let yaml = "problem:\n  type: bpp\n  version: 1\nalgorithms:\n  - id: a1\n    algorithm:\n      type: simulated-annealing\nbudget:\n  evaluations: 1000\n";
        let report = verify_hyper_heuristic_config(&request(yaml, "yaml")).unwrap();
        assert_eq!(report.problem, "bpp");
        assert_eq!(report.algorithms[0].id, "a1");

        let toml = "problem = { type = \"bpp\", version = 1 }\n[[algorithms]]\nid = \"a1\"\nalgorithm = { type = \"simulated-annealing\" }\n[budget]\nevaluations = 1000\n";
        let report = verify_hyper_heuristic_config(&request(toml, "toml")).unwrap();
        assert_eq!(report.problem, "bpp");
        // 三种格式解码到同一个模型：规范化 JSON 必须逐字相同。
        // All three formats decode into the same model, so the normalized JSON must be byte-identical.
        let json_report = verify_hyper_heuristic_config(&request(MINIMAL, "json")).unwrap();
        assert_eq!(report.canonical_json, json_report.canonical_json);
    }

    #[test]
    fn structural_rules_are_enforced_with_the_domain_reason() {
        // 没有全局停止上限：拒绝。 / No global stop limit: refused.
        let no_limit = r#"{"problem":{"type":"bpp"},"algorithms":[{"id":"a1","algorithm":{"type":"sa"}}],
            "budget":{"sliceEvaluations":256,"sliceSteps":1}}"#;
        let error = verify_hyper_heuristic_config(&request(no_limit, "json")).unwrap_err();
        assert_eq!(error.code, "InvalidInput");
        assert!(error.message.contains("global stopping limit"), "{}", error.message);

        // 算法 ID 以非字母数字开头：拒绝。 / An algorithm id starting with a non-alphanumeric: refused.
        let bad_id = r#"{"problem":{"type":"bpp"},"algorithms":[{"id":"_a1","algorithm":{"type":"sa"}}],
            "budget":{"evaluations":10}}"#;
        let error = verify_hyper_heuristic_config(&request(bad_id, "json")).unwrap_err();
        assert!(error.message.contains("invalid configuration version or algorithms"), "{}", error.message);

        // 重复算法 ID：拒绝。 / A duplicate algorithm id: refused.
        let duplicate = r#"{"problem":{"type":"bpp"},"algorithms":[
            {"id":"a1","algorithm":{"type":"sa"}},{"id":"a1","algorithm":{"type":"tabu"}}],
            "budget":{"evaluations":10}}"#;
        let error = verify_hyper_heuristic_config(&request(duplicate, "json")).unwrap_err();
        assert!(error.message.contains("invalid configuration version or algorithms"), "{}", error.message);

        // 并行度为 0：拒绝。 / Zero parallelism: refused.
        let zero_parallel = r#"{"problem":{"type":"bpp"},"algorithms":[{"id":"a1","algorithm":{"type":"sa"}}],
            "budget":{"evaluations":10},"execution":{"parallelism":0}}"#;
        let error = verify_hyper_heuristic_config(&request(zero_parallel, "json")).unwrap_err();
        assert!(error.message.contains("capacities and parallelism must be positive"), "{}", error.message);

        // 自动检查点但没有启用持久化：拒绝。 / Automatic checkpoints without persistence: refused.
        let no_persistence = r#"{"problem":{"type":"bpp"},"algorithms":[{"id":"a1","algorithm":{"type":"sa"}}],
            "budget":{"evaluations":10},"persistence":{"everyDecisions":5}}"#;
        let error = verify_hyper_heuristic_config(&request(no_persistence, "json")).unwrap_err();
        assert!(error.message.contains("automatic checkpoints require persistence"), "{}", error.message);

        // 启用了持久化但没有目标路径：同样拒绝。 / Persistence enabled without a destination: also refused.
        let no_path = r#"{"problem":{"type":"bpp"},"algorithms":[{"id":"a1","algorithm":{"type":"sa"}}],
            "budget":{"evaluations":10},"persistence":{"enabled":true,"everyDecisions":5}}"#;
        let error = verify_hyper_heuristic_config(&request(no_path, "json")).unwrap_err();
        assert!(error.message.contains("automatic checkpoints require persistence"), "{}", error.message);

        // 未知字段：严格解码拒绝（不静默接受）。 / An unknown field: strict decoding refuses it (never silently accepted).
        let unknown = r#"{"problem":{"type":"bpp"},"algorithms":[{"id":"a1","algorithm":{"type":"sa"}}],
            "budget":{"evaluations":10},"nonsense":1}"#;
        let error = verify_hyper_heuristic_config(&request(unknown, "json")).unwrap_err();
        assert!(error.message.contains("unknown field"), "{}", error.message);

        // 重复键：严格解码拒绝。 / A duplicate key: strict decoding refuses it.
        let duplicate_key = r#"{"problem":{"type":"bpp","type":"bpp"},
            "algorithms":[{"id":"a1","algorithm":{"type":"sa"}}],"budget":{"evaluations":10}}"#;
        let error = verify_hyper_heuristic_config(&request(duplicate_key, "json")).unwrap_err();
        assert!(error.message.contains("duplicate key"), "{}", error.message);

        // 未知格式 / 非 JSON / 超限。 / An unknown format, non-JSON and over the limit.
        assert_eq!(verify_hyper_heuristic_config(&request(MINIMAL, "xml")).unwrap_err().code, "InvalidInput");
        assert_eq!(verify_hyper_heuristic_config(&request("not json", "json")).unwrap_err().code, "InvalidInput");
        let huge = "x".repeat(5 * 1024 * 1024);
        assert_eq!(verify_hyper_heuristic_config(&request(&huge, "json")).unwrap_err().code, "InvalidInput");
    }

    #[test]
    fn composite_sessions_count_children_and_forbid_leaf_constraints() {
        let composite = r#"{"problem":{"type":"bpp"},"budget":{"evaluations":10},"algorithms":[
            {"id":"root","algorithm":{"type":"portfolio"},"children":[
                {"id":"leaf-a","algorithm":{"type":"sa"}},
                {"id":"leaf-b","algorithm":{"type":"tabu"}}]}]}"#;
        let report = verify_hyper_heuristic_config(&request(composite, "json")).unwrap();
        // 递归计数：根 + 两个子会话。 / Recursive count: the root plus two children.
        assert_eq!(report.algorithm_count, 3);
        assert_eq!(report.algorithms.len(), 1, "top-level rows only");
        assert_eq!(report.algorithms[0].child_count, 2);
        assert!(!report.algorithms[0].has_constraints);

        // 同时声明子会话与叶会话约束：拒绝（互斥）。
        // Declaring both children and leaf constraints: refused (mutually exclusive).
        let both = r#"{"problem":{"type":"bpp"},"budget":{"evaluations":10},"algorithms":[
            {"id":"root","algorithm":{"type":"portfolio"},"constraints":{},
             "children":[{"id":"leaf-a","algorithm":{"type":"sa"}}]}]}"#;
        let error = verify_hyper_heuristic_config(&request(both, "json")).unwrap_err();
        assert!(error.message.contains("invalid configuration version or algorithms"), "{}", error.message);
    }
}
