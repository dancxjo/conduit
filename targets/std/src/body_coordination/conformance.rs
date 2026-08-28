use super::endpoint::CoordinationEndpoint;
use super::line::{classify_wire_error, CoordinationLineError};
use super::receipt::CoordinationFailure;
use conduit_plan_lowering::lowering::RemoteCordDirection;
use conduit_wire::{SessionBinding, SessionMessage, SessionTerminalDisposition};

pub fn run_in_process(
    forebrain: &mut CoordinationEndpoint,
    motherbrain: &mut CoordinationEndpoint,
) -> Result<(), CoordinationLineError> {
    activate_direction(forebrain, motherbrain)?;
    activate_direction(motherbrain, forebrain)?;
    transfer(forebrain, motherbrain)?;
    transfer(motherbrain, forebrain)?;
    close_direction(forebrain, motherbrain)?;
    close_direction(motherbrain, forebrain)?;
    forebrain
        .finish()
        .map_err(CoordinationLineError::Contract)?;
    motherbrain
        .finish()
        .map_err(CoordinationLineError::Contract)?;
    Ok(())
}

fn activate_direction(
    source: &mut CoordinationEndpoint,
    sink: &mut CoordinationEndpoint,
) -> Result<(), CoordinationLineError> {
    let binding = source.binding(RemoteCordDirection::Egress).clone();
    if sink.binding(RemoteCordDirection::Ingress) != &binding {
        return Err(CoordinationLineError::Classified(
            CoordinationFailure::WrongBoot,
        ));
    }
    let hello = binding.hello_frame();
    source
        .session_mut(RemoteCordDirection::Egress)
        .admit_outbound(hello)
        .map_err(classify_wire_error)?;
    sink.session_mut(RemoteCordDirection::Ingress)
        .admit_inbound(hello)
        .map_err(classify_wire_error)?;
    sink.session_mut(RemoteCordDirection::Ingress)
        .admit_outbound(hello)
        .map_err(classify_wire_error)?;
    source
        .session_mut(RemoteCordDirection::Egress)
        .admit_inbound(hello)
        .map_err(classify_wire_error)?;
    let ready = binding.frame(SessionMessage::Ready);
    source
        .session_mut(RemoteCordDirection::Egress)
        .admit_outbound(ready)
        .map_err(classify_wire_error)?;
    sink.session_mut(RemoteCordDirection::Ingress)
        .admit_inbound(ready)
        .map_err(classify_wire_error)?;
    sink.session_mut(RemoteCordDirection::Ingress)
        .admit_outbound(ready)
        .map_err(classify_wire_error)?;
    source
        .session_mut(RemoteCordDirection::Egress)
        .admit_inbound(ready)
        .map_err(classify_wire_error)?;
    if !source.session_mut(RemoteCordDirection::Egress).is_active()
        || !sink.session_mut(RemoteCordDirection::Ingress).is_active()
    {
        return Err(CoordinationLineError::Contract(
            "coordination session did not become Ready".into(),
        ));
    }
    Ok(())
}

fn transfer(
    source: &mut CoordinationEndpoint,
    sink: &mut CoordinationEndpoint,
) -> Result<(), CoordinationLineError> {
    let offer = source
        .next_offer()
        .map_err(CoordinationLineError::Contract)?;
    let binding = source.binding(RemoteCordDirection::Egress).clone();
    let offered = binding.frame(SessionMessage::Offered {
        sequence: offer.sequence,
        payload: &offer.bytes,
    });
    source
        .session_mut(RemoteCordDirection::Egress)
        .admit_outbound(offered)
        .map_err(classify_wire_error)?;
    sink.session_mut(RemoteCordDirection::Ingress)
        .admit_inbound(offered)
        .map_err(classify_wire_error)?;
    sink.admit_input(offer.sequence, &offer.bytes)
        .map_err(CoordinationLineError::Contract)?;
    lifecycle_receipt(
        source,
        sink,
        &binding,
        SessionMessage::Accepted {
            sequence: offer.sequence,
        },
    )?;
    source
        .accept_offer(offer.sequence)
        .map_err(CoordinationLineError::Contract)?;
    lifecycle_receipt(
        source,
        sink,
        &binding,
        SessionMessage::Delivered {
            sequence: offer.sequence,
        },
    )?;
    source
        .deliver_offer(offer.sequence)
        .map_err(CoordinationLineError::Contract)
}

