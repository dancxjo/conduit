//! Admitted boot-scoped Body lifetime using the canonical lifecycle session.
use alloc::{format, vec};
use conduit_body::*;
use conduit_core::{BootId, HostId, OfferGeneration, Plan, bind_sign};

pub(super) fn start(
    plan: Plan,
    host: &HostId,
    boot: &BootId,
    generation: OfferGeneration,
) -> Result<BodyLifecycleSession, &'static str> {
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let sequence = u64::from_le_bytes(digest_prefix(boot.as_str())) >> 1;
    let body = Body::born_with_plots(
        BodyWorkset::one(resident.clone()).map_err(|_| "startup-workset")?,
        sequence,
        format!("conduitos/startup/born/{}", boot.as_str()).into(),
    )
    .map_err(|_| "startup-body-birth")?;
    let part = PartId::bind(&body.body_id, host.as_str(), 0).map_err(|_| "startup-part")?;
    let proof = MembershipProofId::bind("conduitos/product/local-birth")
        .map_err(|_| "startup-membership")?;
    let mut membership =
        BodyMembership::new(body.body_id.clone()).map_err(|_| "startup-membership")?;
    let mut biography =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Conduit startup".into())
            .map_err(|_| "startup-biography")?;
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "conduitos/startup/part-admitted".into(),
        )
        .map_err(|_| "startup-membership")?;
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: generation,
                proof_id: proof,
                sequence: 0,
            },
            "conduitos/startup/host-attached".into(),
        )
        .map_err(|_| "startup-membership")?;
    let events = membership
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| (e.change_id.clone(), sequence + i as u64 + 1))
        .collect::<alloc::vec::Vec<_>>();
    biography
        .append_membership_events(membership, &events)
        .map_err(|_| "startup-biography")?;
    let mut session = BodyLifecycleSession::open_admitted(biography, host, boot)
        .map_err(|_| "startup-admission")?;
    session
        .propose(
            vec![BodyPlotPlan {
                plot: resident,
                plan,
            }],
            host,
            boot,
        )
        .map_err(|_| "startup-proposal")?;
    let current = session.realization().ok_or("startup-proposal")?;
    let play = BodyPlayIdentity::bind(&current.plan, 1);
    let sign = |n| bind_sign(host, boot, Some(&play.active_play_id), n).sign_id;
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .and_then(|w| w.body_play_started(&current.plan, &play, sign(1)))
        .map_err(|_| "startup-start")?;
    session
        .started(host, boot, play, wake)
        .map_err(|_| "startup-start")?;
    Ok(session)
}
fn digest_prefix(value: &str) -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(value.as_bytes());
    digest[..8].try_into().expect("SHA256 prefix")
}
