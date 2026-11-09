//! 续挖第 18 单：数值文本的 Kotlin 兼容格式与它的单向性
//! （`solver/heuristic/session/codecs.rs::format_number` / `parse_number` / 四个编解码器）
//! Continuation, delivery 18: the Kotlin-compatible number text and how one-way it is.
//!
//! 这套编解码器的存在理由是**跨语言快照可互读**：Kotlin 侧 `Double.toString()` 的文本格式必须逐字复现。所以
//! `format_number` 不是随便的 `Display`，它带着一整组边界：
//!
//! 1. **整数值补 `.0`**：`1` → `"1.0"`（Kotlin 的 `Double.toString()` 行为）。
//! 2. **科学计数法的门槛是 `[1e-3, 1e7)`**（左闭右开）：`1e-3` **在内**、`1e7` **在外**，
//!    所以 `0.001` → `"0.001"` 而 `1e7` → `"1.0E7"`。门槛上下各差一个量级就变形状。
//! 3. **指数用大写 `E`、尾数补 `.0`**：`1e300` → `"1.0E300"`、`1e-4` → `"1.0E-4"`。
//! 4. **`-0.0` 文本保留符号**：`"-0.0"`（数值上 `-0.0 == 0.0`，所以这一条只能看文本）。
//! 5. **非有限值是单向的**：`format_number` 对 `NaN` / `±inf` 直接走 `f64::to_string()`（产 `"NaN"` / `"inf"` /
//!    `"-inf"`），而 `parse_number` **过滤非有限**（返回 `None`）。所以 **`format_number` 的输出不总能被
//!    `parse_number` 读回**——它不是往返函数，只有有限值往返。注意 Rust 的 `"inf"` 也不是 Kotlin 的 `"Infinity"`。
//! 6. **`RawNumberMatrixCodec` 是刻意不对称的**：编码出 **JSON 数字**，解码却**同时接受数字与字符串**（"容忍历史快照"）。
//!    所以它的解码比编码宽，这不是 bug。
//!
//! The codecs exist so snapshots stay readable across languages: Kotlin's `Double.toString()` text must be reproduced verbatim,
//! so `format_number` is not a casual `Display` and carries a set of boundaries. **Integral values get `.0`** (`1` → `"1.0"`).
//! **The scientific threshold is `[1e-3, 1e7)`**, half-open, so `1e-3` is **inside** and `1e7` **outside**: `0.001` renders as
//! `"0.001"` while `1e7` renders as `"1.0E7"` — one order of magnitude apart swaps the shape. **The exponent uses an uppercase
//! `E` with a `.0` mantissa** (`1e300` → `"1.0E300"`). **`-0.0` keeps its sign in text** (`"-0.0"`; numerically
//! `-0.0 == 0.0`, so only the text shows it). **Non-finite values are one-way**: `format_number` delegates them to
//! `f64::to_string()` (yielding `"NaN"` / `"inf"` / `"-inf"`) while `parse_number` **filters non-finite out**, so its output
//! **cannot always be read back** — only finite values round-trip, and Rust's `"inf"` is not Kotlin's `"Infinity"` either.
//! Finally **`RawNumberMatrixCodec` is deliberately asymmetric**: it emits **JSON numbers** but decodes **both numbers and
//! strings** ("tolerate older snapshots"), so its decoder is wider than its encoder — not a bug.

use ospf_rust_core_plugin_heuristic::heuristic::session::codecs::{
    format_number, parse_number, NumberMatrixCodec, NumberVectorCodec, RawNumberMatrixCodec,
    TextNumberCodec,
};
use ospf_rust_core_plugin_heuristic::heuristic::session::json::numbers as json_numbers;
use ospf_rust_core_plugin_heuristic::heuristic::session::JsonValue;
use ospf_rust_core_plugin_heuristic::heuristic::session::SearchCodec;
use ospf_rust_core_plugin_heuristic::heuristic::HeuristicError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个字面浮点（JSON 放不下 `NaN` / `inf`）。 / A literal float (JSON cannot carry NaN or inf).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextFloatSpec {
    /// `number` / `nan` / `inf` / `negInf`。 / `number` / `nan` / `inf` / `negInf`.
    pub kind: String,
    /// 数值文本（`number` 时使用）。 / The numeric text (used when `kind` is `number`).
    #[serde(default)]
    pub text: String,
}

