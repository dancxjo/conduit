// Cold Root transactions using the original equipment and Issuer owners.
use super::*;
use conduit_mcu_host_runtime::{
    execution_authority::Operation, root::InstalledContinuationAcceptance,
};
#[allow(clippy::too_many_arguments)]
pub(super) async fn return_member(
    root: &mut Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    equipment: &mut crate::equipment::Equipment,
    issuers: &mut Issuers<W>,
    operations: &Operations<'_, W>,
    residents: &crate::resident::generated::Initialized,
    recording_operation: Option<&Operation<'_>>,
    arena_bytes: usize,
    link: &mut crate::transport::Link,
    response: &mut [u8; 4096],
) -> Result<(), conduit_mcu_host_runtime::root::Refusal> {
    let policy = Policy {
        preparation: crate::config::PREPARATION_DELEGATION.as_ref(),
        execution: crate::config::EXECUTION_DELEGATION.as_ref(),
        active_worksets: 0,
        maximum_retained_bytes: arena_bytes,
    };
    let authorized = root.return_context(now_ms()).and_then(|context| {
        issuers
            .authorize(
                context,
                policy,
                equipment.pulse.as_ref().expect("ready pulse"),
                equipment.display.as_ref().expect("ready display"),
                &equipment.renderer,
                &equipment.waits,
                operations,
                remaining,
            )
            .map_err(|_| conduit_mcu_host_runtime::root::Refusal::Scope)?;
        match (equipment.recording.selected(), recording_operation) {
            (false, None) => Ok(()),
            (true, Some(operation)) => equipment
                .recording
                .authorize(
                    context,
                    policy,
                    &mut issuers.recording,
                    operation,
                    remaining(),
                )
                .map_err(|_| conduit_mcu_host_runtime::root::Refusal::Scope),
            _ => Err(conduit_mcu_host_runtime::root::Refusal::Scope),
        }
    });
    match authorized {
        Err(error) => Err(error),
        Ok(()) => {
            let native = [
                crate::workset::native_images::PULSE_IMAGE,
                crate::workset::native_images::DISPLAY_IMAGE,
                crate::workset::native_images::WAIT_IMAGE,
                crate::workset::native_images::RENDERER_IMAGE,
            ];
            let resident_images = residents.offers();
            let images = resident_images
                .into_iter()
                .flatten()
                .chain(native.into_iter())
                .chain(crate::workset::native_images::APPEND_IMAGE)
                .chain(source::sampling::OFFER_IMAGES.iter().copied())
                .chain(source::mask::OFFER_IMAGES.iter().copied())
                .chain(source::mask::MASK_ROUTING_IMAGES.iter().copied())
                .chain(source::output::OFFER_IMAGES.iter().copied());
            root.return_member(
                link,
                images,
                || {
                    issuers
                        .copy_grants(remaining())
                        .map_err(|_| conduit_mcu_host_runtime::member_return::Refusal::Scope)
                },
                response,
            )
            .await
        }
    }
}
pub(super) async fn install_continuation(
    root: &mut Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    link: &mut crate::transport::Link,
    previous: Option<&InstalledContinuationAcceptance>,
) -> Result<(), conduit_mcu_host_runtime::root::Refusal> {
    if remaining() < 3 * 8192 {
        return Err(conduit_mcu_host_runtime::root::Refusal::Preparation);
    }
    let mut candidate = prefix_buffer()?;
    let mut selected = prefix_buffer()?;
    let mut scratch = prefix_buffer()?;
    match with_timeout(
        Duration::from_secs(10),
        root.receive_continuation_installation(
            link,
            crate::play::continuation::SOURCE_DIGEST,
            &crate::play::continuation::PINS,
            previous,
            candidate
                .as_mut_slice()
                .try_into()
                .expect("fixed candidate"),
            selected.as_mut_slice().try_into().expect("fixed selected"),
            scratch.as_mut_slice().try_into().expect("fixed scratch"),
        ),
    )
    .await
    {
        Ok(result) => result.map_err(|_| conduit_mcu_host_runtime::root::Refusal::Preparation),
        Err(_) => Err(conduit_mcu_host_runtime::root::Refusal::Preparation),
    }
}

pub(super) async fn inspect(
    root: &Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    link: &mut crate::transport::Link,
    last_refusal: Option<conduit_mcu_host_runtime::root::Refusal>,
) -> Result<(), conduit_mcu_host_runtime::root::Refusal> {
    use conduit_mcu_host_runtime::transport::MessageTransport;
    use core::fmt::Write;
    let mut status = heapless::String::<384>::new();
    let _ = write!(status,
                    "ROOT_INSPECTION@1 unsigned phase=not-started current_member={} retained_input={} heap_used={} last_refusal={:?}",
                    root.has_current_member(), root.retained_input(), allocator_used(), last_refusal);
    link.send(status.as_bytes())
        .await
        .map_err(|_| conduit_mcu_host_runtime::root::Refusal::Carrier)
}

pub(super) async fn prepare(
    root: &mut Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    link: &mut crate::transport::Link,
) -> Result<(), conduit_mcu_host_runtime::root::Refusal> {
    let challenge = crate::runtime::fresh_challenge();
    match with_timeout(
        Duration::from_secs(10),
        root.receive_preparation(link, challenge, now_ms),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            root.clear_unretained_input();
            Err(conduit_mcu_host_runtime::root::Refusal::Preparation)
        }
    }
}

fn prefix_buffer() -> Result<alloc::vec::Vec<u8>, conduit_mcu_host_runtime::root::Refusal> {
    let mut bytes = alloc::vec::Vec::new();
    bytes
        .try_reserve_exact(8192)
        .map_err(|_| conduit_mcu_host_runtime::root::Refusal::Preparation)?;
    bytes.resize(8192, 0);
    Ok(bytes)
}
