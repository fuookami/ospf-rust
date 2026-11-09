# Persistence Backend Plugin

:us: English | :cn: [简体中文](README_ch.md)

`ospf-rust-framework-plugin-persistence` carries the framework persistence-backend translators,
mirroring Kotlin's `ospf-kotlin-framework-plugin-persistence`.

| Module | Backend | Feature |
| --- | --- | --- |
| `sqlx` | SQLx query/CRUD translation plus relational-query compilation | `persistence-sqlx` |
| `diesel` | Diesel query translation | `persistence-diesel` |
| `sea_orm` | SeaORM query translation | `persistence-sea-orm` |
| `toasty` | Toasty query translation | `persistence-toasty` |
| `rbatis` | Rbatis query translation | `persistence-rbatis` |
| `cornucopia` | Cornucopia query translation | `persistence-cornucopia` |
| `mongodb` | MongoDB document translation | `persistence-mongodb` |
| `redis` | Redis command translation | `persistence-redis` |

Contracts, expressions, and relational query plans stay in
`ospf-rust-framework::persistence`; this crate only translates them to backend shapes. The
framework-side `PersistenceBackendFeatures` snapshot remains in framework and is derived from
framework's own feature names, so a host that needs backend capability reporting keeps depending
only on framework.

Kotlin splits this family into one Maven module per backend (`ospf-kotlin-framework-plugin-persistence-*`).
Rust keeps a single crate and maps each of those modules to one Cargo feature, because the boundary they
protect is optional compilation, not artefact separation; splitting would add nine crates without changing
the dependency graph.

```bash
cargo test -p ospf-rust-framework-plugin-persistence --features persistence-sqlx
cargo test -p ospf-rust-framework-plugin-persistence --features persistence-sea-orm
```
