//! Browser Face and Show Fores selected on current owner carrier offers.
//!
//! The interaction return remains the existing provisional route until its
//! finite sharing of the one-item return Line is planned and executable.

use conduit_core::{
    BaseImplementationId, ConnectionTrack, HostAdvertisement, LineOffer, PortDirection,
};
use conduit_planner::{bind_external_fore_lines, ExternalForeLineChoice};
use conduit_presentation::{PlannedMaskPlot, MAX_OWNER_FACE_RESPONSE_BYTES};

use crate::{planned_mask_with_fore_limits, OwnerCarrierForeLimits};

const WEBSOCKET: &conduit_host_browser_make::BrowserLineRealizationDescriptor =
    &conduit_host_browser_make::BROWSER_LINE_REALIZATIONS[0];

/// Seal the browser Mask's Face input and Show output on exact carrier Lines.
/// The one-item Fore budgets are no larger than the admitted directional
/// payloads; the whole Face response is also bounded at 32 KiB by its route.
pub fn planned_owner_face_show_mask(
    browser: &HostAdvertisement,
    owner: &HostAdvertisement,
    face_line: &LineOffer,
    return_line: &LineOffer,
    source: &str,
    name: &str,
) -> Result<PlannedMaskPlot, String> {
    for line in [face_line, return_line] {
        let limits = line.binding.limits;
        if line.binding.base.as_str() != WEBSOCKET.base_implementation_id
            || line.contract != WEBSOCKET.contract
            || limits.maximum_in_flight_items > WEBSOCKET.maximum_in_flight_items
            || limits.maximum_payload_bytes > WEBSOCKET.maximum_payload_bytes
            || limits.maximum_frame_bytes > WEBSOCKET.maximum_frame_bytes
            || limits.maximum_buffered_bytes > WEBSOCKET.maximum_buffered_bytes
        {
            return Err("browser Mask Line is outside the reviewed WebSocket offer".into());
        }
    }
    let face_bytes = face_line
        .binding
        .limits
        .maximum_payload_bytes
        .min(MAX_OWNER_FACE_RESPONSE_BYTES as u32);
    let show_bytes = return_line
        .binding
        .limits
        .maximum_payload_bytes
        .min(MAX_OWNER_FACE_RESPONSE_BYTES as u32);
    let planned = planned_mask_with_fore_limits(
        browser,
        source,
        name,
        Some(OwnerCarrierForeLimits {
            face_bytes,
            show_bytes,
        }),
    )?;
    let choices = [
        ExternalForeLineChoice {
            host_id: browser.host_id.clone(),
            boot_id: browser.boot_id.clone(),
            direction: PortDirection::Input,
            front_port_id: planned.mask.face_input.front_port_id.clone(),
            track: ConnectionTrack::Payload,
            peer_host_id: owner.host_id.clone(),
            peer_boot_id: owner.boot_id.clone(),
            line_id: face_line.line_id.clone(),
        },
        ExternalForeLineChoice {
            host_id: browser.host_id.clone(),
            boot_id: browser.boot_id.clone(),
            direction: PortDirection::Output,
            front_port_id: planned.mask.show_output.front_port_id.clone(),
            track: ConnectionTrack::Payload,
            peer_host_id: owner.host_id.clone(),
            peer_boot_id: owner.boot_id.clone(),
            line_id: return_line.line_id.clone(),
        },
    ];
    let plan = bind_external_fore_lines(
        &planned.plan,
        &choices,
        &[face_line.clone(), return_line.clone()],
        &[BaseImplementationId::from(WEBSOCKET.base_implementation_id)],
    )
    .map_err(|error| format!("browser Mask Fore Line refused: {error}"))?;
    PlannedMaskPlot::admit(&planned.mask, &plan).map_err(|error| format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        process_owned_line_offer_with_limits, resource_offer, BootId, HostId, HostProfileId,
        LineAvailability, LinkLimits, OfferGeneration, PRESENTATION_RESOURCE_CLASS,
        PROTOCOL_VERSION,
    };

    fn host(id: &str, browser: bool) -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from(id),
            boot_id: BootId::from(format!("{id}/boot")),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from(if browser { "browser" } else { "hosted" }),
            bases: vec![],
            resources: if browser {
                vec![resource_offer(
                    "browser/presentation",
                    PRESENTATION_RESOURCE_CLASS,
                    1,
                )]
            } else {
                vec![]
            },
            planner_capabilities: vec![],
            capabilities: if browser {
                vec![crate::offer()]
            } else {
                vec![]
            },
        }
    }

    fn lines(owner: &HostAdvertisement, browser: &HostAdvertisement) -> [LineOffer; 2] {
        let limits = LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 64 * 1024,
            maximum_buffered_bytes: 256 * 1024,
            maximum_frame_bytes: 64 * 1024,
        };
        let mut face = process_owned_line_offer_with_limits(
            "face-line",
            "face-binding",
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "websocket-instance",
            owner,
            browser,
            limits,
        );
        let mut returning = process_owned_line_offer_with_limits(
            "return-line",
            "return-binding",
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "websocket-instance",
            browser,
            owner,
            limits,
        );
        face.contract = WEBSOCKET.contract;
        returning.contract = WEBSOCKET.contract;
        [face, returning]
    }

    #[test]
    fn current_lines_select_only_face_and_show_without_changing_the_local_mask() {
        let owner = host("owner", false);
        let browser = host("browser", true);
        let [face, returning] = lines(&owner, &browser);
        let selected = planned_owner_face_show_mask(
            &browser,
            &owner,
            &face,
            &returning,
            crate::MASK_SOURCE,
            "browser-graphical",
        )
        .unwrap();
        let local = crate::planned_mask(&browser, crate::MASK_SOURCE, "browser-graphical").unwrap();
        assert_ne!(selected.plan.plan_id, local.plan.plan_id);
        let fores = &selected.plan.fragments[0].fore_ports;
        let fore = |name| {
            fores
                .iter()
                .find(|fore| fore.front_port_id.as_str() == name)
                .unwrap()
        };
        assert_eq!(fore("face").selected_line, Some(face.admitted_line()));
        assert_eq!(fore("show").selected_line, Some(returning.admitted_line()));
        assert_eq!(fore("interaction").selected_line, None);
        assert_eq!(fore("face").item_capacity, 1);
        assert_eq!(fore("show").item_capacity, 1);
        assert_eq!(
            fore("face").byte_capacity,
            MAX_OWNER_FACE_RESPONSE_BYTES as u32
        );
        assert_eq!(
            fore("show").byte_capacity,
            MAX_OWNER_FACE_RESPONSE_BYTES as u32
        );
        assert_eq!(
            fore("interaction").byte_capacity,
            conduit_presentation::MAX_FACE_INTERACTION_BYTES as u32
        );
        assert_eq!(
            local.plan.fragments[0].fore_ports[0].byte_capacity,
            crate::MASK_BYTES
        );
        assert!(conduit_core::verify_plan(&selected.plan));
    }

    #[test]
    fn stale_or_unavailable_carrier_offer_refuses_and_smaller_line_reduces_fore_bound() {
        let owner = host("owner", false);
        let browser = host("browser", true);
        let [face, returning] = lines(&owner, &browser);
        let mut stale = face.clone();
        stale.binding.sink.boot_id = BootId::from("stale");
        assert!(planned_owner_face_show_mask(
            &browser,
            &owner,
            &stale,
            &returning,
            crate::MASK_SOURCE,
            "browser-graphical"
        )
        .is_err());
        let mut unavailable = returning.clone();
        unavailable.availability.availability = LineAvailability::Unavailable;
        assert!(planned_owner_face_show_mask(
            &browser,
            &owner,
            &face,
            &unavailable,
            crate::MASK_SOURCE,
            "browser-graphical"
        )
        .is_err());
        let mut wrong_base = returning.clone();
        wrong_base.binding.base = BaseImplementationId::from("conduit.base/unreviewed@1");
        assert!(planned_owner_face_show_mask(
            &browser,
            &owner,
            &face,
            &wrong_base,
            crate::MASK_SOURCE,
            "browser-graphical"
        )
        .is_err());
        let mut smaller = face;
        smaller.binding.limits.maximum_payload_bytes = 8 * 1024;
        let planned = planned_owner_face_show_mask(
            &browser,
            &owner,
            &smaller,
            &returning,
            crate::MASK_SOURCE,
            "browser-graphical",
        )
        .unwrap();
        let face_fore = planned.plan.fragments[0]
            .fore_ports
            .iter()
            .find(|fore| fore.front_port_id.as_str() == "face")
            .unwrap();
        assert_eq!(face_fore.byte_capacity, 8 * 1024);
    }
}
