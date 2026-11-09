use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, Barrier, Mutex, mpsc};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use super::*;

type Job = Box<dyn FnOnce() + Send>;
type ChunkResult = Result<(usize, Vec<Vec<f64>>), Box<dyn Any + Send>>;

static VALIDATION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Default)]
struct Counters {
    active: AtomicUsize,
    peak: AtomicUsize,
    parallel: AtomicUsize,
    busy: AtomicUsize,
    small: AtomicUsize,
}

struct BuildPool {
    sender: Option<mpsc::SyncSender<Job>>,
    threads: Vec<JoinHandle<()>>,
    busy: AtomicBool,
    counters: Arc<Counters>,
}

struct Release<'a>(&'a AtomicBool);

impl Drop for Release<'_> {
    fn drop(&mut self) { self.0.store(false, Ordering::Release); }
}

impl BuildPool {
    fn new(workers: usize) -> Self {
        assert!(matches!(workers, 2 | 4));
        let (sender, receiver) = mpsc::sync_channel::<Job>(workers);
        let receiver = Arc::new(Mutex::new(receiver));
        let counters = Arc::new(Counters::default());
        let threads = (0..workers).map(|index| {
            let receiver = receiver.clone();
            let counters = counters.clone();
            thread::Builder::new().name(format!("coverage-{index}")).spawn(move || {
                loop {
                    let job = receiver.lock().unwrap().recv();
                    let Ok(job) = job else { break; };
                    let active = counters.active.fetch_add(1, Ordering::SeqCst) + 1;
                    counters.peak.fetch_max(active, Ordering::SeqCst);
                    job();
                    counters.active.fetch_sub(1, Ordering::SeqCst);
                }
            }).unwrap()
        }).collect();
        Self { sender: Some(sender), threads, busy: AtomicBool::new(false), counters }
    }

    fn projection(&self, master: &Arc<Csp2dMasterProblem>, fail: bool) -> CoverageProjection {
        let _release = Release(&self.busy);
        let workers = self.threads.len();
        let chunk = master.assignment.len().div_ceil(workers);
        let (sender, receiver) = mpsc::channel::<ChunkResult>();
        let needs_area = master.demands.iter().any(|demand| demand.mode == DemandMode2::Area);
        let mut tasks = 0;
        for start in (0..master.assignment.len()).step_by(chunk) {
            tasks += 1;
            let sender = sender.clone();
            let master = master.clone();
            self.sender.as_ref().unwrap().send(Box::new(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    assert!(!fail || start != 0, "injected build failure");
                    let end = (start + chunk).min(master.assignment.len());
                    let plans: Vec<_> = master.assignment.plans[start..end].iter()
                        .map(|plan| PlanCoverage::new(plan, needs_area)).collect();
                    let rows = master.demands.iter().map(|demand| plans.iter()
                        .map(|plan| plan.coefficient(demand)).collect()).collect();
                    (start, rows)
                }));
                let _ = sender.send(result);
            })).unwrap();
        }
        drop(sender);
        // 先排空全部结果再传播失败，避免提前释放构建配额。 / Drain all results before propagating failure so the build permit is not released early.
        let mut chunks = Vec::with_capacity(tasks);
        let mut failure = None;
        for _ in 0..tasks {
            match receiver.recv().unwrap() {
                Ok(chunk) => chunks.push(chunk),
                Err(payload) => { failure.get_or_insert(payload); },
            }
        }
        if let Some(payload) = failure { resume_unwind(payload); }
        chunks.sort_by_key(|(start, _)| *start);
        let mut rows: Vec<Vec<f64>> = master.demands.iter()
            .map(|_| Vec::with_capacity(master.assignment.len())).collect();
        for (_, chunk) in chunks {
            for (row, values) in rows.iter_mut().zip(chunk) { row.extend(values); }
        }
        CoverageProjection { rows }
    }

    fn solve(&self, master: &Arc<Csp2dMasterProblem>) -> Vec<f64> {
        if master.assignment.len() < 256 {
            self.counters.small.fetch_add(1, Ordering::Relaxed);
            return serial(master);
        }
        // 不排队等待配额；同一进程的其他求解立即走原串行入口。 / Do not queue for a permit; other solves in the process immediately use the original serial entry.
        if self.busy.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            self.counters.busy.fetch_add(1, Ordering::Relaxed);
            return serial(master);
        }
        self.counters.parallel.fetch_add(1, Ordering::Relaxed);
        let projection = self.projection(master, false);
        projected_usage(master, &projection, true)
    }
}

impl Drop for BuildPool {
    fn drop(&mut self) {
        self.sender.take();
        for thread in self.threads.drain(..) { thread.join().unwrap(); }
    }
}

