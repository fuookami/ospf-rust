//! 推荐重量均衡及业载最大化。 / Recommended-weight equalization and payload maximization.

use std::collections::HashMap;
use ospf_rust_core::error::Result;
use ospf_rust_core::model::{LinearObjectiveInput, MetaModel};
use crate::framework::demo2::infrastructure::dto::Demo2Request;

pub(crate) fn register(
    request: &Demo2Request,
    model: &mut MetaModel,
    weights: &[usize],
) -> Result<()> {
    let priority = request.weight_recommendation_objective;
    model.add_linear_objective_input(LinearObjectiveInput::minimize("demo2_recommended_payload")
        .terms(weights.iter().map(|column| (*column, -priority.payload_priority))));
    let positions = request.positions.iter().enumerate().filter(|(_, position)| request.loading.positions[&position.name].recommended_weight.is_some())
        .map(|(p, _)| p).collect::<Vec<_>>();
    let columns = if positions.len() > 1 {
        let mut by_name = HashMap::new();
        for (index, token) in model.tokens().iter().enumerate() {
            by_name.entry(token.name()).or_insert(index);
        }
        positions.iter().map(|position| (
            by_name.get(format!("demo2_recommended_weight_{position}").as_str()).copied(),
            by_name.get(format!("demo2_actual_loaded_{position}").as_str()).copied(),
        )).collect::<Vec<_>>()
    } else { Vec::new() };
    for (i, first) in positions.iter().enumerate() {
        for (j, second) in positions.iter().enumerate().skip(i + 1) {
            let column = |column: Option<usize>| column
                .ok_or_else(|| super::loading_context::invalid("missing recommendation column"));
            let z1 = column(columns[i].0)?;
            let z2 = column(columns[j].0)?;
            let actual1 = column(columns[i].1)?;
            let actual2 = column(columns[j].1)?;
            // 全部推荐舱位两两硬均衡；实装位置按真实容量放松，而不是按输入相邻关系软惩罚。
            // Hard pairwise equalization over all recommended positions, relaxed by true capacity for actual loads, never a list-adjacent soft penalty.
            model.add_linear_constraint(&[(z1, 1.0), (z2, -1.0), (actual2, -request.positions[*first].max_weight)],
                ospf_rust_core::model::ConstraintRelation::LessEqual, 0.0, &format!("demo2_recommended_equalization_{first}_{second}"))?;
            model.add_linear_constraint(&[(z2, 1.0), (z1, -1.0), (actual1, -request.positions[*second].max_weight)],
                ospf_rust_core::model::ConstraintRelation::LessEqual, 0.0, &format!("demo2_recommended_equalization_{second}_{first}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ospf_rust_core::variable::UContinuousVariableItem;
    use super::*;

    fn request() -> Demo2Request {
        let mut request = Demo2Request::sample();
        request.positions.truncate(2);
        for position in &request.positions {
            request.loading.positions.get_mut(&position.name).unwrap().recommended_weight = Some((0.0, position.max_weight));
        }
        request
    }

    #[test]
    fn recommendation_columns_keep_first_name_match_and_pair_order() {
        let request = request();
        let mut model = MetaModel::new("recommendation-first-match");
        for name in ["demo2_recommended_weight_0", "demo2_recommended_weight_1",
            "demo2_actual_loaded_0", "demo2_actual_loaded_1", "demo2_recommended_weight_0"]
        {
            model.register_variable(UContinuousVariableItem::auto(name)).unwrap();
        }
        register(&request, &mut model, &[0, 1]).unwrap();
        let rows = model.constraints();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "demo2_recommended_equalization_0_1");
        assert_eq!(rows[1].name, "demo2_recommended_equalization_1_0");
        let terms = |row: usize| rows[row].inequality.polynomial.monomials().iter()
            .map(|term| (term.var_index(), *term.coefficient())).collect::<Vec<_>>();
        assert_eq!(terms(0), vec![(0, 1.0), (1, -1.0), (3, -request.positions[0].max_weight)]);
        assert_eq!(terms(1), vec![(1, 1.0), (0, -1.0), (2, -request.positions[1].max_weight)]);
    }

    #[test]
    fn recommendation_missing_columns_are_only_required_for_actual_pairs() {
        let mut request = request();
        let mut model = MetaModel::new("recommendation-missing");
        assert!(register(&request, &mut model, &[]).is_err());
        assert!(model.constraints().is_empty());
        request.positions.truncate(1);
        register(&request, &mut model, &[]).unwrap();
        request.positions.clear();
        register(&request, &mut model, &[]).unwrap();
        assert!(model.constraints().is_empty());
    }
}
