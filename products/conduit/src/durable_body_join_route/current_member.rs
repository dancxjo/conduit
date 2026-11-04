//! Current-member authentication on the existing pinned carrier, not PartReturn.
//! Successful authentication names no initialized Line or execution authority.
use super::*;
use conduit_body::{FederationChallenge, FederationInspection, MembershipCredential};
use conduit_core::{HostAdvertisement, LineId, LinkBindingId};
use conduit_std_host::secure_websocket::{SecureWebSocketClientLine, SecureWebSocketLine};

const SCHEMA: &str = "conduit.body/member-carrier@1";
const SESSION_MILLIS: u32 = 60_000;
const SESSION_FRAMES: u32 = 128;

/// Logical label for this accepted/pinned TLS carrier. Naming it issues no
/// LineOffer: actual directional offers and custody remain the routing layer's work.
pub(crate) fn carrier_label(encounter: &MemberEncounter) -> LineId {
    format!(
        "member-carrier/{}",
        crate::durable_host::digest(&encounter.candidate.authentication.transport_binding_sha256)
    )
    .into()
}
pub(super) fn serve(
    state: &Path,
    encounter: &MemberEncounter,
    line: &mut SecureWebSocketLine,
    frame: MemberReturnFrame,
) -> Result<
    (
        crate::durable_host::owner::MemberPresentationBasis,
        LinkBindingId,
    ),
    String,
> {
    serve_with(
        encounter,
        line,
        frame,
        |credential, ad, label, expires_at| {
            crate::durable_host_control::begin_carrier(
                state,
                credential,
                ad,
                label,
                expires_at,
                SESSION_FRAMES,
            )
        },
        |challenge, signature| {
            crate::durable_host_control::complete_carrier(state, challenge, signature)
        },
        |session| crate::durable_host_control::retire_carrier(state, session),
    )
}

pub(crate) fn serve_with(
    encounter: &MemberEncounter,
    line: &mut SecureWebSocketLine,
    frame: MemberReturnFrame,
    begin: impl FnOnce(
        MembershipCredential,
        HostAdvertisement,
        LineId,
        u64,
    ) -> Result<(FederationChallenge, [u8; 64]), String>,
    complete: impl FnOnce(FederationChallenge, [u8; 64]) -> Result<FederationInspection, String>,
    retire: impl FnOnce(LinkBindingId) -> Result<(), String>,
) -> Result<
    (
        crate::durable_host::owner::MemberPresentationBasis,
        LinkBindingId,
    ),
    String,
> {
    let MemberReturnFrame::Resume {
        schema,
        credential,
        advertisement,
    } = frame
    else {
        return Err("current-member carrier requires resume facts".into());
    };
    let now = current_time_millis()?;
    if schema != SCHEMA
        || now >= encounter.candidate.expires_at_millis
        || credential.body_id != encounter.body_id
        || credential.part_id != encounter.part_id
        || credential.host_id != encounter.host_id
        || advertisement.host_id != encounter.host_id
    {
        return Err("current-member carrier differs from selected scope".into());
    }
    // Carry the selected absolute deadline through actor scheduling; a later
    // owner clock sample must never replenish the remaining transport lifetime.
    let expires_at = encounter.candidate.expires_at_millis.min(
        now.checked_add(u64::from(SESSION_MILLIS))
            .ok_or("carrier expiry overflow")?,
    );
    let member = crate::durable_host::owner::MemberPresentationBasis::new(
        credential.clone(),
        (*advertisement).clone(),
    )?;
    let (challenge, signature) = begin(
        credential,
        *advertisement,
        carrier_label(encounter),
        expires_at,
    )?;
    let session = challenge.session_id.clone();
    // Once issued, every failed carrier exchange must release its exact slot.
    // A disconnected or malformed peer cannot strand pending custody.
    let outcome = (|| {
        if challenge.responder != encounter.owner {
            return Err("current-member receiver changed from selected owner".into());
        }
        send(
            line,
            &MemberReturnFrame::CarrierChallenge {
                schema: SCHEMA.into(),
                challenge: Box::new(challenge.clone()),
                signature: signature.to_vec(),
            },
        )?;
        let MemberReturnFrame::CarrierProof {
            schema,
            challenge: received,
            signature,
        } = receive(line)?
        else {
            return Err("current-member carrier proof absent".into());
        };
        if schema != SCHEMA || *received != challenge {
            return Err("current-member carrier proof context differs".into());
        }
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| "current-member signature size")?;
        complete(challenge, signature).and_then(|inspection| {
            send(
                line,
                &MemberReturnFrame::CarrierAuthenticated {
                    schema: SCHEMA.into(),
                    inspection: Box::new(inspection),
                },
            )
        })
    })();
    if let Err(error) = outcome {
        if let Err(cleanup) = retire(session) {
            eprintln!("member carrier cleanup unacknowledged: {cleanup}");
        }
        return Err(error);
    }
    Ok((member, session))
}

