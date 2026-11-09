# ospf-rust-framework-geometry2d

:us: [English](README.md) | :cn: 简体中文

## 简介

`ospf-rust-framework-geometry2d` 是 `ospf-rust` 的共享二维量纲几何内核。它被抽取出来，使装箱框架
`ospf-rust-framework-bpp2d` 与套裁框架 `ospf-rust-framework-csp2d` 复用同一套量纲化二维几何，
而不是各自维护一份。

本 crate 与求解器无关：只依赖 `ospf-rust-base`、`ospf-rust-math` 与 `ospf-rust-quantities`。

`Box2Geometry::new` 与 `of_rectangle` 是纯构造方法，支持运行时单位及原域高精度值，
不要求 `Float`。独立的 `rectangle_overlapped` 路径只依赖原域有序加法，要求量纲及
比例尺相同，贴边不算重叠；单位／形状不兼容或数值不可比较时返回 `None`。
调用方须先校验有限正尺寸；其他形状／包含 API 保留原有 trait 边界。

## 作用范围

| 领域 | 类型 |
| --- | --- |
| 量值算术 | `quantity_plus_safe`、`quantity_minus_safe`、`quantity_multiply`、`quantity_ord_safe`/`quantity_ord_or_null`、`quantity_greater`/`quantity_less`/`quantity_greater_or_equal`/`quantity_less_or_equal`、`quantity_zero_of`、`quantity_of`、`quantity_scaled_by`、`quantity_sum`、`sum_of_quantity`、`sorted_by_quantity`、`sorted_by_descending_quantity`、`count_fitting`、`scaled_quantity`、`quantity_max_safe`/`quantity_min_safe`、`quantity_max_of`、`is_same_dimension` |
| 点与向量 | `QuantityPoint2`、`QuantityVector2`、`point2_of`、`vector2_of` |
| 形状 | `PackingShape2`（`Rectangle`/`Circle`）、`RectanglePackingShape2`、`CirclePackingShape2`、`PackingShape2Type`、`Shape2Candidate`、`candidate_shapes`、`rectangle_shape2`、`circle_shape2` |
| 放置与包含 | `Placement2`、`Box2Geometry` |
| 板材几何 | `Sheet2Geometry`、`Sheet2Shape`、`sheet2_shape`、`origin_box` |
| 刀缝 | `inflated_by_shape`、`inflated_by_placement`、`pitch_of`、`zero_spacing_of` |
| 面积与边界 | `area_of`、`bounding_width`、`bounding_height`、`bounding_area`、`min_x`、`min_y`、`max_x`、`max_y`、`inside_sheet` |

明确非目标：

1. 业务 DTO、请求协议、租户上下文或公式语言。
2. solver backend 及其安装与许可。
3. 外部数据集格式与读取；CSV schema 属于使用方 framework crate。

## 面积口径

`area` 是实际占用面积，`bounding_area` 是外接矩形面积：

1. 矩形件：`area = width × height`，`bounding_area = width × height`。
2. 圆形件：`area = πr²`，`bounding_area = 4r²`。

目标函数与 KPI 使用 `area`；网格生成使用包围尺寸。

## 重叠语义

`Box2Geometry::overlapped` 判定**包围盒内部重叠**，包围盒贴边不算重叠。圆使用外接正方形，
不是精确圆盘相交；斜向分离或相切的圆盘，其外接正方形仍可能重叠。点/盒包含也使用闭区间
包围矩形，而不是圆形包含。这是当前实现行为，不是语言桥接新引入的碰撞算法。

## 刀缝

刀缝通过**膨胀参与重叠判定的形状**来表达：

1. `inflated_by_shape`：矩形宽高各加 `spacing`；圆形直径加 `spacing`。
2. `pitch_of` 返回包围尺寸加间距，使相邻件的真实间隙恰为间距。
3. 几何校验必须**双向膨胀**：只有把任一侧膨胀后仍相交，才说明真实间隙小于刀缝。单向膨胀会漏掉
   另一侧的间隙不足。

`inflated_by_placement` 当前保留原左下角和元数据，尺寸向右/上增加，不是中心不变的膨胀。
此前“中心不变”的描述不准确；本次澄清不改变实现行为。

## TypeScript / WASM 边界

`ospf-wasm/geometry2d` 提供 `Geometry2d` f64 门面及 Rust 生成契约，使用自然类型名
`QuantityPoint2`、`QuantityVector2`、`PackingShape2`、`Box2Geometry`、`Placement2`、`Sheet2Shape`、
`Shape2Candidate`。桥接先将 m/km/cm/mm 归一到原 Meter 特化，再调用现有点/向量、形状、盒、板材
和刀缝 API；输出采用显式 context 单位，面积采用对应平方长度单位，不在 TypeScript 复制算法。

返回 DTO 和 `Uint32Array` 重叠对索引都是独立副本，仅 context 需要 dispose。输入须有限、量纲兼容、
尺寸为正、间距非负。溢出/下溢、坐标精度导致边界塌缩、未知 DTO 字段、超过 512 盒的批量输入
均明确拒绝；文本输入限 8 MiB。`overlapPairs` 使用原盒判定，完整返回全部索引对而不截断。
不因此承诺精确圆盘碰撞、求解器、BPP2D/CSP2D 流程或任意精度 TS 特化。
见[桥接能力矩阵](../../../docs/typescript-bridge_ch.md#共享二维几何)。

## 泛型数值边界

1. 所有类型泛型于 `V`，并保留编译时单位 `U`。
2. 坐标、尺寸与面积使用 `Quantity<V, U>`；无量纲值使用裸 `V`。
3. 面积由 `quantity_multiply` 产生，量纲按基础幂次相加合成，因此长度平方不会被算成立方。
4. 泛型代码不得调用 `to_f64()`；计数与缩放通过 `count_fitting` 与 `scaled_quantity` 表达。

## 使用方

| 框架 | 用途 |
| --- | --- |
| `ospf-rust-framework-bpp2d` | 矩形与圆形件的二维装箱 |
| `ospf-rust-framework-csp2d` | 矩形件的二维套裁 |

## 本地验证

```powershell
$env:RUSTC_BOOTSTRAP = "1"
cargo test -p ospf-rust-framework-geometry2d
```

## 相关文档

- [ospf-rust README](../../../README_ch.md) 列出全部 crate 及其 Kotlin 边界。
- [`tests/geometry2d_contract.rs`](tests/geometry2d_contract.rs) 固化面积、重叠、刀缝与包含契约。
