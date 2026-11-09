//! 续挖第 52 单：中间模型的转储选项——**两个边界开关互相独立、可矛盾**，固定变量表可被整表替换
//! （`model/intermediate/lp_export.rs`）
//! Continuation, delivery 52: the intermediate-model dump options — **the two bound switches are independent and can
//! contradict each other**, and the fixed-variable map can be replaced wholesale (`model/intermediate/lp_export.rs`).
//!
//! §11.14 点名了 `model/intermediate` 的可编码部分。真正导出文件要走 `LPExportableModel` 与 `fs`，那是 I/O 边界；
//! 但 `DumpOptions` 与 `ModelFileFormat` 是**纯数据**，而且它的默认值与建构器语义有明显的空档。七条实测：
//!
//! 1. **默认值是 `{concurrent: false, include_bounds: true, force_bounds: true}`**
//!    （`the_defaults_are_measured()`）：**"强制输出默认边界"默认就是开的**，也就是默认会写更多信息，不是更少。
//! 2. **`with_bounds` 与 `with_force_bounds` 是两个独立开关**（`the_two_bound_switches_are_independent()`）：
//!    `with_bounds(false).with_force_bounds(true)` 完全可表示——**本层不做一致性校验**，"不输出边界"同时"强制输出
//!    默认边界"这种矛盾组合只能由导出函数自己解释。
//! 3. **`with_fixed_variable` 是 map 插入**（`fixed_variable_inserts_are_last_write_wins()`）：同一 solver index
//!    后来居上，不同 index 累加。
//! 4. **`with_fixed_variables` 是整表替换、不是合并**（`with_fixed_variables_replaces_the_whole_map()`）：
//!    先前 `with_fixed_variable` 加的那条会被**丢掉**。
//! 5. **`without_fixed_variables` 只清 map**（`without_fixed_variables_clears_only_the_map()`）：三个布尔开关原样保留。
//! 6. **扩展名只有 `lp` 与 `opm` 两个小写值**（`the_extensions_are_lowercase()`），与 `Debug` 里的变体名无关。
//! 7. **`DumpOptions::new()` 与 `Default` 相同**（`new_equals_default()`）。
//!
//! The §11.14 audit names the codable part of `model/intermediate`. Real dumping needs `LPExportableModel` plus `fs`, which is an
//! I/O boundary; `DumpOptions` and `ModelFileFormat` are **pure data**, with a visible gap in their defaults and builder
//! semantics. **The defaults are `{concurrent: false, include_bounds: true, force_bounds: true}`**
//! (`the_defaults_are_measured()`): **forcing default bounds is on by default**, so the default emits *more*, not less.
//! **`with_bounds` and `with_force_bounds` are independent switches** (`the_two_bound_switches_are_independent()`):
//! `with_bounds(false).with_force_bounds(true)` is perfectly representable and **this layer never cross-checks it**, so the
//! contradictory pair "do not emit bounds" plus "force default bounds" is left for the dump function to interpret.
//! **`with_fixed_variable` is a map insert** (`fixed_variable_inserts_are_last_write_wins()`). **`with_fixed_variables` replaces
//! the whole map instead of merging** (`with_fixed_variables_replaces_the_whole_map()`), so an earlier
//! `with_fixed_variable` entry is **dropped**. **`without_fixed_variables` clears only the map**
//! (`without_fixed_variables_clears_only_the_map()`). **There are only two lowercase extensions, `lp` and `opm`**
//! (`the_extensions_are_lowercase()`). And **`DumpOptions::new()` equals `Default`** (`new_equals_default()`).

use ospf_rust_core::model::intermediate::{DumpOptions, ModelFileFormat};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一次转储选项的字面量。 / One dump-options literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DumpOptionsSpec {
    /// `with_concurrent` 的取值；缺省不动。 / The `with_concurrent` value; absent leaves it alone.
    #[serde(default)]
    pub concurrent: Option<bool>,
    /// `with_bounds` 的取值；缺省不动。 / The `with_bounds` value; absent leaves it alone.
    #[serde(default)]
    pub include_bounds: Option<bool>,
    /// `with_force_bounds` 的取值；缺省不动。 / The `with_force_bounds` value; absent leaves it alone.
    #[serde(default)]
    pub force_bounds: Option<bool>,
    /// 先逐条 `with_fixed_variable`。 / The `with_fixed_variable` calls, in order.
    #[serde(default)]
    pub fixed_variables: Vec<FixedVariableSpec>,
    /// 之后一次 `with_fixed_variables`（整表替换）。 / A single `with_fixed_variables` afterwards (replaces the map).
    #[serde(default)]
    pub replaced_variables: Option<Vec<FixedVariableSpec>>,
    /// 最后是否 `without_fixed_variables`。 / Whether `without_fixed_variables` runs last.
    #[serde(default)]
    pub clear_variables: bool,
}

