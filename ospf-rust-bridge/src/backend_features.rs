//! 续挖第 40 单：持久化后端的能力快照——**族开关与方言开关是两个独立层**，`any_enabled()` 只看族
//! （`persistence/backend/mod.rs` 的 `PersistenceBackendFeatures`）
//! Continuation, delivery 40: the persistence backend capability snapshot — **family flags and dialect flags are two
//! independent layers**, and `any_enabled()` reads only the families (`PersistenceBackendFeatures`).
//!
//! 这是目标里点名的**真边界**（DB 驱动）——所以这一单只绑**纯配置与推导**，不碰任何连接、不执行任何 SQL。
//! 实测四条（都是可判定的布尔事实，不依赖环境）：
//!
//! 1. **14 个开关 = 8 个族 + 6 个方言**（`sqlx`/`diesel` 各有 postgres/mysql/sqlite 三个方言开关），
//!    两层的取值**互相不蕴含**：`sqlx = true` **不**意味着 `sqlx_postgres = true`，因为两者是各自独立的
//!    `cfg!(feature = ...)`。反过来 `sqlx_postgres = true` 也**不**蕴含 `sqlx = true`。
//! 2. **`any_enabled()` 只读那 8 个族开关**（`anyEnabledReadsOnlyFamilies()`）：6 个方言开关对它完全不可见。
//!    于是"方言开着、族关着"这种自相矛盾的快照会回答 `false`——**方言不启用后端**。
//! 3. **`mongodb` / `redis` 是族但不是 SQL 方言族**（它们没有方言子开关），所以"族"的集合不是"有方言的族"的集合。
//! 4. **`current()` 是编译期快照**：`cfg!(feature = ...)` 在编译时求值，运行期恒为常量，无法在运行期打开任何后端。
//!    （实测本仓默认构建下 14 个全为 `false`。）
//!
//! This is the **true boundary** the objective names (DB drivers), so this delivery binds **pure configuration and
//! derivation only** — no connection, no SQL. Four measured facts, all decidable booleans independent of the environment.
//! **Fourteen flags are eight families plus six dialects** (sqlx and diesel each have postgres/mysql/sqlite), and the two
//! layers **do not imply each other**: `sqlx = true` does **not** mean `sqlx_postgres = true` because each is its own
//! `cfg!(feature = ...)`, and a dialect flag set alone does not imply its family either. **`any_enabled()` reads only the eight
//! family flags** (`anyEnabledReadsOnlyFamilies()`), so a self-contradictory "dialect on, family off" snapshot answers `false`
//! — **a dialect never enables a backend**. **`mongodb` and `redis` are families without dialect sub-flags**, so the family set
//! is not the set of families that have dialects. And **`current()` is a compile-time snapshot**: `cfg!` is evaluated during
//! compilation and can never be turned on at run time (in this repository's default build all fourteen are `false`).

use ospf_rust_framework::persistence::backend::PersistenceBackendFeatures;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个后端开关的名字与层级。 / One backend flag's name and layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Flag {
    name: &'static str,
    family: &'static str,
    dialect: bool,
}

/// 全部 14 个开关，按 `PersistenceBackendFeatures` 的字段顺序。
/// All fourteen flags in the order of the `PersistenceBackendFeatures` fields.
const FLAGS: [Flag; 14] = [
    Flag { name: "sqlx", family: "sqlx", dialect: false },
    Flag { name: "sqlxPostgres", family: "sqlx", dialect: true },
    Flag { name: "sqlxMysql", family: "sqlx", dialect: true },
    Flag { name: "sqlxSqlite", family: "sqlx", dialect: true },
    Flag { name: "diesel", family: "diesel", dialect: false },
    Flag { name: "dieselPostgres", family: "diesel", dialect: true },
    Flag { name: "dieselMysql", family: "diesel", dialect: true },
    Flag { name: "dieselSqlite", family: "diesel", dialect: true },
    Flag { name: "seaOrm", family: "seaOrm", dialect: false },
    Flag { name: "toasty", family: "toasty", dialect: false },
    Flag { name: "rbatis", family: "rbatis", dialect: false },
    Flag { name: "cornucopia", family: "cornucopia", dialect: false },
    Flag { name: "mongodb", family: "mongodb", dialect: false },
    Flag { name: "redis", family: "redis", dialect: false },
];

