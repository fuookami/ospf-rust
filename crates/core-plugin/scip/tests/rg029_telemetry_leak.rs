#![cfg(all(feature = "scip", windows))]

//! RG-029 复现：带 `telemetry_callback` 的 SCIP 求解每次都泄漏约 2.5 MiB。
//!
//! RG-029 reproduction: a SCIP solve leaks about 2.5 MiB each time a `telemetry_callback`
//! is registered.
//!
//! 症状（来自 `ospf-python` 侧，`logs/p3/b388`–`b392`）：
//!
//! - 只在 `telemetry_callback` 非空时泄漏；`None` 时不泄漏；
//! - 泄漏量约 **2.5 MiB/次**，与模型规模无关（1 变量与 100 变量几乎一样）；
//! - 三条调用路径（`model.solve` / `bpp1d.solve` / `gantt_task.solve_lp`）量级一致。
//!
//! 本文件把同一症状在**上游仓库内**复现，去掉 `PyO3` / `ospf-python` 两层：
//! 连续求解 N 次，量 `PrivateUsage` 的增长。
//!
//! 两个测试：
//!
//! - `telemetry_callback_does_not_leak_per_solve` —— **必须通过**；修复前它增长 52 MiB，
//! - `without_telemetry_callback_does_not_leak` —— 同一循环的对照。
//!
//! 两个测试必须一起看：只跑失败的那个无法排除「求解本身泄漏」。

use std::ffi::c_void;

use ospf_rust_core::model::ObjectiveCategory;
use ospf_rust_core::model::intermediate::{BasicLinearTriadModel, LinearTriadModel, SparseVector};
use ospf_rust_core::solver::{LinearSolver, SolveOptions, StableVariableId};
use ospf_rust_core::token::Token;
use ospf_rust_core::variable::{BinaryVariableItem, VariableId, VariableType};
use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver, SCIPTelemetryCallback};

/// 连续求解次数。20 轮 × 约 2.5 MiB ≈ 50 MiB，远超噪声。
const ROUNDS: usize = 20;
/// 允许的增长上限（与 `ospf-python` 侧一致）。
const GROWTH_LIMIT: u64 = 4 * 1024 * 1024;

#[repr(C)]
struct ProcessMemoryCounters {
    cb: u32,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_usage: usize,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
}

#[link(name = "psapi")]
unsafe extern "system" {
    fn GetProcessMemoryInfo(
        process: *mut c_void,
        counters: *mut ProcessMemoryCounters,
        cb: u32,
    ) -> i32;
}

/// 当前进程的 `PrivateUsage` 字节数。
///
/// 必须显式传 `cb`，否则调用失败、字段全为 0——这在 `ospf-python` 侧的探针里踩过一次。
/// `cb` must be set, otherwise the call fails and every field stays zero.
fn private_usage() -> u64 {
    let mut counters = ProcessMemoryCounters {
        cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
        page_fault_count: 0,
        peak_working_set_size: 0,
        working_set_size: 0,
        quota_peak_paged_pool_usage: 0,
        quota_paged_pool_usage: 0,
        quota_peak_non_paged_pool_usage: 0,
        quota_non_paged_pool_usage: 0,
        pagefile_usage: 0,
        peak_pagefile_usage: 0,
        private_usage: 0,
    };
    let ok = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<ProcessMemoryCounters>() as u32,
        )
    };
    assert_ne!(ok, 0, "GetProcessMemoryInfo 调用失败");
    assert_ne!(counters.private_usage, 0, "PrivateUsage 为 0，调用未成功");
    counters.private_usage as u64
}

/// 一变量最大化模型。
fn build_model() -> LinearTriadModel {
    let mut basic = BasicLinearTriadModel::new("scip-telemetry-leak");
    let variable =
        BinaryVariableItem::create(VariableId::standalone(70_000), "x");
    basic.add_variable_with_bounds(Token::from_generic(variable, 0), 0.0, 1.0, VariableType::Binary);
    let mut row = SparseVector::new();
    row.add(0, 1.0);
    basic.add_constraint(row, 1.0);
    let mut model = LinearTriadModel::from_basic(basic);
    model.set_objective(vec![1.0], ObjectiveCategory::Maximum);
    model
}

