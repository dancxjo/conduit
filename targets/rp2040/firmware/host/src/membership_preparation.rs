//! Seed-selected command ingress, not an execution or resource-grant shortcut.
use super::*;
use conduit_body::{
    HostCommandReceiveBounds, IssuedHostCommandChallenge, NativePreparationContext,
    HOST_PREPARED_WORKSET_CONTRACT,
};
use conduit_wire::BorrowedBoundedDocumentAssembly;
use embassy_time::{with_deadline, Duration, Instant};

const LIFETIME_MILLIS: u32 = 10_000;
const MAXIMUM_CHUNKS: usize = 64;

impl Membership {
    pub(super) async fn serve_preparation(
        &mut self,
        link: &mut Link,
        input: &mut [u8; LIMIT],
    ) -> Result<(), ()> {
        // No signature, retained owner key or membership silently enables this.
        let Some(policy) = crate::config::PREPARATION_DELEGATION.as_ref() else {
            return link
                .send(br#"{"phase":"refused","reason":"missing-preparation-delegation"}"#)
                .await;
        };
        // Only this Boot's acknowledged membership may receive commands.
        let Some(member) = self.receipt.as_ref() else {
            return link
                .send(br#"{"phase":"refused","reason":"membership-not-current"}"#)
                .await;
        };
        let ceiling = input
            .len()
            .min(usize::try_from(policy.maximum_assignment_bytes).map_err(|_| ())?);
        if !policy.has_finite_bounds()
            || ceiling as u64 > crate::config::DIAGNOSTIC_HTTP_BUFFER_LIMIT
        {
            return Err(());
        }
        if !self.resources.admissions().is_empty() {
            return link
                .send(br#"{"phase":"refused","reason":"awaiting-native-retirement"}"#)
                .await;
        }
        // Reuse the caller's existing exclusive receive workspace. Do not allocate
        // a second document or infer a larger grant from the policy ceiling.
        self.freshness = self.freshness.checked_add(1).ok_or(())?;
        let mut rng = embassy_rp::clocks::RoscRng;
        let mut nonce = [0u8; 32];
        // The Boot-scoped counter prevents reuse even if the entropy source
        // repeats; the remaining bytes retain the physical source entropy.
        nonce[..8].copy_from_slice(&self.freshness.to_le_bytes());
        for chunk in nonce[8..].as_chunks_mut::<8>().0 {
            chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
        }
        let now = Instant::now();
        let slot = IssuedHostCommandChallenge::issue(
            member,
            self.resources.host(),
            &self.seed.owner,
            HostCommandReceiveBounds {
                payload_contract: HOST_PREPARED_WORKSET_CONTRACT,
                maximum_payload_bytes: ceiling as u32,
                lifetime_millis: LIFETIME_MILLIS,
            },
            nonce,
            now.as_millis(),
        )
        .map_err(|_| ())?;
        let n = serde_json_core::to_slice(slot.challenge(), input).map_err(|_| ())?;
        let deadline = now + Duration::from_millis(u64::from(LIFETIME_MILLIS));
        // One original deadline covers publication, all chunks and signature.
        let result = with_deadline(deadline, async {
            link.send(&input[..n]).await?;
            let mut document =
                BorrowedBoundedDocumentAssembly::new(&mut input[..ceiling], MAXIMUM_CHUNKS);
            link.receive_document(&mut document).await?;
            let (payload, digest) = document.completed().ok_or(())?;
            if Sha256::digest(payload).as_slice() != digest {
                return Err(());
            }
            let mut signature = [0u8; 64];
            let signature = link.receive(&mut signature).await?;
            let command = slot
                .authenticate(payload, signature, Instant::now().as_millis())
                .map_err(|_| ())?;
            // The workset receiver checks the installed policy and original
            // signed command before decoding any constituent fragment.

            match crate::resident_preparation::validate(
                Some(policy),
                NativePreparationContext {
                    bootstrap: &self.seed, member, actual: self.resources.host(),
                    image_id: crate::config::IMAGE_ID, configuration_id: crate::config::CONFIGURATION_ID,
                    now_millis: Instant::now().as_millis(),
                }, command,
            ) {
                Ok(()) => link.send(br#"{"phase":"validated-workset","profile":"resident-structured","start":"not-installed"}"#).await,
                Err(crate::resident_preparation::ValidationRefusal::Unavailable) => link.send(br#"{"phase":"attributed","preparation":"inventory-unavailable"}"#).await,
                Err(_) => Err(()),
            }
        })
        .await;
        input.fill(0);
        result.map_err(|_| ())?
    }
}
