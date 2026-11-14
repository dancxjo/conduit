//! Cold owner planning fixture: no physical carrier, admission or execution claim.
use super::*;
use conduit_core as c;

pub(super) fn verify_current_member_line_selection(
    owner: &Owner,
    source: &crate::plot_source::CanonicalSource,
    expanded: &conduit_plot::ExpandedAuthoringPlot,
    boundaries: &std::collections::BTreeMap<
        conduit_planner::ForeBoundaryKey,
        conduit_planner::ConnectionQueueLimits,
    >,
    member: &c::HostAdvertisement,
) -> Vec<conduit_body::BodyPlotPlan> {
    let budgets = std::collections::BTreeMap::from([(resident(expanded), boundaries.clone())]);
    let baseline = owner
        .prepare_workset_partitions(&[(source, expanded)], &budgets)
        .unwrap();
    let fragment = baseline[0]
        .plan
        .fragments
        .iter()
        .find(|f| f.host_id == member.host_id)
        .unwrap();
    let fore = fragment
        .fore_ports
        .iter()
        .find(|f| f.direction == c::PortDirection::Input)
        .unwrap();
    let local = owner.host.advertisement();
    let base = c::BaseImplementationId::from("fixture/owned-fore-carrier");
    let offer = c::LineOffer {
        line_id: "fixture/fore-line".into(),
        binding: c::LinkBinding {
            binding_id: "fixture/fore-binding".into(),
            source: c::LinkEndpoint {
                host_id: local.host_id.clone(),
                boot_id: local.boot_id.clone(),
                endpoint_id: "fixture/source".into(),
            },
            sink: c::LinkEndpoint {
                host_id: member.host_id.clone(),
                boot_id: member.boot_id.clone(),
                endpoint_id: "fixture/sink".into(),
            },
            base: base.clone(),
            base_instance_id: "fixture/instance".into(),
            credential: c::LinkCredentialReference::None,
            authority: c::LinkAuthorityReference::ProcessOwned,
            limits: c::LinkLimits {
                maximum_in_flight_items: fore.item_capacity,
                maximum_payload_bytes: fore.byte_capacity,
                maximum_buffered_bytes: fore.byte_capacity,
                maximum_frame_bytes: fore.byte_capacity + 52,
            },
        },
        contract: c::LineContract {
            scope: c::LineScope::PointToPoint,
            traffic_shape: c::LineTrafficShape::Message,
            duplex: c::LineDuplex::FullDuplex,
            ordering: c::LineOrdering::Ordered,
            reliability: c::LineReliability::Reliable,
            continuation: c::LineContinuation::None,
            security: c::LineSecurity::PhysicalPossession,
        },
        availability: c::LineAvailabilitySign {
            line_id: "fixture/fore-line".into(),
            binding_id: "fixture/fore-binding".into(),
            availability: c::LineAvailability::Ready,
            sign_id: "fixture/ready".into(),
        },
    };
    let choice = conduit_planner::ExternalForeLineChoice {
        host_id: member.host_id.clone(),
        boot_id: member.boot_id.clone(),
        direction: fore.direction,
        front_port_id: fore.front_port_id.clone(),
        track: fore.track,
        peer_host_id: local.host_id.clone(),
        peer_boot_id: local.boot_id.clone(),
        line_id: offer.line_id.clone(),
    };
    let before = owner.session.evidence().clone();
    let choices = [choice.clone()];
    let offers = [offer.clone()];
    let bases = [base.clone()];
    let selected = std::collections::BTreeMap::from([(
        resident(expanded),
        super::super::fore_line_planning::ForeLineSelection {
            choices: &choices,
            offers: &offers,
            allowed_bases: &bases,
        },
    )]);
    let mut planned = baseline.clone();
    owner.select_fore_lines(&mut planned, &selected).unwrap();
    assert!(c::verify_plan(&planned[0].plan));
    assert_ne!(planned[0].plan.plan_id, baseline[0].plan.plan_id);
    assert_eq!(
        planned[0].plan.source_document_id,
        baseline[0].plan.source_document_id
    );
    assert!(planned[0]
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.fore_ports)
        .any(|f| f.selected_line.is_some()));
    for change in 0..5 {
        let mut choice = choice.clone();
        let mut offer = offer.clone();
        let mut bases = vec![base.clone()];
        match change {
            0 => choice.boot_id = "fixture/stale-local".into(),
            1 => choice.peer_boot_id = "fixture/stale-peer".into(),
            2 => choice.front_port_id = "fixture/absent-port".into(),
            3 => offer.availability.availability = c::LineAvailability::Unavailable,
            _ => bases.clear(),
        }
        let choices = [choice];
        let offers = [offer];
        let selected = std::collections::BTreeMap::from([(
            resident(expanded),
            super::super::fore_line_planning::ForeLineSelection {
                choices: &choices,
                offers: &offers,
                allowed_bases: &bases,
            },
        )]);
        assert!(owner
            .select_fore_lines(&mut baseline.clone(), &selected)
            .is_err());
    }
    assert_eq!(owner.session.evidence(), &before);
    assert!(owner.session.realization().is_none());
    planned
}
