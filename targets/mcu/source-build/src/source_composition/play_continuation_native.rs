// Original Ready identities and authorized Issuers only; never received rows.
use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn describe<'a>(
    context: ContinuationNativeContext<'a>,
    policy: Policy<'_>,
    equipment: &'a crate::equipment::Equipment,
    issuers: &'a Issuers<WAIT_SLOTS>,
    operations: &Operations<'_, WAIT_SLOTS>,
    recording: Option<&RecordingOperation<'_>>,
    node: u16,
    kind: &str,
    binding: c::AssignedNativeBinding,
) -> Result<ContinuationNativeDescription<'a>, MaterialRefusal> {
    let check = |facts: NativeFacts<'a>, issuer: &'a Issuer, operation: &Operation<'_>| {
        if operation.kind != kind {
            return Err(MaterialRefusal::Scope);
        }
        issuer
            .describe_continuation(context, policy, &facts, operation, binding)
            .map_err(|_| MaterialRefusal::Authority)
    };
    if kind == conduit_pulse_capture::PULSE_READ_KIND {
        check(
            NativeFacts::pulse(equipment.pulse.as_ref().ok_or(MaterialRefusal::Scope)?),
            &issuers.pulse,
            &operations.pulse,
        )
    } else if kind == conduit_tm1637_wire::WRITE_KIND {
        check(
            NativeFacts::write(equipment.display.as_ref().ok_or(MaterialRefusal::Scope)?),
            &issuers.write,
            &operations.write,
        )
    } else if kind == operations.renderer.kind {
        check(
            NativeFacts::renderer(&equipment.renderer),
            &issuers.renderer,
            &operations.renderer,
        )
    } else if kind == conduit_time::TICK_KIND {
        let mut clocks = source::sampling::CLOCKS
            .iter()
            .enumerate()
            .filter(|(_, clock)| clock.0 == node);
        let (slot, _) = clocks.next().ok_or(MaterialRefusal::Scope)?;
        if clocks.next().is_some() {
            return Err(MaterialRefusal::Scope);
        }
        check(
            NativeFacts::wait(
                equipment
                    .waits
                    .get(slot)
                    .and_then(Option::as_ref)
                    .ok_or(MaterialRefusal::Scope)?,
            ),
            issuers.waits.get(slot).ok_or(MaterialRefusal::Scope)?,
            operations
                .waits
                .get(slot)
                .and_then(Option::as_ref)
                .ok_or(MaterialRefusal::Scope)?,
        )
    } else {
        #[cfg(not(record_service))]
        {
            let operation = recording.filter(|op| op.kind == kind).ok_or(MaterialRefusal::Scope)?;
            check(
                NativeFacts::append(equipment.recording.ready().ok_or(MaterialRefusal::Scope)?),
                &issuers.recording, operation,
            )
        }
        #[cfg(record_service)]
        {
            if kind != conduit_resource_snapshot::service_contract::SERVICE_KIND {
                return Err(MaterialRefusal::Scope);
            }
            let operations = recording.ok_or(MaterialRefusal::Scope)?;
            let mut matching = operations.iter().enumerate().filter(|(_, op)|
                c::AssignedIdentity::from_text(op.call) == binding.call);
            let (ordinal, operation) = matching.next().ok_or(MaterialRefusal::Scope)?;
            if matching.next().is_some() { return Err(MaterialRefusal::Scope); }
            let issuer = match ordinal {
                0 => &issuers.recording,
                1 => &issuers.recording_read,
                _ => return Err(MaterialRefusal::Scope),
            };
            let facts = NativeFacts::record_service(equipment.recording.ready().ok_or(MaterialRefusal::Scope)?);
            issuer.describe_continuation(context, policy, &facts, operation, binding)
                .map_err(|_| MaterialRefusal::Authority)
        }
    }
}
