//! Owner-selected appearance keys and explicit local-refusal/route-failure disposition.
use super::{FaceArrival, face_arrival::OwnerSelectionError};
use crate::{
    display::PixelTarget, identity::BootIdentities, native_guest_face::NativeGuestFace,
    native_owner_return::NativeOwnerReturnRoute,
};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

pub(super) enum SelectionDisposition {
    Changed,
    Refused(&'static str),
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum AppliedSelection {
    Changed,
    LocalRefused(&'static str),
    RouteFailed(&'static str),
}

pub(super) fn apply_result<R>(
    arrival: &mut FaceArrival,
    current: &mut Option<NativeGuestFace>,
    route: &mut Option<R>,
    result: Result<NativeGuestFace, OwnerSelectionError>,
    display: &mut impl PixelTarget,
) -> Result<AppliedSelection, &'static str> {
    match result {
        Ok(fresh) => {
            *current = Some(fresh);
            Ok(AppliedSelection::Changed)
        }
        Err(error @ OwnerSelectionError::ProvisionalInput) => {
            arrival.show_local_refusal(error.code(), display)?;
            Ok(AppliedSelection::LocalRefused(error.code()))
        }
        Err(OwnerSelectionError::Route(reason)) => {
            *route = None;
            Ok(AppliedSelection::RouteFailed(reason))
        }
    }
}

pub(super) fn dispatch(
    event: KeyEvent,
    identities: BootIdentities,
    arrival: &mut FaceArrival,
    current: &mut Option<NativeGuestFace>,
    route: &mut Option<NativeOwnerReturnRoute>,
    standby: bool,
    display: &mut impl PixelTarget,
) -> Result<Option<SelectionDisposition>, &'static str> {
    let modifiers = event.modifiers_after().bits();
    let shift_only = modifiers != 0
        && modifiers & !(KeyModifiers::LEFT_SHIFT.bits() | KeyModifiers::RIGHT_SHIFT.bits()) == 0;
    if standby
        || route.is_none()
        || event.transition() != KeyTransition::Pressed
        || !(modifiers == 0 || (event.usage() == 66 && shift_only))
        || !matches!(event.usage(), 63..=66)
    {
        return Ok(None);
    }
    let prior = current.as_ref().ok_or("native-owner-face-absent")?;
    let owner = route.as_mut().ok_or("native-owner-return-unavailable")?;
    let result = if event.usage() == 66 && shift_only {
        arrival.toggle_owner_forced_colors(prior, owner, identities, display)
    } else if event.usage() == 66 {
        arrival.toggle_owner_text_scale(prior, owner, identities, display)
    } else {
        let (look, recipe) = match event.usage() {
            63 => (
                conduit_face::ComponentMaskLook::Compact,
                conduit_face::TodoComponentRecipe::Flat,
            ),
            64 => (
                conduit_face::ComponentMaskLook::Spacious,
                conduit_face::TodoComponentRecipe::Flat,
            ),
            _ => (
                conduit_face::ComponentMaskLook::Spacious,
                conduit_face::TodoComponentRecipe::Grouped,
            ),
        };
        arrival.select_owner_component(identities, prior, owner, look, recipe, display)
    };
    let disposition = match apply_result(arrival, current, route, result, display)? {
        AppliedSelection::Changed => SelectionDisposition::Changed,
        AppliedSelection::LocalRefused(reason) => SelectionDisposition::Refused(reason),
        AppliedSelection::RouteFailed(reason) => {
            arrival.retire_owner_route(display)?;
            arrival.show_owner_result(false, false, reason, display)?;
            SelectionDisposition::Refused(reason)
        }
    };
    Ok(Some(disposition))
}

pub(super) fn record_refusal(reason: &str) {
    crate::arch::early_write(
        b"CONDUIT_NATIVE_COMPONENT_SELECTION {\"status\":\"refused\",\"code\":\"",
    );
    crate::arch::early_write(reason.as_bytes());
    crate::arch::early_write(b"\"}\n");
}