/// 编解码请求。 / The codec request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumberTextRequest {
    /// 逐个格式化的数值。 / The values to format one by one.
    #[serde(default)]
    pub floats: Vec<TextFloatSpec>,
    /// 一个向量（走 `NumberVectorCodec`）。 / One vector through `NumberVectorCodec`.
    #[serde(default)]
    pub vector: Vec<TextFloatSpec>,
    /// 一个矩阵（走 `NumberMatrixCodec`）。 / One matrix through `NumberMatrixCodec`.
    #[serde(default)]
    pub matrix: Vec<Vec<TextFloatSpec>>,
    /// 一个原生矩阵（走 `RawNumberMatrixCodec`）。 / One matrix through `RawNumberMatrixCodec`.
    #[serde(default)]
    pub raw_matrix: Vec<Vec<TextFloatSpec>>,
}

/// 一个数值的文本结论。 / One value's text verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumberTextRow {
    /// 输入的描述。 / The input description.
    pub input: String,
    /// `format_number` 的输出。 / The output of `format_number`.
    pub formatted: String,
    /// `parse_number` 能否读回。 / Whether `parse_number` reads it back.
    pub reparsed: bool,
    /// 读回的值是否与原值相等（**非有限值恒假**）。 / Whether the value round-trips (**always false for non-finite**).
    pub round_trips: bool,
    /// `TextNumberCodec` 是否接受编码（**非有限值不接受**）。 / Whether the codec encodes it.
    pub codec_encodes: bool,
}

/// 编解码结论。 / The codec verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumberTextReport {
    /// 逐个数值的结论。 / Each value's verdict.
    pub floats: Vec<NumberTextRow>,
    /// 向量的编码文本。 / The encoded vector text.
    pub vector_json: String,
    /// 向量是否往返成功。 / Whether the vector round-trips.
    pub vector_round_trips: bool,
    /// 矩阵的编码文本。 / The encoded matrix text.
    pub matrix_json: String,
    /// 矩阵是否往返成功。 / Whether the matrix round-trips.
    pub matrix_round_trips: bool,
    /// 原生矩阵的编码文本（**JSON 数字**）。 / The raw matrix's encoded text (**JSON numbers**).
    pub raw_json: String,
    /// 原生矩阵是否往返成功。 / Whether the raw matrix round-trips.
    pub raw_round_trips: bool,
    /// 原生矩阵能否读回**字符串**形式的历史快照（**是**）。 / Whether it reads back string-only legacy snapshots (**yes**).
    pub raw_accepts_legacy_strings: bool,
    /// `json::numbers` 把非有限值变成 0（**是**）。 / Whether `json::numbers` turns non-finite into zero (**yes**).
    pub numbers_degrades_non_finite: bool,
    /// 边界探测：`1e-3` 的文本（门槛内）。 / The text of `1e-3` (inside the threshold).
    pub boundary_lower: String,
    /// 边界探测：`1e7` 的文本（门槛外）。 / The text of `1e7` (outside).
    pub boundary_upper: String,
    /// 边界探测：`1` 的文本（补 `.0`）。 / The text of `1` (gets `.0`).
    pub integral: String,
    /// 边界探测：`-0.0` 的文本。 / The text of `-0.0`.
    pub negative_zero: String,
    /// 边界探测：`NaN` 的文本。 / The text of `NaN`.
    pub nan_text: String,
    /// `NaN` 能否被 `parse_number` 读回（**否**）。 / Whether `NaN` can be read back (**no**).
    pub nan_round_trips: bool,
}

/// 条目上限（信任边界）。 / The entry cap (a trust boundary).
const MAX_ENTRIES: usize = 4_096;

