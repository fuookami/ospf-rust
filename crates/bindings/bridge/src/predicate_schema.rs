//! 续挖第 22 单：谓词 schema 的字段映射表，以及它两套解析口径的分歧
//! （`persistence/expression/predicate_schema.rs`）
//! Continuation, delivery 22: the predicate schema's field table and the split between its two lookups.
//!
//! `PredicateSchema` 只有一个字段列表，却提供**两个**查询口径，而且两者在重复注册时给出**不同**答案：
//!
//! 1. **`resolve(path)` 是先到先得**（`fields.iter().find(..)`），**`to_field_map()` 是后到覆盖**（收进 `HashMap`）。
//!    同一个路径注册两次时，`resolve` 给第一个、字段表给最后一个——两者**直接矛盾**。
//! 2. **`with_field` 不去重**，重复注册是允许的；重不重复只能靠调用方自己看 `fields()`。
//! 3. **未知路径是静默 `None`**，不报错。这与同一框架里 `ConditionalIndicatorFunction::index_of` 的做法**正相反**：
//!    后者对未注册的符号**硬报错**（`SymbolNotRegistered`）。所以"路径查不到"到底是"可选"还是"必须"，
//!    取决于走的是哪条 API——把 `None` 当成"没有这个字段"是安全的，但把它当成"这个字段值为空"是错的。
//! 4. **`with_same_field` 的后端名取 `path.value()`**（去掉前缀后的路径本体），不是整条原始字符串。
//!
//! The schema holds one field list but offers **two** lookups, and they disagree on duplicates. **`resolve(path)` is
//! first-wins** (`fields.iter().find(..)`) while **`to_field_map()` is last-wins** (collected into a `HashMap`), so a path
//! registered twice gives the first one through `resolve` and the last one through the map — **a direct contradiction**.
//! **`with_field` does not dedupe**, so duplicates are legal and only visible through `fields()`. **An unknown path is a
//! silent `None`**, not an error — the exact opposite of `ConditionalIndicatorFunction::index_of` in the same framework,
//! which hard-errors on an unregistered symbol (`SymbolNotRegistered`); treating the `None` as "no such field" is safe,
//! treating it as "the field is null" is not. And **`with_same_field` derives the backend name from `path.value()`** (the
//! path body after its prefix), not from the raw string.

use ospf_rust_framework::persistence::expression::predicate_schema::PredicateSchema;
use ospf_rust_framework::persistence::expression::persistence_field_resolver::PersistenceFieldResolver;
use ospf_rust_math::symbol::PropertyPath;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一条字段注册。 / One field registration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaFieldSpec {
    /// 领域属性路径。 / The domain property path.
    pub path: String,
    /// 后端字段表示。 / The backend field representation.
    pub backend_field: String,
}

/// 字段映射请求。 / The field-mapping request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateSchemaRequest {
    /// schema 名。 / The schema name.
    pub name: String,
    /// 注册的字段（**允许重复**）。 / The registered fields (**duplicates allowed**).
    #[serde(default)]
    pub fields: Vec<SchemaFieldSpec>,
    /// 要解析的路径。 / The paths to resolve.
    #[serde(default)]
    pub probes: Vec<String>,
    /// 用同名字段注册的路径（后端名取 `path.value()`）。 / Paths registered with the same-name helper.
    #[serde(default)]
    pub same_field_paths: Vec<String>,
}

/// 一次解析。 / One resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveRow {
    /// 探测的路径。 / The probed path.
    pub path: String,
    /// `resolve` 的答案（未知为无值）。 / What `resolve` says (absent when unknown).
    #[serde(default)]
    pub resolved: Option<String>,
    /// `to_field_map` 的答案。 / What the field map says.
    #[serde(default)]
    pub mapped: Option<String>,
    /// 两者是否一致。 / Whether they agree.
    pub agrees: bool,
}

/// 一条同名字段注册。 / One same-name registration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SameFieldRow {
    /// 原始路径。 / The raw path.
    pub path: String,
    /// 推导出的后端名。 / The derived backend name.
    pub backend_field: String,
    /// 路径的 `value()`（推导来源）。 / The path's `value()`, the derivation source.
    pub value: String,
}

