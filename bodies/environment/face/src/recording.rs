//! Bounded recovery evidence for the ordinary Face. This never acknowledges a
//! durable write or infers producer authenticity from a decoded chunk.
use alloc::{string::String, vec};
use conduit_face::{Face, FaceBasis, FaceProperty, FacePropertyValue as V, FaceRole, FaceSubject};
const SUBJECT: &str = "environment-recording";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveredRecordingRange {
    pub chunks: u16,
    pub samples: u32,
    pub first_sequence: u64,
    pub last_sequence: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordingRecoveryFailure {
    InvalidContent,
    Capacity,
    Cancelled,
}
impl RecordingRecoveryFailure {
    fn label(self) -> &'static str {
        match self {
            Self::InvalidContent => "invalid-content",
            Self::Capacity => "capacity",
            Self::Cancelled => "cancelled",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordingFaceRefusal {
    Configuration,
    Range,
    Reversed,
    Exhausted,
    InvalidFace,
}

pub struct PreparedRecordingFace {
    face: Face,
    range: Option<RecoveredRecordingRange>,
    maximum_chunks: u16,
    maximum_samples: u32,
    maximum_revisions: u64,
}
impl PreparedRecordingFace {
    /// Root supplies the real contribution basis and complete finite update
    /// budget. No recovered history, storage possession or authority is created.
    pub fn prepare(
        basis: FaceBasis,
        maximum_chunks: u16,
        maximum_samples: u32,
        maximum_revisions: u64,
    ) -> Result<Self, RecordingFaceRefusal> {
        if maximum_chunks == 0
            || maximum_samples < u32::from(maximum_chunks)
            || maximum_revisions == 0
        {
            return Err(RecordingFaceRefusal::Configuration);
        }
        let text = |value: &str| {
            let mut text = String::with_capacity(24);
            text.push_str(value);
            V::Text(text)
        };
        let property = |name: &str, value| FaceProperty {
            subject: SUBJECT.into(),
            name: name.into(),
            value,
        };
        let face = Face::new(
            0,
            basis,
            vec![FaceSubject {
                identity: SUBJECT.into(),
                role: FaceRole::Info,
                name: "Recording recovery".into(),
            }],
            vec![],
            vec![
                property("recovery-status", text("empty")),
                property("recovery-failure", text("none")),
                property("has-range", V::Flag(false)),
                property("chunks", V::Count(0)),
                property("samples", V::Count(0)),
                property("first-sequence", V::Count(0)),
                property("last-sequence", V::Count(0)),
                property("missing-sequences", V::Count(0)),
            ],
            vec![],
        )
        .map_err(|_| RecordingFaceRefusal::InvalidFace)?;
        Ok(Self {
            face,
            range: None,
            maximum_chunks,
            maximum_samples,
            maximum_revisions,
        })
    }
    pub fn current(&self) -> &Face {
        &self.face
    }
    /// Project only the Root's committed recovery state. Successful parsing or
    /// publication requests alone must not call this. This describes recovery,
    /// never a new durable acknowledgement or a current sensor reading.
    pub fn publish_recovered(
        &mut self,
        range: RecoveredRecordingRange,
    ) -> Result<(), RecordingFaceRefusal> {
        use RecordingFaceRefusal as E;
        if range.chunks == 0
            || range.chunks > self.maximum_chunks
            || range.samples < u32::from(range.chunks)
            || range.samples > self.maximum_samples
            || range.first_sequence == 0
            || range.last_sequence < range.first_sequence
            || range.last_sequence - range.first_sequence < u64::from(range.samples - 1)
        {
            return Err(E::Range);
        }
        if let Some(previous) = self.range {
            if range.first_sequence != previous.first_sequence
                || range.chunks < previous.chunks
                || range.samples < previous.samples
                || range.last_sequence < previous.last_sequence
                || (range.chunks == previous.chunks && range != previous)
                || u64::from(range.samples - previous.samples)
                    > range.last_sequence - previous.last_sequence
                || range.samples - previous.samples < u32::from(range.chunks - previous.chunks)
            {
                return Err(E::Reversed);
            }
        }
        if self.range == Some(range) && self.status() == "recovered" {
            return Ok(());
        }
        self.check_revision()?;
        self.range = Some(range);
        self.set_text(0, "recovered");
        self.set_text(1, "none");
        self.face.properties[2].value = V::Flag(true);
        for (index, value) in [
            u64::from(range.chunks),
            u64::from(range.samples),
            range.first_sequence,
            range.last_sequence,
        ]
        .into_iter()
        .enumerate()
        {
            self.face.properties[index + 3].value = V::Count(value);
        }
        // Only gaps inside the recovered range are known; nothing before its
        // first sample is inferred.
        self.face.properties[7].value =
            V::Count(range.last_sequence - range.first_sequence + 1 - u64::from(range.samples));
        self.reseal()
    }
    /// Keep the last committed recovered range visible through a later refusal.
    pub fn publish_refusal(
        &mut self,
        failure: RecordingRecoveryFailure,
    ) -> Result<(), RecordingFaceRefusal> {
        let V::Text(previous) = &self.face.properties[1].value else {
            unreachable!()
        };
        if self.status() == "refused" && previous == failure.label() {
            return Ok(());
        }
        self.check_revision()?;
        self.set_text(0, "refused");
        self.set_text(1, failure.label());
        self.reseal()
    }
    fn status(&self) -> &str {
        let V::Text(status) = &self.face.properties[0].value else {
            unreachable!()
        };
        status
    }
    fn set_text(&mut self, index: usize, value: &str) {
        let V::Text(text) = &mut self.face.properties[index].value else {
            unreachable!()
        };
        text.clear();
        text.push_str(value);
    }
    fn check_revision(&self) -> Result<(), RecordingFaceRefusal> {
        if self.face.revision >= self.maximum_revisions {
            Err(RecordingFaceRefusal::Exhausted)
        } else {
            Ok(())
        }
    }
    fn reseal(&mut self) -> Result<(), RecordingFaceRefusal> {
        self.face.revision += 1;
        self.face
            .reseal_prepared()
            .map_err(|_| RecordingFaceRefusal::InvalidFace)
    }
}
