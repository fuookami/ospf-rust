//! 运行心跳与通用响应信封的绑定。
//! Binding for the running heartbeat and the generic response envelope.
//!
//! 这两个模块在框架层没有内部消费方，但它们是对外能力：心跳提供"运行时长 + 心跳计数"的可观测状态，
//! `Response<T>` 提供统一的成功/失败信封。把它们接到 SDK，调用方才能在自己的编排里使用同一套语义，
//! 而不是各自再造一个计时器和信封。
//! Neither module has an internal consumer in the framework, but both are outward capabilities: the heartbeat
//! provides observable uptime and tick counts, and `Response<T>` provides one success/failure envelope. Exposing them
//! lets callers reuse the same semantics in their own orchestration instead of reinventing a timer and an envelope.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 运行心跳的可观测快照。 / An observable snapshot of the running heartbeat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeartBeatSnapshot {
    /// 自启动以来的运行时长（毫秒）。 / Uptime since the heartbeat started, in milliseconds.
    pub uptime_ms: u64,
    /// 已记录的心跳次数。 / The number of recorded ticks.
    pub tick_count: u64,
}

/// 通用响应信封。 / The generic response envelope.
///
/// `success` 与 `message`/`data` 的组合由 Rust 侧校验，绝不允许"成功但带消息"或"失败但带数据"这类自相矛盾的形态。
/// Rust validates how `success` combines with `message`/`data`; self-contradictory shapes such as "success with a
/// message" or "failure with data" are never accepted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResponseEnvelope {
    /// 是否成功。 / Whether the operation succeeded.
    pub success: bool,
    /// 可选消息；失败时必填。 / Optional message; required on failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub message: Option<String>,
    /// 可选载荷；成功时必填。 / Optional payload; required on success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub data: Option<String>,
}

impl ResponseEnvelope {
    /// 构造成功信封；`data` 不得为空。 / Build a success envelope; `data` must not be blank.
    pub fn ok(data: impl Into<String>) -> Result<Self> {
        let data = data.into();
        if data.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "a successful response requires non-blank data"));
        }
        Ok(Self { success: true, message: None, data: Some(data) })
    }

    /// 构造失败信封；`message` 不得为空。 / Build a failure envelope; `message` must not be blank.
    pub fn fail(message: impl Into<String>) -> Result<Self> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput", "a failed response requires a non-blank message"));
        }
        Ok(Self { success: false, message: Some(message), data: None })
    }

    /// 复核信封自洽性；外部报告同样走这条路径。 / Recheck envelope self-consistency; external reports use this path too.
    pub fn validate(&self) -> Result<()> {
        match (self.success, self.message.as_deref(), self.data.as_deref()) {
            (true, None, Some(data)) if !data.trim().is_empty() => Ok(()),
            (false, Some(message), None) if !message.trim().is_empty() => Ok(()),
            (true, Some(_), _) => Err(BridgeError::new("InvalidReport",
                "a successful response must not carry a message")),
            (false, _, Some(_)) => Err(BridgeError::new("InvalidReport",
                "a failed response must not carry data")),
            (true, None, _) => Err(BridgeError::new("InvalidReport",
                "a successful response requires non-blank data")),
            (false, _, None) => Err(BridgeError::new("InvalidReport",
                "a failed response requires a non-blank message")),
        }
    }
}

/// 心跳观测请求：`ticks` 次心跳后返回快照。 / A heartbeat observation request: take `ticks` ticks then return the snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeartBeatRequest {
    /// 要记录的心跳次数。 / How many ticks to record.
    pub ticks: u32,
}

impl HeartBeatRequest {
    /// 心跳次数上限：防止一次请求把宿主拖成忙等。
    /// Tick bound so one request cannot turn the host into a busy wait.
    pub const MAX_TICKS: u32 = 1_000_000;

    /// 校验心跳次数。 / Validate the tick count.
    pub fn validate(&self) -> Result<()> {
        if self.ticks > Self::MAX_TICKS {
            return Err(BridgeError::new("InvalidInput", "heartbeat tick count exceeds the supported bound"));
        }
        Ok(())
    }
}

/// 记录 `ticks` 次心跳并返回真实快照。 / Record `ticks` ticks and return the real snapshot.
pub fn observe_heartbeat(request: &HeartBeatRequest) -> Result<HeartBeatSnapshot> {
    request.validate()?;
    let mut heartbeat = ospf_rust_framework::running_heart_beat::RunningHeartBeat::new();
    for _ in 0..request.ticks {
        heartbeat.tick();
    }
    Ok(HeartBeatSnapshot {
        uptime_ms: u64::try_from(heartbeat.uptime().as_millis()).unwrap_or(u64::MAX),
        tick_count: u64::try_from(heartbeat.tick_count()).map_err(|_| BridgeError::new("InvalidInput", "tick count overflow"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_reports_the_ticks_it_actually_recorded() {
        let snapshot = observe_heartbeat(&HeartBeatRequest { ticks: 3 }).unwrap();
        assert_eq!(snapshot.tick_count, 3);
        // 时长是真实经过的时间，不做任何取整承诺。 / Uptime is real elapsed time, with no rounding promise.
        assert!(snapshot.uptime_ms < 60_000);
        let zero = observe_heartbeat(&HeartBeatRequest { ticks: 0 }).unwrap();
        assert_eq!(zero.tick_count, 0);
        assert!(observe_heartbeat(&HeartBeatRequest { ticks: HeartBeatRequest::MAX_TICKS + 1 }).is_err());
    }

    #[test]
    fn envelopes_refuse_self_contradictory_shapes() {
        let ok = ResponseEnvelope::ok("payload").unwrap();
        assert!(ok.validate().is_ok());
        assert_eq!(ok.message, None);
        let failed = ResponseEnvelope::fail("boom").unwrap();
        assert!(failed.validate().is_ok());
        assert_eq!(failed.data, None);
        // 空载荷与空消息都不能构造。 / Blank payloads and blank messages cannot be constructed.
        assert!(ResponseEnvelope::ok("  ").is_err());
        assert!(ResponseEnvelope::fail("  ").is_err());
        // 自相矛盾的形态在复核时被拒。 / Self-contradictory shapes are refused on recheck.
        let broken = vec![
            ResponseEnvelope { success: true, message: Some("why".into()), data: Some("payload".into()) },
            ResponseEnvelope { success: true, message: None, data: None },
            ResponseEnvelope { success: true, message: None, data: Some("  ".into()) },
            ResponseEnvelope { success: false, message: None, data: Some("payload".into()) },
            ResponseEnvelope { success: false, message: Some("  ".into()), data: None },
        ];
        for case in broken {
            assert_eq!(case.validate().unwrap_err().code, "InvalidReport", "{case:?}");
        }
    }
}
