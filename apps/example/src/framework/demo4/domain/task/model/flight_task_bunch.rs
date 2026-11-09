//! 无损飞行任务束 / Lossless flight task bunch.

use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, OnceLock};
use time::{Duration, OffsetDateTime};
use super::aircraft::Aircraft;
use super::airport::Airport;
use super::flight_cycle::{FlightCycle, FlightHour};
use super::recovery::{FlightTaskRecovery, RecoveryError};

struct RecoverySequence {
    prefix: Option<Arc<RecoverySequence>>,
    entries: Vec<FlightTaskRecovery>,
    length: usize,
}

// 连续切片缓存属于视图，而不是父链；保留后代不能延长祖先物化缓存的寿命。 / Slice caches belong to views, not the spine; descendants must not retain ancestors' materialized caches.
struct RecoveryActivities {
    sequence: Arc<RecoverySequence>,
    flattened: OnceLock<Vec<FlightTaskRecovery>>,
}

impl RecoveryActivities {
    fn as_slice(&self) -> &[FlightTaskRecovery] {
        if self.sequence.prefix.is_none() { return &self.sequence.entries; }
        self.flattened.get_or_init(|| {
            let mut segments = Vec::new();
            let mut current = Some(self.sequence.as_ref());
            while let Some(sequence) = current {
                segments.push(sequence.entries.as_slice());
                current = sequence.prefix.as_deref();
            }
            let mut activities = Vec::with_capacity(self.sequence.length);
            for segment in segments.into_iter().rev() { activities.extend_from_slice(segment); }
            activities
        })
    }
}

impl RecoverySequence {
    fn contains(&self, task_id: &str) -> bool {
        let mut current = Some(self);
        while let Some(sequence) = current {
            if sequence.entries.iter().any(|activity| activity.original().plan.id == task_id) { return true; }
            current = sequence.prefix.as_deref();
        }
        false
    }

    fn first(&self) -> Option<&FlightTaskRecovery> {
        let mut current = self;
        while let Some(prefix) = current.prefix.as_deref() {
            if prefix.length == 0 { break; }
            current = prefix;
        }
        current.entries.first()
    }

    fn last(&self) -> Option<&FlightTaskRecovery> { self.entries.last() }
}

impl Debug for RecoveryActivities {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(formatter)
    }
}

impl Drop for RecoverySequence {
    fn drop(&mut self) {
        // 深前缀链迭代释放，避免最后一个所有者触发递归析构。 / Release deep prefix chains iteratively instead of recursively dropping the last owner.
        while let Some(prefix) = self.prefix.take() {
            let Some(mut prefix) = Arc::into_inner(prefix) else { break; };
            self.prefix = prefix.prefix.take();
        }
    }
}

/// 单架飞机的不可变恢复序列，端点及任务 ID 均从活动派生。 / Immutable recovery sequence with derived endpoints and task IDs.
#[derive(Debug, Clone)]
pub struct FlightTaskBunch {
    id: String,
    aircraft: Aircraft,
    activities: Arc<RecoveryActivities>,
}

impl FlightTaskBunch {
    /// 校验唯一任务、执行飞机和连续时空记录；空束无虚构时间或机场。 / Validate unique tasks, aircraft and continuity; empty bunches have no invented endpoints.
    pub fn new(
        id: String,
        aircraft: Aircraft,
        activities: Vec<FlightTaskRecovery>,
    ) -> Result<Self, RecoveryError> {
        let mut ids = HashSet::new();
        for activity in &activities {
            if !ids.insert(activity.original().plan.id.as_str()) {
                return Err(RecoveryError::DuplicateTask(activity.original().plan.id.clone()));
            }
            if !activity.aircraft().same_configuration(&aircraft) {
                return Err(RecoveryError::AircraftMismatch(activity.original().plan.id.clone()));
            }
        }
        for pair in activities.windows(2) {
            if pair[0].route().arr != pair[1].route().dep
                || pair[0].time().1 > pair[1].time().0
            {
                return Err(RecoveryError::Disconnected(pair[1].original().plan.id.clone()));
            }
        }
        let length = activities.len();
        Ok(Self { id, aircraft, activities: Arc::new(RecoveryActivities {
            sequence: Arc::new(RecoverySequence { prefix: None, entries: activities, length }),
            flattened: OnceLock::new(),
        }) })
    }

