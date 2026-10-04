//! Check and plan the complete Source topology against retained native owners.
extern crate std;
use super::*;
use alloc::vec::Vec;
#[test]
fn automatic_bus_time_topology_plans_against_exact_retained_native_owners() {
    let i2c = crate::i2c_base::contract::I2cContract::prepare().unwrap();
    let (mut startup, mut profile) = i2c.catalogs();
    let clock = crate::monotonic_clock::contract::MonotonicClockContract::prepare().unwrap();
    let clock_types = check_syntax_document(
        &parse_syntax_document(crate::monotonic_clock::contract::CLOCK_TYPES),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    for (name, path) in [
        ("MonotonicClockRequest", "machine/clock/at/request"),
        ("MonotonicClockResult", "machine/clock/at/result"),
    ] {
        startup
            .insert_checked_native_type(
                path,
                clock_types
                    .native_types
                    .iter()
                    .find(|ty| ty.name == name)
                    .unwrap(),
            )
            .unwrap();
    }
    startup
        .insert(conduit_plot::KindSignature {
            kind: "machine/clock/at".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup
        .insert_fore("machine/clock/at", clock.kind().checked_front())
        .unwrap();
    profile.insert_kind(clock.kind().clone()).unwrap();
    let events = include_str!("../../../../../plots/device-protocols/bme280-clock-events.conduit");
    let automatic = include_str!("../../../../../plots/device-protocols/bme280-autonomous.conduit");
    let combine = |units: &[&str]| {
        let imports = units
            .iter()
            .flat_map(|unit| unit.lines())
            .filter(|line| line.starts_with("with "))
            .collect::<alloc::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("\n");
        let bodies = units
            .iter()
            .flat_map(|unit| unit.lines())
            .filter(|line| !line.starts_with("with "))
            .collect::<Vec<_>>()
            .join("\n");
        alloc::format!("{imports}\n{bodies}")
    };
    let types = check_syntax_document(
        &parse_syntax_document(&combine(&[LIFECYCLE, events])),
        &startup,
    )
    .unwrap();
    let schema = |name: &str| {
        &types
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
    };
    let state = schema("BmeProtocolState");
    let event = schema("BmeProtocolEvent");
    let context = schema("BmeClockContext");
    let value = |ty: &StructuredInfoType, maximum| {
        CheckedValueContract::new(ty.profile().unwrap().value_kind().clone(), maximum, vec![])
            .unwrap()
    };
    let state_value = value(state, 4096);
    let event_value = value(event, 4096);
    let context_value = value(context, 4096);
    let result_value = value(clock.result_type(), 512);
    conduit_semantic_catalog::install_seeded_state_flow_kind(
        &state_value,
        state,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    conduit_semantic_catalog::install_flow_zip_feedback_kind(
        &state_value,
        state,
        &event_value,
        event,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    conduit_semantic_catalog::install_flow_merge_finite_kind(
        &context_value,
        context,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    conduit_semantic_catalog::install_flow_zip_finite_kind(
        &context_value,
        context,
        &result_value,
        clock.result_type(),
        &mut startup,
        &mut profile,
    )
    .unwrap();
    let mut owners = ProtocolOperations::default();
    let capabilities = vec![
        owners.states.install_flow(&state_value, state).unwrap(),
        owners
            .joins
            .install_feedback(&state_value, state, &event_value, event)
            .unwrap(),
        owners
            .joins
            .install(&context_value, context, &result_value, clock.result_type())
            .unwrap(),
        owners.merges.install(&context_value, context).unwrap(),
    ];
    // The feedback unit supplies the ordinary tuple-to-transition assembler.
    let checked = check_syntax_document(
        &parse_syntax_document(&combine(&[LIFECYCLE, events, FEEDBACK, automatic])),
        &startup,
    )
    .unwrap();
    for stage in checked
        .plots
        .iter()
        .flat_map(|plot| &plot.cords)
        .flat_map(|cord| &cord.stages)
    {
        if let conduit_plot::CheckedCordStage::StructuredSelector { selector, .. } = stage {
            profile
                .insert(conduit_plot::structured_selector_definition(
                    selector,
                    PortTemporal::Flow { closes: true },
                ))
                .unwrap();
        }
    }
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "bme280-autonomous", &profile).unwrap();
    std::eprintln!(
        "automatic BME280 Source topology: {} gears",
        expanded.expanded.gears.len()
    );
    assert!(
        expanded.expanded.gears.len() > 16,
        "measure the full topology before changing native admission limits"
    );
    for kind in [
        "machine/i2c/transact",
        "machine/clock/at",
        "flow/merge/finite",
        "flow/zip/feedback",
        "state/seeded/flow/finite",
    ] {
        assert!(
            expanded
                .expanded
                .gears
                .iter()
                .any(|gear| gear.kind_id.as_str() == kind),
            "missing {kind}"
        );
    }
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/automatic".into(),
        boot_id: "fixture/automatic-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities,
        planner_capabilities: vec![],
    };
    use crate::i2c_base::{
        installation::{I2cNativeIdentity, ReadyI2cBase},
        owner::I2cAttachment,
    };
    use crate::monotonic_clock::installation::{ClockNativeIdentity, ReadyClockBase};
    // SAFETY: these fixture providers cannot access hardware or issue effects.
    let bus = unsafe {
        ReadyI2cBase::new(
            I2cNativeIdentity {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                base_id: "fixture/bus".into(),
                provider_instance_id: "fixture/bus-provider".into(),
                provider_generation: 1,
                resource_pool_id: "fixture/bus-resource".into(),
                resource_generation_id: ResourceGenerationId("fixture/bus-generation".into()),
                envelope_id: "fixture/bus-envelope".into(),
                artifact_id: "fixture/bus-artifact".into(),
            },
            I2cAttachment {
                generation: 1,
                minimum_address: 8,
                maximum_address: 119,
                resource_bytes: 32,
            },
            InertBus,
        )
    }
    .unwrap();
    // SAFETY: the inert clock never observes a counter or arms a timer.
    let timer = unsafe {
        ReadyClockBase::new(
            ClockNativeIdentity {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                base_id: "fixture/clock".into(),
                provider_instance_id: "fixture/clock-provider".into(),
                provider_generation: 1,
                resource_pool_id: "fixture/clock-resource".into(),
                resource_generation_id: ResourceGenerationId("fixture/clock-generation".into()),
                envelope_id: "fixture/clock-envelope".into(),
                artifact_id: "fixture/clock-artifact".into(),
            },
            InertClock,
        )
    }
    .unwrap();
    bus.append_to_advertisement(&mut host, &i2c).unwrap();
    timer.append_to_advertisement(&mut host, &clock).unwrap();
    for gear in &expanded.expanded.gears {
        if let [entry] = gear.configuration.as_slice()
            && let ConfigurationValue::Text(encoded) = &entry.value
        {
            let temporal = PortTemporal::Flow { closes: true };
            host.capabilities.push(match entry.key.as_str() {
                "program" => crate::expression_host_call::offer(
                    &conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
                    temporal,
                )
                .unwrap(),
                "selector" => crate::structured_selector_host_call::offer(
                    &StructuredSelector::from_canonical_hex(encoded).unwrap(),
                    temporal,
                )
                .unwrap(),
                _ => panic!("unsupported checked configuration"),
            });
        }
    }
    let grants = [
        (
            crate::i2c_base::installation::I2C_AUTHORITY,
            crate::i2c_base::contract::I2C_CALL,
            "machine/i2c/transact",
            "conduitos/i2c-transaction@1",
        ),
        (
            crate::monotonic_clock::installation::CLOCK_AUTHORITY,
            crate::monotonic_clock::contract::CLOCK_CALL,
            "machine/clock/at",
            "conduitos/monotonic-clock-at@1",
        ),
    ]
    .map(|(authority, call, kind, capability)| AuthorityGrant {
        grant_id: alloc::format!("fixture/grant/{kind}").into(),
        contract_id: authority.into(),
        host_call_contract_id: call.into(),
        subject_kind: kind.into(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: capability.into(),
    });
    let hosts = [host];
    let placements = default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let boundaries = expanded
        .input_bindings
        .iter()
        .map(|b| (PortDirection::Input, b))
        .chain(
            expanded
                .output_bindings
                .iter()
                .map(|b| (PortDirection::Output, b)),
        )
        .map(|(direction, b)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: b.front_port_id.clone(),
                    track: b.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: hosts[0]
                        .capabilities
                        .iter()
                        .find(|offer| {
                            offer.capability_id == placements.by_gear[&b.gear_id].capability_id
                        })
                        .unwrap()
                        .limits
                        .max_queue_bytes,
                },
            )
        })
        .collect();
    let plan = plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            // Finite queues admit pressure independently of the value-storage envelope.
            connection_byte_capacity: 512,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let external = hosts[0].capabilities[0].clone();
    owners.states.validate_plan(&plan).unwrap();
    owners.joins.validate_plan(&plan).unwrap();
    owners.merges.validate_plan(&plan).unwrap();
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(plan.fragments[0].placements.len(), 32);
    assert_eq!(
        plan.fragments[0]
            .placements
            .iter()
            .filter(|gear| gear.base.is_some())
            .count(),
        2
    );
    let (table, handle, claim) = super::automatic_admission::possession(&plan, false);
    let (clock_table, clock_handle, clock_claim) =
        super::automatic_admission::possession(&plan, true);
    let definition = crate::protocol_kernel_fixture::definition(plan, external);
    let mut play = crate::protocol_play::PreparedTimedProtocolPlay::prepare(
        definition,
        bus,
        table,
        handle,
        claim,
        owners,
        crate::protocol_play::ClockAdmission {
            ready: timer,
            table: clock_table,
            handle: clock_handle,
            claim: clock_claim,
        },
    )
    .unwrap();
    play.start().unwrap();
    play.cancel().unwrap();
}

struct InertBus;
impl crate::i2c_base::I2cProvider for InertBus {
    fn transact(
        &mut self,
        _: &crate::i2c_base::I2cTransaction<'_>,
        _: &mut [u8],
    ) -> Result<usize, crate::i2c_base::I2cDisposition> {
        panic!("planning cannot issue bus effects")
    }
    fn revoke(&mut self) {}
}
struct InertClock;
impl crate::monotonic_clock::owner::MonotonicDeadlineProvider for InertClock {
    fn poll_until(
        &mut self,
        _: u64,
    ) -> Result<Option<u64>, crate::monotonic_clock::codec::ClockDisposition> {
        panic!("planning cannot observe time")
    }
    fn revoke(&mut self) {}
}
