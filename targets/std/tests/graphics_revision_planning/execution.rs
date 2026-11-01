//! Real external Fore -> installed graphics transforms -> terminal, over U.
use conduit_core::{
    port_id, BaseImplementationId, ConnectionTrack, ObservationKind, PortDirection,
    TerminalDisposition,
};
use conduit_face::{GraphicsScene, MAX_GRAPHICS_SCENE_BYTES};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};

pub(super) const SOURCE: &str = r#"plot instrument (
 >> incoming: GraphicsScene <= 10226B
 transformed: GraphicsScene <= 10226B >>
) {
 text: graphics/text(text = "retained")
 glyph: graphics/icon(icon = "face")
 sink: face/graphics("Instrument")
 incoming >> text.input
 text.scene >> glyph.input
 glyph.scene >> sink.scene
 glyph.scene >> transformed
}
"#;
#[derive(Default)]
struct Collector(Vec<ExternalForeDelivery>);
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value);
        Ok(())
    }
}
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("graphics Play has no timer operation");
    }
}
pub(super) fn packet() -> Vec<u8> {
    include_str!("../../../../proof/fixtures/graphics-scene-union.hex")
        .trim()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn union_packet_survives_installed_transforms_and_terminal_through_planned_host_calls() {
    let mut catalog = conduit_plot::ProfileCatalog::new();
    let mut startup = conduit_plot::StartupCatalog::new();
    conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut catalog).unwrap();
    conduit_semantic_catalog::install_graphics_face_catalog(&mut startup, &mut catalog).unwrap();
    startup
        .insert_value_kind_alias(
            "GraphicsScene",
            conduit_core::kind_id(conduit_face::GRAPHICS_SCENE_KIND),
        )
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "instrument", &catalog).unwrap();
    let mut host = StdHost::new();
    let hosts = [host.advertisement().clone()];
    let choices = default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "incoming"),
        (PortDirection::Output, "transformed"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: port_id(name),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: MAX_GRAPHICS_SCENE_BYTES as u32,
            },
        )
    })
    .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let plan = plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &choices,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: MAX_GRAPHICS_SCENE_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let bytes = packet();
    let before = GraphicsScene::decode(&bytes).unwrap();
    let mut collector = Collector::default();
    let mut output = Vec::new();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: port_id("incoming"),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut collector,
            &mut output,
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(collector.0.len(), 1);
    let after = GraphicsScene::decode(&collector.0[0].bytes).unwrap();
    assert_eq!(&after.commands()[..3], before.commands());
    assert_eq!(after.commands().len(), 5);
    assert_eq!(collector.0[0].bytes[0], 3);
    assert!(String::from_utf8(output)
        .unwrap()
        .contains("graphics scene commands=5"));
    assert!(matches!(
        report.observations.last().map(|o| &o.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.unwrap();
    use conduit_kernel::KernelEventKind::{HostCallCompleted, HostCallRequested};
    assert_eq!(
        kernel
            .kernel_sign
            .iter()
            .filter(|event| event.kind == HostCallRequested)
            .count(),
        3
    );
    assert_eq!(
        kernel
            .kernel_sign
            .iter()
            .filter(|event| event.kind == HostCallCompleted)
            .count(),
        3
    );
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}
