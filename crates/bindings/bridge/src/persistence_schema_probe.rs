//! 续挖第 113 单：**同一张谓词 schema 有两个互相矛盾的查表面**——`resolve()` 用 `find`，于是**同路径先到先得**；
//! `to_field_map()` 用 `HashMap` 收集，于是**同路径后到覆盖**：schema 里写两条同路径、不同后端名时，
//! 两个 API 给出**相反**的答案。另外 `with_same_field()` 取 `path.value()`（**最后一段**）当后端字段名，
//! `resolve()` 对未登记路径给 `None`、`resolver()` 与 `resolve(parse(..))` 完全同款，
//! 而 `PredicateSchema::field()` **根本不校验路径是否登记**
//! （`persistence/expression/predicate_schema.rs` 的 `PredicateSchema` / `FieldMapping` / `FieldPath`）
//! Continuation, delivery 113: **one predicate schema exposes two contradictory lookup faces** — `resolve()` uses `find`, so
//! **the first record of a path wins**, while `to_field_map()` collects into a `HashMap`, so **the last record wins**: with
//! two records of the same path but different backend names the two APIs answer **oppositely**. Besides that,
//! `with_same_field()` takes `path.value()` as the backend field name, which measures as the **whole path text**
//! rather than the last segment, `resolve()` returns `None` for
//! unregistered paths, `resolver()` behaves exactly like `resolve(parse(..))`, and `PredicateSchema::field()` **never checks
//! whether the path is registered** (`PredicateSchema` / `FieldMapping` / `FieldPath` in
//! `persistence/expression/predicate_schema.rs`).
//!
//! 第 112 单量的是排序描述，这一单量**谓词 schema 的映射与解析面**。八条实测：
//!
//! 1. **`with_field` 按顺序追加**（`the_fields_append_in_order()`）。
//! 2. **`resolve()` 先到先得**（`the_resolve_is_first_wins()`）。
//! 3. **`to_field_map()` 后到覆盖**（`the_field_map_is_last_wins()`）——两个面**互相矛盾**。
//! 4. **未登记路径一律 `None`**（`the_unknown_path_resolves_to_nothing()`），`resolver()` 同款。
//! 5. **`with_same_field` 用最后一段当后端名**（`the_same_field_mapping_repeats_the_whole_path()`）。
//! 6. **`field()` 不校验登记**（`the_field_builder_never_checks_registration()`）。
//! 7. **`FieldPath::parse` 与 `new` 等价**（`the_parse_matches_the_constructor()`）。
//! 8. **映射表长度是去重后的路径数**（`the_field_map_is_deduplicated_by_path()`）。
//!
//! Delivery 112 measured the sort descriptor; this one measures **the predicate schema's mapping and resolution faces**. **The
//! fields append in order.** **`resolve()` is first wins** while **`to_field_map()` is last wins**, so the two faces
//! contradict each other. **An unknown path resolves to nothing** and the resolver matches. **`with_same_field` repeats the whole path**
//! as the backend name. **`field()` never checks registration.** **`FieldPath::parse` matches the constructor.**
//! And **the field map is deduplicated by path.**

use ospf_rust_framework::persistence::expression::predicate_schema::{
    FieldMapping, FieldPath, PredicateSchema,
};
use ospf_rust_math::symbol::PropertyPath;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条字段映射的描述。 / The description of one field mapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistenceFieldSpec {
    /// 领域属性路径。 / The domain property path.
    pub path: String,
    /// 后端字段名（缺省走 `with_same_field`）。 / The backend field name; absent means `with_same_field`.
    #[serde(default)]
    pub backend_field: Option<String>,
}

/// 谓词 schema 探针请求。 / The predicate schema probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistenceSchemaRequest {
    /// schema 名称（默认 `probe`）。 / The schema name.
    #[serde(default)]
    pub name: Option<String>,
    /// 字段映射。 / The field mappings.
    #[serde(default)]
    pub fields: Vec<PersistenceFieldSpec>,
    /// 要解析的路径。 / The paths to resolve.
    #[serde(default)]
    pub queries: Vec<String>,
}

/// 一条查询的读数。 / One query reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaQueryRow {
    /// 查询路径。 / The queried path.
    pub path: String,
    /// `resolve()` 的读数。 / What `resolve()` says.
    #[serde(default)]
    pub resolved: Option<String>,
    /// `resolver()` 的读数。 / What `resolver()` says.
    #[serde(default)]
    pub resolved_by_resolver: Option<String>,
    /// `to_field_map()` 里的读数。 / What the field map says.
    #[serde(default)]
    pub resolved_by_map: Option<String>,
}

