//! A second Mask of the same committed Face; it owns no sampling state.
use conduit_seven_segment_mask as m;

/// Fixed owned Show snapshot so physical work happens after releasing the Face
/// owner's lock. This is representation, not a hardware acknowledgement.
pub struct SevenSegmentSnapshot {
    pub face_identity: [u8; 64],
    pub face_revision: u64,
    pub show: m::SevenSegmentShow,
    pub disposition: m::MaskDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SevenSegmentSnapshotRefusal {
    NotInitialized,
    Publication(conduit_environment_face::PublicationRefusal),
    Projection(m::FaceProjectionRefusal),
    Wiring(m::WiringRefusal),
    Identity,
}
pub fn seven_segment_snapshot(
    now: u64,
    selection: m::SevenSegmentSelection,
    profile: &m::Tm1637Profile,
) -> Result<SevenSegmentSnapshot, SevenSegmentSnapshotRefusal> {
    use SevenSegmentSnapshotRefusal as E;
    critical_section::with(|cs| {
        let mut slot = super::PUBLICATION.borrow(cs).borrow_mut();
        let publication = slot.as_deref_mut().ok_or(E::NotInitialized)?;
        let face = publication.face.at(now).map_err(E::Publication)?;
        let view = m::PreparedEnvironmentFace::prepare(face, conduit_environment_face::SUBJECT)
            .map_err(E::Projection)?;
        let rendered = m::render_for_profile(
            &view,
            selection,
            &m::SevenSegmentLook::fahrenheit(),
            profile,
        )
        .map_err(E::Wiring)?;
        let mut identity = [0; 64];
        if rendered.face_content_id.len() != identity.len() {
            return Err(E::Identity);
        }
        identity.copy_from_slice(rendered.face_content_id.as_bytes());
        Ok(SevenSegmentSnapshot {
            face_identity: identity,
            face_revision: rendered.face_revision,
            show: rendered.show,
            disposition: rendered.disposition,
        })
    })
}
