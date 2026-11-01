//! Invitation-backed proof inside an explicitly authorized browser window.
//! This adds no listener or route authority. The existing carrier/window and
//! exact expected Host/key or retained invitation scope are authorized by the owner.
use super::*;
use conduit_body::{SpawnAdmissionProof, SpawnInvitationClaim};

pub(super) struct InvitedAdvertisement {
    pub advertisement: HostAdvertisement,
    pub claim: SpawnInvitationClaim,
    pub verifying_key: Vec<u8>,
    pub encoded_bytes: u32,
}

pub(super) fn begin(
    owner: &Owner,
    window: &mut BrowserWindow,
    binding: &LinkBindingId,
    request: InvitedAdvertisement,
) -> Result<Out, String> {
    let InvitedAdvertisement {
        advertisement,
        claim,
        verifying_key,
        encoded_bytes,
    } = request;
    let at = crate::durable_host::current_time_millis()?;

    if claim.body_id != owner.session.evidence().body_id
        || advertisement.host_id == owner.host.advertisement().host_id
    {
        return Err("invited browser differs from authorized Body or Host".into());
    }
    let key: [u8; 32] = verifying_key
        .try_into()
        .map_err(|_| "invalid browser verifying key")?;
    let fresh_scope = match &window.scope {
        BrowserWindowScope::ExactHost => {
            if window.expected.as_ref() != Some(&advertisement.host_id)
                || window.new_key != Some(key)
            {
                return Err("invited browser Host/key differs from explicit authorization".into());
            }
            false
        }
        BrowserWindowScope::InvitationScope(expected_claim) => {
            if expected_claim != &claim || window.expected.is_some() || window.new_key.is_some() {
                return Err("invited browser differs from exact retained invitation scope".into());
            }
            true
        }
    };
    let manager = owner
        .admissions
        .as_ref()
        .ok_or("owner has no invitation authority")?;
    let already_known = manager
        .receipts
        .iter()
        .any(|r| r.credential.host_id == advertisement.host_id.as_str());
    // An expired claim can only recover its exact durable outcome, never admit
    // again. Complete checks the retained request/key/offer and current Part.
    claim
        .inspect(if already_known && !fresh_scope { 0 } else { at })
        .map_err(debug)?;
    let observation = CandidateObservation {
        advertisement,
        friendly_label: "Invited browser".into(),
        freshness_sequence: 1,
        encoded_bytes,
        observed_binding_id: binding.clone(),
        observation_sign_id: signal(binding, "invited-observed"),
        proof_id: DiscoveryProofId::bind(binding.as_str()).map_err(debug)?,
    };
    // Validate the complete offered snapshot using the ordinary finite ingress.
    // Retaining it supplies planning evidence, not capability or Line authority.
    CandidateInventory::new(manager.body_id.clone())
        .map_err(debug)?
        .observe(observation.clone())
        .map_err(debug)?;
    // Freeze only after exact claim, finite observation and deadline validation.
    // Pending retains the full immutable observed Host/Boot/offer; later proofs
    // cannot replace it with a competing endpoint.
    if fresh_scope {
        window.expected = Some(observation.advertisement.host_id.clone());
        window.new_key = Some(key);
    }
    window.state = WindowState::Pending(Box::new(Pending {
        binding: binding.clone(),
        observation,
        kind: PendingKind::Invited {
            claim: claim.clone(),
        },
    }));
    Ok(Out::SpawnChallenge {
        protocol: PROTOCOL,
        claim,
    })
}

pub(super) fn complete(
    root: &std::path::Path,
    browser_host_key: [u8; 32],
    session: &mut conduit_body::BodyLifecycleSession,
    manager: &mut AdmissionManager,
    pending: &Pending,
    claim: &SpawnInvitationClaim,
    proof: SpawnAdmissionProof,
    authority: &HostAdvertisement,
) -> Result<
    (
        MembershipCredential,
        Option<crate::durable_host::root_signer::receipt::Draft>,
        bool,
    ),
    String,
> {
    let at = crate::durable_host::current_time_millis()?;

    let observed = &pending.observation.advertisement;
    if proof.invitation_id != claim.invitation_id
        || proof.body_id != claim.body_id
        || proof.nonce != claim.nonce
        || proof.host_id != observed.host_id
        || proof.boot_id != observed.boot_id
    {
        return Err("invited browser proof differs from retained claim or offer".into());
    }
    let request = conduit_body::PortableMembershipAdmissionRequest {
        schema: conduit_body::MEMBERSHIP_ADMISSION_REQUEST_SCHEMA.into(),
        protocol_version: conduit_core::PROTOCOL_VERSION,
        invitation_id: proof.invitation_id.clone(),
        body_id: proof.body_id.clone(),
        host_id: proof.host_id.clone(),
        boot_id: proof.boot_id.clone(),
        offer_generation: observed.offer_generation,
        nonce: proof.nonce,
        signature: proof.signature,
    };
    // A prior local-control timeout may have left consume+draft journaled while
    // this actor rolled back its cache. Recover first; never consume it again.
    crate::durable_host::owner::state::recover(root)?;
    let selected = crate::durable_host::root_signer::selected(
        root,
        claim.body_id.as_str(),
        authority.host_id.as_str(),
    )?
    .is_some();
    if selected {
        if let Some(draft) = crate::durable_host::root_signer::receipt::reserve(
            root,
            claim.body_id.as_str(),
            authority.host_id.as_str(),
            &request,
            claim,
            observed,
            &browser_host_key,
        )? {
            let recovered = crate::durable_host::owner::state::load(root)?
                .ok_or("recovered receipt has no Body")?;
            let recovered_manager =
                crate::durable_host::owner::state::admissions(root, &claim.body_id)?
                    .ok_or("recovered receipt has no admission manager")?;
            draft.require_current(&recovered, &recovered_manager)?;
            *session = conduit_body::BodyLifecycleSession::resume_here(
                recovered,
                &authority.host_id,
                &authority.boot_id,
            )
            .map_err(debug)?;
            *manager = recovered_manager;
            return Ok((draft.credential().clone(), Some(draft), true));
        }
    }
    claim.inspect(at).map_err(debug)?;
    // Existing complete_spawn, exact biography changes, and finite receipt
    // policy remain owned by the lifecycle session. Caller persists both before
    // issuing the existing BrowserLineAuthorization or publishing Active state.
    let credential = session
        .admit_invited_host(
            manager,
            observed,
            &proof,
            at,
            &authority.host_id,
            &authority.boot_id,
        )
        .map_err(debug)?;
    let draft = if selected {
        Some(crate::durable_host::root_signer::receipt::prepare(
            root,
            session.evidence(),
            manager,
            authority,
            request,
            claim.clone(),
            observed,
            browser_host_key,
            credential.clone(),
        )?)
    } else {
        None
    };
    Ok((credential, draft, false))
}
