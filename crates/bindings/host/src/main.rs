use std::net::SocketAddr;
use axum::http::HeaderValue;
use ospf_rust_host::{HostState, router};
use ospf_rust_native::NativeEngine;

/// 工作线程栈大小：`SolveRequest` 已经长到 10 KB 以上，而 tokio 默认只给工作线程 2 MiB。
/// 实测：默认栈上解一份真实检查点会直接 `has overflowed its stack`（进程当场消失，客户端只看到
/// `ECONNRESET`）。抬到 8 MiB 与测试侧 `RUST_MIN_STACK` 口径一致，避免每加一个可选载荷字段都要重调。
/// Worker-thread stack size: `SolveRequest` has grown past 10 KB while tokio gives workers 2 MiB by default.
/// Measured: decoding a real checkpoint on the default stack aborts with `has overflowed its stack` (the process
/// vanishes and the client only sees `ECONNRESET`). 8 MiB matches the test-side `RUST_MIN_STACK` so that adding an
/// optional payload field does not require retuning this.
const WORKER_STACK_BYTES: usize = 8 * 1024 * 1024;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(WORKER_STACK_BYTES)
        .build()?
        .block_on(serve())
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("OSPF_TOKEN").map_err(|_| "OSPF_TOKEN is required (at least 32 characters)")?;
    let address: SocketAddr = std::env::var("OSPF_BIND").unwrap_or_else(|_| "127.0.0.1:8787".into()).parse()?;
    if !address.ip().is_loopback() { return Err("only loopback binding is permitted; use a secured reverse proxy".into()); }
    let origins = std::env::var("OSPF_ORIGINS").unwrap_or_default().split(',')
        .map(str::trim).filter(|s| !s.is_empty()).map(str::parse::<HeaderValue>).collect::<Result<Vec<_>, _>>()?;
    let app = router(HostState { engine: NativeEngine::default(), token }, origins)?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("OSPF native host listening on {}", listener.local_addr()?);
    axum::serve(listener, app).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; }).await?;
    Ok(())
}
