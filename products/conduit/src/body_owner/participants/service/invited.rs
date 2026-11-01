//! Invitation-backed proof inside an explicitly authorized browser window.
//! This adds no listener or route authority. The existing carrier/window and
//! exact expected Host/key remain independently authorized by the owner.
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
    claim.inspect(at).map_err(debug)?;
    if claim.body_id != owner.session.evidence().body_id
        || advertisement.host_id != window.expected
        || advertisement.host_id == owner.host.advertisement().host_id
    {
        return Err("invited browser differs from authorized Body or Host".into());
    }
    let key: [u8; 32] = verifying_key
        .try_into()
        .map_err(|_| "invalid browser verifying key")?;
    if window.new_key != Some(key) {
        return Err("invited browser key differs from explicit authorization".into());
    }
    let manager = owner
        .admissions
        .as_ref()
        .ok_or("owner has no invitation authority")?;
    if manager
        .receipts
        .iter()
        .any(|r| r.credential.host_id == window.expected.as_str())
    {
        return Err("admitted browser must present its retained credential".into());
    }
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
    session: &mut conduit_body::BodyLifecycleSession,
    manager: &mut AdmissionManager,
    pending: &Pending,
    claim: &SpawnInvitationClaim,
    proof: SpawnAdmissionProof,
    authority: &HostAdvertisement,
) -> Result<MembershipCredential, String> {
    let at = crate::durable_host::current_time_millis()?;
    claim.inspect(at).map_err(debug)?;
    let observed = &pending.observation.advertisement;
    if proof.invitation_id != claim.invitation_id
        || proof.body_id != claim.body_id
        || proof.nonce != claim.nonce
        || proof.host_id != observed.host_id
        || proof.boot_id != observed.boot_id
    {
        return Err("invited browser proof differs from retained claim or offer".into());
    }
    // Existing complete_spawn, exact biography changes, and finite receipt
    // policy remain owned by the lifecycle session. Caller persists both before
    // issuing the existing BrowserLineAuthorization or publishing Active state.
    session
        .admit_invited_host(
            manager,
            observed,
            &proof,
            at,
            &authority.host_id,
            &authority.boot_id,
        )
        .map_err(debug)
}
