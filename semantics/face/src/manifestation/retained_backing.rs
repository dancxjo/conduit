//! Actual Show-owned heap payload; no Plan, serialized bytes or renderer state.
use super::*;
use core::mem::size_of;

impl ManifestationId {
    /// Owned String payload only; excludes the inline identity wrapper.
    pub fn retained_backing_bytes(&self) -> usize {
        self.0.capacity()
    }
}
impl Manifestation {
    /// Includes every retained Sign and spare vector capacity; excludes inline
    /// Self and allocator overhead. This does not include a MaskShow's Plan.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut n = self
            .signs
            .capacity()
            .checked_mul(size_of::<ManifestationSign>())?;
        for bytes in [
            self.manifestation_id.retained_backing_bytes(),
            self.body_id
                .as_ref()
                .map_or(0, BodyId::retained_backing_bytes),
            self.wake_id
                .as_ref()
                .map_or(0, WakeId::retained_backing_bytes),
            self.face_id.retained_backing_bytes(),
            self.plan_id.0.capacity(),
            self.active_play_id.0.capacity(),
            self.placement_id.0.capacity(),
            self.front_subject.capacity(),
            self.presenter_capability_id.0.capacity(),
            self.presenter_implementation_id.0.capacity(),
            self.presenter_artifact_id.0.capacity(),
            self.host_id.0.capacity(),
            self.boot_id.0.capacity(),
            self.target_subject.capacity(),
        ] {
            n = n.checked_add(bytes)?;
        }
        for sign in &self.signs {
            for bytes in [
                sign.sign_id.0.capacity(),
                sign.manifestation_id.retained_backing_bytes(),
                sign.face_id.retained_backing_bytes(),
                sign.plan_id.0.capacity(),
                sign.active_play_id.0.capacity(),
                sign.placement_id.0.capacity(),
                sign.host_id.0.capacity(),
                sign.boot_id.0.capacity(),
                sign.presenter_implementation_id.0.capacity(),
            ] {
                n = n.checked_add(bytes)?;
            }
        }
        Some(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    #[test]
    fn spare_sign_capacity_and_nested_id_payloads_remain_charged() {
        // Ownership fixture only; no Show validation or availability claim.
        let mut shown = Manifestation {
            manifestation_id: ManifestationId(String::with_capacity(111)),
            body_id: None,
            wake_id: None,
            face_id: FaceContentId(String::with_capacity(137)),
            face_revision: 1,
            plan_id: "".into(),
            active_play_id: "".into(),
            play_sequence: 0,
            placement_id: "".into(),
            front_subject: String::new(),
            presenter_capability_id: "".into(),
            presenter_implementation_id: "".into(),
            presenter_artifact_id: "".into(),
            host_id: "".into(),
            boot_id: "".into(),
            offer_generation: OfferGeneration(1),
            target_subject: String::new(),
            lifecycle: ManifestationLifecycle::Prepared,
            failure: None,
            signs: Vec::with_capacity(8),
        };
        let base = shown.manifestation_id.retained_backing_bytes()
            + shown.face_id.retained_backing_bytes()
            + shown.signs.capacity() * size_of::<ManifestationSign>();
        assert_eq!(shown.retained_backing_bytes(), Some(base));
        let sign = ManifestationSign {
            sign_id: SignId(String::with_capacity(97)),
            manifestation_id: ManifestationId(String::with_capacity(39)),
            face_id: FaceContentId(String::with_capacity(41)),
            plan_id: "".into(),
            active_play_id: "".into(),
            play_sequence: 0,
            placement_id: "".into(),
            host_id: "".into(),
            boot_id: "".into(),
            offer_generation: OfferGeneration(1),
            presenter_implementation_id: "".into(),
            lifecycle: ManifestationLifecycle::Prepared,
            failure: None,
        };
        let nested = sign.sign_id.0.capacity()
            + sign.manifestation_id.retained_backing_bytes()
            + sign.face_id.retained_backing_bytes();
        shown.signs.push(sign);
        assert_eq!(shown.retained_backing_bytes(), Some(base + nested));
        shown.signs.clear();
        assert_eq!(shown.retained_backing_bytes(), Some(base));
    }
}