fn serial(master: &Csp2dMasterProblem) -> Vec<f64> {
    Csp2dHeuristicMasterSolver::new().solve_master(master).unwrap().usage
}

fn master(count: usize) -> Arc<Csp2dMasterProblem> {
    let (assignment, demands) = fixture(count);
    Arc::new(Csp2dMasterProblem::new(assignment, demands).unwrap())
}

#[test]
fn shared_budget_is_bounded_with_serial_small_and_busy_fallback() {
    let _budget = VALIDATION_LOCK.lock().unwrap();
    for workers in [2, 4] {
        let pool = BuildPool::new(workers);
        let small = master(3);
        assert_eq!(pool.solve(&small), serial(&small));
        assert_eq!(pool.counters.peak.load(Ordering::SeqCst), 0);
        let large = master(256);
        let expected = serial(&large);
        pool.busy.store(true, Ordering::Release);
        assert_eq!(pool.solve(&large), expected);
        assert_eq!(pool.counters.busy.load(Ordering::Relaxed), 1);
        pool.busy.store(false, Ordering::Release);
        let barrier = Barrier::new(4);
        thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    barrier.wait();
                    for _ in 0..2 { assert_eq!(pool.solve(&large), expected); }
                });
            }
        });
        assert!(pool.counters.peak.load(Ordering::SeqCst) <= workers);
        assert!(pool.counters.parallel.load(Ordering::Relaxed) > 0);
        assert!(!pool.busy.load(Ordering::Acquire));
    }
}

#[test]
fn failed_worker_build_drains_and_releases_the_shared_budget() {
    let _budget = VALIDATION_LOCK.lock().unwrap();
    let pool = BuildPool::new(2);
    let master = master(256);
    pool.busy.store(true, Ordering::Release);
    assert!(catch_unwind(AssertUnwindSafe(|| pool.projection(&master, true))).is_err());
    assert!(!pool.busy.load(Ordering::Acquire));
    assert_eq!(pool.solve(&master), serial(&master));
    assert!(pool.counters.peak.load(Ordering::SeqCst) <= 2);
}

#[test]
fn chunked_projection_preserves_uneven_and_mutated_inputs() {
    let _budget = VALIDATION_LOCK.lock().unwrap();
    for workers in [2, 4] {
        let pool = BuildPool::new(workers);
        let mut master = master(259);
        let mutable = Arc::get_mut(&mut master).unwrap();
        for (index, plan) in mutable.assignment.plans.iter_mut().enumerate() {
            plan.pattern.placements.truncate(index % 65);
        }
        mutable.assignment.plans.reverse();
        mutable.assignment.usage_upper_bounds[0] = 0;
        mutable.demands.push(Csp2dMasterDemand::new(
            product_id2("missing"), DemandValue2::Amount(2),
        ));
        for variant in 0..3 {
            let mutable = Arc::get_mut(&mut master).unwrap();
            match variant {
                1 => mutable.demands.retain(|demand| demand.mode == DemandMode2::Amount),
                2 => mutable.demands.clear(),
                _ => {},
            }
            pool.busy.store(true, Ordering::Release);
            let projection = pool.projection(&master, false);
            assert_eq!(projection.rows, master.assignment.coverage_projection(&master.demands).rows);
            assert!(!pool.busy.load(Ordering::Acquire));
            assert_eq!(pool.solve(&master), serial(&master));
        }
        assert!(pool.counters.peak.load(Ordering::SeqCst) <= workers);
    }
}

fn report(label: &str, mut values: Vec<f64>, elapsed: f64) {
    values.sort_by(f64::total_cmp);
    let percentile = |percent: usize| values[(values.len() * percent).div_ceil(100) - 1];
    println!("POOL_RESULT {label} n={} p50_ms={:.6} p95_ms={:.6} p99_ms={:.6} min_ms={:.6} max_ms={:.6} wall_s={elapsed:.6} requests_s={:.6}",
        values.len(), percentile(50), percentile(95), percentile(99), values[0], values[values.len() - 1], values.len() as f64 / elapsed);
    println!("POOL_LATENCIES {label} sorted_ms={values:?}");
}

#[cfg(windows)]
fn memory(label: &str) {
    #[repr(C)]
    #[derive(Default)]
    struct Memory {
        size: u32,
        faults: u32,
        peak_working_set: usize,
        working_set: usize,
        peak_paged_pool: usize,
        paged_pool: usize,
        peak_nonpaged_pool: usize,
        nonpaged_pool: usize,
        pagefile: usize,
        peak_pagefile: usize,
        private: usize,
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(process: *mut std::ffi::c_void, info: *mut Memory, size: u32) -> i32;
    }
    let mut info = Memory::default();
    info.size = std::mem::size_of::<Memory>() as u32;
    // Windows 当前进程伪句柄；结构布局对应 PROCESS_MEMORY_COUNTERS_EX。 / Windows current-process pseudo-handle; layout matches PROCESS_MEMORY_COUNTERS_EX.
    assert_ne!(unsafe { GetProcessMemoryInfo(-1isize as *mut _, &mut info, std::mem::size_of::<Memory>() as u32) }, 0);
    println!("POOL_MEMORY {label} working_set={} peak_working_set={} private_bytes={} peak_commit={}",
        info.working_set, info.peak_working_set, info.private, info.peak_pagefile);
}

