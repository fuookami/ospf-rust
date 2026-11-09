//! 批次 E 第 2 单：Benders cut 的往返与结构报告 / Batch E, delivery 2: Benders cut round trip and structure report.
//!
//! cut 本身早就能出线（`BendersCutSpec` / `BendersQuadraticCutSpec` + `to_linear_cuts` / `to_quadratic_cuts`），但**框架产出
//! 的 cut 只能看到数量**（`BendersSubReport::generated_cut_count`），内容读不回来。这一单把方向反过来：把一份 cut 文档
//! 交给领域转换器，再把**领域值投影回同一份线格式**——于是 TS 能确认"我送进去的 cut 就是框架会应用的那条"。
//!
//! 校验全部由领域转换器给出（桥接不重写），实测逐条：
//! * cut 名称不能空白且**不得重复**；
//! * 右端项与每个系数都必须**有限**；
//! * 系数列表不能为空（空 cut 是一条无约束）；
//! * 列索引必须落在 `columnCount` 之内——**越界列绝不静默丢弃**，否则 cut 会变成一条看不见的错约束；
//! * 同一 cut 内**同一列不得重复**（重复说明调用方误用了稀疏表示，拒绝而不是自动累加）；
//! * 二次 cut 另有：列对不得重复，且线性部分与二次项不能同时为空。
//!
//! 报告里的 `canonicalJson` 是**从领域值重建**的线格式，所以把它与输入逐字比较就是"没有静默丢弃或重排"的证明。
//! 报告还给出每条的列跨度与全局列覆盖，便于调用方一眼看出 cut 打在了模型的哪些列上。
//!
//! Cuts already cross the wire, but **framework-produced cuts were only visible as a count**
//! (`BendersSubReport::generated_cut_count`) with no way to read the content back. This delivery runs the other direction: a cut
//! document goes through the domain converter and the **domain values are projected back into the same wire format**, so
//! TypeScript can confirm that the cuts it sent are the rows the framework will apply. All validation comes from the domain
//! converter (the bridge re-implements none of it) and is measured rule by rule: non-blank unique names; finite right-hand
//! sides and coefficients; a non-empty coefficient list (an empty cut is no constraint at all); column indices inside
//! `columnCount` — **an out-of-range column is never dropped silently**, since that would turn the cut into an invisible wrong
//! row; no repeated column inside one cut (a repeat means the caller misused the sparse form, refused rather than
//! auto-summed); and for quadratic cuts, no repeated column pair and not both parts empty. The report's `canonicalJson` is
//! rebuilt **from the domain values**, so comparing it byte-for-byte with the input proves nothing was dropped or reordered;
//! the report also gives each cut's column span and the global column coverage.

use ospf_rust_framework::solver::CutSense;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::linear_benders::{
    to_linear_cuts, to_quadratic_cuts, BendersCutCoefficient, BendersCutSense, BendersCutSpec,
    BendersQuadraticCutSpec, BendersQuadraticTerm,
};
use crate::{BridgeError, Result};

/// cut 往返请求。 / The cut round-trip request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersCutRequest {
    /// 模型的列数；cut 的列索引必须严格小于它。 / The model's column count; every cut column must be below it.
    pub column_count: u64,
    /// 线性 cut。 / Linear cuts.
    #[serde(default)]
    pub linear: Vec<BendersCutSpec>,
    /// 二次 cut。 / Quadratic cuts.
    #[serde(default)]
    pub quadratic: Vec<BendersQuadraticCutSpec>,
}

/// 一条 cut 的结构投影。 / The structure projection of one cut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersCutRow {
    /// cut 名称。 / The cut name.
    pub name: String,
    /// 关系方向（`lessOrEqual` / `greaterOrEqual` / `equal`）。 / The relation sense.
    pub sense: String,
    /// 线性系数条数。 / The number of linear coefficients.
    pub linear_terms: u32,
    /// 二次项条数（线性 cut 恒为 0）。 / The number of quadratic terms (always zero for a linear cut).
    pub quadratic_terms: u32,
    /// 用到的**最小**列索引（无系数时为 `undefined`）。 / The smallest column used (`undefined` without terms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub first_column: Option<u64>,
    /// 用到的**最大**列索引（无系数时为 `undefined`）。 / The largest column used (`undefined` without terms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_column: Option<u64>,
    /// 右端项。 / The right-hand side.
    pub rhs: f64,
}

