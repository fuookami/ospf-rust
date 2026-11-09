//! 续挖第 117 单：**求解值的转换策略只对精确有理数有意义，对 `f64` 本身两种策略完全等价；而上界一律走
//! `floor` 并硬编码"允许舍入"**——`SolveValue for f64` 的 `from_f64_with_policy`/`to_f64_with_policy` 都只看
//! `is_finite`（`_policy` 被忽略），所以 `Strict` 与 `AllowRounding` 给出同一结论；`to_nonnegative_integral_upper_bound`
//! 的**默认实现**里策略写死为 `AllowRounding`、负数报 "cannot be negative"、`>= 2^63` 报溢出、其余**取 `floor`**
//! （`-0.0` 不算负数，给 0）；默认策略是 `Strict`
//! （`solver/value/solve_value.rs` 的 `SolveValue` / `SolveValueConversionPolicy`）
//! Continuation, delivery 117: **the conversion policy only means something for exact rationals — for `f64` itself the two
//! policies are equivalent — and the upper bound goes through `floor` with rounding hardcoded on** —
//! `SolveValue for f64`'s `from_f64_with_policy`/`to_f64_with_policy` read only `is_finite` (the `_policy` is ignored), so
//! `Strict` and `AllowRounding` reach the same verdict; the **default** `to_nonnegative_integral_upper_bound` hardcodes
//! `AllowRounding`, reports "cannot be negative", reports an overflow at `>= 2^63`, and **floors** everything else (`-0.0` is
//! not negative and yields zero); the default policy is `Strict` (`SolveValue` / `SolveValueConversionPolicy` in
//! `solver/value/solve_value.rs`).
//!
//! 第 116 单量的是 lowering 报告的自检缺口，这一单量**求解值的数值域转换**。八条实测：
//!
//! 1. **`from_f64_with_policy` 只拒非有限**（`the_converter_only_refuses_non_finite()`）。
//! 2. **`to_f64_with_policy` 同样只拒非有限**（`the_reader_only_refuses_non_finite()`）。
//! 3. **`f64` 上两种策略等价**（`the_two_policies_agree_on_f64()`）。
//! 4. **默认策略是 `Strict`**（`the_default_policy_is_strict()`）。
//! 5. **上界取 `floor` 而不是截断**（`the_upper_bound_floors()`）。
//! 6. **`-0.0` 不算负数**（`a_negative_zero_is_not_negative()`）。
//! 7. **负数上界报错**（`a_negative_upper_bound_is_refused()`）。
//! 8. **`>= 2^63` 报溢出**（`an_out_of_range_upper_bound_overflows()`）。
//!
//! Delivery 116 measured the lowering report's self-check gaps; this one measures **the solve value's numeric-domain
//! conversion**. **The converter only refuses non-finite values**, **so does the reader**, and **the two policies agree on
//! `f64`**; **the default policy is `Strict`**. **The upper bound floors** rather than truncating, **a negative zero is not
//! negative**, **a negative bound is refused**, and **an out-of-range bound overflows**.

use ospf_rust_core::solver::{
    SolveValue, SolveValueConversionPolicy,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 求解值转换探针请求。 / The solve value conversion probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SolveValueProbeRequest {
    /// 数值文本：十进制，或 `nan` / `inf` / `-inf`（JSON 载不了非有限，故用文本）。
    /// The value text: a decimal, or `nan` / `inf` / `-inf`.
    pub value: String,
    /// 策略名（`Strict` / `AllowRounding`，缺省 `Strict`）。 / The policy name.
    #[serde(default)]
    pub policy: Option<String>,
}

/// 求解值转换探针结论。 / The solve value conversion probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SolveValueProbeReport {
    /// 类型名。 / The type name.
    pub type_name: String,
    /// 策略名。 / The policy name.
    pub policy: String,
    /// `from_f64_with_policy` 是否成功。 / Whether the converter succeeded.
    pub from_ok: bool,
    /// 转换失败的文本。 / The conversion error text.
    pub from_error: String,
    /// `to_f64_with_policy` 是否成功。 / Whether the reader succeeded.
    pub to_ok: bool,
    /// 读取失败的文本。 / The reader error text.
    pub to_error: String,
    /// 两种策略下转换结论是否一致。 / Whether both policies agree on the conversion.
    pub policies_agree: bool,
    /// 上界是否成功。 / Whether the upper bound succeeded.
    pub upper_bound_ok: bool,
    /// 上界的值（成功时）。 / The upper bound, when it succeeds.
    #[serde(default)]
    pub upper_bound: Option<i64>,
    /// 上界的错误文本。 / The upper bound error text.
    pub upper_bound_error: String,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_converter_only_refuses_non_finite: bool,
    pub the_reader_only_refuses_non_finite: bool,
    pub the_two_policies_agree_on_f64: bool,
    pub the_default_policy_is_strict: bool,
    pub the_upper_bound_floors: bool,
    pub a_negative_zero_is_not_negative: bool,
    pub a_negative_upper_bound_is_refused: bool,
    pub an_out_of_range_upper_bound_overflows: bool,
}

