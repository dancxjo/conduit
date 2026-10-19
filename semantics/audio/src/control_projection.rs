//! Finite exact control point sampling, independent of the PCM frame grid.
//! The original trajectories, schedule, declared clock relation and every checked
//! execution survive. No sample-and-hold or continuous reconstruction is claimed.
use crate::source_execution::Program;
use crate::source_programs::{CONTROL_FINAL_TIME, CONTROL_QUERY_TIME};
use crate::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum AudioControlProjectionRefusal {
    Admission(NativeBindingRefusal),
    Source(AudioSourceExecutionRefusal),
    Control(AudioControlRefusal),
    Clock(AudioClockProjectionRefusal),
}
impl From<NativeBindingRefusal> for AudioControlProjectionRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::Admission(value)
    }
}
impl From<AudioSourceExecutionRefusal> for AudioControlProjectionRefusal {
    fn from(value: AudioSourceExecutionRefusal) -> Self {
        Self::Source(value)
    }
}
pub struct AudioControlProjectionPointReceipt {
    query: AudioTrajectoryQuery,
    query_canonical: Vec<u8>,
    control: AudioControlEvaluation,
    clock: AudioClockProjectionReceipt,
}
impl AudioControlProjectionPointReceipt {
    pub fn query(&self) -> &AudioTrajectoryQuery {
        &self.query
    }
    pub fn query_canonical(&self) -> &[u8] {
        &self.query_canonical
    }
    pub fn control(&self) -> &AudioControlEvaluation {
        &self.control
    }
    pub fn clock(&self) -> &AudioClockProjectionReceipt {
        &self.clock
    }
}
pub struct AudioControlProjectionReceipt {
    schedule: AudioControlProjectionSchedule,
    schedule_canonical: Vec<u8>,
    original: AudioControlTrajectorySet,
    original_canonical: Vec<u8>,
    executions: Vec<AudioSourceExecution>,
    points: Vec<AudioControlProjectionPointReceipt>,
    final_query: AudioTrajectoryQuery,
}
impl AudioControlProjectionReceipt {
    pub fn schedule(&self) -> &AudioControlProjectionSchedule {
        &self.schedule
    }
    pub fn schedule_canonical(&self) -> &[u8] {
        &self.schedule_canonical
    }
    pub fn original(&self) -> &AudioControlTrajectorySet {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn executions(&self) -> &[AudioSourceExecution] {
        &self.executions
    }
    pub fn points(&self) -> &[AudioControlProjectionPointReceipt] {
        &self.points
    }
    pub fn final_query(&self) -> &AudioTrajectoryQuery {
        &self.final_query
    }
}
pub struct PreparedAudioControlProjection {
    trajectories: PreparedAudioControlTrajectories,
    query_time: Program,
    final_time: Program,
}
impl PreparedAudioControlProjection {
    pub fn new(original: &[u8]) -> Result<Self, AudioControlProjectionRefusal> {
        Ok(Self {
            trajectories: PreparedAudioControlTrajectories::new(original)
                .map_err(AudioControlProjectionRefusal::Control)?,
            query_time: Program::new(CONTROL_QUERY_TIME)?,
            final_time: Program::new(CONTROL_FINAL_TIME)?,
        })
    }
    pub fn project(
        &self,
        canonical: &[u8],
        clock: &PreparedAudioClockBinding,
    ) -> Result<AudioControlProjectionReceipt, AudioControlProjectionRefusal> {
        let schedule = AudioControlProjectionSchedule::decode(canonical)?;
        let mut executions = Vec::new();
        let final_query: AudioTrajectoryQuery =
            self.final_time.native(schedule.clone(), &mut executions)?;
        let mut points = Vec::with_capacity(16);
        // Only finite collection traversal is native. Source owns time arithmetic.
        for index in 0..*schedule.update_count() {
            let input = AudioControlProjectionPoint::new(index, schedule.clone())?;
            let query: AudioTrajectoryQuery = self.query_time.native(input, &mut executions)?;
            let query_canonical = query.clone().encode()?;
            let clock = clock
                .project(&query_canonical)
                .map_err(AudioControlProjectionRefusal::Clock)?;
            let control = self
                .trajectories
                .evaluate(&query_canonical)
                .map_err(AudioControlProjectionRefusal::Control)?;
            points.push(AudioControlProjectionPointReceipt {
                query,
                query_canonical,
                control,
                clock,
            });
        }
        Ok(AudioControlProjectionReceipt {
            schedule,
            schedule_canonical: canonical.into(),
            original: self.trajectories.original().clone(),
            original_canonical: self.trajectories.original_canonical().into(),
            executions,
            points,
            final_query,
        })
    }
}
