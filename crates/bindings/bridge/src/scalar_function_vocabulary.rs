//! 续挖第 31 单：标量函数词汇表是**约定而不是约束**（`persistence/expression/scalar_function_dsl.rs`）
//! Continuation, delivery 31: the scalar function vocabulary is a **convention, not a constraint**.
//!
//! `scalar_function_dsl` 把六个名字常量与六个构造器转发出来，内容很少但恰好把"词汇"与"校验"的边界摆在一起：
//!
//! 1. **六个常量 ↔ 六个构造器一一对应**：`abs` / `lower` / `upper` / `trim` / `length` 是**单参**，
//!    `coalesce` 是**唯一变参**。构造出的表达式名字就是常量本身的字符串。
//! 2. **`coalesce` 连零个参数都收**：`coalesce([])` 造出 `Function { name: "coalesce", arguments: [] }`，
//!    没有"至少要有一个参数"这种检查。
//! 3. **`scalar_function(name, args)` 完全不做校验**：任意名字（空白、大写、带空格）、任意参数个数都照收。
//!    所以这六个常量是**写给调用方的约定**，不是运行期能指望的约束——写错名字不会有任何报错，
//!    只会在下游后端的函数表里查不到。名字**大小写敏感**：`"ABS"` 与 `"abs"` 是两个不同函数。
//!
//! The module re-exports six name constants and six builders, and that small surface puts the boundary between vocabulary
//! and validation on display. **Six constants map one-to-one onto six builders**: `abs` / `lower` / `upper` / `trim` /
//! `length` take one argument while **`coalesce` is the only variadic one**, and each built expression carries the constant's
//! own string as its name. **`coalesce` even accepts zero arguments** — `coalesce([])` produces
//! `Function { name: "coalesce", arguments: [] }` with no "at least one argument" check. And **`scalar_function(name, args)`
//! validates nothing**: any name (blank, uppercase, padded) with any arity is accepted, so the six constants are a
//! **convention for callers, not a runtime constraint** — a misspelled name never errors here, it merely fails to match the
//! backend's function table later, and name matching is **case sensitive**, so `"ABS"` and `"abs"` are two different
//! functions.

use ospf_rust_framework::persistence::expression::scalar_function_dsl::{
    ScalarFunctionNames, abs, coalesce, length, lower, scalar_function, trim, upper,
};
use ospf_rust_math::symbol::{ExpressionValue, PropertyPath, ScalarExpression};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 标量函数词汇请求。 / The scalar function vocabulary request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScalarFunctionRequest {
    /// 用作参数引用的路径。 / The path used as the argument reference.
    pub path: String,
    /// 给 `coalesce` 的参数个数。 / The argument count for `coalesce`.
    #[serde(default)]
    pub coalesce_arity: u32,
    /// 直接交给 `scalar_function` 的名字（**故意不校验**）。 / The raw name handed to `scalar_function`
    /// (**deliberately unvalidated**).
    #[serde(default)]
    pub raw_name: String,
    /// 直接交给 `scalar_function` 的参数个数。 / The raw argument count.
    #[serde(default)]
    pub raw_arity: u32,
}

/// 一个构造结果。 / One built result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScalarFunctionRow {
    /// 常量名（Rust 侧标识符）。 / The constant name (the Rust identifier).
    pub constant: String,
    /// 构造出的函数名。 / The built function name.
    pub name: String,
    /// 参数个数。 / The argument count.
    pub arity: u64,
}

/// 标量函数词汇结论。 / The vocabulary verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScalarFunctionReport {
    /// 六个常量的形状。 / The six constants' shapes.
    pub rows: Vec<ScalarFunctionRow>,
    /// 直接交给 `scalar_function` 的结果名。 / The name produced by `scalar_function`.
    pub raw_name: String,
    /// 直接交给 `scalar_function` 的结果参数个数。 / The arity produced by `scalar_function`.
    pub raw_arity: u64,
    /// 常量与构造器一一对应（**是**）。 / Whether the constants match their builders (**yes**).
    pub constants_match_builders: bool,
    /// `coalesce` 是唯一的变参（**是**）。 / Whether `coalesce` is the only variadic one (**yes**).
    pub coalesce_is_the_only_variadic: bool,
    /// `coalesce` 接受零个参数（**是**）。 / Whether `coalesce` accepts zero arguments (**yes**).
    pub coalesce_accepts_no_arguments: bool,
    /// `scalar_function` 不校验名字（**是**）。 / Whether `scalar_function` does not validate the name (**yes**).
    pub raw_name_is_unvalidated: bool,
    /// 名字匹配大小写敏感（**是**）。 / Whether name matching is case sensitive (**yes**).
    pub name_matching_is_case_sensitive: bool,
    /// 六个名字互不相同（**是**）。 / Whether the six names are distinct (**yes**).
    pub names_are_distinct: bool,
}

/// 参数个数上限（信任边界）。 / The arity cap (a trust boundary).
const MAX_ARITY: u32 = 64;

