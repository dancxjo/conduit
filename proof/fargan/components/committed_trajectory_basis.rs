//! Root-SDK component: module `owner` must be the actual committed-word owner.
//! Transfer to a different SDK only as canonical bytes plus fresh Source admission.
use super::owner::PreparedCommittedWordPitch;
use conduit_plot::rust_binding::NativeRustBinding;

#[derive(Debug)]
pub enum Refusal {
    ForeignOwner,
    Segment,
    Native,
}
pub struct CommittedTrajectoryBasis<'o, 'a, 'coverage, 'word, 'basis, 'commit> {
    owner: &'o PreparedCommittedWordPitch<'a, 'coverage, 'word, 'basis, 'commit>,
    ordinal: usize,
    pitch_admission: Vec<u8>,
    trajectory: Vec<u8>,
    rich_admission: Vec<u8>,
}
impl<'o, 'a, 'coverage, 'word, 'basis, 'commit>
    CommittedTrajectoryBasis<'o, 'a, 'coverage, 'word, 'basis, 'commit>
{
    pub fn prepare(
        owner: &'o PreparedCommittedWordPitch<'a, 'coverage, 'word, 'basis, 'commit>,
        ordinal: usize,
    ) -> Result<Self, Refusal> {
        let pitch = owner.segment_pitch().get(ordinal).ok_or(Refusal::Segment)?;
        let rich = owner.admissions().get(ordinal).ok_or(Refusal::Segment)?;
        Ok(Self {
            owner,
            ordinal,
            pitch_admission: pitch.clone().encode().map_err(|_| Refusal::Native)?,
            trajectory: pitch
                .trajectory()
                .clone()
                .encode()
                .map_err(|_| Refusal::Native)?,
            rich_admission: rich.clone().encode().map_err(|_| Refusal::Native)?,
        })
    }
    pub fn require_owner(
        &self,
        offered: &PreparedCommittedWordPitch<'a, 'coverage, 'word, 'basis, 'commit>,
    ) -> Result<(), Refusal> {
        if core::ptr::eq(self.owner, offered) {
            Ok(())
        } else {
            Err(Refusal::ForeignOwner)
        }
    }
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    /// Complete Source pitch admission includes original occurrence, revision,
    /// segment/prosody, declared linguistic effect and original full trajectory.
    pub fn pitch_admission(&self) -> &[u8] {
        &self.pitch_admission
    }
    pub fn trajectory(&self) -> &[u8] {
        &self.trajectory
    }
    pub fn rich_admission(&self) -> &[u8] {
        &self.rich_admission
    }
    pub fn owner(&self) -> &'o PreparedCommittedWordPitch<'a, 'coverage, 'word, 'basis, 'commit> {
        self.owner
    }
}
