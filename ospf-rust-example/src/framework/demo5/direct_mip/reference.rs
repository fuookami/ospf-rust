use super::*;

pub(super) fn solve_reference<S, D, T, A>(
    instance: &VrptwInstance<f64>,
    solver: &S,
    policy: &DirectMipPolicy<D, T, A>,
) -> Result<DirectMipResult, String>
where
    S: LinearSolver,
    D: DistanceCalculator<f64> + Sync,
    T: TravelTimeCalculator<f64> + Sync,
    A: ArcCostCalculator<f64> + Sync,
{
    if instance.vehicle_types.len() != 1 {
        return Err(
            "direct-MIP 目前要求单一车辆类型 / direct-MIP currently requires one vehicle type"
                .to_owned(),
        );
    }
    let vehicle = &instance.vehicle_types[0];
    let nodes = std::iter::once(&instance.start_depot.node)
        .chain(instance.customers.iter().map(|customer| &customer.node))
        .chain(std::iter::once(&instance.end_depot.node))
        .collect::<Vec<_>>();
    let start = 0usize;
    let end = nodes.len() - 1;
    let ready_times = std::iter::once(instance.start_depot.time_window.ready_time)
        .chain(
            instance
                .customers
                .iter()
                .map(|customer| customer.time_window.ready_time),
        )
        .chain(std::iter::once(instance.end_depot.time_window.ready_time))
        .map(|instant| instance.scheduling_window.value_of_instant(instant))
        .collect::<Vec<_>>();
    let due_times = std::iter::once(instance.start_depot.time_window.due_time)
        .chain(
            instance
                .customers
                .iter()
                .map(|customer| customer.time_window.due_time),
        )
        .chain(std::iter::once(instance.end_depot.time_window.due_time))
        .map(|instant| instance.scheduling_window.value_of_instant(instant))
        .collect::<Vec<_>>();
    let service_times = std::iter::once(0.0)
        .chain(instance.customers.iter().map(|customer| {
            instance
                .scheduling_window
                .value_of_duration(customer.service_time)
        }))
        .chain(std::iter::once(0.0))
        .collect::<Vec<_>>();
    let capacity = vehicle
        .capacity
        .to_unit(&instance.units.load_unit)
        .map_err(|error| error.to_string())?
        .value;
    let demands = instance
        .customers
        .iter()
        .map(|customer| {
            customer
                .demand
                .to_unit(&instance.units.load_unit)
                .map(|quantity| quantity.value)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let fixed_cost = vehicle
        .fixed_cost
        .to_unit(&instance.units.cost_unit)
        .map_err(|error| error.to_string())?
        .value;

    let mut model = MetaModel::<f64>::new("demo5_direct_mip");
    let mut x_indices = vec![vec![None; nodes.len()]; nodes.len()];
    let mut arcs = Vec::new();
    for from in 0..nodes.len() {
        for to in 0..nodes.len() {
            if from == end || to == start || from == to || (from == start && to == end) {
                continue;
            }
            let distance = policy
                .distance_calculator
                .distance(&nodes[from], &nodes[to], &instance.units.distance_unit)
                .map_err(|error| error.to_string())?;
            let travel_time = policy
                .travel_time_calculator
                .travel_time(&nodes[from], &nodes[to], vehicle, instance)
                .map_err(|error| error.to_string())?;
            let arc_cost = policy
                .arc_cost_calculator
                .cost(
                    &nodes[from],
                    &nodes[to],
                    &distance,
                    travel_time,
                    vehicle,
                    instance,
                )
                .map_err(|error| error.to_string())?
                .to_unit(&instance.units.cost_unit)
                .map_err(|error| error.to_string())?
                .value;
            let token_index = model
                .register_auto_variable::<Binary>(&format!("x_{from}_{to}"))
                .map_err(|error| error.to_string())?;
            x_indices[from][to] = Some(token_index);
            arcs.push(ArcData {
                from,
                to,
                token_index,
                travel_time: instance.scheduling_window.value_of_duration(travel_time),
                cost: arc_cost,
            });
        }
    }

    let service_start = ready_times
        .iter()
        .zip(due_times.iter())
        .enumerate()
        .map(|(index, (&ready, &due))| {
            model
                .register_auto_variable_with_range::<UContinuous>(
                    &format!("service_start_{index}"),
                    VariableRange::bounded(ready, due),
                )
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let load = demands
        .iter()
        .enumerate()
        .map(|(index, &demand)| {
            model
                .register_auto_variable_with_range::<UContinuous>(
                    &format!("load_{}", index + 1),
                    VariableRange::bounded(demand, capacity),
                )
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut objective = arcs
        .iter()
        .map(|arc| {
            (
                arc.token_index,
                arc.cost + if arc.from == start { fixed_cost } else { 0.0 },
            )
        })
        .collect::<Vec<_>>();
    objective.sort_unstable_by_key(|(index, _)| *index);
    model.add_linear_objective(&objective, "direct_mip_cost");
    model.set_objective_category(ObjectiveCategory::Minimum);

    let start_outgoing = arcs
        .iter()
        .filter(|arc| arc.from == start)
        .map(|arc| (arc.token_index, 1.0))
        .collect::<Vec<_>>();
    let end_incoming = arcs
        .iter()
        .filter(|arc| arc.to == end)
        .map(|arc| (arc.token_index, 1.0))
        .collect::<Vec<_>>();
    let depot_balance = difference(&start_outgoing, &end_incoming);
    add_constraint(
        &mut model,
        &depot_balance,
        ConstraintRelation::Equal,
        0.0,
        "depot_route_balance",
    )?;
    add_constraint(
        &mut model,
        &start_outgoing,
        ConstraintRelation::LessEqual,
        vehicle.amount as f64,
        "fleet_size",
    )?;

    for customer in 1..end {
        let incoming = arcs
            .iter()
            .filter(|arc| arc.to == customer)
            .map(|arc| (arc.token_index, 1.0))
            .collect::<Vec<_>>();
        let outgoing = arcs
            .iter()
            .filter(|arc| arc.from == customer)
            .map(|arc| (arc.token_index, 1.0))
            .collect::<Vec<_>>();
        add_constraint(
            &mut model,
            &incoming,
            ConstraintRelation::Equal,
            1.0,
            &format!("customer_{customer}_in"),
        )?;
        add_constraint(
            &mut model,
            &outgoing,
            ConstraintRelation::Equal,
            1.0,
            &format!("customer_{customer}_out"),
        )?;
    }

    let maximum_travel_time = arcs.iter().map(|arc| arc.travel_time).fold(0.0, f64::max);
    let maximum_service_time = service_times.iter().copied().fold(0.0, f64::max);
    let minimum_ready = ready_times.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum_due = due_times.iter().copied().fold(0.0, f64::max);
    let time_big_m = maximum_due - minimum_ready + maximum_travel_time + maximum_service_time + 1.0;
    for arc in &arcs {
        add_constraint(
            &mut model,
            &[
                (service_start[arc.from], 1.0),
                (service_start[arc.to], -1.0),
                (arc.token_index, time_big_m),
            ],
            ConstraintRelation::LessEqual,
            time_big_m - service_times[arc.from] - arc.travel_time,
            &format!("time_propagation_{}_{}", arc.from, arc.to),
        )?;
    }

    for (customer, &demand) in demands.iter().enumerate() {
        let node = customer + 1;
        add_constraint(
            &mut model,
            &[(load[customer], -1.0)],
            ConstraintRelation::LessEqual,
            -demand,
            &format!("load_lower_{node}"),
        )?;
        let from_depot = x_indices[start][node].ok_or_else(|| {
            format!(
                "客户 {} 缺少 depot 弧 / customer {} is missing a depot arc",
                node, node
            )
        })?;
        add_constraint(
            &mut model,
            &[(load[customer], -1.0), (from_depot, capacity)],
            ConstraintRelation::LessEqual,
            capacity - demand,
            &format!("load_from_depot_{node}"),
        )?;
        for other in 1..end {
            if node == other {
                continue;
            }
            let arc = x_indices[node][other].ok_or_else(|| {
                format!(
                    "客户弧缺失：{} -> {} / customer arc is missing: {} -> {}",
                    node, other, node, other
                )
            })?;
            add_constraint(
                &mut model,
                &[
                    (load[customer], 1.0),
                    (load[other - 1], -1.0),
                    (arc, capacity),
                ],
                ConstraintRelation::LessEqual,
                capacity - demands[other - 1],
                &format!("load_propagation_{}_{}", node, other),
            )?;
        }
    }

    let linear_model = model
        .try_to_linear_triad_model()
        .map_err(|error| error.to_string())?;
    let output = solver
        .solve_linear(&linear_model)
        .map_err(|error| error.to_string())?;
    if !output.status.is_feasible() {
        return Err(format!(
            "direct-MIP 求解失败：{:?} / direct-MIP solve failed: {:?}",
            output.status, output.status
        ));
    }
    let objective = output.objective_value.ok_or_else(|| {
        "direct-MIP 缺少目标值 / direct-MIP output has no objective value".to_owned()
    })?;
    let solution = output.solution.as_deref().ok_or_else(|| {
        "direct-MIP 缺少解向量 / direct-MIP output has no solution vector".to_owned()
    })?;
    let customer_sequences = extract_sequences(instance, &model, solution, &arcs, start, end)?;
    Ok(DirectMipResult {
        status: output.status,
        objective,
        best_bound: output.best_bound,
        relative_gap: output.mip_gap,
        customer_sequences,
    })
}
