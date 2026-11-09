//! 续挖第 3 单：谓词字段路径解析与 schema 映射（`persistence/expression`）
//! Continuation, delivery 3: predicate field-path resolution and schema mapping.
//!
//! `persistence/expression/` 里有 9 个模块、19 个未引用类型。逐个看下来，多数是 DSL 构建器与 trait
//! （`BooleanExpressionScope`、`ScalarFunctionDsl`、`RepositoryApi`、`ExpressionRepository`）——构造表达式树、跑查询，
//! 属真边界。**可数据化且真有语义的是这两处**：
//!
//! * `PropertyPath`（`ospf-rust-math`）的**路径词汇与两条不同的解析入口**；
//! * `PredicateSchema` / `FieldMapping` 的**字段映射**，以及 `PersistenceFieldResolution` 的四种结论。
//!
//! 两个必须点明的语义（看签名看不出来）：
//!
//! 1. **`parse` 只裁空白，`try_parse` 才校验标识符**。所以 DSL 里的 `FieldPath::parse("a b")` 是**接受**的，
//!    但它并不是合法标识符路径——报告用 `trimmed` 与 `validIdentifiers` 两个字段把这条区别摊开。`a..b`（空分段）
//!    与 `""` 同理。
//! 2. **`PersistenceFieldResolution` 的四个变体里，`InvalidConfiguration` 从静态映射到达不了**：这个变体留给
//!    运行期配置了坏 resolver 的实现者。本入口只产生 `Resolved` / `Missing` / `Ambiguous` 三种，报告里的
//!    `reachableResolutions` 把这条如实写出，不假装四种都能出现。
//!
//! 同一路径映射到多个**不同**后端字段时判 `Ambiguous` 并列出候选（排序后）；重复映射到**同一个**字段不算歧义。
//!
//! The `persistence/expression/` tree holds nineteen unreferenced types; most are DSL builders and traits
//! (`BooleanExpressionScope`, `ScalarFunctionDsl`, `RepositoryApi`, `ExpressionRepository`) — they construct expression trees
//! and run queries, a true boundary. Two things are both datafiable and semantically real: the **path vocabulary** of
//! `PropertyPath` (which has **two different parse entry points**) and the **field mapping** of `PredicateSchema` /
//! `FieldMapping` behind `PersistenceFieldResolution`. Two semantics must be stated: **`parse` only trims whitespace while
//! `try_parse` validates identifiers**, so the DSL's `FieldPath::parse("a b")` is **accepted** yet not a valid identifier
//! path — the report exposes both as `trimmed` and `validIdentifiers` (`a..b` and `""` behave the same way); and of the four
//! `PersistenceFieldResolution` variants, **`InvalidConfiguration` is unreachable from a static mapping** since it exists for
//! implementers whose resolver carries broken runtime configuration, so the report lists the three reachable ones in
//! `reachableResolutions` rather than pretending all four can occur. Mapping one path to several **different** backend fields
//! yields `Ambiguous` with sorted candidates; repeating the **same** backend field is not ambiguous.

use ospf_rust_framework::persistence::expression::{
    DiagnosticPersistenceFieldResolver, PersistenceFieldResolution, PredicateSchema,
};
use ospf_rust_math::symbol::PropertyPath;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条字段映射。 / One field mapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateFieldMapping {
    /// 领域属性路径（照 DSL 的 `parse` 语义只裁空白）。 / The domain property path (trimmed only, per the DSL's `parse`).
    pub path: String,
    /// 后端字段表示。 / The backend field representation.
    pub backend_field: String,
}

/// 一条探测路径的解析结论。 / The verdict for one probed path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateFieldProbeReport {
    /// 输入原文。 / The input text.
    pub path: String,
    /// `parse` 的结果（只裁空白）。 / The result of `parse` (whitespace only).
    pub trimmed: String,
    /// `try_parse` 是否接受（逐段标识符校验）。 / Whether `try_parse` accepts it (per-segment identifier check).
    pub valid_identifiers: bool,
    /// 路径分段。 / The path segments.
    pub segments: Vec<String>,
    /// 路径深度。 / The path depth.
    pub depth: u32,
    /// 根分段。 / The root segment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub root: Option<String>,
    /// 叶分段。 / The leaf segment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub leaf: Option<String>,
    /// 解析结论：`resolved` / `missing` / `ambiguous`。 / The resolution: `resolved` / `missing` / `ambiguous`.
    pub resolution: String,
    /// 解析到的后端字段（`resolved` 时有）。 / The resolved backend field (`resolved` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub backend_field: Option<String>,
    /// 候选后端字段（`ambiguous` 时按字典序）。 / The candidate backend fields (sorted, `ambiguous` only).
    #[serde(default)]
    pub candidates: Vec<String>,
}

