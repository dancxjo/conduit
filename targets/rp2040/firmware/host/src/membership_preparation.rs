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

impl MembershipState {
    /// Reclaim only an expired, unstarted preparation whose exact resource
    /// scope ended and whose initialized Backs are all available. An abandoned
    /// bound kernel leaves its Back withdrawn and therefore refuses this path.
    pub(super) fn reclaim_expired_preparation(
        &mut self,
        workspace: &mut conduit_body::HostCommandInput<LIMIT>,
    ) -> bool {
        if workspace
            .expires_at_millis()
            .is_none_or(|at| Instant::now().as_millis() < at)
            || !self.resources.admissions().is_empty()
        {
            return false;
        }
        let Ok(inventory) = crate::resident_preparation::inventory() else {
            return false;
        };
        if inventory.offers().iter().any(Option::is_none) {
            return false;
        }
        drop(inventory);
        workspace.retire();
        true
    }

    pub(super) async fn serve_preparation(
        &mut self,
        link: &mut Link,
        workspace: &mut conduit_body::HostCommandInput<LIMIT>,
    ) -> Result<(), ()> {
        self.reclaim_expired_preparation(workspace);
        if workspace.is_retained() {
            return report_retained_input(link, workspace).await;
        }
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
        let ceiling = LIMIT.min(usize::try_from(policy.maximum_assignment_bytes).map_err(|_| ())?);
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
        let deadline = now + Duration::from_millis(u64::from(LIFETIME_MILLIS));
        // One original deadline covers publication, all chunks and signature.
        let result = with_deadline(deadline, async {
            let stamp = {
                let input = workspace.writable().map_err(|_| ())?;
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
                // Carrier custody authenticates bytes only. Persistent Root work
                // performs profile validation, preparation and admission once,
                // outside this cancellable delivery/acknowledgment future.
                conduit_body::check_host_workset_preparation_delegation(
                    Some(policy),
                    NativePreparationContext {
                        bootstrap: &self.seed,
                        member,
                        actual: self.resources.host(),
                        image_id: crate::config::IMAGE_ID,
                        configuration_id: crate::config::CONFIGURATION_ID,
                        now_millis: Instant::now().as_millis(),
                    },
                    &command,
                )
                .map_err(|_| ())?;
                command.into_retained_stamp().map_err(|_| ())?
            };
            workspace.retain(stamp).map_err(|_| ())?;
            report_retained_input(link, workspace).await
        })
        .await;
        if !workspace.is_retained() {
            workspace.writable().map_err(|_| ())?.fill(0);
        }
        result.map_err(|_| ())?
    }
}

pub(super) async fn serve_retained_input(
    link: &mut Link,
    workspace: &conduit_body::HostCommandInput<LIMIT>,
) -> Result<(), ()> {
    // Retain the sealed document while allowing the existing build-bound
    // recovery exchange. This is control scratch, not another document grant.
    let mut request = [0u8; crate::bootsel::CONTROL_FRAME_BYTES];
    link.receive(&mut request).await?;
    report_retained_input(link, workspace).await
}

pub(super) async fn report_retained_input(
    link: &mut Link,
    workspace: &conduit_body::HostCommandInput<LIMIT>,
) -> Result<(), ()> {
    let mut output = [0u8; 192];
    let length = encode_retained_input(workspace, &mut output)?;
    link.send(&output[..length]).await
}

fn encode_retained_input(
    workspace: &conduit_body::HostCommandInput<LIMIT>,
    output: &mut [u8],
) -> Result<usize, ()> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut digest = [0u8; 64];
    for (index, byte) in workspace.payload_digest().ok_or(())?.iter().enumerate() {
        digest[index * 2] = HEX[usize::from(byte >> 4)];
        digest[index * 2 + 1] = HEX[usize::from(byte & 15)];
    }
    #[derive(serde::Serialize)]
    struct Status<'a> {
        phase: &'static str,
        custody: &'static str,
        payload_sha256: &'a str,
        resources: &'static str,
        start: &'static str,
    }
    serde_json_core::to_slice(
        &Status {
            phase: "input-retained",
            custody: "ram",
            payload_sha256: core::str::from_utf8(&digest).map_err(|_| ())?,
            resources: "not-admitted",
            start: "not-started",
        },
        output,
    )
    .map_err(|_| ())
}
