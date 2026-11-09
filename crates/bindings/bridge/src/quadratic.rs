//! 复用远程二次模型 codec 与 core 指纹 / Reuse the remote quadratic codec and core fingerprints.

use std::collections::BTreeMap;
use ospf_rust_core::model::{Quadratic, QuadraticInequality, QuadraticMonomial};
use ospf_rust_core::model::intermediate::{BasicQuadraticTetradModel, QuadraticTetradModel, SparseMatrix, SparseVector};
use ospf_rust_core::solver::quadratic_model_fingerprint;
use ospf_rust_framework::solver::remote::{
    SerializedConstraintSign, SerializedLinearModel, SerializedObjective,
    SerializedQuadraticModel, serialize_quadratic_model,
};
use crate::{BridgeError, ModelArtifact, Result, compile_linear, invalid};

/// 从单次多项式系数构造 Q，不隐含对称矩阵的二倍因子。
/// Construct Q from once-counted polynomial coefficients without an implicit symmetric-matrix factor of two.
pub fn quadratic_objective_matrix(
    count: usize,
    terms: impl IntoIterator<Item = (usize, usize, f64)>,
) -> Result<SparseMatrix<f64>> {
    let mut coefficients = BTreeMap::new();
    for (first, second, value) in terms {
        if first >= count || second >= count || !value.is_finite() {
            return Err(invalid("invalid quadratic objective index or coefficient"));
        }
        let key = (first.min(second), first.max(second));
        let coefficient = coefficients.entry(key).or_insert(0.0f64);
        *coefficient += value;
        if !coefficient.is_finite() { return Err(invalid("quadratic coefficient overflow")); }
    }
    let mut matrix = SparseMatrix { rows: vec![SparseVector::new(); count] };
    for ((row, column), value) in coefficients {
        if value != 0.0 { matrix.rows[row].add(column, value); }
    }
    Ok(matrix)
}

/// 校验并编译现有二次协议；线性部分仍经 MetaModel 编译。
/// Validate and compile the existing quadratic protocol, still compiling its linear part through MetaModel.
pub fn compile_quadratic(input: &SerializedQuadraticModel) -> Result<QuadraticTetradModel> {
    input.validate_contract().map_err(invalid)?;
    let linear = compile_linear(&SerializedLinearModel {
        schema_version: input.schema_version.clone(), name: input.name.clone(),
        variables: input.variables.clone(), constraints: input.linear_constraints.clone(),
        objective: SerializedObjective {
            stable_id: input.objective.stable_id.clone(), category: input.objective.category,
            cells: input.objective.linear_cells.clone(), constant: input.objective.constant,
        },
    })?;
    let count = linear.num_variables();
    let mut model = QuadraticTetradModel::from_basic(BasicQuadraticTetradModel::from_linear(linear.basic));
    model.c = linear.c;
    model.objective_category = linear.objective_category;
    model.Q = quadratic_objective_matrix(count, input.objective.quadratic_cells.iter().map(|cell| (
        cell.col_index1, cell.col_index2, cell.coefficient,
    )))?;
    for (row, constraint) in input.quadratic_constraints.iter().enumerate() {
        if !constraint.rhs.is_finite() { return Err(invalid("non-finite quadratic constraint RHS")); }
        let mut terms = BTreeMap::new();
        for cell in &constraint.linear_cells {
            if cell.row_index != row || cell.col_index >= count || !cell.coefficient.is_finite() {
                return Err(invalid("invalid quadratic constraint linear cell"));
            }
            let value = terms.entry((cell.col_index, None)).or_insert(0.0f64);
            *value += cell.coefficient;
            if !value.is_finite() { return Err(invalid("quadratic constraint coefficient overflow")); }
        }
        for cell in &constraint.quadratic_cells {
            if cell.row_index != row || cell.col_index1 >= count || cell.col_index2 >= count || !cell.coefficient.is_finite() {
                return Err(invalid("invalid quadratic constraint cell"));
            }
            let key = (cell.col_index1.min(cell.col_index2), Some(cell.col_index1.max(cell.col_index2)));
            let value = terms.entry(key).or_insert(0.0f64);
            *value += cell.coefficient;
            if !value.is_finite() { return Err(invalid("quadratic constraint coefficient overflow")); }
        }
        let monomials = terms.into_iter().filter(|(_, value)| *value != 0.0).map(|((first, second), value)| {
            match second {
                Some(second) => QuadraticMonomial::new_quadratic(value, first, second),
                None => QuadraticMonomial::new_linear(value, first),
            }
        }).collect();
        let relation = match constraint.sign {
            SerializedConstraintSign::LessEqual => ospf_rust_core::model::ConstraintRelation::LessEqual,
            SerializedConstraintSign::GreaterEqual => ospf_rust_core::model::ConstraintRelation::GreaterEqual,
            SerializedConstraintSign::Equal => ospf_rust_core::model::ConstraintRelation::Equal,
            _ => return Err(invalid("unknown quadratic constraint sign")),
        };
        model.add_quadratic_constraint_with_metadata(
            QuadraticInequality::new(Quadratic::new(monomials, 0.0), relation, constraint.rhs),
            constraint.name.clone(), None, false, 0, None, None,
        );
    }
    quadratic_model_fingerprint(&model).map_err(invalid)?;
    Ok(model)
}

