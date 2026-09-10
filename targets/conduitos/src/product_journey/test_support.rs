//! Shared exact Host/keyboard and native lifecycle fixtures.
use super::*;
use crate::{
    keyboard_offer::KeyboardRealization, offer::CpuFeatures, pointer_offer::PointerRealization,
};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

pub(crate) fn fixture() -> (BootIdentities, HostOffer<'static>, ProductJourney) {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        1_048_576,
    )
    .with_keyboard(
        KeyboardRealization {
            mechanism: crate::keyboard_offer::KeyboardMechanism::UsbHid,
            controller_id: [3; 32],
            device_id: [4; 32],
            interface_id: [5; 32],
            endpoint_id: [6; 32],
            report_buffers: 2,
            transition_slots: 8,
            operation_slots: 2,
        },
        "build",
    )
    .unwrap();
    let offer = offer
        .with_pointer(
            PointerRealization {
                mechanism: crate::pointer_offer::PointerMechanism::UsbHid,
                controller_id: [3; 32],
                device_id: [7; 32],
                interface_id: [8; 32],
                endpoint_id: [9; 32],
                report_buffers: 2,
                event_slots: 8,
                operation_slots: 1,
            },
            "build",
        )
        .unwrap();
    let journey = ProductJourney::new(
        HostId::from(crate::identity::hex(&identities.host)),
        BootId::from(crate::identity::hex(&identities.boot)),
        OfferGeneration(offer.generation),
    )
    .unwrap();
    (identities, offer, journey)
}

fn target(journey: &ProductJourney, action: JourneyAction) -> String {
    let projection = journey.projection();
    match action {
        JourneyAction::OpenBack | JourneyAction::Birth => {
            format!("form/{}", projection.checked_form_id.as_str())
        }
        JourneyAction::Wake
        | JourneyAction::Plan
        | JourneyAction::Play
        | JourneyAction::Stop
        | JourneyAction::Lull => format!("body/{}", projection.body_id.unwrap().as_str()),
        _ => panic!("unsupported journey test action"),
    }
}

pub(crate) fn invoke(
    journey: &mut ProductJourney,
    action: JourneyAction,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
) -> Result<(), JourneyError> {
    let revision = journey.revision();
    let request = journey.next_request(action, target(journey, action), revision)?;
    journey.apply(request, identities, offer, "build", revision)
}

pub(crate) fn key(usage: u8, transition: KeyTransition) -> KeyEvent {
    KeyEvent::new(usage, transition, KeyModifiers::from_bits(0)).unwrap()
}