/// 求解一次；`with_callback` 决定是否注册遥测回调。
fn solve_once(with_callback: bool) {
    let model = build_model();
    let callback: Option<SCIPTelemetryCallback> = with_callback.then(|| {
        std::sync::Arc::new(|_snapshot: &ospf_rust_core_plugin_scip::SCIPTelemetryStatus| Ok(()))
            as SCIPTelemetryCallback
    });
    let solver = SCIPSolver::with_config(SCIPConfig {
        output_flag: false,
        telemetry_callback: callback,
        ..Default::default()
    });
    solver
        .solve_linear_report_with_options(&model, &SolveOptions::builder().finish())
        .expect("SCIP 求解应返回报告");
}

/// 跑 `ROUNDS` 轮并返回 `PrivateUsage` 的增量。
fn measure(with_callback: bool) -> u64 {
    solve_once(with_callback); // 预热：让一次性池先建立
    let before = private_usage();
    for _ in 0..ROUNDS {
        solve_once(with_callback);
    }
    let after = private_usage();
    let growth = after.saturating_sub(before);
    // 只在 `--nocapture` 时可见；用于记录实际增长量，而不是只看通过/失败。
    // Visible with `--nocapture`; records the actual growth rather than just pass/fail.
    eprintln!(
        "telemetry_callback={with_callback}: {ROUNDS} 轮增长 {growth} B（约 {} B/轮）",
        growth / ROUNDS as u64
    );
    growth
}

#[test]
fn telemetry_callback_does_not_leak_per_solve() {
    let growth = measure(true);
    assert!(
        growth <= GROWTH_LIMIT,
        "RG-029 复现：{ROUNDS} 次带 telemetry_callback 的求解增长 {growth} B \
         （上限 {GROWTH_LIMIT}），约 {} B/次",
        growth / ROUNDS as u64
    );
}

#[test]
fn without_telemetry_callback_does_not_leak() {
    let growth = measure(false);
    assert!(
        growth <= GROWTH_LIMIT,
        "对照失败：不带回调也不应泄漏，但 {ROUNDS} 次求解增长 {growth} B（上限 {GROWTH_LIMIT}）"
    );
}

#[test]
fn telemetry_incumbent_values_stay_correct_after_cycle_break() {
    // 修复把取值路径从 `Solution::val(&Variable)` 换成 `SCIPgetSolVal(scip, best_sol, var)`，
    // 所以必须验证拿到的仍是正确解值——1 变量最大化、上界 1，最优值应为 1.0。
    // The fix swapped `Solution::val(&Variable)` for `SCIPgetSolVal(scip, best_sol, var)`, so the
    // values must still be right: one binary variable maximised with upper bound 1 gives 1.0.
    use std::sync::Arc;
    use std::sync::Mutex;

    let seen: Arc<Mutex<Vec<Option<Vec<f64>>>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let callback: SCIPTelemetryCallback = Arc::new(move |snapshot| {
        let mut guard = sink.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.push(snapshot.incumbent_solution.clone());
        Ok(())
    });
    let model = build_model();
    let solver = SCIPSolver::with_config(SCIPConfig {
        output_flag: false,
        telemetry_callback: Some(callback),
        ..Default::default()
    });
    solver
        .solve_linear_report_with_options(&model, &SolveOptions::builder().finish())
        .expect("SCIP 求解应返回报告");

    let samples = seen.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    let with_values: Vec<Vec<f64>> = samples.into_iter().flatten().collect();
    assert!(
        !with_values.is_empty(),
        "回调从未携带过 incumbent 解值，无法验证取值路径"
    );
    let last = with_values.last().expect("至少一个样本");
    assert_eq!(last.len(), 1, "变量数应为 1，实际 {}", last.len());
    assert!(
        (last[0] - 1.0).abs() < 1e-9,
        "最优解应为 1.0，实际 {:?}",
        last
    );
}

#[test]
fn telemetry_callback_is_actually_invoked() {
    // 证明「注册了回调」这件事本身成立，否则泄漏测试会变成空转。
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let calls = Arc::new(AtomicUsize::new(0));
    let model = build_model();
    let counted = Arc::clone(&calls);
    let callback: SCIPTelemetryCallback = Arc::new(move |_snapshot| {
        counted.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let solver = SCIPSolver::with_config(SCIPConfig {
        output_flag: false,
        telemetry_callback: Some(callback),
        ..Default::default()
    });
    solver
        .solve_linear_report_with_options(&model, &SolveOptions::builder().finish())
        .expect("SCIP 求解应返回报告");
    assert!(calls.load(Ordering::SeqCst) > 0, "回调一次都没被调用");
}
