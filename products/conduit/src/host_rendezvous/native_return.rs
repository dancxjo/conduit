//! Canonical retained-Part Return protocol. No transport or execution authority.
//!
//! The connector must select/authenticate the retained rendezvous pins before
//! passing documents here. A protocol result alone is deliberately not a
//! current owner session, Face subscription, persisted credential, or Show.
use crate::durable_host::RetainedPart;
use conduit_body::MembershipCredential;
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress as Out, BrowserAdmissionIngress as In,
    BROWSER_ADMISSION_PROTOCOL as PROTOCOL, MAX_BROWSER_ADMISSION_FRAME_BYTES as LIMIT,
};

pub(crate) struct NativeReturn {
    part: RetainedPart,
    phase: Phase,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    Challenge,
    Admission,
    Finished,
    Failed,
}
impl NativeReturn {
    pub(crate) fn new(part: RetainedPart) -> Result<Self, String> {
        let route = part
            .rendezvous()
            .ok_or("retained Part has no pinned rendezvous route")?;
        // Structural inspection only; an eventual connector must validate its
        // selected candidate's actual clock/attempt budget before any effect.
        route
            .validate(0)
            .map_err(|_| "retained rendezvous structure or pins invalid")?;
        if route.body_id != part.credential().body_id.as_str() {
            return Err("retained rendezvous belongs to another Body".into());
        }
        Ok(Self {
            part,
            phase: Phase::Ready,
        })
    }
    /// Exact original route, not a newly supplied invitation or discovered pin.
    pub(crate) fn advertisement(&self) -> &conduit_core::HostAdvertisement {
        self.part.advertisement()
    }
    pub(crate) fn rendezvous(&self) -> &conduit_body::SpawnRendezvousDescriptor {
        self.part
            .rendezvous()
            .expect("constructor required retained route")
    }
    pub(crate) fn advertise(&mut self) -> Result<Vec<u8>, String> {
        if self.phase != Phase::Ready {
            return Err("Return advertisement out of order".into());
        }
        let result = encode(&In::ReturnAdvertise {
            protocol: PROTOCOL,
            credential: self.part.credential().clone(),
            advertisement: self.part.advertisement().clone(),
        });
        self.phase = if result.is_ok() {
            Phase::Challenge
        } else {
            Phase::Failed
        };
        result
    }
    /// One canonical challenge, bounded before decoding; refusal is terminal.
    pub(crate) fn challenge(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        if self.phase != Phase::Challenge {
            return Err("Return challenge out of order".into());
        }
        self.phase = Phase::Failed;
        let Out::ReturnChallenge {
            protocol: PROTOCOL,
            challenge,
        } = decode(bytes)?
        else {
            return Err("expected canonical Return challenge".into());
        };
        let proof = self.part.prove(&challenge)?;
        let bytes = encode(&In::ReturnProof {
            protocol: PROTOCOL,
            admission_id: proof.admission_id,
            body_id: proof.body_id,
            part_id: proof.part_id,
            host_id: proof.host_id,
            boot_id: proof.boot_id,
            nonce: proof.nonce.to_vec(),
            signature: proof.signature.to_vec(),
        })?;
        self.phase = Phase::Admission;
        Ok(bytes)
    }
    /// Correlate the response only. Authentication belongs to the protected
    /// connector; this method neither persists nor grants anything.
    pub(crate) fn admitted(&mut self, bytes: &[u8]) -> Result<MembershipCredential, String> {
        if self.phase != Phase::Admission {
            return Err("Return admission out of order".into());
        }
        self.phase = Phase::Failed;
        let Out::Admitted {
            protocol: PROTOCOL,
            credential,
            signed_membership_receipt,
        } = decode(bytes)?
        else {
            return Err("expected canonical Return admission".into());
        };
        // A Return does not silently convert the retained raw TLS receipt into
        // Root-signed provenance. A signed receipt needs its own verifier.
        if signed_membership_receipt.is_some() {
            return Err("unexpected Return receipt provenance".into());
        }
        let old = self.part.credential();
        let ad = self.part.advertisement();
        if credential.body_id != old.body_id
            || credential.part_id != old.part_id
            || credential.host_id != ad.host_id
            || credential.boot_id != ad.boot_id
        {
            return Err("Return admission changed exact Part or current Boot".into());
        }
        self.phase = Phase::Finished;
        Ok(credential)
    }
}
/// One read-only encounter on the exact already selected protected session.
/// The caller retains the returned credential until reviewed durable renewal;
/// this function cannot persist membership or claim a remote Mask route.
pub(crate) fn snapshot<C: conduit_protected_line::ProtectedFrameCarrier>(
    mut client: NativeReturn,
    verified: &crate::durable_host_control::native_snapshot::VerifiedSelection,
    mut line: super::ProtectedDocumentLine<C>,
) -> Result<
    (
        MembershipCredential,
        conduit_face::OwnerFaceSnapshotResponse,
    ),
    String,
