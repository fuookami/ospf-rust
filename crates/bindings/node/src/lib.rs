//! Node 原生入口，和 HTTP 复用同一执行器 / Node native entry point sharing the HTTP executor.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ospf_rust_bridge::BridgeError;
use ospf_rust_native::NativeEngine;

fn boundary(error: impl std::fmt::Display) -> Error { Error::from_reason(error.to_string()) }

fn protocol_error(error: BridgeError) -> Error {
    boundary(serde_json::to_string(&error).unwrap_or_else(|_| "bridge error serialization failed".into()))
}

/// Node 原生客户端 / Node native client.
#[napi]
pub struct NativeClient { engine: NativeEngine }

#[napi]
impl NativeClient {
    /// 创建有界单任务客户端 / Create a bounded single-job client.
    #[napi(constructor)]
    pub fn new() -> Self { Self { engine: NativeEngine::default() } }

    /// 编译能力 JSON / Compiled capabilities JSON.
    #[napi]
    pub fn capabilities(&self) -> Result<String> { serde_json::to_string(&self.engine.capabilities()).map_err(boundary) }

    /// 在线程池校验并提交模型 / Validate and submit a model on the thread pool.
    #[napi(ts_return_type = "Promise<string>")]
    pub fn submit(&self, request_json: String) -> AsyncTask<SubmitTask> {
        AsyncTask::new(SubmitTask { engine: self.engine.clone(), request_json })
    }

    /// 当前任务视图 / Current job view.
    #[napi]
    pub fn get(&self, id: String) -> Result<String> {
        let view = self.engine.get(&id).map_err(protocol_error)?;
        serde_json::to_string(&view).map_err(boundary)
    }

    /// 请求取消 / Request cancellation.
    #[napi]
    pub fn cancel(&self, id: String) -> Result<String> {
        let view = self.engine.cancel(&id).map_err(protocol_error)?;
        serde_json::to_string(&view).map_err(boundary)
    }

    /// 释放终态任务 / Release a terminal job.
    #[napi]
    pub fn release(&self, id: String) -> Result<()> {
        self.engine.release(&id).map_err(protocol_error)
    }
}

impl Default for NativeClient {
    fn default() -> Self { Self::new() }
}

/// 模型提交线程池任务 / Model submission thread-pool task.
pub struct SubmitTask { engine: NativeEngine, request_json: String }

impl Task for SubmitTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<String> {
        if self.request_json.len() > 9 * 1024 * 1024 {
            return Err(protocol_error(BridgeError::new("PayloadTooLarge", "request exceeds 9 MiB")));
        }
        let request = serde_json::from_str(&self.request_json)
            .map_err(|error| protocol_error(BridgeError::new("InvalidInput", error)))?;
        let view = self.engine.submit(request).map_err(protocol_error)?;
        serde_json::to_string(&view).map_err(boundary)
    }

    fn resolve(&mut self, _: Env, output: String) -> Result<String> { Ok(output) }
}
