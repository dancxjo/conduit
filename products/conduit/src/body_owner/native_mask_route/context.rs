//! Preparation-only two-context bundle. This value is not a reservation,
//! admission or Show; its caller must retain original preparation custody.
//! A future funded factory must separately retain original caller custody for
//! escaping Result::Err(String) diagnostics and captures: the local factory
//! lease refunds on Err, and no returned ImagePreparedValue owns that error.
//! Complete peak, failure/unwind destruction and cold Source/catalog custody
//! remain independent obligations; this uninstalled helper proves none of them.
use super::*;
use conduit_body::BodyState;
use conduit_conduitos_mask_offer::{prepare_layout_component_source, NativeMaskStorageProfile};
use conduit_face::{AmbientClockEncounter, ConfiguredMaskConfiguration, PlannedMaskPlot};

struct PreparedContext {
    plan: PlannedMaskPlot,
    configuration: ConfiguredMaskConfiguration,
}

/// Exactly two checked presentations for one observed native offer and owner
/// workload. No Source/compiler representation is needed by the later consumer.
/// This does not witness funding: production retention must carry the original
/// counter reservation alongside this value before the application starts.
pub(crate) struct PreparedNativeContexts {
    owner_offer: [u8; 32],
    native_offer: [u8; 32],
    application: conduit_body::BodyPlayIdentity,
    workload_revision: u64,
    subjects: [String; 3],
    tile: PreparedContext,
    fullscreen: PreparedContext,
}

impl Owner {
    /// Called after the exact application proposal and before its actual Start.
    /// The explicit identities must be supplied by its actual Face producer;
    /// neither human names nor future presentation success are inferred here.
    pub(crate) fn prepare_native_ambient_contexts(
        &self,
        member: &MemberPresentationBasis,
        clock_subject: &str,
        weather_subject: &str,
        image_subject: &str,
    ) -> Result<PreparedNativeContexts, String> {
        if self.host.is_playing() {
            return Err("native-mask-context-preparation-requires-idle-owner".into());
        }
        self.require_current_native_member(member)?;
        let application = prospective_context_play(self)?;
        let native_offer = &member.host_advertisement;
        let native_identity = offer_identity(native_offer)?;
        let observed = self
            .invited_mask_offers
            .get(&native_offer.host_id)
            .ok_or("native-mask-context-offer-not-observed")?;
        if !matches_digest(&native_identity, &observed.digest) {
            return Err("native-mask-context-offer-not-observed".into());
        }
        let source =
            conduit_face::ambient_clock_mask_source(clock_subject, weather_subject, image_subject)
                .map_err(|error| format!("native-mask-context-subject-refused:{error:?}"))?;
        let checked = prepare_layout_component_source(
            native_offer,
            &source,
            NativeMaskStorageProfile::Standard,
        )
        .map_err(|error| format!("native-mask-context-source-refused:{error:?}"))?;
        let prepare = |encounter: AmbientClockEncounter| {
            let (stage, configuration) = checked
                .prepare_root(native_offer, encounter.root())
                .map_err(|error| format!("native-mask-context-plan-refused:{error:?}"))?;
            Ok::<_, String>(PreparedContext {
                plan: stage.planned_mask,
                configuration,
            })
        };
        Ok(PreparedNativeContexts {
            owner_offer: offer_identity(self.host.advertisement())?,
            native_offer: native_identity,
            application,
            workload_revision: self.session.evidence().body.workload_revision,
            subjects: [
                clock_subject.into(),
                weather_subject.into(),
                image_subject.into(),
            ],
            tile: prepare(AmbientClockEncounter::Tile)?,
            fullscreen: prepare(AmbientClockEncounter::Fullscreen)?,
        })
    }
}

