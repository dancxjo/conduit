//! Shared mandatory Sign retention classification and capacity admission.
use crate::{KernelEvent, KernelEventKind, SignError, SignRetentionGap};
use core::mem::size_of;

#[inline]
pub(super) fn transient_sign(kind: KernelEventKind) -> bool {
    !matches!(
        kind,
        KernelEventKind::StepFuelExceeded
            | KernelEventKind::RemoteValueOffered
            | KernelEventKind::RemoteValueAccepted
            | KernelEventKind::RemoteValueDelivered
            | KernelEventKind::RemoteOutputClosed
            | KernelEventKind::RemoteInputAdmitted
            | KernelEventKind::RemoteInputClosed
            | KernelEventKind::BackCompleted
            | KernelEventKind::BackFailed
            | KernelEventKind::SemanticAbnormal
            | KernelEventKind::SemanticAbnormalRecovered
            | KernelEventKind::RunCancelled
    )
}

#[inline]
pub(super) fn ensure_retained_sign_capacity(
    events: impl Iterator<Item = KernelEvent>,
    item_capacity: u16,
    byte_capacity: u32,
    occupied: u16,
    used_bytes: u32,
    additional: u16,
) -> Result<(), SignError> {
    // Physical headroom is sufficient without evicting or inspecting history.
    // Both callers maintain exact occupied/used counts. If either physical
    // budget is tight, retain the existing transient-aware admission rule.
    let item_headroom = occupied
        .checked_add(additional)
        .is_some_and(|required| required <= item_capacity);
    let byte_headroom = u32::try_from(size_of::<KernelEvent>())
        .ok()
        .and_then(|charge| charge.checked_mul(u32::from(additional)))
        .and_then(|charge| used_bytes.checked_add(charge))
        .is_some_and(|required| required <= byte_capacity);
    if item_headroom && byte_headroom {
        return Ok(());
    }
    let retained = events.filter(|event| !transient_sign(event.kind)).count();
    if retained
        .checked_add(usize::from(additional))
        .filter(|required| *required <= usize::from(item_capacity))
        .is_none()
    {
        return Err(SignError::ItemCapacityExceeded);
    }
    let charge = u32::try_from(size_of::<KernelEvent>()).map_err(|_| SignError::InvalidBudget)?;
    let required = u32::try_from(retained)
        .map_err(|_| SignError::InvalidBudget)?
        .checked_add(u32::from(additional))
        .and_then(|items| items.checked_mul(charge))
        .ok_or(SignError::InvalidBudget)?;
    if required > byte_capacity {
        return Err(SignError::ByteCapacityExceeded);
    }
    Ok(())
}

pub(super) fn extend_sign_gap(
    gap: &mut Option<SignRetentionGap>,
    sequence: u32,
) -> Result<(), SignError> {
    *gap = Some(match *gap {
        None => SignRetentionGap {
            first_sequence: sequence,
            last_sequence: sequence,
            entries: 1,
        },
        Some(existing) => SignRetentionGap {
            first_sequence: existing.first_sequence.min(sequence),
            last_sequence: existing.last_sequence.max(sequence),
            entries: existing
                .entries
                .checked_add(1)
                .ok_or(SignError::SequenceOverflow)?,
        },
    });
    Ok(())
}

#[cfg(test)]
mod tests;
