# 启发式插件

:us: [English](README.md) | :cn: 简体中文

`ospf-rust-core-plugin-heuristic` 承载元启发式框架与选择型超启发式，对应 Kotlin 的
`ospf-kotlin-core-plugin-heuristic`。Kotlin 在该插件内同样保留这两个包，因此 Rust 的模块路径
保持原有形状：

| 模块 | 内容 |
| --- | --- |
| `heuristic` | 生命周期骨架、18 个算子族、组合工厂、算法入口、计费会话与原生约束。 |
| `hyper_heuristic` | 双层自适应信用、算子注册表与组合、HV 奖励、JSON/YAML/TOML 配置、持久化会话与调度重放。 |

两个模块只消费 `ospf-rust-core` 的 model/variable/callback 类型，不涉及任何求解器适配，
因此本插件在没有本地 solver 安装的环境下同样可用。

```rust,ignore
use ospf_rust_core_plugin_heuristic::heuristic::HeuristicSolver;
use ospf_rust_core_plugin_heuristic::hyper_heuristic::HyperHeuristicSolver;
```

模块级文档仍在代码旁：[`src/heuristic/README_ch.md`](src/heuristic/README_ch.md) 与
[`src/hyper_heuristic/README_ch.md`](src/hyper_heuristic/README_ch.md)。

## 验证

```bash
cargo test -p ospf-rust-core-plugin-heuristic --lib
```

可运行示例仍留在 core crate 的 `examples/`：`heuristic_demo` 与 `hyper_heuristic_demo`。