/// 走一遍数值文本编解码。非法输入是**错误**。 / Walk the number-text codecs.
pub fn verify_number_text(request: &NumberTextRequest) -> Result<NumberTextReport> {
    let total = request.floats.len()
        + request.vector.len()
        + request.matrix.iter().map(Vec::len).sum::<usize>()
        + request.raw_matrix.iter().map(Vec::len).sum::<usize>();
    if total > MAX_ENTRIES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "number text failed validation: at most {MAX_ENTRIES} components are accepted")));
    }
    let mut floats = Vec::with_capacity(request.floats.len());
    for spec in &request.floats {
        floats.push(observe(spec)?);
    }
    let vector = request.vector.iter().map(literal).collect::<Result<Vec<f64>>>()?;
    let vector_encoded = NumberVectorCodec.encode(&vector).map_err(codec_error)?;
    let vector_back = NumberVectorCodec.decode(&vector_encoded).map_err(codec_error)?;
    let matrix = request.matrix.iter()
        .map(|row| row.iter().map(literal).collect::<Result<Vec<f64>>>())
        .collect::<Result<Vec<Vec<f64>>>>()?;
    let matrix_encoded = NumberMatrixCodec.encode(&matrix).map_err(codec_error)?;
    let matrix_back = NumberMatrixCodec.decode(&matrix_encoded).map_err(codec_error)?;
    let raw = request.raw_matrix.iter()
        .map(|row| row.iter().map(literal).collect::<Result<Vec<f64>>>())
        .collect::<Result<Vec<Vec<f64>>>>()?;
    let raw_encoded = RawNumberMatrixCodec.encode(&raw).map_err(codec_error)?;
    let raw_back = RawNumberMatrixCodec.decode(&raw_encoded).map_err(codec_error)?;
    // 历史快照只有字符串：把原生矩阵的编码结果整体转成字符串数组，看解码器是否仍接受。
    // A legacy snapshot is strings only: stringify the whole raw encoding and see whether the decoder still takes it.
    let legacy = stringify(&raw_encoded);
    let raw_accepts_legacy_strings = RawNumberMatrixCodec.decode(&legacy).is_ok();
    // `json::numbers` 用 `Number::from_f64(..).unwrap_or(0)`：非有限值变成 0。
    // `json::numbers` falls back to zero via `Number::from_f64(..).unwrap_or(0)`.
    // 用 `as_f64` 比较：整型数与浮点数在 `serde_json::Number` 里不相等，但数字值相等。
    // Compare through `as_f64`: integer and float numbers are unequal as `serde_json::Number` but equal as values.
    let degraded = json_numbers([f64::NAN, f64::INFINITY, 1.0]);
    let numbers_degrades_non_finite = degraded.as_array()
        .map(|items| items.iter().map(|item| item.as_f64()).collect::<Vec<_>>())
        == Some(vec![Some(0.0), Some(0.0), Some(1.0)]);
    let nan_text = format_number(f64::NAN);
    Ok(NumberTextReport {
        floats,
        vector_json: vector_encoded.to_string(),
        vector_round_trips: same(&vector, &vector_back),
        matrix_json: matrix_encoded.to_string(),
        matrix_round_trips: same_matrix(&matrix, &matrix_back),
        raw_json: raw_encoded.to_string(),
        raw_round_trips: same_matrix(&raw, &raw_back),
        raw_accepts_legacy_strings,
        numbers_degrades_non_finite,
        boundary_lower: format_number(1.0e-3),
        boundary_upper: format_number(1.0e7),
        integral: format_number(1.0),
        negative_zero: format_number(-0.0),
        nan_round_trips: parse_number(&nan_text).is_some(),
        nan_text,
    })
}

/// 观察一个数值的文本行为。 / Observe one value's text behaviour.
fn observe(spec: &TextFloatSpec) -> Result<NumberTextRow> {
    let value = literal(spec)?;
    let formatted = format_number(value);
    let reparsed = parse_number(&formatted);
    Ok(NumberTextRow {
        input: input_text(spec, value),
        round_trips: reparsed.is_some_and(|back| back == value),
        reparsed: reparsed.is_some(),
        codec_encodes: TextNumberCodec.encode(&value).is_ok(),
        formatted,
    })
}

