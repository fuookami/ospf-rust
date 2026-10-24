use std::sync::atomic::{AtomicBool, Ordering};
use crate::domain::route_compilation::RouteCompilationAggregation;
use crate::error::NetworkSchedulingError;
use super::*;

struct FailingLifecycle {
    register: Arc<AtomicBool>,
    add: Arc<AtomicBool>,
    remove: Arc<AtomicBool>,
}

fn fail_once(flag: &AtomicBool, model: &mut MetaModel<f64>) -> crate::Result<()> {
    if flag.swap(false, Ordering::SeqCst) {
        model.register_auto_variable::<ospf_rust_core::variable::Continuous>("failed-hook-variable").unwrap();
        return Err(NetworkSchedulingError::validation("注入列生命周期失败 / injected column lifecycle failure"));
    }
    Ok(())
}

impl RouteCompilationExtension<f64> for FailingLifecycle {
    fn register(&self, model: &mut MetaModel<f64>, _: &RouteCompilationAggregation<f64>) -> crate::Result<()> {
        fail_once(&self.register, model)
    }

    fn add_columns(&self, _: &[Route<f64>], model: &mut MetaModel<f64>) -> crate::Result<()> {
        fail_once(&self.add, model)
    }

    fn remove_columns(&self, _: &[Route<f64>], model: &mut MetaModel<f64>) -> crate::Result<()> {
        fail_once(&self.remove, model)
    }
}

#[test]
fn route_journal_restores_payload_storage_order_and_serials_after_extension_failures() {
    let instance = multi_customer_instance(3);
    let routes = InitialRouteGenerator::new(instance.clone(), EuclideanDistanceCalculator,
        DistanceAsTravelTimeCalculator::new(EuclideanDistanceCalculator), DistanceArcCostCalculator,
        FixedPlusArcCostPolicy).generate(None).unwrap();
    assert_eq!(routes.len(), 3);
    let register = Arc::new(AtomicBool::new(true));
    let add = Arc::new(AtomicBool::new(false));
    let remove = Arc::new(AtomicBool::new(false));
    let mut context = RouteCompilationContext::with_extensions(instance, vec![Box::new(FailingLifecycle {
        register, add: add.clone(), remove: remove.clone(),
    })]);
    let mut model = MetaModel::new("route-journal");
    assert!(context.register(&mut model).is_err());
    assert!(model.tokens().is_empty());
    context.register(&mut model).unwrap();
    context.add_columns(0, routes[..2].iter().cloned(), &mut model).unwrap();
    let original_pointers = context.aggregation.routes().iter().map(|route| route.stops.as_ptr()).collect::<Vec<_>>();
    let original_routes = format!("{:?}", context.aggregation.routes());
    let original_indices = context.aggregation.route_variable_indices().clone();
    let original_model = linear_model_fingerprint(&model.try_to_linear_triad_model().unwrap()).unwrap();
    for duplicate in [false, true] {
        add.store(true, Ordering::SeqCst);
        let candidate = if duplicate { routes[0].clone() } else { routes[2].clone() };
        assert!(context.add_columns(1, [candidate], &mut model).is_err());
        assert_eq!(format!("{:?}", context.aggregation.routes()), original_routes);
        assert_eq!(context.aggregation.route_variable_indices(), &original_indices);
        assert_eq!(linear_model_fingerprint(&model.try_to_linear_triad_model().unwrap()).unwrap(), original_model);
        assert_eq!(context.aggregation.routes().iter().map(|route| route.stops.as_ptr()).collect::<Vec<_>>(), original_pointers);
    }
    remove.store(true, Ordering::SeqCst);
    assert!(context.remove_columns([routes[1].clone(), routes[0].clone(), routes[1].clone()], &mut model).is_err());
    assert_eq!(format!("{:?}", context.aggregation.routes()), original_routes);
    assert_eq!(context.aggregation.route_variable_indices(), &original_indices);
    assert_eq!(context.aggregation.routes().iter().map(|route| route.stops.as_ptr()).collect::<Vec<_>>(), original_pointers);
    assert_eq!(linear_model_fingerprint(&model.try_to_linear_triad_model().unwrap()).unwrap(), original_model);
    context.add_columns(1, [routes[2].clone()], &mut model).unwrap();
    let serial = context.aggregation.route_variable_indices()[&routes[2].signature()].model_index;
    assert_eq!(model.tokens()[serial].name(), "route_column_2");
    context.remove_columns([routes[1].clone()], &mut model).unwrap();
    let kept_pointers = context.aggregation.routes().iter().map(|route| route.stops.as_ptr()).collect::<Vec<_>>();
    assert_eq!(kept_pointers[0], original_pointers[0]);
    context.add_columns(2, [routes[1].clone()], &mut model).unwrap();
    let new = context.aggregation.route_variable_indices()[&routes[1].signature()].model_index;
    assert_ne!(new, original_indices[&routes[1].signature()].model_index);
    assert_eq!(context.aggregation.routes()[0].signature(), routes[0].signature());
    assert_eq!(context.aggregation.routes()[1].signature(), routes[2].signature());
    assert_eq!(context.aggregation.routes()[2].signature(), routes[1].signature());
    context.aggregation.validate_pricing_model(&model).unwrap();
    context.switch_to_phase_two(&mut model).unwrap();
    context.aggregation.validate_pricing_model(&model).unwrap();
}
