//! Read-only projection of the actual file writer's ordinary commit observer.
//! A pending fact is not inferred work status, authority or durable recovery.
use conduit_core as c;
use conduit_environment_runtime::recording::RecordingCommitObserver;
use conduit_face::{Face, FaceBasis, FaceProperty, FacePropertyValue as V, FaceRole, FaceSubject};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilePublicationFaceRefusal {
    Scope,
    Face,
    Range,
}
pub struct PreparedFilePublicationFace {
    active: c::ActivePlayIdentity,
    observer: RecordingCommitObserver,
    face: Face,
}
impl PreparedFilePublicationFace {
    pub(crate) fn prepare(
        active: &c::ActivePlayIdentity,
        observer: RecordingCommitObserver,
        basis: FaceBasis,
    ) -> Result<Self, FilePublicationFaceRefusal> {
        let property = |name: &str, value| FaceProperty {
            subject: "environment-recording-publication".into(),
            name: name.into(),
            value,
        };
        let mut status = String::with_capacity(16);
        status.push_str("not-committed");
        let face = Face::new(
            0,
            basis,
            vec![FaceSubject {
                identity: "environment-recording-publication".into(),
                role: FaceRole::Info,
                name: "Recording publication".into(),
            }],
            vec![],
            vec![
                property("publication-status", V::Text(status)),
                property("has-committed-range", V::Flag(false)),
                property("samples", V::Count(0)),
                property("first-sequence", V::Count(0)),
                property("last-sequence", V::Count(0)),
                property("missing-sequences", V::Count(0)),
            ],
            vec![],
        )
        .map_err(|_| FilePublicationFaceRefusal::Face)?;
        Ok(Self {
            active: active.clone(),
            observer,
            face,
        })
    }
    /// Historical publication facts, never a current sensor observation. The
    /// Body owner still decides how this contribution participates in its Face.
    pub fn current(&self) -> &Face {
        &self.face
    }
    /// Root obtains this exact scope from its retained started Body before each
    /// refresh. Equal public identity data does not itself confer lifecycle rights.
    pub fn refresh(
        &mut self,
        active: &c::ActivePlayIdentity,
    ) -> Result<(), FilePublicationFaceRefusal> {
        use FilePublicationFaceRefusal as E;
        if active != &self.active {
            return Err(E::Scope);
        }
        let Some(range) = self.observer.committed() else {
            return Ok(());
        };
        if self.face.revision != 0 {
            return Ok(());
        }
        let samples = u64::try_from(range.samples).map_err(|_| E::Range)?;
        if samples == 0
            || range.first_sequence == 0
            || range.last_sequence < range.first_sequence
            || range.last_sequence - range.first_sequence < samples - 1
        {
            return Err(E::Range);
        }
        let V::Text(status) = &mut self.face.properties[0].value else {
            unreachable!()
        };
        status.clear();
        status.push_str("committed");
        self.face.properties[1].value = V::Flag(true);
        for (index, value) in [
            samples,
            range.first_sequence,
            range.last_sequence,
            range.last_sequence - range.first_sequence - (samples - 1),
        ]
        .into_iter()
        .enumerate()
        {
            self.face.properties[index + 2].value = V::Count(value);
        }
        self.face.revision = 1;
        self.face.reseal_prepared().map_err(|_| E::Face)
    }
}