/// 字段映射结论。 / The field-mapping verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateSchemaReport {
    /// schema 名。 / The schema name.
    pub name: String,
    /// 注册条数（含重复）。 / The registration count, duplicates included.
    pub field_count: u64,
    /// 去重后的路径数。 / The number of distinct paths.
    pub distinct_paths: u64,
    /// 重复注册的路径。 / The duplicated paths.
    pub duplicate_paths: Vec<String>,
    /// 每次探测。 / Every probe.
    pub resolves: Vec<ResolveRow>,
    /// 同名字段注册。 / The same-name registrations.
    pub same_fields: Vec<SameFieldRow>,
    /// `resolve` 对重复路径取第一个（**是**）。 / Whether `resolve` takes the first (**yes**).
    pub resolve_is_first_wins: bool,
    /// 字段表对重复路径取最后一个（**是**）。 / Whether the map takes the last (**yes**).
    pub map_is_last_wins: bool,
    /// 未知路径静默返回无值（**是**）。 / Whether an unknown path is silently absent (**yes**).
    pub unknown_is_silent: bool,
    /// `resolver()` 与 `resolve()` 一致（**是**）。 / Whether `resolver()` agrees with `resolve()` (**yes**).
    pub resolver_matches_resolve: bool,
}

/// 上限（信任边界）。 / The caps (a trust boundary).
const MAX_ENTRIES: usize = 4_096;

/// 走一遍字段映射表。非法输入是**错误**。 / Walk the field table.
pub fn verify_predicate_schema(request: &PredicateSchemaRequest) -> Result<PredicateSchemaReport> {
    if request.fields.len() > MAX_ENTRIES
        || request.probes.len() > MAX_ENTRIES
        || request.same_field_paths.len() > MAX_ENTRIES
    {
        return Err(BridgeError::new("InvalidInput", &format!(
            "predicate schema failed validation: at most {MAX_ENTRIES} entries are accepted")));
    }
    if request.name.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "predicate schema failed validation: the schema name must not be blank"));
    }
    let mut schema = PredicateSchema::new(request.name.clone());
    let mut order: Vec<String> = Vec::with_capacity(request.fields.len());
    for field in &request.fields {
        if field.path.trim().is_empty() || field.backend_field.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "predicate schema failed validation: a field needs a path and a backend name"));
        }
        schema = schema.with_field(PropertyPath::parse(&field.path), field.backend_field.clone());
        order.push(PropertyPath::parse(&field.path).value().to_owned());
    }
    let map = schema.to_field_map();
    let resolver = schema.resolver();
    // 每条路径用三种口径各查一次。 / Look every probed path up through the three entries.
    let mut resolves = Vec::with_capacity(request.probes.len());
    let mut resolver_matches_resolve = true;
    for probe in &request.probes {
        let path = PropertyPath::parse(probe);
        let resolved = schema.resolve(path.clone());
        let mapped = map.get(&path).cloned();
        resolver_matches_resolve &= resolver(probe) == resolved;
        resolves.push(ResolveRow { path: probe.clone(), agrees: resolved == mapped,
            resolved, mapped });
    }
    // 重复路径：`resolve` 与字段表在此时必然分歧。 / Duplicates: `resolve` and the map must disagree here.
    let mut seen: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for path in &order {
        *seen.entry(path.clone()).or_insert(0) += 1;
    }
    let duplicate_paths: Vec<String> = seen.iter()
        .filter(|(_, count)| **count > 1).map(|(path, _)| path.clone()).collect();
    let mut resolve_is_first_wins = true;
    let mut map_is_last_wins = true;
    for path in &duplicate_paths {
        let registered: Vec<&str> = request.fields.iter()
            .filter(|field| PropertyPath::parse(&field.path).value() == path.as_str())
            .map(|field| field.backend_field.as_str()).collect();
        guard(registered.len(), path)?;
        let resolved = schema.resolve(PropertyPath::parse(path));
        let mapped = map.get(&PropertyPath::parse(path)).cloned();
        resolve_is_first_wins &= resolved.as_deref() == registered.first().copied();
        map_is_last_wins &= mapped.as_deref() == registered.last().copied();
    }
    // 未知路径：静默无值，而不是报错。 / An unknown path is silently absent instead of an error.
    let unknown = format!("{}__absent", request.name);
    let unknown_is_silent = schema.resolve(PropertyPath::parse(&unknown)).is_none()
        && resolver(&unknown).is_none() && !map.contains_key(&PropertyPath::parse(&unknown));
    // 同名字段：后端名取 `path.value()`。 / The same-name helper derives from `path.value()`.
    let mut same_fields = Vec::with_capacity(request.same_field_paths.len());
    for path in &request.same_field_paths {
        let mut probe = PredicateSchema::new("same");
        probe = probe.with_same_field(PropertyPath::parse(path));
        let backend_field = probe.resolve(PropertyPath::parse(path)).ok_or_else(|| {
            BridgeError::new("Internal", "predicate schema failed to resolve a same-name field")
        })?;
        same_fields.push(SameFieldRow { path: path.clone(), backend_field,
            value: PropertyPath::parse(path).value().to_owned() });
    }
    Ok(PredicateSchemaReport {
        name: schema.name().to_owned(),
        field_count: schema.fields().len() as u64,
        distinct_paths: seen.len() as u64,
        duplicate_paths,
        resolves,
        same_fields,
        resolve_is_first_wins,
        map_is_last_wins,
        unknown_is_silent,
        resolver_matches_resolve,
    })
}

