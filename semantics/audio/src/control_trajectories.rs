//! Finite independent control scopes at one exact abstract query time.
use crate::source_execution::Program;
use crate::source_programs::{CONTROL_PRIORITY, TRAJECTORY_RATIO};
use crate::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum AudioControlRefusal {
    Admission(NativeBindingRefusal),
    Source(AudioSourceExecutionRefusal),
    Trajectory {
        index: usize,
        refusal: AudioTrajectoryRefusal,
    },
    DuplicateScope,
    MixedControlDomain,
    Domain,
    Conflict {
        first: usize,
        second: usize,
    },
}
impl From<NativeBindingRefusal> for AudioControlRefusal {
    fn from(e: NativeBindingRefusal) -> Self {
        Self::Admission(e)
    }
}
impl From<AudioSourceExecutionRefusal> for AudioControlRefusal {
    fn from(e: AudioSourceExecutionRefusal) -> Self {
        Self::Source(e)
    }
}
pub struct PreparedAudioControlTrajectories {
    original: AudioControlTrajectorySet,
    canonical: Vec<u8>,
    trajectories: Vec<PreparedAudioQuantityTrajectory>,
    admission: Vec<AudioSourceExecution>,
    priority: Program,
}
/// All independent results survive resolution; selected indices name originals.
pub struct AudioControlEvaluation {
    original: AudioControlTrajectorySet,
    canonical: Vec<u8>,
    query: Vec<u8>,
    evaluations: Vec<AudioTrajectoryEvaluation>,
    selected: Vec<usize>,
    executions: Vec<AudioSourceExecution>,
}
impl AudioControlEvaluation {
    pub fn original(&self) -> &AudioControlTrajectorySet {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.canonical
    }
    pub fn query_canonical(&self) -> &[u8] {
        &self.query
    }
    pub fn evaluations(&self) -> &[AudioTrajectoryEvaluation] {
        &self.evaluations
    }
    pub fn selected_indices(&self) -> &[usize] {
        &self.selected
    }
    pub fn executions(&self) -> &[AudioSourceExecution] {
        &self.executions
    }
}
impl PreparedAudioControlTrajectories {
    /// Exact checked Source executions retained during trajectory admission.
    pub fn admission_executions(&self) -> &[AudioSourceExecution] {
        &self.admission
    }
    pub fn original(&self) -> &AudioControlTrajectorySet {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.canonical
    }

    pub fn new(canonical: &[u8]) -> Result<Self, AudioControlRefusal> {
        let original = AudioControlTrajectorySet::decode(canonical)?;
        let controls = original.controls().as_slice();
        let ratio = Program::new(TRAJECTORY_RATIO)?;
        let mut trajectories = Vec::with_capacity(controls.len());
        let mut admission = Vec::new();
        for (index, control) in controls.iter().enumerate() {
            for prior in &controls[..index] {
                if prior.source_scope() == control.source_scope()
                    && prior.control_identity() == control.control_identity()
                {
                    return Err(AudioControlRefusal::DuplicateScope);
                }
                if prior.control_identity() == control.control_identity()
                    && prior.domain() != control.domain()
                {
                    return Err(AudioControlRefusal::MixedControlDomain);
                }
            }
            // The existing checked Source chooses each endpoint's quantity domain.
            for segment in control.trajectory().segments().as_slice() {
                for endpoint in [segment.left(), segment.right()] {
                    let projected: crate::generated::AudioTrajectoryRatio =
                        ratio.native(endpoint.clone(), &mut admission)?;
                    AudioControlDomainMatch::new(*control.domain(), *projected.domain())
                        .map_err(|_| AudioControlRefusal::Domain)?;
                }
            }
            trajectories.push(
                PreparedAudioQuantityTrajectory::new(&control.trajectory().clone().encode()?)
                    .map_err(|refusal| AudioControlRefusal::Trajectory { index, refusal })?,
            );
        }
        Ok(Self {
            original,
            canonical: canonical.into(),
            trajectories,
            admission,
            priority: Program::new(CONTROL_PRIORITY)?,
        })
    }
    pub fn evaluate(&self, query: &[u8]) -> Result<AudioControlEvaluation, AudioControlRefusal> {
        let mut evaluations = Vec::with_capacity(self.trajectories.len());
        // The sole admitted gap policy refuses any uncovered original control.
        for (index, trajectory) in self.trajectories.iter().enumerate() {
            evaluations.push(
                trajectory
                    .evaluate(query)
                    .map_err(|refusal| AudioControlRefusal::Trajectory { index, refusal })?,
            );
        }
        let controls = self.original.controls().as_slice();
        let mut selected: Vec<usize> = Vec::with_capacity(controls.len());
        let mut executions = self.admission.clone();
        for (index, control) in controls.iter().enumerate() {
            if controls[..index]
                .iter()
                .any(|c| c.control_identity() == control.control_identity())
            {
                continue;
            }
            let mut winner = index;
            let mut tied = None;
            for (candidate, c) in controls
                .iter()
                .enumerate()
                .skip(index + 1)
                .filter(|(_, c)| c.control_identity() == control.control_identity())
            {
                if matches!(
                    self.original.conflict(),
                    AudioControlConflictPolicy::RefuseOverlap
                ) {
                    return Err(AudioControlRefusal::Conflict {
                        first: index,
                        second: candidate,
                    });
                }
                let input =
                    AudioControlPriorityInput::new(*c.priority(), *controls[winner].priority())?;
                let decision: AudioControlPriorityDecision =
                    self.priority.native(input, &mut executions)?;
                if *decision.candidate_wins() {
                    winner = candidate;
                    tied = None;
                } else if *decision.equal_priority() {
                    tied = Some(candidate);
                }
            }
            if let Some(second) = tied {
                return Err(AudioControlRefusal::Conflict {
                    first: winner,
                    second,
                });
            }
            selected.push(winner);
        }
        Ok(AudioControlEvaluation {
            original: self.original.clone(),
            canonical: self.canonical.clone(),
            query: query.into(),
            evaluations,
            selected,
            executions,
        })
    }
}
