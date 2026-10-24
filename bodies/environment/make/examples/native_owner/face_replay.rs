//! Replay verified raw publications independently of the device Face summary.
//! The caller must authenticate publication bytes and clocks before entry.
use conduit_environment_runtime::{
    publication::{PublicationChannel, PublicationSnapshot},
    retained_publication_face::RetainedPublicationFace,
};
use conduit_face::{Face, FaceBasis, FacePropertyValue};
use sha2::{Digest, Sha256};
use std::error::Error;

pub struct Committed<'a> {
    pub channel: u8,
    pub sequence: u64,
    pub canonical: &'a [u8],
    pub committed_ms: u64,
    pub observed_ms: u64,
}
#[derive(serde::Deserialize)]
struct Report {
    source_sequence: u64,
    revision: u64,
    content_digest: [u8; 32],
    publisher_retained: bool,
    fresh: bool,
    available: bool,
    refusal: Option<String>,
}
pub struct Replay {
    face: RetainedPublicationFace,
    last: Option<(u64, u8, u64, [u8; 32])>,
}
impl Replay {
    pub fn prepare(basis: FaceBasis, count: u32, age_ms: u64) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            face: RetainedPublicationFace::prepare(basis, count, age_ms)
                .map_err(|e| format!("native Face replay preparation: {e:?}"))?,
            last: None,
        })
    }
    pub fn verify(
        &mut self,
        committed: Committed<'_>,
        report: &serde_json::Value,
    ) -> Result<&Face, Box<dyn Error>> {
        let report: Report = serde_json::from_value(report.clone())?;
        if committed.observed_ms < committed.committed_ms {
            return Err("native Face observation precedes commit".into());
        }
        let identity = (
            committed.sequence,
            committed.channel,
            committed.committed_ms,
            Sha256::digest(committed.canonical).into(),
        );
        if self
            .last
            .is_some_and(|last| last.0 == identity.0 && last != identity)
        {
            return Err("native Face publication changed at the same sequence".into());
        }
        if self.last != Some(identity) {
            let channel = PublicationChannel::ALL
                .get(usize::from(committed.channel))
                .copied()
                .ok_or("native Face publication channel")?;
            self.face
                .publish(
                    PublicationSnapshot {
                        channel,
                        sequence: committed.sequence,
                        canonical: committed.canonical,
                    },
                    committed.committed_ms,
                )
                .map_err(|e| format!("native Face replay publication: {e:?}"))?;
            self.last = Some(identity);
        }
        self.face
            .at(committed.observed_ms)
            .map_err(|e| format!("native Face replay clock: {e:?}"))?;
        let face = self.face.current();
        let property = |name| {
            face.properties
                .iter()
                .find(|p| p.name == name)
                .map(|p| &p.value)
        };
        let digest: String = report
            .content_digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if report.source_sequence != committed.sequence
            || report.revision != face.revision
            || digest != face.identity.as_str()
            || !report.publisher_retained
            || report.refusal.is_some()
            || report.fresh != matches!(property("fresh"), Some(FacePropertyValue::Flag(true)))
            || report.available
                != matches!(property("sensor-status"), Some(FacePropertyValue::Text(t)) if t == "available")
        {
            return Err("native Face differs from verified publication replay".into());
        }
        Ok(face)
    }
}