// This validates proposal/identity only. Its allocations and the complete Source
// factory still require the caller's original custody before production use.
fn prospective_context_play(owner: &Owner) -> Result<conduit_body::BodyPlayIdentity, String> {
    if owner.host.is_playing() {
        return Err("native-mask-context-preparation-requires-idle-owner".into());
    }
    let proposal = owner
        .session
        .realization()
        .filter(|proposal| proposal.play.is_none())
        .ok_or("native-mask-context-preparation-requires-unstarted-proposal")?;
    if !matches!(&owner.session.evidence().body.state,
        BodyState::Awake { wake_id } if wake_id == &proposal.wake.wake_id)
    {
        return Err("native-mask-context-preparation-requires-unstarted-proposal".into());
    }
    owner
        .host
        .prospective_body_play(&proposal.wake, &proposal.plan)
}

impl PreparedNativeContexts {
    pub(super) fn plan_for(
        &self,
        owner: &Owner,
        native_offer: &HostAdvertisement,
        face: &Face,
        encounter: AmbientClockEncounter,
    ) -> Result<&PlannedMaskPlot, String> {
        if self.owner_offer != offer_identity(owner.host.advertisement())?
            || self.native_offer != offer_identity(native_offer)?
            || self.application.body_id != owner.session.evidence().body_id
            || face.basis.body_id.as_ref() != Some(&self.application.body_id)
            || face.basis.wake_id.as_ref() != Some(&self.application.wake_id)
            || face.basis.plan_id.as_ref() != Some(&self.application.plan_id)
            || face.basis.active_play_id.as_ref() != Some(&self.application.active_play_id)
            || self.workload_revision != owner.session.evidence().body.workload_revision
        {
            return Err("native-mask-context-preparation-stale".into());
        }
        for (role, expected) in [
            "time/civil-observation",
            "weather/conditions",
            "media/image",
        ]
        .into_iter()
        .zip(&self.subjects)
        {
            let mut matches = face.subjects.iter().filter(|subject| {
                matches!(&subject.role, conduit_face::FaceRole::Semantic(kind) if kind.0 == role)
            });
            if matches
                .next()
                .is_none_or(|subject| &subject.identity != expected)
                || matches.next().is_some()
            {
                return Err("native-mask-context-subjects-changed".into());
            }
        }
        let context = match encounter {
            AmbientClockEncounter::Tile => &self.tile,
            AmbientClockEncounter::Fullscreen => &self.fullscreen,
        };
        let decode = |value: &conduit_core::StructuredConfigurationValue| {
            conduit_core::StructuredInfoValue::from_canonical_bytes(value.canonical_value())
                .map_err(|_| "native-mask-context-configuration-invalid".to_string())
        };
        let layout = context
            .configuration
            .layout
            .as_ref()
            .ok_or("native-mask-context-layout-absent")?;
        conduit_face::prepare_component_layout(
            decode(layout)?,
            decode(&context.configuration.recipe)?,
            face,
        )
        .map_err(|error| format!("native-mask-context-face-refused:{error:?}"))?;
        Ok(&context.plan)
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;

/// The complete observed advertisement is still checked exactly, without
/// duplicating independently owned catalogs into the retained presentation.
/// Streaming SHA256 yields the same bytes as the existing JSON offer digest.
fn offer_identity(offer: &HostAdvertisement) -> Result<[u8; 32], String> {
    use sha2::{Digest, Sha256};
    struct Sink(Sha256);
    impl std::io::Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Sink(Sha256::new());
    serde_json::to_writer(&mut sink, offer)
        .map_err(|_| "native-mask-context-offer-encoding".to_string())?;
    Ok(sink.0.finalize().into())
}
fn matches_digest(identity: &[u8; 32], digest: &str) -> bool {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let Some(digest) = digest.strip_prefix("sha256:") else {
        return false;
    };
    digest.len() == 64
        && identity.iter().enumerate().all(|(i, byte)| {
            digest.as_bytes()[2 * i] == HEX[usize::from(byte >> 4)]
                && digest.as_bytes()[2 * i + 1] == HEX[usize::from(byte & 15)]
        })
}