/// 一个固定变量。 / One fixed variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixedVariableSpec {
    /// solver index。 / The solver index.
    pub index: u32,
    /// 固定值。 / The fixed value.
    pub value: f64,
}

/// 转储选项的结论。 / The dump-options verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DumpOptionsReport {
    /// 最终 `concurrent`。 / The final `concurrent`.
    pub concurrent: bool,
    /// 最终 `include_bounds`。 / The final `include_bounds`.
    pub include_bounds: bool,
    /// 最终 `force_bounds`。 / The final `force_bounds`.
    pub force_bounds: bool,
    /// 最终固定变量条数。 / The final fixed-variable count.
    pub fixed_count: u64,
    /// 固定变量的 index（升序）。 / The fixed indices (ascending).
    pub fixed_indices: Vec<u64>,
    /// 固定变量的值（与 `fixed_indices` 对齐）。 / The fixed values (aligned with `fixed_indices`).
    pub fixed_values: Vec<f64>,
    /// 默认值的字面量。 / The default literal.
    pub defaults: DumpOptionsReportDefaults,
    /// 两种格式的扩展名。 / The extensions of both formats.
    pub extensions: Vec<String>,
    /// 默认值就是那三个（**是**）。 / Whether the defaults are exactly those three (**yes**).
    pub the_defaults_are_measured: bool,
    /// 两个边界开关互相独立（**是**）。 / Whether the two bound switches are independent (**yes**).
    pub the_two_bound_switches_are_independent: bool,
    /// 同 index 后来居上（**是**）。 / Whether the same index is last-write-wins (**yes**).
    pub fixed_variable_inserts_are_last_write_wins: bool,
    /// `with_fixed_variables` 整表替换（**是**）。 / Whether `with_fixed_variables` replaces the map (**yes**).
    pub with_fixed_variables_replaces_the_whole_map: bool,
    /// `without_fixed_variables` 只清 map（**是**）。 / Whether `without_fixed_variables` clears only the map (**yes**).
    pub without_fixed_variables_clears_only_the_map: bool,
    /// 扩展名是小写二值（**是**）。 / Whether the extensions are two lowercase values (**yes**).
    pub the_extensions_are_lowercase: bool,
    /// `new()` 与 `Default` 相同（**是**）。 / Whether `new()` equals `Default` (**yes**).
    pub new_equals_default: bool,
}

/// 默认值的字面量。 / The default literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DumpOptionsReportDefaults {
    pub concurrent: bool,
    pub include_bounds: bool,
    pub force_bounds: bool,
    pub fixed_count: u64,
}

/// 固定变量条数上限（信任边界）。 / The fixed-variable cap (a trust boundary).
const MAX_VARIABLES: usize = 256;

/// 造一次转储选项。 / Build one set of dump options.
fn options_of(spec: &DumpOptionsSpec) -> Result<DumpOptions> {
    if spec.fixed_variables.len() > MAX_VARIABLES
        || spec.replaced_variables.as_ref().map(|items| items.len()).unwrap_or(0) > MAX_VARIABLES
    {
        return Err(BridgeError::new("InvalidInput", &format!(
            "dump options failed validation: at most {MAX_VARIABLES} fixed variables are accepted")));
    }
    let mut options = DumpOptions::new();
    if let Some(concurrent) = spec.concurrent {
        options = options.with_concurrent(concurrent);
    }
    if let Some(include_bounds) = spec.include_bounds {
        options = options.with_bounds(include_bounds);
    }
    if let Some(force_bounds) = spec.force_bounds {
        options = options.with_force_bounds(force_bounds);
    }
    for variable in &spec.fixed_variables {
        if !variable.value.is_finite() {
            return Err(BridgeError::new("InvalidInput",
                "dump options failed validation: a fixed value must be finite"));
        }
        options = options.with_fixed_variable(variable.index as usize, variable.value);
    }
    if let Some(replaced) = &spec.replaced_variables {
        if replaced.iter().any(|variable| !variable.value.is_finite()) {
            return Err(BridgeError::new("InvalidInput",
                "dump options failed validation: a fixed value must be finite"));
        }
        options = options.with_fixed_variables(replaced.iter()
            .map(|variable| (variable.index as usize, variable.value)));
    }
    if spec.clear_variables {
        options = options.without_fixed_variables();
    }
    Ok(options)
}