/// cut 往返结论。 / The cut round-trip verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BendersCutReport {
    /// 模型的列数。 / The model's column count.
    pub column_count: u64,
    /// 线性 cut 条数。 / The number of linear cuts.
    pub linear_cut_count: u32,
    /// 二次 cut 条数。 / The number of quadratic cuts.
    pub quadratic_cut_count: u32,
    /// 线性系数总条数。 / The total number of linear coefficients.
    pub linear_term_count: u32,
    /// 二次项总条数。 / The total number of quadratic terms.
    pub quadratic_term_count: u32,
    /// 被任一 cut 引用的**不同**列数。 / The number of distinct columns referenced by any cut.
    pub distinct_column_count: u32,
    /// 被引用的最大列索引（无 cut 时为 `undefined`）。 / The largest referenced column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_column: Option<u64>,
    /// 线性 cut 的投影。 / The linear-cut projections.
    pub linear: Vec<BendersCutRow>,
    /// 二次 cut 的投影。 / The quadratic-cut projections.
    pub quadratic: Vec<BendersCutRow>,
    /// **从领域值重建**的线格式（与输入逐字比较即"无静默丢弃"的证明）。
    /// The wire format **rebuilt from the domain values** (a byte comparison with the input proves nothing was dropped).
    pub canonical_json: String,
}

/// 把一份 cut 文档送进领域转换器，再把领域值投影回同一份线格式。
/// Run a cut document through the domain converter and project the domain values back into the same wire format.
pub fn verify_benders_cuts(request: &BendersCutRequest) -> Result<BendersCutReport> {
    let column_count = usize::try_from(request.column_count).map_err(|_| BridgeError::new("InvalidInput",
        "benders cut verification: column count exceeds the platform index range"))?;
    // 领域转换器：所有 cut 结构规则都在这里，桥接不重写任何一条。
    // The domain converters: every structural cut rule lives here and the bridge re-implements none of it.
    let linear = to_linear_cuts(&request.linear, column_count)?;
    let quadratic = to_quadratic_cuts(&request.quadratic, column_count)?;
    // 从**领域值**重建线格式：这一步是"送进去的就是会被应用的那条"的证明。
    // Rebuild the wire format **from the domain values**: this is the proof that what was sent is what will be applied.
    let rebuilt_linear = linear.iter().map(rebuild_linear).collect::<Vec<_>>();
    let rebuilt_quadratic = quadratic.iter().map(|cut| BendersQuadraticCutSpec {
        name: cut.linear.name.clone(),
        linear: cut.linear.coefficients.iter().map(|(column, coefficient)| BendersCutCoefficient {
            column: u32::try_from(*column).unwrap_or(u32::MAX), coefficient: *coefficient,
        }).collect(),
        quadratic: cut.quadratic_coefficients.iter().map(|((left, right), coefficient)| BendersQuadraticTerm {
            left: u32::try_from(*left).unwrap_or(u32::MAX),
            right: u32::try_from(*right).unwrap_or(u32::MAX),
            coefficient: *coefficient,
        }).collect(),
        sense: wire_sense(cut.linear.sense),
        rhs: cut.linear.rhs,
    }).collect::<Vec<_>>();
    let mut columns = std::collections::BTreeSet::new();
    for cut in &linear {
        columns.extend(cut.coefficients.iter().map(|(column, _)| *column));
    }
    for cut in &quadratic {
        columns.extend(cut.linear.coefficients.iter().map(|(column, _)| *column));
        for ((left, right), _) in &cut.quadratic_coefficients {
            columns.insert(*left);
            columns.insert(*right);
        }
    }
    let canonical_json = serde_json::to_string(&serde_json::json!({
        "linear": rebuilt_linear,
        "quadratic": rebuilt_quadratic,
    })).map_err(|error| BridgeError::new("CodecFailure", &format!("cut document could not be encoded: {error}")))?;
    Ok(BendersCutReport {
        column_count: request.column_count,
        linear_cut_count: count(linear.len()),
        quadratic_cut_count: count(quadratic.len()),
        linear_term_count: linear.iter().map(|cut| cut.coefficients.len())
            .try_fold(0usize, |total, count| total.checked_add(count)).map_or(u32::MAX,
                |total| u32::try_from(total).unwrap_or(u32::MAX)),
        quadratic_term_count: quadratic.iter().map(|cut| cut.quadratic_coefficients.len())
            .try_fold(0usize, |total, count| total.checked_add(count)).map_or(u32::MAX,
                |total| u32::try_from(total).unwrap_or(u32::MAX)),
        distinct_column_count: count(columns.len()),
        max_column: columns.iter().next_back().map(|column| u64::try_from(*column).unwrap_or(u64::MAX)),
        linear: linear.iter().map(|cut| BendersCutRow {
            name: cut.name.clone(),
            sense: sense_name(cut.sense),
            linear_terms: count(cut.coefficients.len()),
            quadratic_terms: 0,
            first_column: cut.coefficients.iter().map(|(column, _)| *column).min()
                .map(|column| u64::try_from(column).unwrap_or(u64::MAX)),
            last_column: cut.coefficients.iter().map(|(column, _)| *column).max()
                .map(|column| u64::try_from(column).unwrap_or(u64::MAX)),
            rhs: cut.rhs,
        }).collect(),
        quadratic: quadratic.iter().map(|cut| BendersCutRow {
            name: cut.linear.name.clone(),
            sense: sense_name(cut.linear.sense),
            linear_terms: count(cut.linear.coefficients.len()),
            quadratic_terms: count(cut.quadratic_coefficients.len()),
            first_column: cut.linear.coefficients.iter().map(|(column, _)| *column)
                .chain(cut.quadratic_coefficients.iter().flat_map(|((left, right), _)| [*left, *right]))
                .min().map(|column| u64::try_from(column).unwrap_or(u64::MAX)),
            last_column: cut.linear.coefficients.iter().map(|(column, _)| *column)
                .chain(cut.quadratic_coefficients.iter().flat_map(|((left, right), _)| [*left, *right]))
                .max().map(|column| u64::try_from(column).unwrap_or(u64::MAX)),
            rhs: cut.linear.rhs,
        }).collect(),
        canonical_json,
    })
}