> {
    use conduit_protected_line::{Role, SessionDisposition};
    let selected = verified.binding();
    let authorization = verified.authorization();
    let maximum_millis = authorization.maximum_millis;
    let result = (|| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "native snapshot clock precedes epoch")?
            .as_millis();
        client
            .rendezvous()
            .validate(
                now.try_into()
                    .map_err(|_| "native snapshot clock overflow")?,
            )
            .map_err(|_| "native snapshot retained route expired or refused")?;
        if authorization.body_id != client.part.credential().body_id
            || !client.rendezvous().candidates.contains(verified.route())
            || selected.responder.host_id != authorization.owner_host_id.as_str()
            || selected.responder.boot_id != authorization.owner_boot_id.as_str()
        {
            return Err("native snapshot verified owner or retained route mismatch".into());
        }
        let evidence = line.evidence();
        let ad = client.advertisement();
        if evidence.role != Role::Initiator
            || evidence.disposition != SessionDisposition::Open
            || evidence.binding != selected
            || selected.initiator.host_id != ad.host_id.as_str()
            || selected.initiator.boot_id != ad.boot_id.as_str()
            || !(1000..=60_000).contains(&maximum_millis)
        {
            return Err("native snapshot selected protected initiator mismatch".into());
        }
        line.restrict_deadline(
            std::time::Instant::now() + std::time::Duration::from_millis(maximum_millis),
        )?;
        line.send(&client.advertise()?)?;
        let proof = client.challenge(&line.receive()?)?;
        line.send(&proof)?;
        let credential = client.admitted(&line.receive()?)?;
        let request = conduit_face::OwnerFaceSnapshotRequest {
            schema: conduit_face::OWNER_FACE_REQUEST_SCHEMA.into(),
            credential_id: credential.credential_id.as_str().into(),
            body_id: credential.body_id.clone(),
            part_id: credential.part_id.clone(),
            host_id: credential.host_id.clone(),
            boot_id: credential.boot_id.clone(),
            last_seen_revision: None,
            last_seen_identity: None,
        };
        line.send(&encode(&In::FaceSnapshotRequest {
            protocol: PROTOCOL,
            request,
        })?)?;
        let Out::FaceSnapshotResponse {
            protocol: PROTOCOL,
            response,
        } = decode(&line.receive()?)?
        else {
            return Err("native snapshot expected canonical Face response".into());
        };
        if let conduit_face::OwnerFaceSnapshotResponse::Snapshot {
            schema,
            face,
            interactions_admitted,
            route,
        } = &response
        {
            if schema != conduit_face::OWNER_FACE_RESPONSE_SCHEMA
                || face.basis.body_id.as_ref() != Some(&credential.body_id)
                || face.validate().is_err()
                || *interactions_admitted
                || route.is_some()
            {
                return Err("read-only snapshot cannot grant Show or input".into());
            }
        }
        Ok((credential, response))
    })();
    let close = line.close();
    match (result, close) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (result, Err(close)) => Err(format!(
            "{};retirement:{close}",
            result.err().unwrap_or_else(|| "snapshot received".into())
        )),
    }
}

fn encode(value: &In) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "Return request encoding")?;
    if bytes.len() > LIMIT {
        return Err("Return request exceeds canonical bound".into());
    }
    Ok(bytes)
}
fn decode(bytes: &[u8]) -> Result<Out, String> {
    if bytes.is_empty() || bytes.len() > LIMIT {
        return Err("Return response exceeds canonical bound".into());
    }
    serde_json::from_slice(bytes).map_err(|_| "Return response encoding".into())
}
#[cfg(test)]
#[path = "native_return_tests.rs"]
mod tests;
