# 持久化后端插件

:us: [English](README.md) | :cn: 简体中文

`ospf-rust-framework-plugin-persistence` 承载 framework 层的持久化后端 translator，对应
Kotlin 的 `ospf-kotlin-framework-plugin-persistence`。

| 模块 | 后端 | feature |
| --- | --- | --- |
| `sqlx` | SQLx 查询/CRUD 转换与关系查询编译 | `persistence-sqlx` |
| `diesel` | Diesel 查询转换 | `persistence-diesel` |
| `sea_orm` | SeaORM 查询转换 | `persistence-sea-orm` |
| `toasty` | Toasty 查询转换 | `persistence-toasty` |
| `rbatis` | Rbatis 查询转换 | `persistence-rbatis` |
| `cornucopia` | Cornucopia 查询转换 | `persistence-cornucopia` |
| `mongodb` | MongoDB 文档转换 | `persistence-mongodb` |
| `redis` | Redis 命令转换 | `persistence-redis` |

持久化契约、表达式与关系查询计划仍位于 `ospf-rust-framework::persistence`，本 crate 只负责把它们
转换为后端形态。framework 侧的 `PersistenceBackendFeatures` 快照仍留在 framework，并按
framework 自身的 feature 名推导，因此只需后端能力上报的宿主仍只依赖 framework。

Kotlin 把这一族拆成每后端一个 Maven 模块（`ospf-kotlin-framework-plugin-persistence-*`）。Rust 保留
单 crate，并把每个模块映射为一个 Cargo feature：它们保护的是可选编译边界，而不是独立产物；拆开只会
多出九个 crate，依赖图不会因此改变。

```bash
cargo test -p ospf-rust-framework-plugin-persistence --features persistence-sqlx
cargo test -p ospf-rust-framework-plugin-persistence --features persistence-sea-orm
```
