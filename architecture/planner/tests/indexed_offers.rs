#![cfg(feature = "indexed-source")]
//! Same ordinary planning from stable compiler-description fixtures. No test
//! constructor supplies a real ready provider, Body membership or Play authority.
use conduit_core as c;
use conduit_planner::{indexed_source as indexed, PlanningOptions};
use std::{cell::Cell, collections::BTreeMap};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

struct Software<'a> {
    bytes: &'a [Vec<u8>],
    decodes: Cell<usize>,
}
// SAFETY: immutable bytes are produced from complete ordinary offers before the
// borrow; repeated decoding has exact equality, order and no external effects.
unsafe impl indexed::IndexedSoftwareOffers for Software<'_> {
    fn len(&self) -> usize {
        self.bytes.len()
    }
    fn decode(&self, index: usize) -> Result<c::CapabilityOffer, conduit_planner::PlannerError> {
        self.decodes.set(self.decodes.get() + 1);
        serde_json::from_slice(&self.bytes[index])
            .map_err(|_| conduit_planner::PlannerError::InvalidPlotIdentity("fixture offer".into()))
    }
}
fn source() -> conduit_plot::ExpandedCanonicalPlot {
    let syntax = conduit_plot::parse_syntax_document("plot indexed-offers {\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n show: face/show(\"Signal\")\n pulse >> show\n}.\n");
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_signal::signal_startup_catalog())
            .unwrap();
    conduit_plot::expand_canonical_plot(
        &checked,
        "indexed-offers",
        &conduit_signal::signal_profile_catalog(),
    )
    .unwrap()
}
fn host() -> c::HostAdvertisement {
    c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: c::OfferGeneration(7),
        profile: "fixture/host-profile".into(),
        capabilities: vec![
            conduit_std_offers::signal_pulse_offer(),
            conduit_std_offers::signal_show_offer(),
        ],
        resources: conduit_signal::signal_resource_offers("fixture/timer", "fixture/face", 4),
        planner_capabilities: vec![],
        bases: vec![c::BaseProviderAdvertisement {
            base_id: "fixture/base".into(),
            provider_instance_id: "fixture/provider".into(),
            provider_generation: 3,
            implementation_id: "fixture/implementation".into(),
            mechanism_family: "fixture/family".into(),
            enforcement_class: c::BaseEnforcementClass::Cooperative,
            lifecycle: c::BaseLifecycle::Ready,
            capability_ids: vec!["pulse-1".into(), "stdout-show-1".into()],
            resource_pool_ids: vec!["fixture/timer".into(), "fixture/face".into()],
        }],
    }
}
fn parity(full: &c::HostAdvertisement, items: u16) {
    let expanded = source();
    let bytes = conduit_plot::flat_image::indexed::encode(&expanded).unwrap();
    // SAFETY: this exact checked expansion was encoded above.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes) }
            .unwrap();
    let mut native = full.clone();
    let software = native.capabilities.split_off(1);
    let payloads = software
        .iter()
        .map(|offer| serde_json::to_vec(offer).unwrap())
        .collect::<Vec<_>>();
    let software = Software {
        bytes: &payloads,
        decodes: Cell::new(0),
    };
    let hosts = [indexed::IndexedHostOffers {
        native: &native,
        software: &software,
    }];
    let eager = indexed::default_placements(&image, std::slice::from_ref(full));
    let choices = indexed::default_placements_with_indexed_offers(&image, &hosts);
    assert_eq!(choices, eager);
    let Ok(choices) = choices else { return };
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let queues = BTreeMap::new();
    let bases = [c::BaseImplementationId::from("conduit.base/local@1")];
    let options = PlanningOptions {
        connection_bases: &empty,
        line_candidates: &lines,
        connection_item_capacity: items,
        connection_byte_capacity: conduit_signal::SIGNAL_ENCODED_LEN,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let expected = indexed::plan_compact_with_connection_limits(
        &image,
        std::slice::from_ref(full),
        &choices,
        &bases,
        options,
        &queues,
    )
    .and_then(|p| p.materialize());
    let actual = indexed::plan_compact_with_indexed_offers_and_connection_limits(
        &image, &hosts, &choices, &bases, options, &queues,
    )
    .and_then(|p| p.materialize());
    assert_eq!(actual, expected);
    if let Ok(plan) = actual {
        assert!(c::verify_plan(&plan));
        assert_eq!(plan.fragments[0].offer_generation, full.offer_generation);
        assert_eq!(plan.fragments[0].boot_id, full.boot_id);
        assert!(plan.fragments[0]
            .placements
            .iter()
            .all(|p| p.base.is_some()));
    }
    assert!(software.decodes.get() > 0);
}
#[test]
fn same_complete_plans_base_provenance_and_queue_refusals() {
    for items in [0, 1, u16::MAX] {
        parity(&host(), items);
    }
    let mut invalid = host();
    invalid.bases[0].provider_generation = 0;
    parity(&invalid, 1);
    invalid = host();
    invalid.bases[0]
        .capability_ids
        .push("absent/capability".into());
    parity(&invalid, 1);
    invalid = host();
    invalid.resources[0].capacity_units = 0;
    parity(&invalid, 1);
    invalid = host();
    invalid.capabilities[1].limits.max_active_instances = 0;
    parity(&invalid, 1);
    invalid = host();
    invalid.capabilities[1].semantic_contract.configuration[0].default_value =
        c::ConfigurationValue::Text("changed complete default".into());
    parity(&invalid, 1);
}
#[test]
fn indexed_selection_retires_unselected_complete_definitions_one_at_a_time() {
    let expanded = source();
    let bytes = conduit_plot::flat_image::indexed::encode(&expanded).unwrap();
    // SAFETY: exact checked Make encoding.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes) }
            .unwrap();
    let mut complete = host();
    for index in 0..16 {
        let mut unrelated = conduit_std_offers::signal_show_offer();
        unrelated.capability_id = format!("unused/{index}").into();
        unrelated.kind_id = format!("unused/kind/{index}").into();
        unrelated.semantic_contract.configuration[0].default_value =
            c::ConfigurationValue::Text("immutable definition ".repeat(1024).into());
        complete.capabilities.push(unrelated);
    }
    let mut native = complete.clone();
    let software = native.capabilities.split_off(1);
    let payloads = software
        .iter()
        .map(|o| serde_json::to_vec(o).unwrap())
        .collect::<Vec<_>>();
    let expected = indexed::default_placements(&image, std::slice::from_ref(&complete)).unwrap();
    let (eager, old) = allocation_probe::observe(|| {
        let mut full = native.clone();
        for bytes in &payloads {
            full.capabilities
                .push(serde_json::from_slice(bytes).unwrap());
        }
        indexed::default_placements(&image, &[full]).unwrap()
    });
    let software = Software {
        bytes: &payloads,
        decodes: Cell::new(0),
    };
    let (actual, current) = allocation_probe::observe(|| {
        let native = native.clone();
        indexed::default_placements_with_indexed_offers(
            &image,
            &[indexed::IndexedHostOffers {
                native: &native,
                software: &software,
            }],
        )
        .unwrap()
    });
    assert_eq!(eager, expected);
    assert_eq!(actual, expected);
    println!(
        "complete software catalogue selection peak: eager={} indexed={} bytes; decodes={}",
        old.peak_bytes,
        current.peak_bytes,
        software.decodes.get()
    );
    assert!(
        current.peak_bytes < old.peak_bytes / 2,
        "old={old:?}, indexed={current:?}"
    );
    // Fixture CPU peak only; selected Plan material and actual MCU workspace
    // custody are independent measurements, not implied by this preparation test.
}

#[test]
fn composite_offer_extent_refuses_before_decode() {
    struct Huge(Cell<usize>);
    // SAFETY: every attempted decode deterministically refuses; no changing
    // descriptor or external readiness evidence is supplied.
    unsafe impl indexed::IndexedSoftwareOffers for Huge {
        fn len(&self) -> usize {
            usize::MAX
        }
        fn decode(&self, _: usize) -> Result<c::CapabilityOffer, conduit_planner::PlannerError> {
            self.0.set(self.0.get() + 1);
            Err(conduit_planner::PlannerError::InvalidPlotIdentity(
                "unavailable".into(),
            ))
        }
    }
    let expanded = source();
    let bytes = conduit_plot::flat_image::indexed::encode(&expanded).unwrap();
    // SAFETY: exact checked expansion encoded above.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes) }
            .unwrap();
    let native = host();
    let software = Huge(Cell::new(0));
    let result = indexed::default_placements_with_indexed_offers(
        &image,
        &[indexed::IndexedHostOffers {
            native: &native,
            software: &software,
        }],
    );
    assert!(matches!(
        result,
        Err(conduit_planner::PlannerError::PlannerLimitExceeded(_))
    ));
    assert_eq!(software.0.get(), 0);
}

#[test]
fn checked_authoring_fore_preserves_complete_plan_and_refusal() {
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(
            "plot divide (\n >> value: Distance\n result: Distance >>\n) = (. / 3)\n",
        ),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    let authored = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "divide",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    let c::ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let offer =
        conduit_std_offers::pure_expression_std_offer(&program, c::PortTemporal::Value).unwrap();
    let mut full = host();
    full.capabilities = vec![offer];
    full.resources.clear();
    full.bases.clear();
    let mut native = full.clone();
    let payloads = native
        .capabilities
        .drain(..)
        .map(|offer| serde_json::to_vec(&offer).unwrap())
        .collect::<Vec<_>>();
    let software = Software {
        bytes: &payloads,
        decodes: Cell::new(0),
    };
    let hosts = [indexed::IndexedHostOffers {
        native: &native,
        software: &software,
    }];
    let bytes = conduit_plot::flat_image::indexed::encode(&authored.expanded).unwrap();
    // SAFETY: both projections come from the same checked authoring expansion.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes) }
            .unwrap();
    let boundary = unsafe {
        indexed::IndexedAuthoringBoundary::from_make(
            image.identity().unwrap(),
            authored.front,
            authored.input_bindings,
            authored.output_bindings,
        )
    }
    .unwrap();
    let choices = indexed::default_placements(&image, std::slice::from_ref(&full)).unwrap();
    assert_eq!(
        indexed::default_placements_with_indexed_offers(&image, &hosts).unwrap(),
        choices
    );
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let queues = BTreeMap::new();
    let bases = [c::BaseImplementationId::from("conduit.base/local@1")];
    let options = PlanningOptions {
        connection_bases: &empty,
        line_candidates: &lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 8192,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    for items in [1, 0] {
        let limits = boundary
            .bindings()
            .map(|(direction, id, track)| {
                (
                    conduit_planner::ForeBoundaryKey {
                        direction,
                        front_port_id: id.clone(),
                        track,
                    },
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: items,
                        byte_capacity: 8192,
                    },
                )
            })
            .collect();
        let expected = indexed::plan_compact_authoring_with_connection_limits(
            &image,
            &boundary,
            std::slice::from_ref(&full),
            &choices,
            &bases,
            options,
            &queues,
            &limits,
        )
        .and_then(|plan| plan.materialize());
        let actual = indexed::plan_compact_authoring_with_indexed_offers_and_connection_limits(
            &image, &boundary, &hosts, &choices, &bases, options, &queues, &limits,
        )
        .and_then(|plan| plan.materialize());
        assert_eq!(actual, expected);
        if items == 1 {
            let plan = actual.unwrap();
            assert!(c::verify_plan(&plan));
            assert!(plan
                .fragments
                .iter()
                .any(|fragment| !fragment.fore_ports.is_empty()));
        } else {
            assert!(actual.is_err());
        }
    }
}
