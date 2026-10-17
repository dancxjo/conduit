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
    retained: u16,
    item_capacity: u16,
    byte_capacity: u32,
    additional: u16,
) -> Result<(), SignError> {
    if usize::from(retained)
        .checked_add(usize::from(additional))
        .filter(|required| *required <= usize::from(item_capacity))
        .is_none()
    {
        return Err(SignError::ItemCapacityExceeded);
    }
    let charge = u32::try_from(size_of::<KernelEvent>()).map_err(|_| SignError::InvalidBudget)?;
    let required = u32::from(retained)
        .checked_add(u32::from(additional))
        .and_then(|items| items.checked_mul(charge))
        .ok_or(SignError::InvalidBudget)?;
    if required > byte_capacity {
        return Err(SignError::ByteCapacityExceeded);
    }
    Ok(())
}

/// Plan an eviction without mutating history; append may still refuse later.
pub(super) fn prepare_transient_eviction(
    entries: &[Option<KernelEvent>],
    mut gap: Option<SignRetentionGap>,
) -> Result<(usize, Option<SignRetentionGap>), SignError> {
    let index = entries
        .iter()
        .position(|event| event.is_some_and(|event| transient_sign(event.kind)))
        .ok_or(SignError::ItemCapacityExceeded)?;
    let event = entries[index].ok_or(SignError::ItemCapacityExceeded)?;
    extend_sign_gap(&mut gap, event.sequence)?;
    Ok((index, gap))
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