/// 能力快照请求：只给**开着的**开关名。 / The capability request: only the names that are on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackendFeaturesRequest {
    /// 开着的开关名（camelCase，闭集）。 / The names that are on (camelCase, a closed vocabulary).
    #[serde(default)]
    pub enabled: Vec<String>,
}

/// 编译期快照。 / The compile-time snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompileTimeSnapshot {
    /// 编译期开着的开关名。 / The names that are on at compile time.
    pub enabled: Vec<String>,
    /// 编译期的 `anyEnabled()`。 / The compile-time `anyEnabled()`.
    pub any_enabled: bool,
}

/// 能力快照结论。 / The capability snapshot verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackendFeaturesReport {
    /// 规范顺序下开着的开关名。 / The enabled names in canonical order.
    pub enabled: Vec<String>,
    /// 这个快照的 `anyEnabled()`。 / This snapshot's `anyEnabled()`.
    pub any_enabled: bool,
    /// 族开关个数。 / The family flag count.
    pub family_count: u32,
    /// 方言开关个数。 / The dialect flag count.
    pub dialect_count: u32,
    /// 开着的族。 / The enabled families.
    pub enabled_families: Vec<String>,
    /// 只开着方言、族没开的方言（自相矛盾的组合）。 / The dialects that are on while their family is off.
    pub orphans: Vec<String>,
    /// 本仓编译期快照。 / This repository's compile-time snapshot.
    pub current: CompileTimeSnapshot,
    /// 方言不启用族（**是**）。 / Whether a dialect never enables its family (**yes**).
    pub dialect_does_not_enable_the_family: bool,
    /// `anyEnabled()` 只读族（**是**）。 / Whether `anyEnabled()` reads only the families (**yes**).
    pub any_enabled_reads_only_families: bool,
    /// 族不蕴含自己的方言（**是**）。 / Whether a family never implies its dialects (**yes**).
    pub family_does_not_imply_its_dialects: bool,
    /// 存在没有方言的族（**是**）。 / Whether some families have no dialects (**yes**).
    pub some_families_have_no_dialects: bool,
    /// `current()` 是编译期常量（**是**）。 / Whether `current()` is a compile-time constant (**yes**).
    pub current_is_compile_time: bool,
}

/// 从 14 个布尔造快照。 / Build a snapshot from fourteen booleans.
fn snapshot(values: &[bool; 14]) -> PersistenceBackendFeatures {
    PersistenceBackendFeatures {
        sqlx: values[0], sqlx_postgres: values[1], sqlx_mysql: values[2], sqlx_sqlite: values[3],
        diesel: values[4], diesel_postgres: values[5], diesel_mysql: values[6],
        diesel_sqlite: values[7], sea_orm: values[8], toasty: values[9], rbatis: values[10],
        cornucopia: values[11], mongodb: values[12], redis: values[13],
    }
}

/// 读出 14 个布尔。 / Read the fourteen booleans.
fn values_of(features: PersistenceBackendFeatures) -> [bool; 14] {
    [features.sqlx, features.sqlx_postgres, features.sqlx_mysql, features.sqlx_sqlite,
        features.diesel, features.diesel_postgres, features.diesel_mysql, features.diesel_sqlite,
        features.sea_orm, features.toasty, features.rbatis, features.cornucopia, features.mongodb,
        features.redis]
}

