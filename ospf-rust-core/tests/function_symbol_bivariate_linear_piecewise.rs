use ospf_rust_core::symbol::function::BivariateLinearPiecewiseFunction;
use ospf_rust_core::symbol::FunctionSymbol;

fn assert_function_symbol<T: FunctionSymbol<f64>>() {}

#[test]
fn BivariateLinearPiecewise_has_a_dedicated_contract_test_file() {
    assert_function_symbol::<BivariateLinearPiecewiseFunction<f64>>();
}