    /// 从已校验不可变前缀追加活动，只检查新增结构不变量。 / Append to a validated immutable prefix, checking only new structural invariants.
    pub(crate) fn append(
        &self,
        id: String,
        activity: FlightTaskRecovery,
    ) -> Result<Self, RecoveryError> {
        if self.contains(&activity.original().plan.id) {
            return Err(RecoveryError::DuplicateTask(activity.original().plan.id.clone()));
        }
        if !activity.aircraft().same_configuration(&self.aircraft) {
            return Err(RecoveryError::AircraftMismatch(activity.original().plan.id.clone()));
        }
        if self.activities.sequence.last().is_some_and(|previous| previous.route().arr != activity.route().dep
            || previous.time().1 > activity.time().0)
        {
            return Err(RecoveryError::Disconnected(activity.original().plan.id.clone()));
        }
        Ok(Self { id, aircraft: self.aircraft.clone(), activities: Arc::new(RecoveryActivities {
            sequence: Arc::new(RecoverySequence {
                prefix: Some(self.activities.sequence.clone()), entries: vec![activity], length: self.activities.sequence.length + 1,
            }),
            flattened: OnceLock::new(),
        }) })
    }

    /// 束标识 / Bunch identifier.
    pub fn id(&self) -> &str { &self.id }

    /// 实际执行飞机 / Actual executor aircraft.
    pub fn aircraft(&self) -> &Aircraft { &self.aircraft }

    /// 无损恢复活动 / Lossless recovery activities.
    pub fn activities(&self) -> &[FlightTaskRecovery] { self.activities.as_slice() }

    /// 派生的任务 ID 视图 / Derived task ID view.
    pub fn task_ids(&self) -> impl Iterator<Item = &str> {
        self.activities().iter().map(|activity| activity.original().plan.id.as_str())
    }

    /// 按原始任务 ID 查询恢复记录 / Find the recovery record by original task ID.
    pub fn recovery(&self, task_id: &str) -> Option<&FlightTaskRecovery> {
        self.activities().iter().find(|activity| activity.original().plan.id == task_id)
    }

    /// 是否覆盖原任务 / Whether the original task is covered.
    pub fn contains(&self, task_id: &str) -> bool { self.activities.sequence.contains(task_id) }

    /// 两任务是否连续 / Whether two tasks are consecutive.
    pub fn consecutive(&self, from: &str, to: &str) -> bool {
        self.activities().windows(2).any(|pair| {
            pair[0].original().plan.id == from && pair[1].original().plan.id == to
        })
    }

    /// 恢复后的出发机场 / Recovered departure airport.
    pub fn dep(&self) -> Option<&Airport> {
        self.activities.sequence.first().map(|activity| &activity.route().dep)
    }

    /// 恢复后的到达机场 / Recovered arrival airport.
    pub fn arr(&self) -> Option<&Airport> {
        self.activities.sequence.last().map(|activity| &activity.route().arr)
    }

    /// 恢复后的完整时间范围 / Complete recovered time range.
    pub fn time(&self) -> Option<(OffsetDateTime, OffsetDateTime)> {
        self.activities.sequence.first().zip(self.activities.sequence.last())
            .map(|(first, last)| (first.time().0, last.time().1))
    }

    /// 查询全部活动的到达事件，包含内部中转机场；窗口为闭区间。 / Query all arrival events including internal airports; bounds are inclusive.
    pub fn arrived_when(&self, airport: &Airport, time: &(OffsetDateTime, OffsetDateTime)) -> bool {
        self.activities().iter().any(|activity| {
            &activity.route().arr == airport && time.0 <= activity.time().1 && activity.time().1 <= time.1
        })
    }

    /// 查询全部活动的出发事件，包含内部中转机场；窗口为闭区间。 / Query all departure events including internal airports; bounds are inclusive.
    pub fn departed_when(&self, airport: &Airport, time: &(OffsetDateTime, OffsetDateTime)) -> bool {
        self.activities().iter().any(|activity| {
            &activity.route().dep == airport && time.0 <= activity.time().0 && activity.time().0 <= time.1
        })
    }

    /// 飞行活动的实际飞行时长，不含地面等待。 / Actual duration of flight activities, excluding ground waiting.
    pub fn flight_hour(&self) -> FlightHour {
        FlightHour::new(self.activities().iter().filter(|activity| activity.is_flight())
            .map(|activity| activity.time().1 - activity.time().0)
            .fold(Duration::ZERO, |sum, duration| sum + duration))
    }

    /// 实际飞行活动的循环数 / Number of actual flight activity cycles.
    pub fn flight_cycle(&self) -> FlightCycle {
        FlightCycle::new(self.activities().iter().filter(|activity| activity.is_flight()).count() as u64)
    }
}

#[cfg(test)]
mod tests;
