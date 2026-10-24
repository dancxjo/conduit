//! Cold reply framing; its serialization/signing scratch never accompanies
//! asynchronous native kernel preparation on the CPU stack.
use super::{owner, NativeRoot};
pub(super) struct Outcome {
    pub authenticated: bool,
    pub operation: Option<owner::Operation>,
    pub applied: bool,
    pub refusal: Option<&'static str>,
    pub inspect_command: Option<[u8; 32]>,
    pub acknowledgement: Option<crate::native_owner_receipt::Receipt>,
}
pub(super) struct Encoded {
    pub metadata_bytes: usize,
    pub publication_bytes: usize,
}
#[derive(serde::Serialize)]
struct Reply<'a> {
    event: &'static str,
    authenticated: bool,
    operation: Option<owner::Operation>,
    refusal: Option<&'static str>,
    sequence: u64,
    current_offer_generation: u64,
    setup_status: &'static str,
    next_play_sequence: Option<u64>,
    reservation: Option<&'a crate::native_owner_reservation::OwnerReservation>,
    control_applied: bool,
    body_installed: bool,
    native_plan_installed: bool,
    appliance_kernel_prepared: bool,
    execution_context_prepared: bool,
    execution_context_bytes: Option<usize>,
    worker: Option<crate::native_task::Status>,
    retired_native_attachments: Option<[u64; 2]>,
    kernel_additional_preparation_bound: Option<usize>,
    capability_possession_issued: bool,
    acknowledgement: Option<&'a crate::native_owner_receipt::Receipt>,
    acknowledgement_signature: Option<&'a [u8]>,
    publication_bytes: usize,
}

#[inline(never)]
pub(super) fn encode(
    root: &NativeRoot,
    buffer: &mut [u8],
    outcome: Outcome,
) -> Result<Encoded, ()> {
    let Outcome {
        authenticated,
        operation,
        applied,
        refusal,
        inspect_command,
        mut acknowledgement,
    } = outcome;
    // Reuse the existing control buffer: one bounded committed value and one
    // bounded metadata frame, without adding a second 4-KiB task/stack carrier.
    let (publication, output) = buffer.split_at_mut(4096);
    let snapshot = if inspect_command.is_some() {
        root.copy_worker_publication(publication).map_err(|_| ())?
    } else {
        None
    };
    let publication_bytes = snapshot.map_or(0, |s| s.publication_bytes);
    if let (Some(command), Some(snapshot)) = (inspect_command, snapshot) {
        use sha2::{Digest, Sha256};
        acknowledgement = Some(crate::native_owner_receipt::Receipt {
            command,
            state: crate::native_owner_receipt::State::Observed,
            snapshot: Some(crate::native_owner_receipt::Snapshot {
                sequence: snapshot.publication_sequence,
                committed_millis: snapshot.committed_millis.ok_or(())?,
                observed_millis: embassy_time::Instant::now().as_millis(),
                channel: snapshot.publication_channel.ok_or(())?,
                bytes: publication_bytes.try_into().map_err(|_| ())?,
                sha256: Sha256::digest(&publication[..publication_bytes]).into(),
            }),
        });
    }
    let acknowledgement_signature = acknowledgement
        .as_ref()
        .map(|receipt| root.sign_owner_receipt(receipt));
    let reply = Reply {
        event: "native-owner-control",
        authenticated,
        operation,
        refusal,
        sequence: root.owner_control_sequence(),
        current_offer_generation: root.offer_scope().2 .0,
        setup_status: root.control_status(),
        next_play_sequence: root.next_play_sequence(),
        reservation: root.owner_reservation(),
        control_applied: applied,
        body_installed: false,
        native_plan_installed: false,
        appliance_kernel_prepared: root.appliance_kernel_prepared(),
        execution_context_prepared: root.execution_context_prepared(),
        execution_context_bytes: root.execution_context_bytes(),
        worker: snapshot.or_else(|| root.worker_status()),
        retired_native_attachments: root.retired_native_attachments(),
        kernel_additional_preparation_bound: crate::native_kernel::requirements()
            .ok()
            .map(|(bytes, _)| bytes),
        capability_possession_issued: root.native_possession_prepared(),
        publication_bytes,
        acknowledgement: acknowledgement.as_ref(),
        acknowledgement_signature: acknowledgement_signature
            .as_ref()
            .map(|value| value.as_slice()),
    };
    let metadata_bytes = serde_json_core::to_slice(&reply, output).map_err(|_| ())?;
    Ok(Encoded {
        metadata_bytes,
        publication_bytes,
    })
}
