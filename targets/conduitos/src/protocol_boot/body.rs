//! Local administrator-directed Birth and current Boot membership.
use conduit_body::*;
use conduit_core::{BootId, HostId, OfferGeneration};

pub(super) fn admit(
    resident: ResidentPlot,
    host: &HostId,
    boot: &BootId,
) -> Result<BodyLifecycleSession, &'static str> {
    let body = Body::born(
        resident.source_document_id,
        resident.checked_plot_id,
        1,
        "conduitos/protocol/born".into(),
    )
    .map_err(|_| "protocol-body-birth-refused")?;
    let mut membership =
        BodyMembership::new(body.body_id.clone()).map_err(|_| "protocol-membership-refused")?;
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Device protocol".into())
            .map_err(|_| "protocol-biography-refused")?;
    let part =
        PartId::bind(&body.body_id, host.as_str(), 1).map_err(|_| "protocol-part-refused")?;
    let proof = MembershipProofId::bind("conduitos/local-administrator/protocol-birth")
        .map_err(|_| "protocol-membership-refused")?;
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "conduitos/protocol/admitted".into(),
        )
        .map_err(|_| "protocol-membership-refused")?;
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            "conduitos/protocol/present".into(),
        )
        .map_err(|_| "protocol-membership-refused")?;
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .map_err(|_| "protocol-biography-refused")?;
    BodyLifecycleSession::open_admitted(evidence, host, boot)
        .map_err(|_| "protocol-body-admission-refused")
}
