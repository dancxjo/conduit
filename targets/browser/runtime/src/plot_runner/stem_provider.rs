//! Actual pending-kernel custody checked by the private browser Stem provider.
#[cfg(feature = "creche-surface")]
use super::*;
#[cfg(feature = "creche-surface")]
use conduit_body_make::stem_runtime::StemRequest;

#[cfg(feature = "creche-surface")]
pub(crate) fn validate_pending(
    play: &conduit_body::BodyPlayIdentity,
    gear: &conduit_core::PlannedGear,
    request_sequence: u32,
    request: StemRequest,
) -> Result<(), String> {
    abi::SESSION.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| "Stem provider cannot reenter kernel dispatch")?;
        let session = slot
            .as_ref()
            .ok_or("Stem has no actual browser kernel Play")?;
        if session.active_play_id != play.active_play_id
            || session.host_id != gear.host_id
            || session.boot_id != gear.boot_id
        {
            return Err("Stem provider differs from the actual kernel Play".into());
        }
        let pending = session
            .pending
            .iter()
            .find(|pending| {
                pending.request.request.0 == request_sequence
                    && matches!(pending.effect, engine::BrowserHostEffect::StemPrepare)
                    && placement_in_fragments(&session.fragments, pending.request.node)
                        .is_some_and(|(_, actual)| actual == gear)
            })
            .ok_or("Stem has no exact pending kernel Host Call")?;
        if session
            .scheduler
            .host_value(pending.request.input.value)
            .map_err(|error| format!("{error:?}"))?
            != request.0
        {
            return Err("Stem kernel request differs from the private provision".into());
        }
        Ok(())
    })
}