/// 解析数值文本。 / Parse the value text.
fn value_of(text: &str) -> Result<f64> {
    let trimmed = text.trim();
    let parsed = match trimmed {
        "nan" | "NaN" => Ok(f64::NAN),
        "inf" | "infinity" | "Infinity" => Ok(f64::INFINITY),
        "-inf" | "-infinity" | "-Infinity" => Ok(f64::NEG_INFINITY),
        _ => trimmed.parse::<f64>().map_err(|_| BridgeError::new("InvalidInput",
            "solve value probe failed validation: the value reads as a decimal, nan, inf or -inf")),
    };
    parsed
}

/// 解析策略名。 / Parse the policy name.
fn policy_of(name: Option<&str>) -> Result<SolveValueConversionPolicy> {
    match name {
        None => Ok(SolveValueConversionPolicy::default()),
        Some("Strict") => Ok(SolveValueConversionPolicy::Strict),
        Some("AllowRounding") => Ok(SolveValueConversionPolicy::AllowRounding),
        Some(_) => Err(BridgeError::new("InvalidInput",
            "solve value probe failed validation: policy reads Strict or AllowRounding")),
    }
}

/// 策略名。 / The policy name.
fn policy_name(policy: SolveValueConversionPolicy) -> String {
    match policy {
        SolveValueConversionPolicy::Strict => "Strict",
        SolveValueConversionPolicy::AllowRounding => "AllowRounding",
    }.to_owned()
}

/// 走一遍求解值转换探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_solve_value_probe(
    request: &SolveValueProbeRequest,
) -> Result<SolveValueProbeReport> {
    let findings = canonical_findings();
    let value = value_of(&request.value)?;
    let policy = policy_of(request.policy.as_deref())?;
    let other = match policy {
        SolveValueConversionPolicy::Strict => SolveValueConversionPolicy::AllowRounding,
        SolveValueConversionPolicy::AllowRounding => SolveValueConversionPolicy::Strict,
    };
    let converted = <f64 as SolveValue>::from_f64_with_policy(value, policy);
    let from_ok = converted.is_ok();
    let from_error = converted.err().map(|error| error.to_string()).unwrap_or_default();
    let swapped = <f64 as SolveValue>::from_f64_with_policy(value, other);
    let read = <f64 as SolveValue>::to_f64_with_policy(&value, policy);
    let bound = <f64 as SolveValue>::to_nonnegative_integral_upper_bound(&value);
    Ok(SolveValueProbeReport {
        type_name: <f64 as SolveValue>::type_name().to_owned(),
        policy: policy_name(policy),
        from_ok,
        from_error,
        to_ok: read.is_ok(),
        to_error: read.err().map(|error| error.to_string()).unwrap_or_default(),
        policies_agree: from_ok == swapped.is_ok(),
        upper_bound_ok: bound.is_ok(),
        upper_bound: bound.as_ref().ok().copied(),
        upper_bound_error: bound.err().map(|error| error.to_string()).unwrap_or_default(),
        the_converter_only_refuses_non_finite: findings.the_converter_only_refuses_non_finite,
        the_reader_only_refuses_non_finite: findings.the_reader_only_refuses_non_finite,
        the_two_policies_agree_on_f64: findings.the_two_policies_agree_on_f64,
        the_default_policy_is_strict: findings.the_default_policy_is_strict,
        the_upper_bound_floors: findings.the_upper_bound_floors,
        a_negative_zero_is_not_negative: findings.a_negative_zero_is_not_negative,
        a_negative_upper_bound_is_refused: findings.a_negative_upper_bound_is_refused,
        an_out_of_range_upper_bound_overflows: findings.an_out_of_range_upper_bound_overflows,
    })
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_converter_only_refuses_non_finite: bool,
    the_reader_only_refuses_non_finite: bool,
    the_two_policies_agree_on_f64: bool,
    the_default_policy_is_strict: bool,
    the_upper_bound_floors: bool,
    a_negative_zero_is_not_negative: bool,
    a_negative_upper_bound_is_refused: bool,
    an_out_of_range_upper_bound_overflows: bool,
}