/// 从领域 cut 重建线格式。 / Rebuild the wire format from a domain cut.
fn rebuild_linear(cut: &ospf_rust_framework::solver::LinearCut) -> BendersCutSpec {
    BendersCutSpec {
        name: cut.name.clone(),
        coefficients: cut.coefficients.iter().map(|(column, coefficient)| BendersCutCoefficient {
            column: u32::try_from(*column).unwrap_or(u32::MAX), coefficient: *coefficient,
        }).collect(),
        sense: wire_sense(cut.sense),
        rhs: cut.rhs,
    }
}

/// 领域方向 → 线格式方向。 / Domain sense to wire sense.
fn wire_sense(sense: CutSense) -> BendersCutSense {
    match sense {
        CutSense::LessOrEqual => BendersCutSense::LessOrEqual,
        CutSense::GreaterOrEqual => BendersCutSense::GreaterOrEqual,
        CutSense::Equal => BendersCutSense::Equal,
    }
}

/// 方向的线格式名。 / The wire name of a sense.
fn sense_name(sense: CutSense) -> String {
    serde_json::to_value(wire_sense(sense)).ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// 计数字段的安全转换。 / A safe cast for count fields.
fn count(value: usize) -> u32 { u32::try_from(value).unwrap_or(u32::MAX) }

#[cfg(test)]
mod tests {
    use super::*;

    fn coefficient(column: u32, coefficient: f64) -> BendersCutCoefficient {
        BendersCutCoefficient { column, coefficient }
    }

    fn linear_cut(name: &str, sense: BendersCutSense, coefficients: Vec<BendersCutCoefficient>,
        rhs: f64) -> BendersCutSpec
    {
        BendersCutSpec { name: name.to_owned(), coefficients, sense, rhs }
    }

    fn request() -> BendersCutRequest {
        BendersCutRequest {
            column_count: 4,
            linear: vec![
                linear_cut("budget", BendersCutSense::LessOrEqual,
                    vec![coefficient(0, 1.0), coefficient(2, -2.5)], 10.0),
                linear_cut("floor", BendersCutSense::GreaterOrEqual, vec![coefficient(3, 1.0)], 0.0),
            ],
            quadratic: vec![BendersQuadraticCutSpec {
                name: "pair".to_owned(),
                linear: vec![coefficient(1, 0.5)],
                quadratic: vec![BendersQuadraticTerm { left: 0, right: 1, coefficient: 1.0 }],
                sense: BendersCutSense::Equal, rhs: 2.0,
            }],
        }
    }

    #[test]
    fn cuts_round_trip_and_are_projected() {
        let document = request();
        let report = verify_benders_cuts(&document).unwrap();
        assert_eq!(report.column_count, 4);
        assert_eq!(report.linear_cut_count, 2);
        assert_eq!(report.quadratic_cut_count, 1);
        assert_eq!(report.linear_term_count, 3);
        assert_eq!(report.quadratic_term_count, 1);
        // 列覆盖：0,1,2,3 都被引用。 / Column coverage: 0, 1, 2 and 3 are all referenced.
        assert_eq!(report.distinct_column_count, 4);
        assert_eq!(report.max_column, Some(3));
        assert_eq!(report.linear[0].name, "budget");
        assert_eq!(report.linear[0].sense, "lessOrEqual");
        assert_eq!(report.linear[0].linear_terms, 2);
        assert_eq!(report.linear[0].first_column, Some(0));
        assert_eq!(report.linear[0].last_column, Some(2));
        assert_eq!(report.linear[0].rhs, 10.0);
        assert_eq!(report.linear[1].sense, "greaterOrEqual");
        assert_eq!(report.quadratic[0].name, "pair");
        assert_eq!(report.quadratic[0].linear_terms, 1);
        assert_eq!(report.quadratic[0].quadratic_terms, 1);
        // 二次 cut 的列跨度覆盖线性部分与二次项。 / The span covers both the linear part and the terms.
        assert_eq!(report.quadratic[0].first_column, Some(0));
        assert_eq!(report.quadratic[0].last_column, Some(1));

        // 重建的线格式必须与输入逐字一致：没有静默丢弃、没有重排。
        // The rebuilt wire format must equal the input byte for byte: nothing dropped and nothing reordered.
        let rebuilt: serde_json::Value = serde_json::from_str(&report.canonical_json).unwrap();
        assert_eq!(rebuilt["linear"], serde_json::to_value(&document.linear).unwrap());
        assert_eq!(rebuilt["quadratic"], serde_json::to_value(&document.quadratic).unwrap());
    }

    #[test]
    fn domain_rules_are_measured() {
        // 空白名称 / 重复名称。 / A blank and a repeated name.
        let mut blank = request();
        blank.linear[0].name = "  ".to_owned();
        assert!(verify_benders_cuts(&blank).unwrap_err().message.contains("names must not be blank"));
        let mut repeated = request();
        repeated.linear[1].name = "budget".to_owned();
        assert!(verify_benders_cuts(&repeated).unwrap_err().message.contains("repeat a name"));
        // 越界列：绝不静默丢弃。 / An out-of-range column: never dropped silently.
        let mut outside = request();
        outside.linear[1].coefficients = vec![coefficient(4, 1.0)];
        let error = verify_benders_cuts(&outside).unwrap_err();
        assert!(error.message.contains("outside the model"), "{}", error.message);
        // 空系数：一条无约束。 / An empty coefficient list: no constraint at all.
        let mut empty = request();
        empty.linear[1].coefficients.clear();
        assert!(verify_benders_cuts(&empty).unwrap_err().message.contains("at least one coefficient"));
        // 同列重复：误用稀疏表示，拒绝而不是累加。 / A repeated column is refused rather than summed.
        let mut summed = request();
        summed.linear[1].coefficients = vec![coefficient(3, 1.0), coefficient(3, 2.0)];
        assert!(verify_benders_cuts(&summed).unwrap_err().message.contains("repeats a column"));
        // 非有限右端项与系数。 / Non-finite right-hand sides and coefficients.
        let mut infinite_rhs = request();
        infinite_rhs.linear[1].rhs = f64::INFINITY;
        assert!(verify_benders_cuts(&infinite_rhs).unwrap_err().message.contains("right-hand sides must be finite"));
        let mut infinite_coefficient = request();
        infinite_coefficient.linear[1].coefficients = vec![coefficient(3, f64::NAN)];
        assert!(verify_benders_cuts(&infinite_coefficient).unwrap_err().message.contains("coefficients must be finite"));
        // 二次：重复列对、两部分都空。 / Quadratic: a repeated column pair and both parts empty.
        let mut repeated_pair = request();
        repeated_pair.quadratic[0].quadratic = vec![
            BendersQuadraticTerm { left: 0, right: 1, coefficient: 1.0 },
            BendersQuadraticTerm { left: 0, right: 1, coefficient: 2.0 }];
        assert!(verify_benders_cuts(&repeated_pair).is_err());
        let mut empty_quadratic = request();
        empty_quadratic.quadratic[0].linear.clear();
        empty_quadratic.quadratic[0].quadratic.clear();
        assert!(verify_benders_cuts(&empty_quadratic).is_err());
        // 空文档：合法，全部为零。 / An empty document: legal, everything zero.
        let report = verify_benders_cuts(&BendersCutRequest {
            column_count: 0, linear: Vec::new(), quadratic: Vec::new() }).unwrap();
        assert_eq!(report.linear_cut_count, 0);
        assert_eq!(report.distinct_column_count, 0);
        assert_eq!(report.max_column, None);
    }
}