/// 解析一个字面浮点。 / Parse a literal float.
fn literal(spec: &TextFloatSpec) -> Result<f64> {
    match spec.kind.as_str() {
        "nan" => Ok(f64::NAN),
        "inf" => Ok(f64::INFINITY),
        "negInf" => Ok(f64::NEG_INFINITY),
        "number" => spec.text.trim().parse::<f64>().map_err(|_| BridgeError::new("InvalidInput",
            &format!("number text failed validation: `{}` is not a float", spec.text))),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "number text failed validation: unknown float kind `{other}`"))),
    }
}

/// 输入的描述文本。 / The input description.
fn input_text(spec: &TextFloatSpec, value: f64) -> String {
    match spec.kind.as_str() {
        "number" if value == 0.0 && value.is_sign_negative() => "-0".to_owned(),
        "number" => spec.text.trim().to_owned(),
        other => other.to_owned(),
    }
}

/// 两个向量是否相等。 / Whether two vectors are equal.
fn same(left: &[f64], right: &[f64]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| a == b)
}

/// 两个矩阵是否相等。 / Whether two matrices are equal.
fn same_matrix(left: &[Vec<f64>], right: &[Vec<f64>]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| same(a, b))
}

/// 把编码结果里的所有数字改成字符串（模拟历史快照）。 / Turn every number into a string (a legacy snapshot).
fn stringify(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(stringify).collect()),
        JsonValue::Number(number) => JsonValue::String(number.to_string()),
        other => other.clone(),
    }
}