/// 转换一次。 / Convert once.
fn from(value: f64, policy: SolveValueConversionPolicy) -> bool {
    <f64 as SolveValue>::from_f64_with_policy(value, policy).is_ok()
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        let strict = SolveValueConversionPolicy::Strict;
        let rounding = SolveValueConversionPolicy::AllowRounding;
        // 1) 转换只拒非有限。 / The converter only refuses non-finite values.
        let the_converter_only_refuses_non_finite = {
            from(0.0, strict) && from(-1.5, strict) && from(1e300, strict)
                && !from(f64::NAN, strict) && !from(f64::INFINITY, strict)
                && !from(f64::NEG_INFINITY, strict)
                // 小数与非整数值照样通过。 / Fractions and non-integral values pass untouched.
                && from(0.1 + 0.2, strict)
        };
        // 2) 读取只拒非有限。 / The reader only refuses non-finite values.
        let the_reader_only_refuses_non_finite = {
            <f64 as SolveValue>::to_f64_with_policy(&0.5, strict).is_ok()
                && <f64 as SolveValue>::to_f64_with_policy(&f64::NAN, strict).is_err()
                && <f64 as SolveValue>::to_f64_with_policy(&f64::INFINITY, strict).is_err()
        };
        // 3) f64 上两种策略等价。 / The two policies agree on `f64`.
        let the_two_policies_agree_on_f64 = {
            [0.0, -0.0, 1.5, 1e300, f64::NAN, f64::INFINITY, f64::NEG_INFINITY].iter()
                .all(|value| from(*value, strict) == from(*value, rounding))
        };
        // 4) 默认策略是 Strict。 / The default policy is `Strict`.
        let the_default_policy_is_strict = {
            SolveValueConversionPolicy::default() == SolveValueConversionPolicy::Strict
        };
        // 5) 上界取 floor。 / The upper bound floors.
        let the_upper_bound_floors = {
            let two_point_seven = <f64 as SolveValue>::to_nonnegative_integral_upper_bound(&2.7);
            let zero_point_nine = <f64 as SolveValue>::to_nonnegative_integral_upper_bound(&0.9);
            // 2.7 → 2、0.9 → 0；而 2.0 就是 2。 / Two point seven floors to two, zero point nine to zero.
            two_point_seven.ok() == Some(2) && zero_point_nine.ok() == Some(0)
                && <f64 as SolveValue>::to_nonnegative_integral_upper_bound(&2.0).ok()
                    == Some(2)
        };
        // 6) -0.0 不算负数。 / A negative zero is not negative.
        let a_negative_zero_is_not_negative = {
            <f64 as SolveValue>::to_nonnegative_integral_upper_bound(&-0.0).ok() == Some(0)
        };
        // 7) 负数上界被拒。 / A negative bound is refused.
        let a_negative_upper_bound_is_refused = {
            let error = <f64 as SolveValue>::to_nonnegative_integral_upper_bound(&-1.5)
                .err().map(|error| error.to_string()).unwrap_or_default();
            error.contains("cannot be negative")
        };
        // 8) 超范围上界溢出。 / An out-of-range bound overflows.
        let an_out_of_range_upper_bound_overflows = {
            let error = <f64 as SolveValue>::to_nonnegative_integral_upper_bound(&1e19)
                .err().map(|error| error.to_string()).unwrap_or_default();
            // 非有限值连 floor 都走不到，先被读取拒掉。 
            // A non-finite value never reaches the floor: the reader refuses it first.
            let infinite = <f64 as SolveValue>::to_nonnegative_integral_upper_bound(&f64::INFINITY)
                .err().map(|error| error.to_string()).unwrap_or_default();
            error.contains("exceeds i64") && !infinite.is_empty()
        };
        Findings {
            the_converter_only_refuses_non_finite,
            the_reader_only_refuses_non_finite,
            the_two_policies_agree_on_f64,
            the_default_policy_is_strict,
            the_upper_bound_floors,
            a_negative_zero_is_not_negative,
            a_negative_upper_bound_is_refused,
            an_out_of_range_upper_bound_overflows,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: SolveValueProbeRequest) -> SolveValueProbeReport {
        verify_solve_value_probe(&request).unwrap()
    }

    fn request(value: &str) -> SolveValueProbeRequest {
        SolveValueProbeRequest { value: value.to_owned(), policy: None }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request("1.5"));
        assert_eq!(report.type_name, "f64");
        assert_eq!(report.policy, "Strict");
        assert!(report.from_ok && report.to_ok);
        assert_eq!(report.from_error, "");
        assert!(report.policies_agree);
        assert!(report.upper_bound_ok);
        assert_eq!(report.upper_bound, Some(1));
        assert_eq!(report.upper_bound_error, "");
        assert!(report.the_converter_only_refuses_non_finite);
        assert!(report.the_reader_only_refuses_non_finite);
        assert!(report.the_two_policies_agree_on_f64);
        assert!(report.the_default_policy_is_strict);
        assert!(report.the_upper_bound_floors);
        assert!(report.a_negative_zero_is_not_negative);
        assert!(report.a_negative_upper_bound_is_refused);
        assert!(report.an_out_of_range_upper_bound_overflows);
    }

    #[test]
    fn non_finite_texts_are_refused_by_both_faces() {
        for text in ["nan", "inf", "-inf"] {
            let report = ask(request(text));
            assert!(!report.from_ok, "{text}");
            assert!(report.from_error.contains("cannot convert non-finite value"), "{text}");
            assert!(!report.to_ok, "{text}");
            // 非有限值走到上界时先被读取拒掉。 
            // At the bound the reader refuses a non-finite value first.
            assert!(!report.upper_bound_ok, "{text}");
            assert!(!report.upper_bound_error.is_empty(), "{text}");
        }
    }

    #[test]
    fn the_policies_agree_even_on_the_boundary_texts() {
        for text in ["0", "-0", "2.7", "1e300", "nan"] {
            for policy in [None, Some("Strict".to_owned()), Some("AllowRounding".to_owned())] {
                let report = ask(SolveValueProbeRequest { value: text.to_owned(), policy });
                assert!(report.policies_agree, "{text}");
            }
        }
    }

    #[test]
    fn the_upper_bound_floors_and_refuses_negatives() {
        // 2.7 → 2、0.9 → 0、-0.0 → 0。 / Two point seven floors to two, zero point nine to zero, -0.0 to zero.
        assert_eq!(ask(request("2.7")).upper_bound, Some(2));
        assert_eq!(ask(request("0.9")).upper_bound, Some(0));
        assert_eq!(ask(request("-0")).upper_bound, Some(0));
        let negative = ask(request("-1.5"));
        assert!(!negative.upper_bound_ok);
        assert!(negative.upper_bound_error.contains("cannot be negative"),
            "{}", negative.upper_bound_error);
        // 转换本身对负数没有意见。 / The conversion itself has no objection to negatives.
        assert!(negative.from_ok);
        let huge = ask(request("1e19"));
        assert!(!huge.upper_bound_ok);
        assert!(huge.upper_bound_error.contains("exceeds i64"), "{}", huge.upper_bound_error);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_solve_value_probe(&request("nearly")).unwrap_err()
            .message.contains("reads as a decimal"));
        assert!(verify_solve_value_probe(&SolveValueProbeRequest { value: "1".to_owned(),
            policy: Some("Nearly".to_owned()) }).unwrap_err()
            .message.contains("policy reads Strict or AllowRounding"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request("1.5"));
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"typeName\""), "{json}");
        assert!(json.contains("\"upperBoundOk\""), "{json}");
        assert!(json.contains("\"theUpperBoundFloors\""), "{json}");
        assert!(!json.contains("upper_bound_ok"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<SolveValueProbeReport>(&copy.to_string()).is_err());
    }
}
