use std::collections::HashMap;
use crate::domain::vrp::{ArcFeasibilityPolicy, VrpNode};
use super::*;

fn builder(instance: Arc<VrptwInstance<f64>>) -> RouteGraphBuilder<f64> {
    RouteGraphBuilder::new(instance, EuclideanDistanceCalculator,
        DistanceAsTravelTimeCalculator::new(EuclideanDistanceCalculator), DistanceArcCostCalculator, FixedPlusArcCostPolicy)
}

#[test]
fn graph_indexing_preserves_implicit_explicit_order_branch_filtering_and_unknown_endpoints() {
    let implicit = multi_customer_instance(3);
    let duals = PricingDuals::new(PricingPhase::PhaseTwo, [("c1".into(), 3.0)], []);
    let full = builder(implicit.clone()).build(&"v1".into(), &duals, None).unwrap();
    let mut explicit = (*implicit).clone();
    explicit.arcs = full.arcs.iter().map(|arc| VrptwArc::new(
        arc.arc_id.clone(), full.nodes[arc.from].node_id.clone(), full.nodes[arc.to].node_id.clone(),
        Quantity::new(arc.distance, explicit.units.distance_unit.clone()), TravelTime::new(arc.travel_time).unwrap(),
        Quantity::new(arc.arc_cost, explicit.units.cost_unit.clone()),
    ).unwrap()).collect();
    let mask = BranchMask::new(implicit.start_depot.node.id.clone(), implicit.end_depot.node.id.clone(),
        BTreeMap::from([("v1".into(), BTreeSet::from([implicit.customers[1].node.id.clone()]))]),
        BTreeMap::new(), BTreeSet::new(), BTreeSet::new()).unwrap();
    for mask in [None, Some(&mask)] {
        let expected = builder(implicit.clone()).build(&"v1".into(), &duals, mask).unwrap();
        let actual = builder(Arc::new(explicit.clone())).build(&"v1".into(), &duals, mask).unwrap();
        assert_eq!(format!("{:?}", (&expected.nodes, &expected.arcs, &expected.outgoing)),
            format!("{:?}", (&actual.nodes, &actual.arcs, &actual.outgoing)));
    }
    let mut unknown = explicit.arcs[0].clone();
    unknown.id = "unknown-endpoint".into();
    unknown.from = "missing".into();
    explicit.arcs.push(unknown);
    let mut infeasible = explicit.arcs[0].clone().with_feasibility(false);
    infeasible.id = "infeasible".into();
    explicit.arcs.push(infeasible);
    let actual = builder(Arc::new(explicit)).build(&"v1".into(), &duals, None).unwrap();
    assert_eq!(format!("{:?}", &actual.arcs), format!("{:?}", &full.arcs));
}

#[test]
fn graph_endpoint_index_keeps_original_resolution_precedence_for_unvalidated_duplicates() {
    struct CheckStart;
    impl ArcFeasibilityPolicy<f64> for CheckStart {
        fn is_feasible(&self, from: &VrpNode<f64>, to: &VrpNode<f64>, _: &VehicleType<f64>) -> crate::Result<bool> {
            for node in [from, to] {
                if node.id.as_str() == "start" { assert_eq!(node.payload.axes["x"].value, 0.0); }
            }
            Ok(from.id != to.id)
        }
    }
    let mut instance = (*multi_customer_instance(2)).clone();
    instance.customers[0].node.id = instance.start_depot.node.id.clone();
    let graph = builder(Arc::new(instance)).with_arc_feasibility_policy(Arc::new(CheckStart))
        .build(&"v1".into(), &PricingDuals::new(PricingPhase::PhaseTwo, [], []), None).unwrap();
    assert_eq!(graph.nodes.len(), 4);
    assert!(graph.has_valid_integrity());
}

#[test]
#[ignore = "manual release-mode graph endpoint lookup timings"]
fn performance_b03_graph_endpoints() {
    use std::hint::black_box;

    for amount in [16, 64, 128] {
        let instance = multi_customer_instance(amount);
        let ids = std::iter::once(&instance.start_depot.node.id).chain(instance.customers.iter().map(|customer| &customer.node.id))
            .chain(std::iter::once(&instance.end_depot.node.id)).collect::<Vec<_>>();
        for indexed in [false, true] {
            let mut samples = Vec::new();
            for _ in 0..5 {
                let start = Instant::now();
                let mut original_nodes = HashMap::new();
                if indexed {
                    for node in std::iter::once(&instance.start_depot.node).chain(std::iter::once(&instance.end_depot.node))
                        .chain(instance.customers.iter().map(|customer| &customer.node))
                    { original_nodes.entry(&node.id).or_insert(node); }
                }
                for from in black_box(&ids) {
                    for to in &ids {
                        if from == to || to == &ids[0] || from == ids.last().unwrap() { continue; }
                        for id in [from, to] {
                            let node = if indexed { original_nodes[id] }
                                else if *id == &instance.start_depot.node.id { &instance.start_depot.node }
                                else if *id == &instance.end_depot.node.id { &instance.end_depot.node }
                                else { &instance.customers.iter().find(|customer| &customer.node.id == *id).unwrap().node };
                            black_box(node);
                        }
                    }
                }
                samples.push(start.elapsed().as_nanos());
            }
            samples.sort_unstable();
            println!("P02 endpoints={amount}, indexed={indexed}: median={} ns; samples={samples:?}", samples[2]);
        }
    }
}