/// 编码现有二次模型，拒绝 codec 无法保留的非规范信息。
/// Encode an existing quadratic model, rejecting noncanonical information the codec cannot retain.
/// 非零约束多项式常数不能由当前 codec 无损保留。
/// Nonzero constraint polynomial constants cannot be retained losslessly by the current codec.
pub fn encode_quadratic_model(model: &QuadraticTetradModel) -> Result<ModelArtifact> {
    let fingerprint = quadratic_model_fingerprint(model).map_err(invalid)?.value;
    if model.quadratic_constraints.iter().any(|constraint| *constraint.polynomial.constant() != 0.0) {
        return Err(BridgeError::new("Unsupported", "quadratic constraint constants cannot be preserved by this codec"));
    }
    let serialized = serialize_quadratic_model(model);
    // 编码前闭环校验，禁止元数据或常数归一化导致指纹漂移后才交给用户。
    // Check the codec round trip before handing users an artifact with drifting metadata or normalized constants.
    let decoded = compile_quadratic(&serialized)?;
    if quadratic_model_fingerprint(&decoded).map_err(invalid)?.value != fingerprint
        || model.basic.linear.var_types != decoded.basic.linear.var_types
    {
        return Err(BridgeError::new("Unsupported", "quadratic model contains information not preserved by this codec"));
    }
    Ok(ModelArtifact { model_json: serde_json::to_string(&serialized).map_err(invalid)?, fingerprint })
}

/// 解码并校验二次模型指纹 / Decode and validate a quadratic model fingerprint.
pub fn decode_quadratic_model(artifact: &ModelArtifact) -> Result<QuadraticTetradModel> {
    let input: SerializedQuadraticModel = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
    let model = compile_quadratic(&input)?;
    if quadratic_model_fingerprint(&model).map_err(invalid)?.value != artifact.fingerprint {
        return Err(BridgeError::new("FingerprintMismatch", "quadratic model fingerprint mismatch"));
    }
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ospf_rust_core::model::{ConstraintRelation, MetaModel};
    use ospf_rust_core::variable::{ContinuousVariableItem, VariableRange};

    fn example() -> QuadraticTetradModel {
        let mut source = MetaModel::<f64>::new("quadratic-codec");
        source.add_variable(ContinuousVariableItem::auto_with_range("x", VariableRange::new(None, Some(5.0)))).unwrap();
        source.add_variable(ContinuousVariableItem::auto_with_range("y", VariableRange::new(Some(0.0), Some(5.0)))).unwrap();
        let mut model = source.try_to_quadratic_tetrad_model().unwrap();
        model.Q = quadratic_objective_matrix(2, [(0, 0, 1.0), (0, 1, 2.0), (1, 1, 3.0)]).unwrap();
        model.add_quadratic_constraint_with_metadata(
            QuadraticInequality::new(Quadratic::new(vec![QuadraticMonomial::new_quadratic(1.0, 0, 1)], 0.0), ConstraintRelation::LessEqual, 4.0),
            "capacity".into(), None, false, 0, None, None,
        );
        model
    }

    #[test]
    fn quadratic_codec_and_core_fingerprints_round_trip() {
        let artifact = encode_quadratic_model(&example()).unwrap();
        assert!(artifact.model_json.contains("-Infinity"));
        let decoded = decode_quadratic_model(&artifact).unwrap();
        assert_eq!(encode_quadratic_model(&decoded).unwrap().fingerprint, artifact.fingerprint);
        assert_eq!(decoded.Q.rows[0].entries, vec![(0, 1.0), (1, 2.0)]);
        let mut tampered = artifact;
        tampered.fingerprint = "wrong".into();
        assert_eq!(decode_quadratic_model(&tampered).unwrap_err().code, "FingerprintMismatch");
    }

    #[test]
    fn malformed_quadratic_cells_constants_and_metadata_never_degrade_silently() {
        let model = example();
        let mut input = serialize_quadratic_model(&model);
        input.objective.quadratic_cells[0].col_index1 = 3;
        assert_eq!(compile_quadratic(&input).unwrap_err().code, "InvalidInput");
        input = serialize_quadratic_model(&model);
        input.quadratic_constraints[0].quadratic_cells[0].row_index = 1;
        assert!(compile_quadratic(&input).is_err());
        input = serialize_quadratic_model(&model);
        input.objective.constant = 1.0;
        assert_eq!(compile_quadratic(&input).unwrap_err().code, "Unsupported");
        assert!(quadratic_objective_matrix(2, [(0, 1, f64::MAX), (1, 0, f64::MAX)]).is_err());
        let mut metadata = model;
        metadata.quadratic_constraint_priorities[0] = 1;
        assert_eq!(encode_quadratic_model(&metadata).unwrap_err().code, "Unsupported");
    }

    #[test]
    fn quadratic_codec_rejects_lost_solver_variable_type() {
        use ospf_rust_core::variable::VariableType;

        let mut model = example();
        model.basic.linear.var_types[0] = VariableType::UContinuous;
        let restored = compile_quadratic(&serialize_quadratic_model(&model)).unwrap();
        assert_ne!(restored.basic.linear.var_types, model.basic.linear.var_types);
        assert_eq!(encode_quadratic_model(&model).unwrap_err().code, "Unsupported");
    }

    #[test]
    fn quadratic_codec_rejects_a_constraint_constant_hidden_by_rounding() {
        let mut model = example();
        model.quadratic_constraints[0].polynomial = Quadratic::new(
            vec![QuadraticMonomial::new_quadratic(1.0, 0, 1)], 1.0,
        );
        model.quadratic_constraints[0].rhs = 1e300;
        let restored = compile_quadratic(&serialize_quadratic_model(&model)).unwrap();
        assert_eq!(restored.quadratic_constraints[0].rhs, model.quadratic_constraints[0].rhs);
        assert_ne!(restored.quadratic_constraints[0].polynomial.constant(), model.quadratic_constraints[0].polynomial.constant());
        assert_eq!(encode_quadratic_model(&model).unwrap_err().code, "Unsupported");
    }
}