#[cfg(not(windows))]
fn memory(label: &str) { println!("POOL_MEMORY {label} unavailable=true"); }

#[test]
#[ignore = "新进程手动release共享线程配额验证 / Manual release shared-worker budget validation in fresh processes"]
fn performance_shared_worker_budget() {
    let _budget = VALIDATION_LOCK.lock().unwrap();
    let workers: usize = std::env::var("OSPF_BUILD_WORKERS").unwrap().parse().unwrap();
    assert!(matches!(workers, 0 | 2 | 4));
    memory(&format!("workers={workers} stage=startup"));
    let start = Instant::now();
    let pool = (workers > 0).then(|| BuildPool::new(workers));
    println!("POOL_INIT workers={workers} elapsed_us={:.3}", start.elapsed().as_secs_f64() * 1e6);
    memory(&format!("workers={workers} stage=pool_ready"));
    let solve = |master: &Arc<Csp2dMasterProblem>| match &pool {
        Some(pool) => pool.solve(master),
        None => serial(master),
    };
    for count in [32, 256, 1024] {
        let master = master(count);
        let expected = serial(&master);
        let start = Instant::now();
        assert_eq!(solve(&master), expected);
        println!("POOL_FIRST workers={workers} plans={count} elapsed_ms={:.6}", start.elapsed().as_secs_f64() * 1000.0);
        for outer in [1, 4] {
            let barrier = Barrier::new(outer);
            let start = Instant::now();
            let latencies: Vec<_> = thread::scope(|scope| {
                let handles: Vec<_> = (0..outer).map(|_| scope.spawn(|| {
                    let mut latencies = Vec::with_capacity(200 / outer);
                    barrier.wait();
                    for _ in 0..200 / outer {
                        let start = Instant::now();
                        let actual = solve(black_box(&master));
                        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                        assert_eq!(actual, expected);
                        latencies.push(elapsed);
                    }
                    latencies
                })).collect();
                handles.into_iter().flat_map(|handle| handle.join().unwrap()).collect()
            });
            report(&format!("workers={workers} plans={count} outer={outer}"), latencies, start.elapsed().as_secs_f64());
            memory(&format!("workers={workers} plans={count} outer={outer}"));
        }
    }
    // 三个大请求调用者持续施压，小请求作为可观察的前景主线。 / Three large-request callers provide load while small requests form the observed foreground.
    let large = master(1024);
    let small = master(32);
    let expected_large = serial(&large);
    let expected_small = serial(&small);
    let stop = AtomicBool::new(false);
    let barrier = Barrier::new(4);
    let (foreground, background, elapsed) = thread::scope(|scope| {
        let handles: Vec<_> = (0..3).map(|_| scope.spawn(|| {
            let mut latencies = Vec::new();
            barrier.wait();
            while !stop.load(Ordering::Acquire) {
                let start = Instant::now();
                let actual = solve(&large);
                latencies.push(start.elapsed().as_secs_f64() * 1000.0);
                assert_eq!(actual, expected_large);
            }
            latencies
        })).collect();
        barrier.wait();
        let start = Instant::now();
        let mut foreground = Vec::with_capacity(200);
        for _ in 0..200 {
            let start = Instant::now();
            let actual = solve(&small);
            foreground.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(actual, expected_small);
        }
        stop.store(true, Ordering::Release);
        let background: Vec<_> = handles.into_iter().flat_map(|handle| handle.join().unwrap()).collect();
        (foreground, background, start.elapsed().as_secs_f64())
    });
    report(&format!("workers={workers} foreground=small32 background_callers=3"), foreground, elapsed);
    report(&format!("workers={workers} background=large1024 foreground_callers=1"), background, elapsed);
    memory(&format!("workers={workers} mixed=true"));
    if let Some(pool) = &pool {
        println!("POOL_COUNTERS workers={workers} peak_active={} parallel={} busy_fallback={} small_fallback={}",
            pool.counters.peak.load(Ordering::SeqCst), pool.counters.parallel.load(Ordering::Relaxed),
            pool.counters.busy.load(Ordering::Relaxed), pool.counters.small.load(Ordering::Relaxed));
        assert!(pool.counters.peak.load(Ordering::SeqCst) <= workers);
    }
}