/// 走一遍转储选项。非法输入是**错误**。 / Walk the dump options.
pub fn verify_dump_options(request: &DumpOptionsSpec) -> Result<DumpOptionsReport> {
    let options = options_of(request)?;
    let defaults = DumpOptions::new();
    let findings = canonical_findings();
    Ok(DumpOptionsReport { concurrent: options.concurrent,
        include_bounds: options.include_bounds, force_bounds: options.force_bounds,
        fixed_count: options.fixed_variables.len() as u64,
        fixed_indices: options.fixed_variables.keys().map(|index| *index as u64).collect(),
        fixed_values: options.fixed_variables.values().copied().collect(),
        defaults: DumpOptionsReportDefaults { concurrent: defaults.concurrent,
            include_bounds: defaults.include_bounds, force_bounds: defaults.force_bounds,
            fixed_count: defaults.fixed_variables.len() as u64 },
        extensions: vec![ModelFileFormat::Lp.extension().to_owned(),
            ModelFileFormat::Opm.extension().to_owned()],
        the_defaults_are_measured: findings.the_defaults_are_measured,
        the_two_bound_switches_are_independent: findings.the_two_bound_switches_are_independent,
        fixed_variable_inserts_are_last_write_wins:
            findings.fixed_variable_inserts_are_last_write_wins,
        with_fixed_variables_replaces_the_whole_map:
            findings.with_fixed_variables_replaces_the_whole_map,
        without_fixed_variables_clears_only_the_map:
            findings.without_fixed_variables_clears_only_the_map,
        the_extensions_are_lowercase: findings.the_extensions_are_lowercase,
        new_equals_default: findings.new_equals_default })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    the_defaults_are_measured: bool,
    the_two_bound_switches_are_independent: bool,
    fixed_variable_inserts_are_last_write_wins: bool,
    with_fixed_variables_replaces_the_whole_map: bool,
    without_fixed_variables_clears_only_the_map: bool,
    the_extensions_are_lowercase: bool,
    new_equals_default: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 默认值就是那三个开关的组合。 / The defaults are exactly that combination.
    let the_defaults_are_measured = {
        let defaults = DumpOptions::default();
        !defaults.concurrent && defaults.include_bounds && defaults.force_bounds
            && defaults.fixed_variables.is_empty()
    };
    // 2) 两个边界开关互相独立（可以互相矛盾）。 / The two bound switches are independent, even contradictory.
    let the_two_bound_switches_are_independent = {
        let contradictory = DumpOptions::new().with_bounds(false).with_force_bounds(true);
        !contradictory.include_bounds && contradictory.force_bounds
            && DumpOptions::new().with_force_bounds(false).include_bounds
    };
    // 3) 同 index 后来居上、不同 index 累加。 / The same index is last-write-wins; different indices accumulate.
    let fixed_variable_inserts_are_last_write_wins = {
        let options = DumpOptions::new().with_fixed_variable(3, 1.5)
            .with_fixed_variable(3, 2.5).with_fixed_variable(7, 9.0);
        options.fixed_variables.len() == 2
            && options.fixed_variables.get(&3).copied() == Some(2.5)
            && options.fixed_variables.get(&7).copied() == Some(9.0)
    };
    // 4) `with_fixed_variables` 整表替换。 / `with_fixed_variables` replaces the whole map.
    let with_fixed_variables_replaces_the_whole_map = {
        let options = DumpOptions::new().with_fixed_variable(1, 1.0)
            .with_fixed_variables([(2usize, 2.0)]);
        options.fixed_variables.len() == 1 && options.fixed_variables.get(&1).is_none()
            && options.fixed_variables.get(&2).copied() == Some(2.0)
    };
    // 5) `without_fixed_variables` 只清 map。 / `without_fixed_variables` clears only the map.
    let without_fixed_variables_clears_only_the_map = {
        let options = DumpOptions::new().with_concurrent(true).with_bounds(false)
            .with_force_bounds(false).with_fixed_variable(5, 1.0).without_fixed_variables();
        options.fixed_variables.is_empty() && options.concurrent && !options.include_bounds
            && !options.force_bounds
    };
    // 6) 扩展名是小写二值。 / The extensions are two lowercase values.
    let the_extensions_are_lowercase = ModelFileFormat::Lp.extension() == "lp"
        && ModelFileFormat::Opm.extension() == "opm"
        && ModelFileFormat::Lp != ModelFileFormat::Opm;
    // 7) `new()` 与 `Default` 相同。 / `new()` equals `Default`.
    let new_equals_default = DumpOptions::new() == DumpOptions::default();
    Findings { the_defaults_are_measured, the_two_bound_switches_are_independent,
        fixed_variable_inserts_are_last_write_wins, with_fixed_variables_replaces_the_whole_map,
        without_fixed_variables_clears_only_the_map, the_extensions_are_lowercase,
        new_equals_default }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(index: u32, value: f64) -> FixedVariableSpec {
        FixedVariableSpec { index, value }
    }

    fn request() -> DumpOptionsSpec {
        DumpOptionsSpec { concurrent: Some(true), include_bounds: Some(false),
            force_bounds: Some(true), fixed_variables: vec![fixed(3, 1.5), fixed(3, 2.5)],
            replaced_variables: None, clear_variables: false }
    }

    #[test]
    fn the_defaults_are_measured() {
        let report = verify_dump_options(&DumpOptionsSpec { concurrent: None,
            include_bounds: None, force_bounds: None, fixed_variables: Vec::new(),
            replaced_variables: None, clear_variables: false }).unwrap();
        assert!(!report.defaults.concurrent);
        assert!(report.defaults.include_bounds);
        assert!(report.defaults.force_bounds);
        assert_eq!(report.defaults.fixed_count, 0);
        // 不传任何开关时，最终值就是默认值。 / With no switches passed, the result is the default.
        assert!(!report.concurrent);
        assert!(report.include_bounds);
        assert!(report.force_bounds);
        assert_eq!(report.extensions, vec!["lp".to_owned(), "opm".to_owned()]);
    }

    #[test]
    fn the_two_bound_switches_are_independent() {
        let report = verify_dump_options(&request()).unwrap();
        assert!(!report.include_bounds);
        assert!(report.force_bounds);
        assert!(report.the_two_bound_switches_are_independent);
        assert!(report.the_defaults_are_measured);
        assert!(report.the_extensions_are_lowercase);
        assert!(report.new_equals_default);
    }

    #[test]
    fn fixed_variables_are_last_write_wins() {
        let report = verify_dump_options(&request()).unwrap();
        assert_eq!(report.fixed_count, 1);
        assert_eq!(report.fixed_indices, vec![3]);
        assert_eq!(report.fixed_values, vec![2.5]);
        assert!(report.fixed_variable_inserts_are_last_write_wins);
    }

    #[test]
    fn replacing_the_map_drops_earlier_entries() {
        let report = verify_dump_options(&DumpOptionsSpec {
            fixed_variables: vec![fixed(1, 1.0)],
            replaced_variables: Some(vec![fixed(2, 2.0), fixed(4, 4.0)]),
            ..request() }).unwrap();
        assert_eq!(report.fixed_indices, vec![2, 4]);
        assert!(report.with_fixed_variables_replaces_the_whole_map);
        assert!(report.without_fixed_variables_clears_only_the_map);
        // 清空之后开关不动。 / Clearing leaves the switches alone.
        let report = verify_dump_options(&DumpOptionsSpec { clear_variables: true,
            ..request() }).unwrap();
        assert_eq!(report.fixed_count, 0);
        assert!(report.concurrent);
        assert!(!report.include_bounds);
        assert!(report.force_bounds);
    }

    #[test]
    fn bad_input_is_refused() {
        let non_finite = DumpOptionsSpec { fixed_variables: vec![fixed(0, f64::NAN)],
            ..request() };
        assert!(verify_dump_options(&non_finite).unwrap_err().message
            .contains("must be finite"));
        let too_many = DumpOptionsSpec {
            fixed_variables: (0..257).map(|index| fixed(index as u32, 0.0)).collect(),
            ..request() };
        assert!(verify_dump_options(&too_many).unwrap_err().message
            .contains("at most 256 fixed variables"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_dump_options(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"includeBounds\""), "{json}");
        assert!(json.contains("\"fixedIndices\""), "{json}");
        assert!(json.contains("\"theTwoBoundSwitchesAreIndependent\""), "{json}");
        assert!(!json.contains("include_bounds"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<DumpOptionsReport>(&copy.to_string()).is_err());
    }
}
