//! Local installation of a pristine linked biography into its actual Root.
//! This is a trusted local installation boundary, never remote admission.
use conduit_body::{
    AuthenticatedHostObservation, BodyBiographyEvidence, BodyBiographyRecordKind,
    BodyLifecycleOwner, BodyLifecycleSession, BodyPlanEvidence, BodyState, MembershipProofId,
    PartId, SealedBodyLifecycleSession,
};
use conduit_core::{bind_sign, HostAdvertisement};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageInstallationRefusal {
    Biography,
    NotPristine,
    HostScope,
    SequenceExhausted,
    Membership,
}

/// The caller must own the installed image and supply its initialized local
/// Root's current advertisement. Discovery, a network document, or a compiled
/// Host template is not evidence of installation. No resource/Plot authority
/// is granted here. Existing or resumed membership uses ordinary continuity,
/// not this first-install entrance.
pub fn open_installed_image(
    evidence: BodyBiographyEvidence,
    initialized: &HostAdvertisement,
) -> Result<BodyLifecycleSession, ImageInstallationRefusal> {
    open_installed::<conduit_body::BodyPlan>(evidence, initialized)
}

/// The same installed-image admission, retaining bounded immutable Plan seals
/// when later proposals are prepared. It does not plan or start the image.
pub fn open_installed_image_sealed(
    evidence: BodyBiographyEvidence,
    initialized: &HostAdvertisement,
) -> Result<SealedBodyLifecycleSession, ImageInstallationRefusal> {
    open_installed::<conduit_body::BodyPlanSeal>(evidence, initialized)
}

fn open_installed<P: BodyPlanEvidence>(
    mut evidence: BodyBiographyEvidence,
    initialized: &HostAdvertisement,
) -> Result<BodyLifecycleOwner<P>, ImageInstallationRefusal> {
    use ImageInstallationRefusal as E;
    evidence.validate().map_err(|_| E::Biography)?;
    if evidence.body.state != BodyState::Lulled
        || evidence.records.len() != 1
        || !matches!(
            evidence.records[0].kind,
            BodyBiographyRecordKind::Born { .. }
        )
        || !evidence.membership.parts.is_empty()
        || evidence.membership.revision.0 != 0
        || evidence.compaction.is_some()
        || !evidence.wakes.is_empty()
    {
        return Err(E::NotPristine);
    }
    if initialized.protocol_version != conduit_core::PROTOCOL_VERSION
        || initialized.host_id.as_str().is_empty()
        || initialized.boot_id.as_str().is_empty()
        || initialized.offer_generation.0 == 0
    {
        return Err(E::HostScope);
    }
    let admitted_sequence = evidence
        .last_sequence()
        .checked_add(1)
        .ok_or(E::SequenceExhausted)?;
    let present_sequence = admitted_sequence
        .checked_add(1)
        .ok_or(E::SequenceExhausted)?;
    let part = PartId::bind(&evidence.body_id, initialized.host_id.as_str(), 0)
        .map_err(|_| E::Membership)?;
    let proof = MembershipProofId::bind("conduit/environment/installed-image/local-root@1")
        .map_err(|_| E::Membership)?;
    let sign =
        |sequence| bind_sign(&initialized.host_id, &initialized.boot_id, None, sequence).sign_id;
    let mut membership = evidence.membership.clone();
    let admitted = membership
        .admit(
            &evidence.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            sign(admitted_sequence),
        )
        .map_err(|_| E::Membership)?;
    let present = membership
        .observe_present(
            &evidence.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: initialized.host_id.clone(),
                boot_id: initialized.boot_id.clone(),
                offer_generation: initialized.offer_generation,
                proof_id: proof,
                sequence: 0,
            },
            sign(present_sequence),
        )
        .map_err(|_| E::Membership)?;
    evidence
        .append_membership_events(
            membership,
            &[(admitted, admitted_sequence), (present, present_sequence)],
        )
        .map_err(|_| E::Biography)?;
    BodyLifecycleOwner::<P>::open_admitted(evidence, &initialized.host_id, &initialized.boot_id)
        .map_err(|_| E::Biography)
}
