use super::{basis::finite, Basis, Observation, Refusal};
use crate::transport::DocumentTransport;
use conduit_body::{
    MemberReturnImageAcknowledgementSeed, MemberReturnProofDocument, MembershipCredential,
    PartReturnChallenge, PartReturnProof, SignedPortableAdmissionReceipt,
};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::de::DeserializeSeed;
use sha2::{Digest, Sha256};
/// Carry a challenge already issued by the original owner, then check the member
/// proof. Its expiry uses the owner's clock; this helper issues no admission.
/// The durable owner rechecks time at completion after this bounded exchange.
pub async fn prove<T: DocumentTransport>(
    link: &mut T,
    basis: Basis<'_>,
    observation: &Observation,
    challenge: &PartReturnChallenge,
    now_millis: u64,
    frame: &mut [u8],
) -> Result<PartReturnProof, Refusal> {
    basis.check(&observation.presentation)?;
    let maximum = frame.len().min(crate::transport::LIMIT);
    let frame = &mut frame[..maximum];
    let actual = observation.advertisement();
    if challenge.body_id != basis.credential.body_id
        || challenge.part_id != basis.credential.part_id
        || challenge.host_id != actual.host_id
        || challenge.boot_id != actual.boot_id
        || challenge.offer_generation != actual.offer_generation
        || !finite(challenge.admission_id.as_str())
        || challenge.nonce == [0; 32]
        || now_millis < challenge.issued_at_millis
        || now_millis >= challenge.expires_at_millis
    {
        return Err(Refusal::Challenge);
    }
    let n = serde_json_core::to_slice(challenge, frame).map_err(|_| Refusal::Encoding)?;
    link.send(&frame[..n])
        .await
        .map_err(|_| Refusal::Transport)?;
    let bytes = link.receive(frame).await.map_err(|_| Refusal::Transport)?;
    let (document, used) = serde_json_core::from_slice::<MemberReturnProofDocument>(bytes)
        .map_err(|_| Refusal::Encoding)?;
    if used != bytes.len() {
        return Err(Refusal::Encoding);
    }
    let proof = document.decode().map_err(|_| Refusal::Encoding)?;
    if proof.admission_id != challenge.admission_id
        || proof.body_id != challenge.body_id
        || proof.part_id != challenge.part_id
        || proof.host_id != challenge.host_id
        || proof.boot_id != challenge.boot_id
        || proof.nonce != challenge.nonce
    {
        return Err(Refusal::Challenge);
    }
    VerifyingKey::from_bytes(basis.member_verifying_key)
        .map_err(|_| Refusal::Scope)?
        .verify_strict(
            &challenge.signing_transcript(),
            &Signature::from_bytes(&proof.signature),
        )
        .map_err(|_| Refusal::Signature)?;
    Ok(proof)
}
/// Deliver an existing exact owner-attested acknowledgement. Before calling,
/// the product must complete and retain its durable Core return and receipt.
/// This verifies bytes/relationship only and cannot establish that commitment.
/// A transport error leaves delivery uncertain and does not undo a Core commit.
pub async fn deliver<T: DocumentTransport>(
    link: &mut T,
    basis: Basis<'_>,
    observation: &Observation,
    signed: &SignedPortableAdmissionReceipt,
    frame: &mut [u8],
) -> Result<MembershipCredential, Refusal> {
    basis.check(&observation.presentation)?;
    let maximum = frame.len().min(crate::transport::LIMIT);
    let frame = &mut frame[..maximum];
    if frame.len() <= conduit_wire::BOUNDED_DOCUMENT_HEADER_BYTES
        || signed.receipt_json.is_empty()
        || signed.receipt_json.len() > 16 * 1024
    {
        return Err(Refusal::Bounds);
    }
    let metadata = signed.transport_metadata().map_err(|_| Refusal::Receipt)?;
    let raw = signed.receipt_json.as_bytes();
    metadata
        .verify(raw, basis.owner, basis.member_verifying_key)
        .map_err(|_| Refusal::Receipt)?;
    let mut decoder = serde_json_core::de::Deserializer::new(raw, None);
    let credential = MemberReturnImageAcknowledgementSeed {
        retained: basis.credential,
        actual: observation.projection().borrowed(),
    }
    .deserialize(&mut decoder)
    .map_err(|_| Refusal::Receipt)?;
    if decoder.end().map_err(|_| Refusal::Encoding)? != raw.len() {
        return Err(Refusal::Encoding);
    }
    let length = serde_json_core::to_slice(&metadata, frame).map_err(|_| Refusal::Encoding)?;
    link.send(&frame[..length])
        .await
        .map_err(|_| Refusal::Transport)?;
    let digest = Sha256::digest(raw).into();
    let mut offset = 0;
    while offset < raw.len() {
        let (length, next) =
            conduit_wire::encode_bounded_document_chunk(frame, raw, offset, &digest)
                .map_err(|_| Refusal::Encoding)?;
        link.send(&frame[..length])
            .await
            .map_err(|_| Refusal::Transport)?;
        offset = next;
    }
    Ok(credential)
}
