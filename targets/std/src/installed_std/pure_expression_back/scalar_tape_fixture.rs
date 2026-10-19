//! Shared checked Source, genuine offers, sealed Plan and Value Fore fixture.
use crate::{ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, TimerAdapter};
use conduit_core::{BaseImplementationId, ConfigurationValue, ConnectionTrack, PortDirection};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use std::{collections::BTreeMap, time::Duration};

pub(crate) struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("scalar expression has no timer/device effect");
    }
}
#[derive(Default)]
pub(crate) struct Outputs(pub(crate) Vec<crate::ExternalForeBytes>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}
pub(crate) fn prepare(
    source: &str,
    name: &str,
    tape: bool,
) -> (StdHost, conduit_core::Plan, PortableExpressionProgram) {
    prepare_with_offer(source, name, |program, temporal| {
        if tape {
            conduit_std_offers::pure_scalar_tape_std_offer(program, temporal).unwrap()
        } else {
            conduit_std_offers::pure_expression_std_offer(program, temporal).unwrap()
        }
    })
}
pub(crate) fn prepare_with_offer(
    source: &str,
    name: &str,
    make_offer: impl FnOnce(
        &PortableExpressionProgram,
        conduit_core::PortTemporal,
    ) -> conduit_core::CapabilityOffer,
) -> (StdHost, conduit_core::Plan, PortableExpressionProgram) {
    let mut make_offer = Some(make_offer);
    let (host, plan, mut programs) = prepare_with_offers(source, name, |program, temporal| {
        vec![make_offer.take().expect("one primitive expression")(
            program, temporal,
        )]
    });
    assert_eq!(programs.len(), 1);
    (host, plan, programs.remove(0))
}
pub(crate) fn prepare_with_offers(
    source: &str,
    name: &str,
    mut make_offers: impl FnMut(
        &PortableExpressionProgram,
        conduit_core::PortTemporal,
    ) -> Vec<conduit_core::CapabilityOffer>,
) -> (StdHost, conduit_core::Plan, Vec<PortableExpressionProgram>) {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new()).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    let mut programs = Vec::new();
    for gear in &authored.expanded.gears {
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offers = make_offers(&program, gear.inputs[0].temporal);
        assert!(!offers.is_empty());
        assert!(offers.iter().all(|offer| gear.accepts_realization(offer)));
        advertisement.capabilities.extend(offers);
        programs.push(program);
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 4096,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    (host, plan, programs)
}