/// 走一遍标量函数词汇。非法输入是**错误**。 / Walk the scalar function vocabulary.
pub fn verify_scalar_functions(request: &ScalarFunctionRequest) -> Result<ScalarFunctionReport> {
    if request.path.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "scalar functions failed validation: the path must not be blank"));
    }
    if request.coalesce_arity > MAX_ARITY || request.raw_arity > MAX_ARITY {
        return Err(BridgeError::new("InvalidInput", &format!(
            "scalar functions failed validation: at most {MAX_ARITY} arguments are accepted")));
    }
    let argument = ScalarExpression::reference(PropertyPath::parse(&request.path));
    let coalesce_arguments: Vec<ScalarExpression<ExpressionValue>> =
        (0..request.coalesce_arity).map(|_| argument.clone()).collect();
    let rows = vec![
        row("ABS", abs(argument.clone())),
        row("LOWER", lower(argument.clone())),
        row("UPPER", upper(argument.clone())),
        row("TRIM", trim(argument.clone())),
        row("LENGTH", length(argument.clone())),
        row("COALESCE", coalesce(coalesce_arguments)),
    ];
    let raw_arguments: Vec<ScalarExpression<ExpressionValue>> =
        (0..request.raw_arity).map(|_| argument.clone()).collect();
    let raw = scalar_function(request.raw_name.clone(), raw_arguments);
    let (raw_name, raw_arity) = name_and_arity(&raw, request.raw_name.clone());
    let findings = canonical_findings(argument);
    Ok(ScalarFunctionReport {
        rows,
        raw_name,
        raw_arity,
        constants_match_builders: findings.constants_match_builders,
        coalesce_is_the_only_variadic: findings.coalesce_is_the_only_variadic,
        coalesce_accepts_no_arguments: findings.coalesce_accepts_no_arguments,
        raw_name_is_unvalidated: findings.raw_name_is_unvalidated,
        name_matching_is_case_sensitive: findings.name_matching_is_case_sensitive,
        names_are_distinct: findings.names_are_distinct,
    })
}

/// 把一个构造结果变成一行。 / Turn one built expression into a row.
fn row(constant: &str, expression: ScalarExpression<ExpressionValue>) -> ScalarFunctionRow {
    let expected = match constant {
        "ABS" => ScalarFunctionNames::ABS,
        "LOWER" => ScalarFunctionNames::LOWER,
        "UPPER" => ScalarFunctionNames::UPPER,
        "TRIM" => ScalarFunctionNames::TRIM,
        "LENGTH" => ScalarFunctionNames::LENGTH,
        "COALESCE" => ScalarFunctionNames::COALESCE,
        other => other,
    };
    let (name, arity) = name_and_arity(&expression, expected.to_owned());
    ScalarFunctionRow { constant: constant.to_owned(), name, arity }
}

/// 读出函数名与参数个数。 / Read the function name and arity back.
fn name_and_arity(
    expression: &ScalarExpression<ExpressionValue>,
    fallback: String,
) -> (String, u64) {
    match expression {
        ScalarExpression::Function { name, arguments } => (name.clone(), arguments.len() as u64),
        _ => (fallback, 0),
    }
}

