use super::{Malformed, codec::*};
use conduit_kernel::{
    scheduler::{CanonicalValue, SchedulerError},
    *,
};
pub(super) fn failure_write(w: &mut Writer<'_>, value: FailureCode) -> Result<(), Malformed> {
    w.u8(match value {
        FailureCode::InvalidInput => 0,
        FailureCode::InvalidPort => 1,
        FailureCode::InvalidLifecycle => 2,
        FailureCode::StorageExhausted => 3,
        FailureCode::StateCapacityExhausted => 4,
        FailureCode::WorkBudgetExhausted => 5,
        FailureCode::IdentityCapacityExhausted => 6,
        FailureCode::HostCallDenied => 7,
        FailureCode::HostCallFailed => 8,
        FailureCode::Cancelled => 9,
    })
}
pub(super) fn failure_read(r: &mut Reader<'_>) -> Result<FailureCode, Malformed> {
    Ok(match r.u8()? {
        0 => FailureCode::InvalidInput,
        1 => FailureCode::InvalidPort,
        2 => FailureCode::InvalidLifecycle,
        3 => FailureCode::StorageExhausted,
        4 => FailureCode::StateCapacityExhausted,
        5 => FailureCode::WorkBudgetExhausted,
        6 => FailureCode::IdentityCapacityExhausted,
        7 => FailureCode::HostCallDenied,
        8 => FailureCode::HostCallFailed,
        9 => FailureCode::Cancelled,
        _ => return Err(Malformed),
    })
}
pub(super) fn storage_write(w: &mut Writer<'_>, value: StorageError) -> Result<(), Malformed> {
    w.u8(match value {
        StorageError::InvalidBudget => 0,
        StorageError::ItemCapacityExceeded => 1,
        StorageError::ByteCapacityExceeded => 2,
        StorageError::ValueTooLarge => 3,
        StorageError::StaleReference => 4,
        StorageError::ReferenceOverflow => 5,
        StorageError::GenerationExhausted => 6,
        StorageError::RepresentationMismatch => 7,
    })
}
pub(super) fn storage_read(r: &mut Reader<'_>) -> Result<StorageError, Malformed> {
    Ok(match r.u8()? {
        0 => StorageError::InvalidBudget,
        1 => StorageError::ItemCapacityExceeded,
        2 => StorageError::ByteCapacityExceeded,
        3 => StorageError::ValueTooLarge,
        4 => StorageError::StaleReference,
        5 => StorageError::ReferenceOverflow,
        6 => StorageError::GenerationExhausted,
        7 => StorageError::RepresentationMismatch,
        _ => return Err(Malformed),
    })
}
pub(super) fn sign_write(w: &mut Writer<'_>, value: SignError) -> Result<(), Malformed> {
    w.u8(match value {
        SignError::InvalidBudget => 0,
        SignError::ItemCapacityExceeded => 1,
        SignError::ByteCapacityExceeded => 2,
        SignError::RemoteItemCapacityExceeded => 3,
        SignError::RemoteByteCapacityExceeded => 4,
        SignError::SequenceOverflow => 5,
    })
}
pub(super) fn sign_read(r: &mut Reader<'_>) -> Result<SignError, Malformed> {
    Ok(match r.u8()? {
        0 => SignError::InvalidBudget,
        1 => SignError::ItemCapacityExceeded,
        2 => SignError::ByteCapacityExceeded,
        3 => SignError::RemoteItemCapacityExceeded,
        4 => SignError::RemoteByteCapacityExceeded,
        5 => SignError::SequenceOverflow,
        _ => return Err(Malformed),
    })
}
pub(super) fn routing_write(w: &mut Writer<'_>, value: ProtocolError) -> Result<(), Malformed> {
    w.u8(match value {
        ProtocolError::HostCallInputExceeded => 0,
        ProtocolError::HostCallMissing => 1,
        ProtocolError::HostCallTableInvalid => 2,
        ProtocolError::HostCallTableSealed => 3,
        ProtocolError::RouteTableInvalid => 4,
        ProtocolError::RouteTableSealed => 5,
        ProtocolError::RouteMissing => 6,
        ProtocolError::RouteTargetExceeded => 7,
    })
}
pub(super) fn routing_read(r: &mut Reader<'_>) -> Result<ProtocolError, Malformed> {
    Ok(match r.u8()? {
        0 => ProtocolError::HostCallInputExceeded,
        1 => ProtocolError::HostCallMissing,
        2 => ProtocolError::HostCallTableInvalid,
        3 => ProtocolError::HostCallTableSealed,
        4 => ProtocolError::RouteTableInvalid,
        5 => ProtocolError::RouteTableSealed,
        6 => ProtocolError::RouteMissing,
        7 => ProtocolError::RouteTargetExceeded,
        _ => return Err(Malformed),
    })
}
pub(super) fn write(w: &mut Writer<'_>, error: SchedulerError) -> Result<(), Malformed> {
    match error {
        SchedulerError::InvalidPlan => w.u8(0),
        SchedulerError::InvalidActiveCapacity => w.u8(1),
        SchedulerError::InvalidPortAccess => w.u8(2),
        SchedulerError::InvalidHostCallAccess => w.u8(3),
        SchedulerError::HostCallCancellationRejected => w.u8(4),
        SchedulerError::HostCallCancellationDuplicate => w.u8(5),
        SchedulerError::HostCallCancellationUndispatched => w.u8(6),
        SchedulerError::OutputBlocked => w.u8(7),
        SchedulerError::QueueCapacityExceeded => w.u8(8),
        SchedulerError::QueueByteCapacityExceeded => w.u8(9),
        SchedulerError::SemanticValueBoundExceeded => w.u8(10),
        SchedulerError::StepFuelExceeded => w.u8(11),
        SchedulerError::FalseProgress => w.u8(12),
        SchedulerError::DecisionLimitExceeded => w.u8(13),
        SchedulerError::HostCallCapacityExceeded => w.u8(14),
        SchedulerError::HostCallRequestDuplicate => w.u8(15),
        SchedulerError::HostCallCompletionRejected => w.u8(16),
        SchedulerError::HostCallOutputExceeded => w.u8(17),
        SchedulerError::InvalidRemoteCordAccess => w.u8(18),
        SchedulerError::RemoteSequenceRejected => w.u8(19),
        SchedulerError::RemoteDeliveryRejected => w.u8(20),
        SchedulerError::ValueOwnershipViolation => w.u8(21),
        SchedulerError::Cancelled => w.u8(22),
        SchedulerError::DebugSuspended => w.u8(23),
        SchedulerError::BackFailed(f) => {
            w.u8(24)?;
            failure_write(w, f.code)?;
            w.u16(f.detail)
        }
        SchedulerError::SemanticAbnormal {
            node,
            port,
            terminal,
        } => {
            w.u8(25)?;
            w.u16(node.0)?;
            w.u16(port.0)?;
            w.u8(terminal.as_slice().len() as u8)?;
            w.raw(terminal.as_slice())
        }
        SchedulerError::Storage(e) => {
            w.u8(26)?;
            storage_write(w, e)
        }
        SchedulerError::Sign(e) => {
            w.u8(27)?;
            sign_write(w, e)
        }
        SchedulerError::Routing(e) => {
            w.u8(28)?;
            routing_write(w, e)
        }
    }
}
pub(super) fn read(r: &mut Reader<'_>) -> Result<SchedulerError, Malformed> {
    Ok(match r.u8()? {
        0 => SchedulerError::InvalidPlan,
        1 => SchedulerError::InvalidActiveCapacity,
        2 => SchedulerError::InvalidPortAccess,
        3 => SchedulerError::InvalidHostCallAccess,
        4 => SchedulerError::HostCallCancellationRejected,
        5 => SchedulerError::HostCallCancellationDuplicate,
        6 => SchedulerError::HostCallCancellationUndispatched,
        7 => SchedulerError::OutputBlocked,
        8 => SchedulerError::QueueCapacityExceeded,
        9 => SchedulerError::QueueByteCapacityExceeded,
        10 => SchedulerError::SemanticValueBoundExceeded,
        11 => SchedulerError::StepFuelExceeded,
        12 => SchedulerError::FalseProgress,
        13 => SchedulerError::DecisionLimitExceeded,
        14 => SchedulerError::HostCallCapacityExceeded,
        15 => SchedulerError::HostCallRequestDuplicate,
        16 => SchedulerError::HostCallCompletionRejected,
        17 => SchedulerError::HostCallOutputExceeded,
        18 => SchedulerError::InvalidRemoteCordAccess,
        19 => SchedulerError::RemoteSequenceRejected,
        20 => SchedulerError::RemoteDeliveryRejected,
        21 => SchedulerError::ValueOwnershipViolation,
        22 => SchedulerError::Cancelled,
        23 => SchedulerError::DebugSuspended,
        24 => SchedulerError::BackFailed(Failure {
            code: failure_read(r)?,
            detail: r.u16()?,
        }),
        25 => {
            let node = NodeId(r.u16()?);
            let port = PortId(r.u16()?);
            let len = usize::from(r.u8()?);
            SchedulerError::SemanticAbnormal {
                node,
                port,
                terminal: CanonicalValue::new(r.raw(len)?).map_err(|_| Malformed)?,
            }
        }
        26 => SchedulerError::Storage(storage_read(r)?),
        27 => SchedulerError::Sign(sign_read(r)?),
        28 => SchedulerError::Routing(routing_read(r)?),
        _ => return Err(Malformed),
    })
}