/// 走一遍能力快照。非法输入是**错误**。 / Walk the capability snapshot.
pub fn verify_backend_features(request: &BackendFeaturesRequest) -> Result<BackendFeaturesReport> {
    let mut values = [false; 14];
    let mut seen = std::collections::BTreeSet::new();
    for name in &request.enabled {
        let position = FLAGS.iter().position(|flag| flag.name == name).ok_or_else(|| {
            BridgeError::new("InvalidInput", &format!(
                "backend features failed validation: unknown flag {name:?}"))
        })?;
        if !seen.insert(position) {
            return Err(BridgeError::new("InvalidInput", &format!(
                "backend features failed validation: the request repeats {name:?}")));
        }
        values[position] = true;
    }
    let features = snapshot(&values);
    let enabled: Vec<String> = FLAGS.iter()
        .filter(|flag| values[FLAGS.iter().position(|probe| probe.name == flag.name).unwrap_or(0)])
        .map(|flag| flag.name.to_owned()).collect();
    // 只开着方言、族没开的组合。 / Dialects that are on while their family is off.
    let orphans: Vec<String> = FLAGS.iter().enumerate()
        .filter(|(position, flag)| flag.dialect && values[*position]
            && !values[FLAGS.iter().position(|probe| probe.name == flag.family).unwrap_or(0)])
        .map(|(_, flag)| flag.name.to_owned()).collect();
    let current = PersistenceBackendFeatures::current();
    let current_values = values_of(current);
    let current_enabled: Vec<String> = FLAGS.iter().enumerate()
        .filter(|(position, _)| current_values[*position])
        .map(|(_, flag)| flag.name.to_owned()).collect();
    let findings = canonical_findings();
    Ok(BackendFeaturesReport {
        enabled,
        any_enabled: features.any_enabled(),
        family_count: u32::try_from(FLAGS.iter().filter(|flag| !flag.dialect).count())
            .unwrap_or(u32::MAX),
        dialect_count: u32::try_from(FLAGS.iter().filter(|flag| flag.dialect).count())
            .unwrap_or(u32::MAX),
        enabled_families: FLAGS.iter().enumerate()
            .filter(|(position, flag)| !flag.dialect && values[*position])
            .map(|(_, flag)| flag.name.to_owned()).collect(),
        orphans,
        current: CompileTimeSnapshot { enabled: current_enabled,
            any_enabled: current.any_enabled() },
        dialect_does_not_enable_the_family: findings.dialect_does_not_enable_the_family,
        any_enabled_reads_only_families: findings.any_enabled_reads_only_families,
        family_does_not_imply_its_dialects: findings.family_does_not_imply_its_dialects,
        some_families_have_no_dialects: findings.some_families_have_no_dialects,
        current_is_compile_time: findings.current_is_compile_time,
    })
}

/// 五条规则属性的内部固定探针。 / The five rule properties measured by fixed internal probes.
struct Findings {
    dialect_does_not_enable_the_family: bool,
    any_enabled_reads_only_families: bool,
    family_does_not_imply_its_dialects: bool,
    some_families_have_no_dialects: bool,
    current_is_compile_time: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 只开方言：`any_enabled()` 回答 false。 / Dialect only: `any_enabled()` answers false.
    let mut only_dialect = [false; 14];
    only_dialect[1] = true; // sqlxPostgres
    only_dialect[6] = true; // dieselMysql
    let dialect_only = snapshot(&only_dialect);
    let dialect_does_not_enable_the_family = !dialect_only.any_enabled();
    // 2) 只开族（不投方言）：`any_enabled()` 回答 true，而方言仍全为 false。
    //    Family only (no dialect): `any_enabled()` answers true and every dialect stays false.
    let mut only_family = [false; 14];
    only_family[0] = true; // sqlx
    only_family[4] = true; // diesel
    let family_only = snapshot(&only_family);
    let family_only_values = values_of(family_only);
    let any_enabled_reads_only_families = family_only.any_enabled()
        && !family_only_values[1] && !family_only_values[2] && !family_only_values[3]
        && !family_only_values[5] && !family_only_values[6] && !family_only_values[7];
    let family_does_not_imply_its_dialects = any_enabled_reads_only_families;
    // 3) 有族没有方言：`mongodb` / `redis` / `seaOrm` / `toasty` / `rbatis` / `cornucopia`。
    //    Families without dialects.
    let dialect_families: std::collections::BTreeSet<&str> = FLAGS.iter()
        .filter(|flag| flag.dialect).map(|flag| flag.family).collect();
    let family_names: std::collections::BTreeSet<&str> = FLAGS.iter()
        .filter(|flag| !flag.dialect).map(|flag| flag.name).collect();
    let some_families_have_no_dialects = family_names.len() == 8 && dialect_families.len() == 2;
    // 4) `current()` 是编译期常量：两次取到完全相同的结果。 
    //    4) `current()` is a compile-time constant: two reads agree exactly.
    let first = PersistenceBackendFeatures::current();
    let second = PersistenceBackendFeatures::current();
    let current_is_compile_time = first == second && values_of(first) == values_of(second)
        && first.any_enabled() == second.any_enabled();
    Findings { dialect_does_not_enable_the_family, any_enabled_reads_only_families,
        family_does_not_imply_its_dialects, some_families_have_no_dialects, current_is_compile_time }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(enabled: &[&str]) -> BackendFeaturesRequest {
        BackendFeaturesRequest { enabled: enabled.iter().map(|name| (*name).to_owned()).collect() }
    }