/// 防御：注册条数为零时这条判据无从验证。 / A guard: with no registrations there is nothing to check.
fn guard(length: usize, path: &str) -> Result<()> {
    if length == 0 {
        return Err(BridgeError::new("InvalidInput", &format!(
            "predicate schema failed validation: duplicated path `{path}` has no registration")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(path: &str, backend: &str) -> SchemaFieldSpec {
        SchemaFieldSpec { path: path.to_owned(), backend_field: backend.to_owned() }
    }

    fn request(fields: Vec<SchemaFieldSpec>, probes: &[&str]) -> PredicateSchemaRequest {
        PredicateSchemaRequest { name: "orders".to_owned(), fields,
            probes: probes.iter().map(|probe| (*probe).to_owned()).collect(),
            same_field_paths: Vec::new() }
    }

    #[test]
    fn the_two_lookups_disagree_on_duplicates() {
        // 同一路径注册两次：`resolve` 先到先得，字段表后到覆盖，两者直接矛盾。 
        // The same path twice: `resolve` is first-wins, the map last-wins — a direct contradiction.
        let report = verify_predicate_schema(&request(
            vec![field("id", "orders.id"), field("id", "legacy_id")], &["id"])).unwrap();
        assert_eq!(report.field_count, 2);
        assert_eq!(report.distinct_paths, 1);
        assert_eq!(report.duplicate_paths, vec!["id".to_owned()]);
        let row = &report.resolves[0];
        assert_eq!(row.resolved.as_deref(), Some("orders.id"));
        assert_eq!(row.mapped.as_deref(), Some("legacy_id"));
        assert!(!row.agrees);
        assert!(report.resolve_is_first_wins);
        assert!(report.map_is_last_wins);
    }

    #[test]
    fn without_duplicates_the_lookups_agree_and_unknown_paths_are_silent() {
        let report = verify_predicate_schema(&request(
            vec![field("user.name", "name"), field("total", "amount")],
            &["user.name", "total", "missing"])).unwrap();
        assert_eq!(report.resolves[0].resolved.as_deref(), Some("name"));
        assert!(report.resolves[0].agrees);
        assert!(report.resolves[1].agrees);
        // 未知路径静默无值，不报错。 / An unknown path is silently absent.
        assert!(report.resolves[2].resolved.is_none());
        assert!(report.resolves[2].mapped.is_none());
        assert!(report.resolves[2].agrees);
        assert!(report.unknown_is_silent);
        assert!(report.resolver_matches_resolve);
        assert!(report.duplicate_paths.is_empty());
    }

    #[test]
    fn the_same_name_helper_derives_from_the_path_value() {
        let mut document = request(Vec::new(), &[]);
        document.same_field_paths = vec!["user.name".to_owned(), "id".to_owned()];
        let report = verify_predicate_schema(&document).unwrap();
        assert_eq!(report.same_fields.len(), 2);
        for row in &report.same_fields {
            // 后端名就是 `path.value()`，不是整条原始字符串。 / The backend name is `path.value()`.
            assert_eq!(row.backend_field, row.value, "{}", row.path);
        }
    }

    #[test]
    fn empty_schema_and_bad_input() {
        let report = verify_predicate_schema(&request(Vec::new(), &["anything"])).unwrap();
        assert_eq!(report.field_count, 0);
        assert_eq!(report.distinct_paths, 0);
        assert!(report.resolves[0].resolved.is_none());
        assert!(report.unknown_is_silent);
        // 空名字被拒。 / A blank name is refused.
        let mut blank = request(Vec::new(), &[]);
        blank.name = "   ".to_owned();
        assert!(verify_predicate_schema(&blank).unwrap_err().message.contains("must not be blank"));
        // 缺后端名被拒。 / A missing backend name is refused.
        let mut partial = request(vec![field("id", "")], &[]);
        partial.name = "orders".to_owned();
        assert!(verify_predicate_schema(&partial).unwrap_err().message.contains("needs a path"));
        // 上限。 / The cap.
        let many = request(vec![field("id", "a"); MAX_ENTRIES + 1], &[]);
        assert!(verify_predicate_schema(&many).unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        // 同名字段也注册一条，好让 `backendField` 出现在文本里。
        // Register one same-name field so `backendField` shows up in the text.
        let mut document = request(vec![field("id", "orders.id")], &["id"]);
        document.same_field_paths = vec!["id".to_owned()];
        let report = verify_predicate_schema(&document).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"fieldCount\""), "{json}");
        assert!(json.contains("\"resolveIsFirstWins\""), "{json}");
        assert!(json.contains("\"backendField\""), "{json}");
        assert!(!json.contains("field_count"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<PredicateSchemaReport>(&copy.to_string()).is_err());
    }
}
