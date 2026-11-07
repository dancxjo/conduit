//! Existing signed preparation command, retained before its delivery ACK.
use crate::transport::DocumentTransport;
use conduit_body::{
    HostCommandInput, HostCommandReceiveBounds, HostPreparationDelegation,
    IssuedHostCommandChallenge, NativePreparationContext, HOST_PREPARED_WORKSET_CONTRACT,
};
use conduit_wire::BorrowedBoundedDocumentAssembly;
use sha2::{Digest, Sha256};
pub const LIFETIME_MILLIS: u32 = 10_000;
const MAXIMUM_CHUNKS: usize = 64;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Scope,
    Bounds,
    Encoding,
    Carrier,
    Document,
    Authentication,
    Policy,
    Retention,
}
/// Caller wraps the complete future in LIFETIME_MILLIS using its actual local
/// timer, including challenge, chunks and signature. This function does not
/// prepare, reserve, bind, start or cancel a kernel.
pub async fn receive<T: DocumentTransport, const N: usize>(
    link: &mut T,
    workspace: &mut HostCommandInput<N>,
    policy: &HostPreparationDelegation,
    context: NativePreparationContext<'_>,
    nonce: [u8; 32],
    now: impl Fn() -> u64,
) -> Result<(), Refusal> {
    if workspace.is_retained() || !policy.has_finite_bounds() {
        return Err(Refusal::Scope);
    }
    let ceiling =
        N.min(usize::try_from(policy.maximum_assignment_bytes).map_err(|_| Refusal::Bounds)?);
    let stamp = {
        let input = workspace.writable().map_err(|_| Refusal::Retention)?;
        let slot = IssuedHostCommandChallenge::issue(
            context.member,
            context.actual,
            &context.bootstrap.owner,
            HostCommandReceiveBounds {
                payload_contract: HOST_PREPARED_WORKSET_CONTRACT,
                maximum_payload_bytes: ceiling as u32,
                lifetime_millis: LIFETIME_MILLIS,
            },
            nonce,
            context.now_millis,
        )
        .map_err(|_| Refusal::Scope)?;
        let n =
            serde_json_core::to_slice(slot.challenge(), input).map_err(|_| Refusal::Encoding)?;
        link.send(&input[..n]).await.map_err(|_| Refusal::Carrier)?;
        let mut document =
            BorrowedBoundedDocumentAssembly::new(&mut input[..ceiling], MAXIMUM_CHUNKS);
        link.receive_document(&mut document)
            .await
            .map_err(|_| Refusal::Carrier)?;
        let (payload, digest) = document.completed().ok_or(Refusal::Document)?;
        if Sha256::digest(payload).as_slice() != digest {
            return Err(Refusal::Document);
        }
        let mut signature = [0; 64];
        let signature = link
            .receive(&mut signature)
            .await
            .map_err(|_| Refusal::Carrier)?;
        let checked_now = now();
        let command = slot
            .authenticate(payload, signature, checked_now)
            .map_err(|_| Refusal::Authentication)?;
        conduit_body::check_host_workset_preparation_delegation(
            Some(policy),
            NativePreparationContext {
                now_millis: checked_now,
                ..context
            },
            &command,
        )
        .map_err(|_| Refusal::Policy)?;
        command
            .into_retained_stamp()
            .map_err(|_| Refusal::Retention)?
    };
    workspace.retain(stamp).map_err(|_| Refusal::Retention)?;
    // Persistent Root owns the bytes even if this ACK is lost. Losing carrier
    // custody must never withdraw an admitted/started Play.
    report(link, workspace).await
}
pub async fn report<T: DocumentTransport, const N: usize>(
    link: &mut T,
    workspace: &HostCommandInput<N>,
) -> Result<(), Refusal> {
    let digest = workspace.payload_digest().ok_or(Refusal::Retention)?;
    let mut text = [0u8; 64];
    const HEX: &[u8] = b"0123456789abcdef";
    for (i, b) in digest.iter().enumerate() {
        text[i * 2] = HEX[(b >> 4) as usize];
        text[i * 2 + 1] = HEX[(b & 15) as usize];
    }
    #[derive(serde::Serialize)]
    struct Report<'a> {
        phase: &'static str,
        custody: &'static str,
        payload_sha256: &'a str,
        resources: &'static str,
        start: &'static str,
    }
    let mut output = [0; 192];
    let n = serde_json_core::to_slice(
        &Report {
            phase: "input-retained",
            custody: "ram",
            resources: "not-admitted",
            start: "not-started",
            payload_sha256: core::str::from_utf8(&text).map_err(|_| Refusal::Encoding)?,
        },
        &mut output,
    )
    .map_err(|_| Refusal::Encoding)?;
    link.send(&output[..n]).await.map_err(|_| Refusal::Carrier)
}
