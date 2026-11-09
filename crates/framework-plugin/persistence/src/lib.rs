//! 持久化后端插件
//! Persistence backend plugin
//!
//! 本 crate 承载 framework 层的各持久化后端 translator，对应 Kotlin 的
//! `ospf-kotlin-framework-plugin-persistence`；持久化契约与查询计划仍位于
//! `ospf-rust-framework::persistence`。
//! This crate hosts the framework persistence-backend translators, mirroring Kotlin's
//! `ospf-kotlin-framework-plugin-persistence`; the contracts and query plans stay in
//! `ospf-rust-framework::persistence`.

#[cfg(feature = "persistence-cornucopia")]
pub mod cornucopia;
#[cfg(feature = "persistence-diesel")]
pub mod diesel;
#[cfg(feature = "persistence-mongodb")]
pub mod mongodb;
#[cfg(feature = "persistence-rbatis")]
pub mod rbatis;
#[cfg(feature = "persistence-redis")]
pub mod redis;
#[cfg(feature = "persistence-sea-orm")]
pub mod sea_orm;
#[cfg(feature = "persistence-sqlx")]
pub mod sqlx;
#[cfg(feature = "persistence-toasty")]
pub mod toasty;