/// 编解码错误。 / A codec error.
fn codec_error(error: HeuristicError) -> BridgeError {
    BridgeError::new("InvalidInput", &format!("number text failed validation: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(text: &str) -> TextFloatSpec {
        TextFloatSpec { kind: "number".to_owned(), text: text.to_owned() }
    }

    fn special(kind: &str) -> TextFloatSpec {
        TextFloatSpec { kind: kind.to_owned(), text: String::new() }
    }

    fn request(floats: Vec<TextFloatSpec>) -> NumberTextRequest {
        NumberTextRequest { floats, vector: Vec::new(), matrix: Vec::new(),
            raw_matrix: Vec::new() }
    }

    #[test]
    fn integral_values_get_a_point_zero_and_the_scientific_threshold_is_half_open() {
        let report = verify_number_text(&request(Vec::new())).unwrap();
        // `1` → `"1.0"`。 / Integral values get `.0`.
        assert_eq!(report.integral, "1.0");
        // 门槛左闭：`1e-3` 不用科学计数法。 / The lower bound is inclusive.
        assert_eq!(report.boundary_lower, "0.001");
        // 门槛右开：`1e7` 用科学计数法，尾数补 `.0`、指数大写。 / The upper bound is exclusive.
        assert_eq!(report.boundary_upper, "1.0E7");
        // `-0.0` 的文本保留符号。 / `-0.0` keeps its sign in text.
        assert_eq!(report.negative_zero, "-0.0");
        // 指数形式：尾数补 `.0`、指数不带正号。 / The exponent form pads the mantissa.
        assert_eq!(format_number(1.0e300), "1.0E300");
        assert_eq!(format_number(1.0e-4), "1.0E-4");
        assert_eq!(format_number(9999999.0), "9999999.0");
        assert_eq!(format_number(0.0), "0.0");
        // 有限值往返。 / Finite values round-trip.
        let report = verify_number_text(&request(vec![number("0"), number("-0"), number("1"),
            number("0.1"), number("-3.25"), number("1e300"), number("1e-4")])).unwrap();
        for row in &report.floats {
            assert!(row.reparsed, "{}", row.input);
            assert!(row.round_trips, "{}", row.input);
            assert!(row.codec_encodes, "{}", row.input);
        }
        assert_eq!(report.floats[0].formatted, "0.0");
        assert_eq!(report.floats[1].formatted, "-0.0");
        assert_eq!(report.floats[1].input, "-0");
    }

    #[test]
    fn non_finite_values_format_but_do_not_parse() {
        // `format_number` 对非有限值给文本，但 `parse_number` 拒收 —— 这不是往返函数。 
        // `format_number` emits text for non-finite values while `parse_number` rejects it.
        let report = verify_number_text(&request(vec![special("nan"), special("inf"),
            special("negInf")])).unwrap();
        assert_eq!(report.nan_text, "NaN");
        assert!(!report.nan_round_trips);
        for row in &report.floats {
            assert!(!row.reparsed, "{}", row.input);
            assert!(!row.round_trips, "{}", row.input);
            // `TextNumberCodec` 编码也拒收非有限值。 / The codec refuses to encode them too.
            assert!(!row.codec_encodes, "{}", row.input);
            assert!(!row.formatted.is_empty(), "{}", row.input);
        }
        assert_eq!(report.floats[1].formatted, "inf");
        assert_eq!(report.floats[2].formatted, "-inf");
    }

    #[test]
    fn vector_and_matrix_round_trip_through_their_codecs() {
        let mut document = request(Vec::new());
        document.vector = vec![number("1.5"), number("-2"), number("1e-5")];
        document.matrix = vec![vec![number("1"), number("2")], vec![number("3")]];
        let report = verify_number_text(&document).unwrap();
        assert!(report.vector_round_trips);
        assert!(report.matrix_round_trips);
        // 编码是**字符串**数组。 / The encoding is an array of strings.
        assert!(report.vector_json.contains("\"1.5\""));
        assert!(report.matrix_json.contains('['));
        // 空向量与空矩阵也合法。 / Empty shapes are legal.
        let report = verify_number_text(&request(Vec::new())).unwrap();
        assert!(report.vector_round_trips);
        assert!(report.matrix_round_trips);
        assert_eq!(report.vector_json, "[]");
    }

    #[test]
    fn the_raw_matrix_codec_is_deliberately_asymmetric() {
        let mut document = request(Vec::new());
        document.raw_matrix = vec![vec![number("1.5"), number("2")]];
        let report = verify_number_text(&document).unwrap();
        // 编码出 **JSON 数字**。 / It emits **JSON numbers**.
        assert_eq!(report.raw_json, "[[1.5,2.0]]");
        assert!(report.raw_round_trips);
        // 但解码同时接受字符串（容忍历史快照）。 / Decoding also accepts strings.
        assert!(report.raw_accepts_legacy_strings);
        // `json::numbers` 把非有限值变成 0。 / `json::numbers` degrades non-finite to zero.
        assert!(report.numbers_degrades_non_finite);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_number_text(&request(vec![number("one")])).unwrap_err().message
            .contains("is not a float"));
        assert!(verify_number_text(&request(vec![special("zero")])).unwrap_err().message
            .contains("unknown float kind"));
        // 非有限值放进向量 / 矩阵里 → 编解码器报错，整次失败。 / A non-finite component fails the codec.
        let mut document = request(Vec::new());
        document.vector = vec![special("nan")];
        assert!(verify_number_text(&document).unwrap_err().message.contains("non-finite"));
        let mut document = request(Vec::new());
        document.raw_matrix = vec![vec![special("inf")]];
        assert!(verify_number_text(&document).unwrap_err().message.contains("non-finite"));
        // 上限。 / The cap.
        let many = request(vec![number("1"); MAX_ENTRIES + 1]);
        assert!(verify_number_text(&many).unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。漏掉 `deny_unknown_fields` 会让读侧防护变成静默空操作
    /// （续挖第 16 单就踩过一次）。
    /// A guard: DTOs must be camelCase and reject unknown fields. Dropping `deny_unknown_fields` silently turns the
    /// read-side checks into no-ops (this bit us once in continuation delivery 16).
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_number_text(&request(Vec::new())).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"boundaryLower\""), "{json}");
        assert!(json.contains("\"nanRoundTrips\""), "{json}");
        assert!(!json.contains("boundary_lower"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<NumberTextReport>(&copy.to_string()).is_err());
    }
}