/// 六条规则属性的内部固定探针。 / The six rule properties measured by fixed internal probes.
struct Findings {
    constants_match_builders: bool,
    coalesce_is_the_only_variadic: bool,
    coalesce_accepts_no_arguments: bool,
    raw_name_is_unvalidated: bool,
    name_matching_is_case_sensitive: bool,
    names_are_distinct: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings(argument: ScalarExpression<ExpressionValue>) -> Findings {
    // 六个常量与六个构造器一一对应，且名字互不相同。
    // The six constants match their six builders and their names are distinct.
    let built = [
        (ScalarFunctionNames::ABS, abs(argument.clone())),
        (ScalarFunctionNames::LOWER, lower(argument.clone())),
        (ScalarFunctionNames::UPPER, upper(argument.clone())),
        (ScalarFunctionNames::TRIM, trim(argument.clone())),
        (ScalarFunctionNames::LENGTH, length(argument.clone())),
        (ScalarFunctionNames::COALESCE, coalesce(Vec::new())),
    ];
    let constants_match_builders = built.iter()
        .all(|(constant, expression)| name_and_arity(expression, (*constant).to_owned()).0
            == *constant);
    let mut names: Vec<&str> = built.iter().map(|(constant, _)| *constant).collect();
    names.sort_unstable();
    let distinct = names.len();
    names.dedup();
    let names_are_distinct = names.len() == distinct;
    // 只有 `coalesce` 变参：给它两个参数，其余五个仍是单参。
    // Only `coalesce` is variadic: give it two arguments while the other five stay unary.
    let two: Vec<ScalarExpression<ExpressionValue>> =
        vec![argument.clone(), argument.clone()];
    let coalesce_two = name_and_arity(&coalesce(two), ScalarFunctionNames::COALESCE.to_owned()).1;
    let others_unary = [abs(argument.clone()), lower(argument.clone()), upper(argument.clone()),
        trim(argument.clone()), length(argument.clone())]
        .iter()
        .all(|expression| name_and_arity(expression, String::new()).1 == 1);
    let coalesce_is_the_only_variadic = others_unary && coalesce_two == 2;
    // `coalesce` 连零参数都收。 / `coalesce` accepts even zero arguments.
    let coalesce_accepts_no_arguments =
        name_and_arity(&coalesce(Vec::new()), ScalarFunctionNames::COALESCE.to_owned()).1 == 0;
    // `scalar_function` 不校验名字：空白名照收。 / `scalar_function` validates nothing: a blank name is kept.
    let blank = scalar_function("", vec![argument.clone()]);
    let raw_name_is_unvalidated = name_and_arity(&blank, String::new()).0.is_empty();
    // 大小写敏感：`"ABS"` 构造出的名字与常量 `"abs"` 不相等。
    // Case sensitive: the name built from `"ABS"` never equals the constant `"abs"`.
    let upper_name = scalar_function("ABS", vec![argument]);
    let name_matching_is_case_sensitive =
        name_and_arity(&upper_name, String::new()).0 != ScalarFunctionNames::ABS;
    Findings { constants_match_builders, coalesce_is_the_only_variadic,
        coalesce_accepts_no_arguments, raw_name_is_unvalidated, name_matching_is_case_sensitive,
        names_are_distinct }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ScalarFunctionRequest {
        ScalarFunctionRequest { path: "user.name".to_owned(), coalesce_arity: 2,
            raw_name: "abs".to_owned(), raw_arity: 1 }
    }

    #[test]
    fn the_six_constants_match_their_builders() {
        let report = verify_scalar_functions(&request()).unwrap();
        assert!(report.constants_match_builders);
        assert_eq!(report.rows.len(), 6);
        // 五行单参，coalesce 用请求给的元数。 / Five unary rows, `coalesce` at the requested arity.
        let by = |constant: &str| report.rows.iter().find(|row| row.constant == constant).unwrap();
        assert_eq!(by("ABS").name, "abs");
        assert_eq!(by("ABS").arity, 1);
        assert_eq!(by("LOWER").name, "lower");
        assert_eq!(by("UPPER").name, "upper");
        assert_eq!(by("TRIM").name, "trim");
        assert_eq!(by("LENGTH").name, "length");
        assert_eq!(by("COALESCE").name, "coalesce");
        assert_eq!(by("COALESCE").arity, 2);
        assert!(report.names_are_distinct);
    }

    #[test]
    fn coalesce_is_the_only_variadic_and_accepts_nothing() {
        let report = verify_scalar_functions(&request()).unwrap();
        assert!(report.coalesce_is_the_only_variadic);
        assert!(report.coalesce_accepts_no_arguments);
        // 零参的 coalesce 也是合法的请求值。 / A zero-argument coalesce is a legal request value.
        let mut empty = request();
        empty.coalesce_arity = 0;
        let report = verify_scalar_functions(&empty).unwrap();
        assert_eq!(report.rows.iter().find(|row| row.constant == "COALESCE").unwrap().arity, 0);
    }

    #[test]
    fn the_raw_constructor_validates_nothing() {
        let mut document = request();
        document.raw_name = "  ".to_owned();
        let report = verify_scalar_functions(&document).unwrap();
        assert!(report.raw_name_is_unvalidated);
        assert_eq!(report.raw_name, "  ");
        assert_eq!(report.raw_arity, 1);
        // 零参数的裸构造也照收。 / A zero-argument raw construction is accepted too.
        document.raw_arity = 0;
        assert_eq!(verify_scalar_functions(&document).unwrap().raw_arity, 0);
        // 大写名不被归一化。 / An uppercase name is not normalised.
        let mut upper = request();
        upper.raw_name = "ABS".to_owned();
        let report = verify_scalar_functions(&upper).unwrap();
        assert_eq!(report.raw_name, "ABS");
        assert!(report.name_matching_is_case_sensitive);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let mut other = request();
        other.path = "a.b.c".to_owned();
        other.coalesce_arity = 0;
        other.raw_name = String::new();
        other.raw_arity = 3;
        let report = verify_scalar_functions(&other).unwrap();
        assert!(report.constants_match_builders);
        assert!(report.coalesce_is_the_only_variadic);
        assert!(report.coalesce_accepts_no_arguments);
        assert!(report.raw_name_is_unvalidated);
        assert!(report.name_matching_is_case_sensitive);
        assert!(report.names_are_distinct);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut blank = request();
        blank.path = "  ".to_owned();
        assert!(verify_scalar_functions(&blank).unwrap_err().message
            .contains("path must not be blank"));
        let mut many = request();
        many.raw_arity = MAX_ARITY + 1;
        assert!(verify_scalar_functions(&many).unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_scalar_functions(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"constantsMatchBuilders\""), "{json}");
        assert!(json.contains("\"coalesceIsTheOnlyVariadic\""), "{json}");
        assert!(json.contains("\"rawName\""), "{json}");
        assert!(!json.contains("constants_match"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<ScalarFunctionReport>(&copy.to_string()).is_err());
    }
}
