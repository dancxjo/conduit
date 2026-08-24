use conduit_body::{
    AdmissionManager, AdmissionSigns, AmbientAdmissionProof, BodyMembership, CandidateInventory,
    CandidateObservation, DiscoveryProofId, MembershipCredential,
};
use conduit_core::HostAdvertisement;
use conduit_core::{LinkBindingId, SignId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionSocket,
    BROWSER_ADMISSION_PROTOCOL,
};

pub(super) struct Peer {
    pub(super) socket: BrowserAdmissionSocket,
    pub(super) credential: MembershipCredential,
    pub(super) session_id: LinkBindingId,
    pub(super) advertisement: HostAdvertisement,
}

pub(super) fn admit(
    mut socket: BrowserAdmissionSocket,
    index: usize,
    candidates: &mut CandidateInventory,
    admission: &mut AdmissionManager,
    membership: &mut BodyMembership,
) -> Result<Peer, String> {
    let (frame, encoded_bytes) = socket
        .receive_with_size()
        .map_err(super::protocol::debug("advertise"))?;
    let BrowserAdmissionIngress::Advertise {
        advertisement,
        friendly_label,
        verifying_key,
        freshness_sequence,
        ..
    } = frame
    else {
        return Err("peer did not advertise".into());
    };
    let verifying_key = verifying_key
        .try_into()
        .map_err(|_| "browser verifying key was not 32 bytes")?;
    let proof_id = format!("proof/probe/{index}");
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement: advertisement.clone(),
            friendly_label,
            observed_binding_id: LinkBindingId::from(format!("probe/admission/{index}")),
            observation_sign_id: SignId::from(format!("sign/probe/observed/{index}")),
            proof_id: DiscoveryProofId::bind(&proof_id)
                .map_err(super::protocol::debug("proof id"))?,
            freshness_sequence,
            encoded_bytes,
        })
        .map_err(super::protocol::debug("observe"))?;
    let challenge = admission
        .begin_ambient(
            candidates,
            &candidate,
            verifying_key,
            [index as u8 + 1; 32],
            1_000,
            2_000,
            SignId::from(format!("sign/probe/requested/{index}")),
        )
        .map_err(super::protocol::debug("challenge"))?;
    socket
        .send(&BrowserAdmissionEgress::Challenge {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            challenge,
        })
        .map_err(super::protocol::debug("send challenge"))?;
    let BrowserAdmissionIngress::AmbientProof {
        admission_id,
        body_id,
        host_id,
        boot_id,
        nonce,
        signature,
        ..
    } = socket.receive().map_err(super::protocol::debug("proof"))?
    else {
        return Err("peer did not prove admission".into());
    };
    let proof = AmbientAdmissionProof {
        admission_id,
        body_id,
        host_id,
        boot_id,
        nonce: nonce.try_into().map_err(|_| "invalid proof nonce")?,
        signature: signature
            .try_into()
            .map_err(|_| "invalid proof signature")?,
    };
    let credential = admission
        .complete_ambient(
            candidates,
            membership,
            &proof,
            1_100,
            AdmissionSigns {
                part_admitted: SignId::from(format!("sign/probe/admitted/{index}")),
                host_attached: SignId::from(format!("sign/probe/attached/{index}")),
                candidate_admitted: SignId::from(format!("sign/probe/candidate/{index}")),
            },
        )
        .map_err(super::protocol::debug("complete"))?;
    socket
        .send(&BrowserAdmissionEgress::Admitted {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            credential: credential.clone(),
        })
        .map_err(super::protocol::debug("send admitted"))?;
    Ok(Peer {
        socket,
        session_id: LinkBindingId::from(format!("probe/presence/{index}")),
        credential,
        advertisement,
    })
}