/// 谓词 schema 探针结论。 / The predicate schema probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistenceSchemaReport {
    /// schema 名称。 / The schema name.
    pub name: String,
    /// 映射条数。 / The mapping count.
    pub field_count: u32,
    /// 映射表条数（去重后）。 / The field-map count, deduplicated.
    pub map_count: u32,
    /// 每条查询的读数。 / Every query reading.
    pub queries: Vec<SchemaQueryRow>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_fields_append_in_order: bool,
    pub the_resolve_is_first_wins: bool,
    pub the_field_map_is_last_wins: bool,
    pub the_unknown_path_resolves_to_nothing: bool,
    pub the_same_field_mapping_repeats_the_whole_path: bool,
    pub the_field_builder_never_checks_registration: bool,
    pub the_parse_matches_the_constructor: bool,
    pub the_field_map_is_deduplicated_by_path: bool,
}

/// 走一遍谓词 schema 探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_persistence_schema_probe(
    request: &PersistenceSchemaRequest,
) -> Result<PersistenceSchemaReport> {
    if request.fields.len() > 8 || request.queries.len() > 8 {
        return Err(BridgeError::new("InvalidInput", "persistence schema probe failed validation: \
            at most eight entries per list"));
    }
    let findings = canonical_findings();
    let mut schema = PredicateSchema::new(request.name.clone().unwrap_or_else(|| "probe".to_owned()));
    for spec in &request.fields {
        if spec.path.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "persistence schema probe failed \
                validation: a field path must not be blank"));
        }
        schema = match spec.backend_field.as_deref() {
            None => schema.with_same_field(spec.path.as_str()),
            Some(backend) => schema.with_field_name(spec.path.as_str(), backend),
        };
    }
    let field_map = schema.to_field_map();
    let resolver = schema.resolver();
    let queries = request.queries.iter().map(|path| SchemaQueryRow {
        path: path.clone(),
        resolved: schema.resolve(path.as_str()),
        resolved_by_resolver: resolver(path.as_str()),
        resolved_by_map: field_map.get(&PropertyPath::parse(path)).cloned(),
    }).collect();
    Ok(PersistenceSchemaReport {
        name: schema.name().to_owned(),
        field_count: u32::try_from(schema.fields().len()).unwrap_or(u32::MAX),
        map_count: u32::try_from(field_map.len()).unwrap_or(u32::MAX),
        queries,
        the_fields_append_in_order: findings.the_fields_append_in_order,
        the_resolve_is_first_wins: findings.the_resolve_is_first_wins,
        the_field_map_is_last_wins: findings.the_field_map_is_last_wins,
        the_unknown_path_resolves_to_nothing: findings.the_unknown_path_resolves_to_nothing,
        the_same_field_mapping_repeats_the_whole_path:
            findings.the_same_field_mapping_repeats_the_whole_path,
        the_field_builder_never_checks_registration:
            findings.the_field_builder_never_checks_registration,
        the_parse_matches_the_constructor: findings.the_parse_matches_the_constructor,
        the_field_map_is_deduplicated_by_path: findings.the_field_map_is_deduplicated_by_path,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_fields_append_in_order: bool,
    the_resolve_is_first_wins: bool,
    the_field_map_is_last_wins: bool,
    the_unknown_path_resolves_to_nothing: bool,
    the_same_field_mapping_repeats_the_whole_path: bool,
    the_field_builder_never_checks_registration: bool,
    the_parse_matches_the_constructor: bool,
    the_field_map_is_deduplicated_by_path: bool,
}

/// 取映射路径序列。 / Take the mapping path sequence.
fn schema_paths(schema: &PredicateSchema<String>) -> Vec<String> {
    schema.fields().iter().map(|mapping| mapping.path.to_string()).collect()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        // 1) with_field 按顺序追加。 / `with_field` appends in order.
        let the_fields_append_in_order = {
            let schema = PredicateSchema::new("s").with_field("a", "A".to_owned())
                .with_field("b", "B".to_owned()).with_field_name("c", "C");
            schema_paths(&schema) == vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
                && schema.name() == "s"
                && PredicateSchema::<String>::new("empty").fields().is_empty()
        };
        // 2) resolve 先到先得。 / `resolve` is first wins.
        let the_resolve_is_first_wins = {
            let duplicated = PredicateSchema::new("s").with_field("a", "FIRST".to_owned())
                .with_field("a", "SECOND".to_owned());
            duplicated.resolve("a") == Some("FIRST".to_owned())
        };
        // 3) to_field_map 后到覆盖。 / `to_field_map` is last wins.
        let the_field_map_is_last_wins = {
            let duplicated = PredicateSchema::new("s").with_field("a", "FIRST".to_owned())
                .with_field("a", "SECOND".to_owned());
            let map = duplicated.to_field_map();
            // 同一个 schema，两个面给出相反答案。 
            // The same schema, with the two faces answering oppositely.
            map.get(&PropertyPath::parse("a")) == Some(&"SECOND".to_owned())
                && duplicated.resolve("a") == Some("FIRST".to_owned())
        };
        // 4) 未登记路径一律 None，resolver 同款。 
        // An unknown path resolves to nothing, and the resolver matches.
        let the_unknown_path_resolves_to_nothing = {
            let schema = PredicateSchema::new("s").with_field("a", "A".to_owned());
            let resolver = schema.resolver();
            schema.resolve("nope").is_none() && resolver("nope").is_none()
                && schema.resolve("a") == Some("A".to_owned())
                && resolver("a") == Some("A".to_owned())
                // 前缀不算命中。 / A prefix is not a hit.
                && schema.resolve("a.b").is_none()
        };
        // 5) with_same_field 用最后一段。 / `with_same_field` uses the last segment.
        let the_same_field_mapping_repeats_the_whole_path = {
            let schema = PredicateSchema::new("s")
                .with_same_field("order.customer.name")
                .with_same_field("id");
            let backend: Vec<String> = schema.fields().iter()
                .map(|mapping| mapping.backend_field.clone()).collect();
            // 实测：`path.value()` 给的是**整条路径文本**，不是最后一段。 
            // Measured: `path.value()` yields the **whole path text**, not the last segment.
            backend == vec!["order.customer.name".to_owned(), "id".to_owned()]
                && schema.resolve("order.customer.name")
                    == Some("order.customer.name".to_owned())
        };
        // 6) field() 不校验登记。 / `field()` never checks registration.
        let the_field_builder_never_checks_registration = {
            let schema = PredicateSchema::new("s").with_field("a", "A".to_owned());
            let unregistered: FieldPath<ospf_rust_math::symbol::ExpressionValue> =
                schema.field("never.registered");
            // 未登记的路径照样能造出字段路径。 
            // An unregistered path still yields a field path.
            unregistered.path().to_string() == "never.registered"
                && schema.resolve("never.registered").is_none()
        };
        // 7) parse 与 new 等价。 / `parse` matches the constructor.
        let the_parse_matches_the_constructor = {
            let parsed: FieldPath<ospf_rust_math::symbol::ExpressionValue> =
                FieldPath::parse("a.b");
            let built: FieldPath<ospf_rust_math::symbol::ExpressionValue> =
                FieldPath::new(PropertyPath::parse("a.b"));
            parsed.path() == built.path() && parsed.path().to_string() == "a.b"
        };
        // 8) 映射表按路径去重。 / The field map is deduplicated by path.
        let the_field_map_is_deduplicated_by_path = {
            let duplicated = PredicateSchema::new("s").with_field("a", "FIRST".to_owned())
                .with_field("a", "SECOND".to_owned())
                .with_field("b", "B".to_owned());
            // 三条记录、两张表项。 / Three records, two map entries.
            duplicated.fields().len() == 3 && duplicated.to_field_map().len() == 2
                && FieldMapping::new("a", "A".to_owned()).backend_field == "A".to_owned()
        };
        Findings {
            the_fields_append_in_order,
            the_resolve_is_first_wins,
            the_field_map_is_last_wins,
            the_unknown_path_resolves_to_nothing,
            the_same_field_mapping_repeats_the_whole_path,
            the_field_builder_never_checks_registration,
            the_parse_matches_the_constructor,
            the_field_map_is_deduplicated_by_path,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: PersistenceSchemaRequest) -> PersistenceSchemaReport {
        verify_persistence_schema_probe(&request).unwrap()
    }

    fn spec(path: &str, backend: Option<&str>) -> PersistenceFieldSpec {
        PersistenceFieldSpec { path: path.to_owned(), backend_field: backend.map(str::to_owned) }
    }

    fn request() -> PersistenceSchemaRequest {
        PersistenceSchemaRequest { name: None,
            fields: vec![spec("a", Some("A")), spec("b", None)],
            queries: vec!["a".to_owned(), "b".to_owned()] }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request());
        assert_eq!(report.name, "probe");
        assert_eq!(report.field_count, 2);
        assert_eq!(report.map_count, 2);
        assert_eq!(report.queries[0].resolved, Some("A".to_owned()));
        // `with_same_field` 把整条路径文本当后端名。 
        // `with_same_field` repeats the whole path text as the backend name.
        assert_eq!(report.queries[1].resolved, Some("b".to_owned()));
        assert_eq!(report.queries[0].resolved_by_resolver, Some("A".to_owned()));
        assert_eq!(report.queries[0].resolved_by_map, Some("A".to_owned()));
        assert!(report.the_fields_append_in_order);
        assert!(report.the_resolve_is_first_wins);
        assert!(report.the_field_map_is_last_wins);
        assert!(report.the_unknown_path_resolves_to_nothing);
        assert!(report.the_same_field_mapping_repeats_the_whole_path);
        assert!(report.the_field_builder_never_checks_registration);
        assert!(report.the_parse_matches_the_constructor);
        assert!(report.the_field_map_is_deduplicated_by_path);
    }

    #[test]
    fn the_two_faces_answer_oppositely_on_a_duplicate() {
        let report = ask(PersistenceSchemaRequest {
            fields: vec![spec("a", Some("FIRST")), spec("a", Some("SECOND"))],
            queries: vec!["a".to_owned()], ..request() });
        assert_eq!(report.field_count, 2);
        assert_eq!(report.map_count, 1);
        // 同一个 schema：`resolve` 给 FIRST，`to_field_map` 给 SECOND。 
        // One schema: `resolve` says FIRST while `to_field_map` says SECOND.
        assert_eq!(report.queries[0].resolved, Some("FIRST".to_owned()));
        assert_eq!(report.queries[0].resolved_by_map, Some("SECOND".to_owned()));
        assert_eq!(report.queries[0].resolved_by_resolver, Some("FIRST".to_owned()));
    }

    #[test]
    fn unknown_paths_resolve_to_nothing() {
        let report = ask(PersistenceSchemaRequest { queries: vec!["nope".to_owned(),
            "a.b".to_owned()], ..request() });
        assert_eq!(report.queries[0].resolved, None);
        assert_eq!(report.queries[0].resolved_by_resolver, None);
        assert_eq!(report.queries[0].resolved_by_map, None);
        // 前缀不算命中。 / A prefix is not a hit.
        assert_eq!(report.queries[1].resolved, None);
    }

    #[test]
    fn the_whole_path_becomes_the_backend_name() {
        let report = ask(PersistenceSchemaRequest {
            fields: vec![spec("order.customer.name", None), spec("id", None)],
            queries: vec!["order.customer.name".to_owned()], ..request() });
        // 实测：后端名就是整条路径文本。 / Measured: the backend name is the whole path text.
        assert_eq!(report.queries[0].resolved, Some("order.customer.name".to_owned()));
        assert_eq!(report.field_count, 2);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_persistence_schema_probe(&PersistenceSchemaRequest {
            fields: (0..9).map(|index| spec(&format!("f{index}"), None)).collect(),
            ..request() }).unwrap_err().message.contains("at most eight entries"));
        assert!(verify_persistence_schema_probe(&PersistenceSchemaRequest {
            queries: (0..9).map(|index| format!("q{index}")).collect(),
            ..request() }).unwrap_err().message.contains("at most eight entries"));
        assert!(verify_persistence_schema_probe(&PersistenceSchemaRequest {
            fields: vec![spec("   ", None)], ..request() }).unwrap_err()
            .message.contains("must not be blank"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"fieldCount\""), "{json}");
        assert!(json.contains("\"mapCount\""), "{json}");
        assert!(json.contains("\"theFieldMapIsLastWins\""), "{json}");
        assert!(json.contains("\"theSameFieldMappingRepeatsTheWholePath\""), "{json}");
        assert!(!json.contains("field_count"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<PersistenceSchemaReport>(&copy.to_string()).is_err());
    }
}