fn lifecycle_receipt(
    source: &mut CoordinationEndpoint,
    sink: &mut CoordinationEndpoint,
    binding: &SessionBinding,
    message: SessionMessage<'_>,
) -> Result<(), CoordinationLineError> {
    let frame = binding.frame(message);
    sink.session_mut(RemoteCordDirection::Ingress)
        .admit_outbound(frame)
        .map_err(classify_wire_error)?;
    source
        .session_mut(RemoteCordDirection::Egress)
        .admit_inbound(frame)
        .map_err(classify_wire_error)
}

fn close_direction(
    source: &mut CoordinationEndpoint,
    sink: &mut CoordinationEndpoint,
) -> Result<(), CoordinationLineError> {
    let binding = source.binding(RemoteCordDirection::Egress).clone();
    let final_sequence = 1;
    let closed = binding.frame(SessionMessage::InputClosed { final_sequence });
    source
        .session_mut(RemoteCordDirection::Egress)
        .admit_outbound(closed)
        .map_err(classify_wire_error)?;
    sink.session_mut(RemoteCordDirection::Ingress)
        .admit_inbound(closed)
        .map_err(classify_wire_error)?;
    sink.close_input()
        .map_err(CoordinationLineError::Contract)?;
    let terminal = binding.frame(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Completed,
        final_sequence,
    });
    source
        .session_mut(RemoteCordDirection::Egress)
        .admit_outbound(terminal)
        .map_err(classify_wire_error)?;
    sink.session_mut(RemoteCordDirection::Ingress)
        .admit_inbound(terminal)
        .map_err(classify_wire_error)?;
    sink.session_mut(RemoteCordDirection::Ingress)
        .admit_outbound(terminal)
        .map_err(classify_wire_error)?;
    source
        .session_mut(RemoteCordDirection::Egress)
        .admit_inbound(terminal)
        .map_err(classify_wire_error)?;
    if !source
        .session_mut(RemoteCordDirection::Egress)
        .is_terminal()
        || !sink.session_mut(RemoteCordDirection::Ingress).is_terminal()
    {
        return Err(CoordinationLineError::Classified(
            CoordinationFailure::TerminalDisagreement,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_wire::WireError;

    #[test]
    fn wire_failures_remain_distinct_machine_outcomes() {
        assert_eq!(
            classify_wire_error(WireError::BootMismatch),
            CoordinationLineError::Classified(CoordinationFailure::WrongBoot)
        );
        assert_eq!(
            classify_wire_error(WireError::DuplicateFrame),
            CoordinationLineError::Classified(CoordinationFailure::Duplicate)
        );
        assert_eq!(
            classify_wire_error(WireError::OversizedPayload),
            CoordinationLineError::Classified(CoordinationFailure::Oversized)
        );
        assert_eq!(
            classify_wire_error(WireError::TrailingGarbage),
            CoordinationLineError::Classified(CoordinationFailure::Malformed)
        );
        assert_eq!(
            classify_wire_error(WireError::LateFrame),
            CoordinationLineError::Classified(CoordinationFailure::TerminalDisagreement)
        );
        assert_ne!(
            CoordinationFailure::LossBeforeAcceptance,
            CoordinationFailure::LossAfterAcceptance
        );
        assert_ne!(
            CoordinationFailure::Pressure,
            CoordinationFailure::PeerAbsent
        );
    }
}
