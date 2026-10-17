//! The source checker's exact Cord and Fore binding laws, shared with authoring.
use crate::CanonicalExpansionDiagnostic;
use conduit_core::{ConnectionTrack, KindId, PortDescriptor, PortTemporal};

/// Validate semantic contracts after endpoint direction and identity resolution.
/// Fore inputs act as sources inside their Plot; descriptor direction alone
/// therefore cannot establish a Cord's direction.
pub fn validate_connection_contract(
    source: &PortDescriptor,
    sink: &PortDescriptor,
    track: ConnectionTrack,
) -> Result<(), CanonicalExpansionDiagnostic> {
    let compatible = match track {
        ConnectionTrack::Payload => {
            let reactively_lifted_value = matches!(
                (source.temporal, sink.temporal),
                (PortTemporal::Flow { .. }, PortTemporal::Value)
            );
            let finite_flow_into_standing_consumer = matches!(
                (source.temporal, sink.temporal),
                (
                    PortTemporal::Flow { closes: true },
                    PortTemporal::Flow { closes: false }
                )
            );
            source.value_kind == sink.value_kind
                && (source.temporal == sink.temporal
                    || reactively_lifted_value
                    || finite_flow_into_standing_consumer)
        }
        ConnectionTrack::NormalClose => {
            matches!(source.temporal, PortTemporal::Flow { closes: true })
                && sink.temporal == PortTemporal::Value
                && sink.value_kind.as_str() == conduit_core::EMPTY_INFO_ID
        }
        ConnectionTrack::Quiescence => {
            matches!(source.temporal, PortTemporal::Flow { .. })
                && sink.temporal == PortTemporal::Value
                && sink.value_kind.as_str() == conduit_core::EMPTY_INFO_ID
        }
        ConnectionTrack::AbnormalTerminal => {
            sink.temporal == PortTemporal::Value
                && source.abnormal_kind.as_ref() == Some(&sink.value_kind)
        }
    };
    if compatible {
        return Ok(());
    }
    Err(CanonicalExpansionDiagnostic::new(
        "CND-FRM-045",
        format!(
            "cord connects incompatible {} contracts: source {} {} -> sink {} {}",
            track.as_str(),
            source.value_kind.as_str(),
            source.temporal.as_str(),
            sink.value_kind.as_str(),
            sink.temporal.as_str()
        ),
    ))
}

/// Validate a payload binding across the containing Plot's callable Fore.
pub fn validate_front_contract(
    name: &str,
    value_kind: &KindId,
    temporal: PortTemporal,
    actual: &PortDescriptor,
    front_is_source: bool,
) -> Result<(), CanonicalExpansionDiagnostic> {
    let reactive_flow_boundary = matches!(
        (temporal, actual.temporal),
        (PortTemporal::Flow { .. }, PortTemporal::Value)
            | (PortTemporal::Value, PortTemporal::Flow { .. })
    );
    let finite_flow_into_standing_consumer = front_is_source
        && matches!(
            (temporal, actual.temporal),
            (
                PortTemporal::Flow { closes: true },
                PortTemporal::Flow { closes: false }
            )
        );
    if value_kind != &actual.value_kind
        || (temporal != actual.temporal
            && !reactive_flow_boundary
            && !finite_flow_into_standing_consumer)
    {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-045",
            format!(
                "runtime front port '{name}' declares '{}' ({temporal:?}) but binds '{}' ({:?})",
                value_kind.as_str(),
                actual.value_kind.as_str(),
                actual.temporal,
            ),
        ));
    }
    Ok(())
}
