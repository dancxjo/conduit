//! Source declarations → typed startup → offered Back → Plan → Dimension port.
use conduit_core::{
    BaseImplementationId, ConnectionTrack, Dimension, PortDirection, DIMENSION_ENCODED_LEN,
};
use conduit_plot::{
    check_syntax_document, dimension_operation, expand_canonical_plot_for_authoring,
    parse_syntax_document, quantity_conversion, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, StdHostComposition, StdHostConfig,
    TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};

struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("Dimension introspection has no timer effect");
    }
}
#[derive(Default)]
struct Collector(Vec<conduit_std_host::ExternalForeBytes>);
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
        match delivery.track {
            ConnectionTrack::Payload => self.0.push(delivery.bytes),
            ConnectionTrack::NormalClose => {}
            _ => panic!("no abnormal Dimension effect"),
        }
        Ok(())
    }
}

#[test]
fn installed_introspection_emits_pinned_dimension_through_the_shared_kernel() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    quantity_conversion::install(&mut startup, &mut profiles).unwrap();
    for (kind, ty, value) in [
        (dimension_operation::UNIT_KIND, "Unit", "doublestep"),
        (
            dimension_operation::QUANTITY_KIND,
            "Quantity",
            "3doublestep",
        ),
    ] {
        let source = format!("dimension travel\ndimension clock\ndimension velocity = travel / clock\ntype Motion = quantity {{dimension:velocity}}\nunit stepper : Motion = {{reference:origin,scale:1}}\nunit doublestep : Motion = {{reference:stepper,scale:2}}\nplot forward (\n source: {ty}\n dimension: Dimension >>\n) {{\n operation: {kind}(source=source)\n operation.dimension >> dimension\n}}\nplot demo (\n dimension: Dimension >>\n) {{\n operation: forward(source={value})\n operation.dimension >> dimension\n}}.\n");
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let authored = expand_canonical_plot_for_authoring(&checked, "demo", &profiles).unwrap();
        let mut host = StdHost::new_with_composition(
            StdHostConfig {
                host_id: "dimension-host".into(),
                boot_id: "dimension-boot".into(),
                offer_generation: conduit_core::OfferGeneration(1),
            },
            StdHostComposition::minimal().with_math(),
        );
        let hosts = [host.advertisement().clone()];
        let placements =
            conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
        let maximum = DIMENSION_ENCODED_LEN as u32;
        let boundaries = [(
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: "dimension".into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: maximum,
            },
        )]
        .into_iter()
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
                connection_byte_capacity: maximum,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .unwrap();
        // Admit a later source snapshot before playing the captured Plan. The
        // same names now denote a different dimensional law; neither admission
        // nor the shared host may reinterpret the prior configured capsule.
        let rebound = source.replace("velocity = travel / clock", "velocity = travel * clock");
        let later_checked =
            check_syntax_document(&parse_syntax_document(&rebound), &startup).unwrap();
        let later = expand_canonical_plot_for_authoring(&later_checked, "demo", &profiles).unwrap();
        assert_ne!(
            authored.expanded.gears[0].configuration,
            later.expanded.gears[0].configuration
        );
        let mut collector = Collector::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[],
                &mut collector,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        let expected = Dimension::basis("travel")
            .unwrap()
            .quotient(Dimension::basis("clock").unwrap())
            .unwrap();
        assert_eq!(collector.0, [expected.encode().to_vec()]);
        let kernel = report.kernel.unwrap();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|sign| sign.kind == conduit_kernel::KernelEventKind::BackCompleted));
    }
}