/// The owner peer comes from the independently selected descriptor, not from
/// the challenge. TLS pinning authenticates this finite encounter's messages.
/// This produces authentication facts only, never a Line/Show/effect grant.
#[allow(dead_code)] // Native renewal caller follows the exercised wire exchange.
pub(crate) fn resume(
    state: &Path,
    credential: &MembershipCredential,
    actual: &HostAdvertisement,
    encounter: &MemberEncounter,
    line: &mut SecureWebSocketClientLine,
) -> Result<FederationInspection, String> {
    if credential.body_id != encounter.body_id
        || credential.part_id != encounter.part_id
        || credential.host_id != encounter.host_id
        || current_time_millis()? >= encounter.candidate.expires_at_millis
    {
        return Err("current-member carrier selection scope expired or changed".into());
    }
    line.send_binary(
        &serde_json::to_vec(&MemberReturnFrame::Resume {
            schema: SCHEMA.into(),
            credential: credential.clone(),
            advertisement: Box::new(actual.clone()),
        })
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("send member carrier: {e:?}"))?;
    let MemberReturnFrame::CarrierChallenge {
        schema,
        challenge,
        signature: receiver_signature,
    } = client_receive(line)?
    else {
        return Err("current-member carrier challenge absent".into());
    };
    if schema != SCHEMA
        || challenge.responder != encounter.owner
        || challenge.line_id != carrier_label(encounter)
        || challenge.line_security != conduit_core::LineSecurity::AuthenticatedEncrypted
        || challenge.expires_at_millis > encounter.candidate.expires_at_millis
    {
        return Err("current-member carrier differs from pinned owner/context".into());
    }
    let receiver_signature: [u8; 64] = receiver_signature
        .try_into()
        .map_err(|_| "owner carrier signature size")?;
    let context = crate::durable_host::membership::MemberCarrierSigningContext {
        receiver: &encounter.owner,
        session: &challenge.session_id,
        line: &challenge.line_id,
        security: conduit_core::LineSecurity::AuthenticatedEncrypted,
        maximum_frames: SESSION_FRAMES,
        maximum_millis: SESSION_MILLIS,
    };
    let now = current_time_millis()?;
    let signature = crate::durable_host::membership::sign_member_carrier_challenge(
        state, credential, actual, &context, &challenge, now,
    )?;
    // Both signatures are checked before using any returned authentication facts.
    conduit_body::FederatedReceivingSession::establish(
        (*challenge).clone(),
        signature,
        receiver_signature,
        now,
    )
    .map_err(|e| format!("member carrier mutual authentication: {e:?}"))?;
    line.send_binary(
        &serde_json::to_vec(&MemberReturnFrame::CarrierProof {
            schema: SCHEMA.into(),
            challenge: challenge.clone(),
            signature: signature.to_vec(),
        })
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("send carrier proof: {e:?}"))?;
    let MemberReturnFrame::CarrierAuthenticated { schema, inspection } = client_receive(line)?
    else {
        return Err("member carrier completion absent".into());
    };
    if schema != SCHEMA
        || inspection.session_id != challenge.session_id
        || inspection.line_id != challenge.line_id
        || inspection.line_security != challenge.line_security
        || inspection.authenticated_peer_host != actual.host_id
        || inspection.authenticated_peer_boot != actual.boot_id
        || inspection.peer_offer_generation != actual.offer_generation
        || !inspection.membership_current
        || !inspection.credential_current
        || inspection.next_sequence != 0
        || inspection.maximum_frames != challenge.maximum_frames
    {
        return Err("member carrier completion scope differs".into());
    }
    Ok(*inspection)
}
fn client_receive(line: &mut SecureWebSocketClientLine) -> Result<MemberReturnFrame, String> {
    let mut bytes = vec![0; MAXIMUM_ROUTE_FRAME_BYTES];
    let n = line
        .receive_binary(&mut bytes)
        .map_err(|e| format!("member carrier input: {e:?}"))?;
    serde_json::from_slice(&bytes[..n]).map_err(|_| "invalid member carrier frame".into())
}