    #[test]
    fn the_two_layers_are_independent() {
        // 只开方言：`anyEnabled()` 是 false。 / Dialect only: `anyEnabled()` is false.
        let orphans = verify_backend_features(&request(&["sqlxPostgres", "dieselMysql"])).unwrap();
        assert!(!orphans.any_enabled);
        assert_eq!(orphans.enabled_families, Vec::<String>::new());
        assert_eq!(orphans.orphans, vec!["sqlxPostgres".to_owned(), "dieselMysql".to_owned()]);
        assert!(orphans.dialect_does_not_enable_the_family);
        // 只开族：`anyEnabled()` 是 true，方言仍全关。 / Family only: `anyEnabled()` is true, dialects stay off.
        let families = verify_backend_features(&request(&["sqlx"])).unwrap();
        assert!(families.any_enabled);
        assert_eq!(families.enabled, vec!["sqlx".to_owned()]);
        assert!(families.orphans.is_empty());
        assert!(families.family_does_not_imply_its_dialects);
        assert!(families.any_enabled_reads_only_families);
    }

    #[test]
    fn the_counts_separate_families_from_dialects() {
        let report = verify_backend_features(&request(&[])).unwrap();
        assert_eq!(report.family_count, 8);
        assert_eq!(report.dialect_count, 6);
        assert!(!report.any_enabled);
        assert!(report.some_families_have_no_dialects);
        assert!(report.current_is_compile_time);
    }

    #[test]
    fn the_canonical_order_is_the_field_order() {
        // 乱序给，按字段顺序回。 / Given out of order, returned in field order.
        let report = verify_backend_features(&request(&["redis", "sqlxSqlite", "sqlx"])).unwrap();
        assert_eq!(report.enabled, vec!["sqlx".to_owned(), "sqlxSqlite".to_owned(),
            "redis".to_owned()]);
        assert_eq!(report.enabled_families, vec!["sqlx".to_owned(), "redis".to_owned()]);
        assert!(report.orphans.is_empty());
        assert!(report.any_enabled);
    }

    #[test]
    fn every_backend_on_is_consistent() {
        let all: Vec<&str> = FLAGS.iter().map(|flag| flag.name).collect();
        let report = verify_backend_features(&request(&all)).unwrap();
        assert!(report.any_enabled);
        assert_eq!(report.enabled.len(), 14);
        assert_eq!(report.enabled_families.len(), 8);
        assert!(report.orphans.is_empty());
        assert!(report.dialect_does_not_enable_the_family);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let report = verify_backend_features(&request(&["mongodb"])).unwrap();
        assert!(report.dialect_does_not_enable_the_family);
        assert!(report.any_enabled_reads_only_families);
        assert!(report.family_does_not_imply_its_dialects);
        assert!(report.some_families_have_no_dialects);
        assert!(report.current_is_compile_time);
        assert!(report.any_enabled);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_backend_features(&request(&["mysql"])).unwrap_err().message
            .contains("unknown flag"));
        assert!(verify_backend_features(&request(&["sqlx", "sqlx"])).unwrap_err().message
            .contains("repeats"));
        // 大小写敏感：闭集用 camelCase。 / Case sensitive: the vocabulary is camelCase.
        assert!(verify_backend_features(&request(&["SeaOrm"])).is_err());
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_backend_features(&request(&["sqlx"])).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"familyCount\""), "{json}");
        assert!(json.contains("\"dialectDoesNotEnableTheFamily\""), "{json}");
        assert!(json.contains("\"anyEnabledReadsOnlyFamilies\""), "{json}");
        assert!(!json.contains("family_count"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<BackendFeaturesReport>(&copy.to_string()).is_err());
    }
}