/// 谓词字段解析请求。 / The predicate field-resolution request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateFieldResolutionRequest {
    /// schema 名称（不能空白）。 / The schema name (must not be blank).
    pub schema_name: String,
    /// 字段映射。 / The field mappings.
    #[serde(default)]
    pub mappings: Vec<PredicateFieldMapping>,
    /// 要探测的路径（原文照收，不用先自己 parse）。 / The paths to probe.
    #[serde(default)]
    pub probes: Vec<String>,
}

/// 谓词字段解析结论。 / The predicate field-resolution verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateFieldResolutionReport {
    /// schema 名称。 / The schema name.
    pub schema_name: String,
    /// 映射条数。 / The number of mappings.
    pub mapping_count: u32,
    /// 逐条映射的规范化结果（`path` 为 `parse` 后的值，附可解析性）。 / Each mapping, normalized.
    pub mappings: Vec<PredicateFieldProbeReport>,
    /// 逐条探测的结论。 / Each probe's verdict.
    pub probes: Vec<PredicateFieldProbeReport>,
    /// 映射里**同一路径指向多个不同字段**的那些路径（桥接级一致性检查）。 / Paths mapped to several fields.
    pub ambiguous_paths: Vec<String>,
    /// 本入口能产生的解析结论（`InvalidConfiguration` 从静态映射到达不了）。 / The reachable resolutions.
    pub reachable_resolutions: Vec<String>,
}

/// 静态映射上的 `DiagnosticPersistenceFieldResolver`：不猜、不排序输入，只按 schema 的字段表查。
/// The `DiagnosticPersistenceFieldResolver` over a static mapping: no guessing, no reordering, just the schema's field table.
struct SchemaResolver<'a>(&'a [PredicateFieldMapping]);

impl DiagnosticPersistenceFieldResolver<String> for SchemaResolver<'_> {
    fn resolve_detailed(&self, path: &PropertyPath) -> PersistenceFieldResolution<String> {
        let mut candidates = self.0.iter()
            .filter(|mapping| mapping.path.trim() == path.value())
            .map(|mapping| mapping.backend_field.clone())
            .collect::<Vec<_>>();
        candidates.sort();
        candidates.dedup();
        match candidates.len() {
            0 => PersistenceFieldResolution::Missing { path: path.value().to_owned() },
            1 => PersistenceFieldResolution::Resolved(candidates.remove(0)),
            _ => PersistenceFieldResolution::Ambiguous {
                path: path.value().to_owned(),
                candidates,
            },
        }
    }
}

/// schema 上限（信任边界）。 / The mapping cap (a trust boundary).
const MAX_MAPPINGS: usize = 4_096;
/// 探测上限。 / The probe cap.
const MAX_PROBES: usize = 4_096;

/// 解析 predicate schema 里的字段映射并探测路径。非法输入是**错误**。
/// Resolve the field mappings of a predicate schema and probe paths. Illegal input is an **error**.
pub fn verify_predicate_field_resolution(request: &PredicateFieldResolutionRequest)
    -> Result<PredicateFieldResolutionReport>
{
    if request.schema_name.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "predicate field resolution failed validation: the schema name must not be blank"));
    }
    if request.mappings.len() > MAX_MAPPINGS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "predicate field resolution failed validation: at most {MAX_MAPPINGS} mappings are accepted")));
    }
    if request.probes.len() > MAX_PROBES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "predicate field resolution failed validation: at most {MAX_PROBES} probes are accepted")));
    }
    for mapping in &request.mappings {
        if mapping.backend_field.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "predicate field resolution failed validation: backend fields must not be blank"));
        }
    }
    // 用**领域构建器**堆出 schema，映射表从它读回来（不是自己再造一份）。
    // Build the schema with the **domain builder** and read the mapping table back from it.
    let mut schema = PredicateSchema::<String>::new(request.schema_name.trim());
    for mapping in &request.mappings {
        schema = schema.with_field(PropertyPath::parse(&mapping.path), mapping.backend_field.clone());
    }
    let fields = schema.fields().iter()
        .map(|field| PredicateFieldMapping { path: field.path.value().to_owned(),
            backend_field: field.backend_field.clone() })
        .collect::<Vec<_>>();
    let resolver = SchemaResolver(&fields);
    let describe = |path_text: &str| describe_path(path_text, &resolver);
    let mappings = fields.iter().map(|field| describe(&field.path)).collect::<Vec<_>>();
    let probes = request.probes.iter().map(|probe| describe(probe)).collect::<Vec<_>>();
    let mut ambiguous_paths = fields.iter()
        .map(|field| field.path.clone())
        .filter(|path| fields.iter().filter(|field| &field.path == path)
            .map(|field| field.backend_field.as_str())
            .collect::<std::collections::BTreeSet<_>>().len() > 1)
        .collect::<Vec<_>>();
    ambiguous_paths.sort();
    ambiguous_paths.dedup();
    Ok(PredicateFieldResolutionReport {
        schema_name: schema.name().to_owned(),
        mapping_count: u32::try_from(fields.len()).unwrap_or(u32::MAX),
        mappings,
        probes,
        ambiguous_paths,
        reachable_resolutions: vec!["resolved".to_owned(), "missing".to_owned(),
            "ambiguous".to_owned()],
    })
}

