//! 持久化后端能力快照
//! Persistence backend capability snapshot
//!
//! 具体 translator 实现位于 `ospf-rust-framework-plugin-persistence`；本模块只保留编译期
//! feature 快照，供 bridge 等下游按后端能力分支。
//! The concrete translator implementations live in `ospf-rust-framework-plugin-persistence`;
//! this module keeps only the compile-time feature snapshot used by downstream callers such as
//! the bridge to branch on backend capability.

/// 后端 feature 能力快照。
/// Backend feature capability snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistenceBackendFeatures {
    /// 是否启用 sqlx / Whether sqlx is enabled
    pub sqlx: bool,
    /// 是否启用 sqlx postgres / Whether sqlx postgres is enabled
    pub sqlx_postgres: bool,
    /// 是否启用 sqlx mysql / Whether sqlx mysql is enabled
    pub sqlx_mysql: bool,
    /// 是否启用 sqlx sqlite / Whether sqlx sqlite is enabled
    pub sqlx_sqlite: bool,
    /// 是否启用 diesel / Whether diesel is enabled
    pub diesel: bool,
    /// 是否启用 diesel postgres / Whether diesel postgres is enabled
    pub diesel_postgres: bool,
    /// 是否启用 diesel mysql / Whether diesel mysql is enabled
    pub diesel_mysql: bool,
    /// 是否启用 diesel sqlite / Whether diesel sqlite is enabled
    pub diesel_sqlite: bool,
    /// 是否启用 sea-orm / Whether sea-orm is enabled
    pub sea_orm: bool,
    /// 是否启用 toasty / Whether toasty is enabled
    pub toasty: bool,
    /// 是否启用 rbatis / Whether rbatis is enabled
    pub rbatis: bool,
    /// 是否启用 cornucopia / Whether cornucopia is enabled
    pub cornucopia: bool,
    /// 是否启用 mongodb / Whether mongodb is enabled
    pub mongodb: bool,
    /// 是否启用 redis / Whether redis is enabled
    pub redis: bool,
}

impl PersistenceBackendFeatures {
    /// 从编译期 feature 创建快照。
    /// Create a snapshot from compile-time features.
    pub const fn current() -> Self {
        Self {
            sqlx: cfg!(feature = "persistence-sqlx"),
            sqlx_postgres: cfg!(feature = "persistence-sqlx-postgres"),
            sqlx_mysql: cfg!(feature = "persistence-sqlx-mysql"),
            sqlx_sqlite: cfg!(feature = "persistence-sqlx-sqlite"),
            diesel: cfg!(feature = "persistence-diesel"),
            diesel_postgres: cfg!(feature = "persistence-diesel-postgres"),
            diesel_mysql: cfg!(feature = "persistence-diesel-mysql"),
            diesel_sqlite: cfg!(feature = "persistence-diesel-sqlite"),
            sea_orm: cfg!(feature = "persistence-sea-orm"),
            toasty: cfg!(feature = "persistence-toasty"),
            rbatis: cfg!(feature = "persistence-rbatis"),
            cornucopia: cfg!(feature = "persistence-cornucopia"),
            mongodb: cfg!(feature = "persistence-mongodb"),
            redis: cfg!(feature = "persistence-redis"),
        }
    }

    /// 判断是否启用任意后端。
    /// Check whether any backend is enabled.
    pub const fn any_enabled(self) -> bool {
        self.sqlx
            || self.diesel
            || self.sea_orm
            || self.toasty
            || self.rbatis
            || self.cornucopia
            || self.mongodb
            || self.redis
    }
}
