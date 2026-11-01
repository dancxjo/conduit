//! Planning refusal is observed before the real installed execution entrance.
//! There is no kernel trace for a refused Plan; do not invent one to count zeros.
use conduit_core::{BaseImplementationId, ConnectionTrack, HostAdvertisement, PortDirection};
use conduit_planner::{
    ConnectionQueueLimits, ForeBoundaryKey, PlacementChoice, PlacementChoices, PlannerError,
    PlanningOptions,
};
use conduit_plot::{ExpandedAuthoringPlot, ProfileCatalog, StartupCatalog};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, KernelTimingPhase, StdHost,
    TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};

fn authored(current: bool) -> ExpandedAuthoringPlot {
    let mut startup = StartupCatalog::new();
    let mut unused = ProfileCatalog::new();
    conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut unused).unwrap();
    conduit_semantic_catalog::install_graphics_face_catalog(&mut startup, &mut unused).unwrap();
    let mut catalog = ProfileCatalog::new();
    for contract in [
        conduit_semantic_catalog::graphics_text_contract(),
        conduit_semantic_catalog::graphics_icon_contract(),
        conduit_semantic_catalog::graphics_face_contract(),
    ] {
        catalog
            .insert_kind(super::portable(contract, current))
            .unwrap();
    }
    startup
        .insert_value_kind_alias(
            "GraphicsScene",
            conduit_core::kind_id(if current {
                "face/graphics-scene@2"
            } else {
                "face/graphics-scene@1"
            }),
        )
        .unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(super::execution::SOURCE),
        &startup,
    )
    .unwrap();
    conduit_plot::expand_canonical_plot_for_authoring(&checked, "instrument", &catalog).unwrap()
}
fn advertisement(host: &StdHost, current: bool) -> HostAdvertisement {
    let mut advertised = host.advertisement().clone();
    if !current {
        for offer in &mut advertised.capabilities {
            if !matches!(
                offer.kind_id.as_str(),
                "graphics/rect" | "graphics/text" | "graphics/icon" | "face/graphics"
            ) {
                continue;
            }
            offer.kind_contract_revision =
                super::revision(offer.kind_id.as_str() == "face/graphics", false).into();
            super::legacy_ports_and_laws(&mut offer.inputs, &mut offer.semantic_contract.laws);
            super::legacy_ports_and_laws(&mut offer.outputs, &mut offer.semantic_contract.laws);
        }
    }
    advertised
}
fn plan(
    authored: &ExpandedAuthoringPlot,
    host: &HostAdvertisement,
    explicit: bool,
) -> Result<conduit_core::Plan, PlannerError> {
    let hosts = std::slice::from_ref(host);
    let choices = if explicit {
        PlacementChoices {
            by_gear: authored
                .expanded
                .gears
                .iter()
                .map(|gear| {
                    let offered = host
                        .capabilities
                        .iter()
                        .find(|offer| offer.kind_id == gear.kind_id)
                        .unwrap();
                    (
                        gear.gear_id.clone(),
                        PlacementChoice {
                            host_id: host.host_id.clone(),
                            capability_id: offered.capability_id.clone(),
                        },
                    )
                })
                .collect(),
        }
    } else {
        conduit_planner::default_expanded_placements(&authored.expanded, hosts)?
    };
    let boundaries = [
        (PortDirection::Input, "incoming"),
        (PortDirection::Output, "transformed"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: conduit_core::port_id(name),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: conduit_face::MAX_GRAPHICS_SCENE_BYTES as u32,
            },
        )
    })
    .collect();
    conduit_planner::plan_expanded_authoring_with_options(
        authored,
        hosts,
        &choices,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_face::MAX_GRAPHICS_SCENE_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
}
#[derive(Default, Debug, PartialEq)]
struct Effects {
    entries: usize,
    preparations: usize,
    plays: usize,
    finishes: usize,
    terminal_bytes: usize,
    deliveries: usize,
    host_calls: usize,
}
#[derive(Default)]
struct Timer {
    preparations: usize,
    plays: usize,
    finishes: usize,
}
impl TimerAdapter for Timer {
    fn wait(&mut self, _: Duration) {
        panic!("no timer operation in graphics plot");
    }
    fn observe_kernel_phase(&mut self, phase: KernelTimingPhase) {
        match phase {
            KernelTimingPhase::PreparationStarted => self.preparations += 1,
            KernelTimingPhase::PlayStarted => self.plays += 1,
            KernelTimingPhase::PlayFinished => self.finishes += 1,
        }
    }
}
#[derive(Default)]
struct Output(Vec<ExternalForeDelivery>);
impl ExternalForeOutputAdapter for Output {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value);
        Ok(())
    }
}
fn attempt(
    host: &mut StdHost,
    authored: &ExpandedAuthoringPlot,
    advertised: &HostAdvertisement,
    explicit: bool,
) -> (bool, Effects) {
    let mut effects = Effects::default();
    let plan = match plan(authored, advertised, explicit) {
        Ok(plan) => plan,
        Err(_) => return (false, effects),
    };
    effects.entries += 1;
    let mut timer = Timer::default();
    let mut output = Output::default();
    let mut text = Vec::new();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: conduit_core::port_id("incoming"),
                track: ConnectionTrack::Payload,
                bytes: super::execution::packet(),
            }],
            &mut output,
            &mut text,
            &mut timer,
        )
        .unwrap();
    effects.preparations = timer.preparations;
    effects.plays = timer.plays;
    effects.finishes = timer.finishes;
    effects.terminal_bytes = text.len();
    effects.deliveries = output.0.len();
    effects.host_calls = report
        .kernel
        .unwrap()
        .kernel_sign
        .iter()
        .filter(|event| event.kind == conduit_kernel::KernelEventKind::HostCallRequested)
        .count();
    (true, effects)
}
#[test]
fn revision_mismatch_stops_before_preparation_play_and_output_for_both_selection_modes() {
    for explicit in [false, true] {
        let mut host = StdHost::new();
        // Both matching domains are eligible. Only U is the installed runtime:
        // relabeling its offers does not turn it into a historical executable.
        for current in [false, true] {
            assert!(plan(&authored(current), &advertisement(&host, current), explicit).is_ok());
        }
        for current in [false, true] {
            let offered = advertisement(&host, !current);
            let (entered, effects) = attempt(&mut host, &authored(current), &offered, explicit);
            assert!(!entered);
            assert_eq!(effects, Effects::default());
        }
        let offered = advertisement(&host, true);
        let (entered, effects) = attempt(&mut host, &authored(true), &offered, explicit);
        assert!(entered);
        assert_eq!(effects.entries, 1);
        assert_eq!(effects.preparations, 1);
        assert_eq!(effects.plays, 1);
        assert_eq!(effects.finishes, 1);
        assert_eq!(effects.deliveries, 1);
        assert!(effects.terminal_bytes > 0);
        assert_eq!(effects.host_calls, 3);
    }
}