/// 描述一条路径：词汇事实 + 解析结论。 / Describe one path: vocabulary facts plus the resolution.
fn describe_path(path_text: &str, resolver: &impl DiagnosticPersistenceFieldResolver<String>)
    -> PredicateFieldProbeReport
{
    let trimmed = PropertyPath::parse(path_text);
    let valid_identifiers = PropertyPath::try_parse(path_text).is_ok();
    let (resolution, backend_field, candidates) = match resolver.resolve_detailed(&trimmed) {
        PersistenceFieldResolution::Resolved(field) => ("resolved".to_owned(), Some(field), Vec::new()),
        PersistenceFieldResolution::Missing { path } => {
            debug_assert_eq!(path, trimmed.value());
            ("missing".to_owned(), None, Vec::new())
        }
        PersistenceFieldResolution::Ambiguous { candidates, .. } =>
            ("ambiguous".to_owned(), None, candidates),
        // 静态映射不产生这个变体；真出现了要**明说**而不是装成 `missing`。
        // A static mapping never produces this variant; if it ever appears, say so instead of posing as `missing`.
        PersistenceFieldResolution::InvalidConfiguration { reason } => {
            return PredicateFieldProbeReport { path: path_text.to_owned(),
                trimmed: trimmed.value().to_owned(), valid_identifiers, segments: Vec::new(),
                depth: 0, root: None, leaf: None, resolution: "invalidConfiguration".to_owned(),
                backend_field: None, candidates: vec![reason] };
        }
    };
    PredicateFieldProbeReport {
        path: path_text.to_owned(),
        trimmed: trimmed.value().to_owned(),
        valid_identifiers,
        segments: trimmed.segments().iter().map(|segment| (*segment).to_owned()).collect(),
        depth: u32::try_from(trimmed.depth()).unwrap_or(u32::MAX),
        root: trimmed.root().map(str::to_owned),
        leaf: trimmed.leaf().map(str::to_owned),
        resolution,
        backend_field,
        candidates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(path: &str, backend: &str) -> PredicateFieldMapping {
        PredicateFieldMapping { path: path.to_owned(), backend_field: backend.to_owned() }
    }

    /// 一份含**歧义路径**、**重复同字段**与**非标识符路径**的映射表。
    /// A mapping table with an **ambiguous path**, a **repeated identical field** and a **non-identifier path**.
    fn request() -> PredicateFieldResolutionRequest {
        PredicateFieldResolutionRequest {
            schema_name: "  orders  ".to_owned(),
            mappings: vec![
                mapping("id", "orders.id"),
                mapping("status", "orders.status"),
                mapping("status", "legacy.status"),
                mapping("name", "orders.name"),
                mapping("name", "orders.name"),
                mapping("a b", "orders.a_b"),
            ],
            probes: vec!["id".to_owned(), "status".to_owned(), " nope ".to_owned(),
                "a b".to_owned(), "a..b".to_owned(), String::new(), "a.b.c".to_owned()],
        }
    }

    #[test]
    fn path_vocabulary_separates_trimming_from_identifier_validation() {
        let report = verify_predicate_field_resolution(&request()).unwrap();
        // schema 名称走领域构建器，会被裁空白。 / The schema name goes through the domain builder and is trimmed.
        assert_eq!(report.schema_name, "orders");
        assert_eq!(report.mapping_count, 6);
        let probe = |path: &str| report.probes.iter().find(|item| item.path == path)
            .unwrap_or_else(|| panic!("probe {path}"));
        // 普通路径：解析到后端字段。 / An ordinary path resolves to its backend field.
        let id = probe("id");
        assert_eq!(id.trimmed, "id");
        assert!(id.valid_identifiers);
        assert_eq!(id.segments, vec!["id".to_owned()]);
        assert_eq!(id.depth, 1);
        assert_eq!(id.root.as_deref(), Some("id"));
        assert_eq!(id.leaf.as_deref(), Some("id"));
        assert_eq!(id.resolution, "resolved");
        assert_eq!(id.backend_field.as_deref(), Some("orders.id"));
        assert!(id.candidates.is_empty());
        // 同一路径两个不同后端字段 → 歧义，候选按字典序。 / Two different fields: ambiguous, candidates sorted.
        let status = probe("status");
        assert_eq!(status.resolution, "ambiguous");
        assert_eq!(status.backend_field, None);
        assert_eq!(status.candidates, vec!["legacy.status".to_owned(), "orders.status".to_owned()]);
        // 空白被裁掉，剩余部分查表；查不到 → missing。 / Whitespace is trimmed before the lookup; no match is missing.
        let nope = probe(" nope ");
        assert_eq!(nope.trimmed, "nope");
        assert_eq!(nope.resolution, "missing");
        assert_eq!(nope.backend_field, None);
        // **关键区别**：`a b` 被 `parse` 接受（只裁空白），但不是合法标识符路径。
        // **The key difference**: `a b` is accepted by `parse` (trim only) yet is not a valid identifier path.
        let spaced = probe("a b");
        assert_eq!(spaced.trimmed, "a b");
        assert!(!spaced.valid_identifiers);
        assert_eq!(spaced.resolution, "resolved");
        assert_eq!(spaced.backend_field.as_deref(), Some("orders.a_b"));
        // 空分段与空路径同样"接受但不合法"。 / An empty segment and an empty path are likewise accepted but invalid.
        let empty_segment = probe("a..b");
        assert_eq!(empty_segment.trimmed, "a..b");
        assert!(!empty_segment.valid_identifiers);
        assert_eq!(empty_segment.segments, vec!["a".to_owned(), String::new(), "b".to_owned()]);
        assert_eq!(empty_segment.depth, 3);
        assert_eq!(empty_segment.resolution, "missing");
        let empty = probe("");
        assert_eq!(empty.trimmed, "");
        assert!(!empty.valid_identifiers);
        assert!(empty.segments.is_empty());
        assert_eq!(empty.depth, 0);
        assert_eq!(empty.root, None);
        assert_eq!(empty.leaf, None);
        assert_eq!(empty.resolution, "missing");
        // 深路径的根/叶。 / Root and leaf of a deep path.
        let deep = probe("a.b.c");
        assert_eq!(deep.segments, vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        assert_eq!(deep.depth, 3);
        assert_eq!(deep.root.as_deref(), Some("a"));
        assert_eq!(deep.leaf.as_deref(), Some("c"));
    }

    #[test]
    fn repeated_identical_mappings_are_not_ambiguous_but_different_ones_are() {
        let report = verify_predicate_field_resolution(&request()).unwrap();
        // `name` 映射了两次但**同一个**后端字段：不是歧义。 / `name` repeats the **same** field: not ambiguous.
        let name = report.mappings.iter().find(|item| item.trimmed == "name").unwrap();
        assert_eq!(name.resolution, "resolved");
        assert_eq!(name.backend_field.as_deref(), Some("orders.name"));
        // 只有 `status` 是真歧义路径。 / Only `status` is a genuinely ambiguous path.
        assert_eq!(report.ambiguous_paths, vec!["status".to_owned()]);
        // `InvalidConfiguration` 不可达，如实只列三种。 / `InvalidConfiguration` is unreachable, so only three are listed.
        assert_eq!(report.reachable_resolutions, vec!["resolved".to_owned(), "missing".to_owned(),
            "ambiguous".to_owned()]);
        // 逐条映射也带词汇事实。 / Each mapping carries its vocabulary facts too.
        assert!(report.mappings.iter().all(|item| !item.path.is_empty()));
        assert_eq!(report.mappings.len(), report.mapping_count as usize);
    }

    #[test]
    fn blank_names_and_caps_are_refused() {
        let mut blank_schema = request();
        blank_schema.schema_name = "   ".to_owned();
        assert!(verify_predicate_field_resolution(&blank_schema).unwrap_err().message
            .contains("schema name must not be blank"));
        let mut blank_field = request();
        blank_field.mappings = vec![mapping("id", "  ")];
        assert!(verify_predicate_field_resolution(&blank_field).unwrap_err().message
            .contains("backend fields must not be blank"));
        let mut too_many = request();
        too_many.mappings = (0..=MAX_MAPPINGS).map(|index| mapping(&format!("f{index}"), "c")).collect();
        assert!(verify_predicate_field_resolution(&too_many).unwrap_err().message
            .contains("at most"));
        let mut too_many_probes = request();
        too_many_probes.probes = (0..=MAX_PROBES).map(|index| format!("p{index}")).collect();
        assert!(verify_predicate_field_resolution(&too_many_probes).unwrap_err().message
            .contains("at most"));
        // 空映射 + 空探针：合法，全空。 / An empty mapping and no probes: legal and empty.
        let report = verify_predicate_field_resolution(&PredicateFieldResolutionRequest {
            schema_name: "s".to_owned(), mappings: Vec::new(), probes: Vec::new() }).unwrap();
        assert_eq!(report.mapping_count, 0);
        assert!(report.mappings.is_empty());
        assert!(report.probes.is_empty());
        assert!(report.ambiguous_paths.is_empty());
    }
}
