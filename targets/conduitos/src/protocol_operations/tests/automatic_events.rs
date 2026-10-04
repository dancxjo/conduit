//! Check and plan the complete Source topology against retained native owners.
extern crate std;
use super::*;
use alloc::vec::Vec;
#[test]
fn automatic_bus_time_topology_plans_against_exact_retained_native_owners() {
    let (mut play, _, _) = prepare(InertBus, InertClock);
    play.start().unwrap();
    play.cancel().unwrap();
}

pub(super) fn prepare<
    P: crate::i2c_base::I2cProvider,
    C: crate::monotonic_clock::owner::MonotonicDeadlineProvider,
>(
    provider: P,
    clock_provider: C,
) -> (
    crate::protocol_play::PreparedTimedProtocolPlay<P, C>,
    StructuredInfoType,
    StructuredInfoType,
) {
    let i2c = crate::i2c_base::contract::I2cContract::prepare().unwrap();
    let (mut startup, _) = i2c.catalogs();
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
    let events = include_str!("../../../../../plots/device-protocols/bme280-clock-events.conduit");
    let decoding = include_str!("../../../../../plots/device-protocols/main.conduit");
    let observation =
        include_str!("../../../../../plots/device-protocols/bme280-capture-observation.conduit");
    let frames =
        include_str!("../../../../../plots/device-protocols/bme280-protocol-frame.conduit");
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
        &parse_syntax_document(&combine(&[
            LIFECYCLE,
            events,
            decoding,
            observation,
            frames,
        ])),
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
    let frame = schema("BmeProtocolFrame");
    let event = schema("BmeProtocolEvent");
    let context = schema("BmeClockContext");
    let value = |ty: &StructuredInfoType, maximum| {
        CheckedValueContract::new(ty.profile().unwrap().value_kind().clone(), maximum, vec![])
            .unwrap()
    };
    let state_value = value(state, 4096);
    let frame_value = value(frame, 4096);
    let event_value = value(event, 4096);
    let context_value = value(context, 4096);
    let result_value = value(clock.result_type(), 512);
    use crate::protocol_source::{
        ProtocolSourcePackage, ProtocolSpecialization as Specialization, ProtocolValue,
    };
    let typed = |schema: &StructuredInfoType, contract: &CheckedValueContract| ProtocolValue {
        schema: schema.clone(),
        contract: contract.clone(),
    };
    let package = ProtocolSourcePackage {
        schema: crate::protocol_source::PACKAGE_SCHEMA.into(),
        source: combine(&[
            LIFECYCLE,
            events,
            FEEDBACK,
            frames,
            decoding,
            observation,
            automatic,
        ]),
        specializations: vec![
            Specialization::SeededFlow {
                value: typed(state, &state_value),
            },
            Specialization::SeededUntil {
                value: typed(frame, &frame_value),
            },
            Specialization::FeedbackZip {
                left: typed(state, &state_value),
                right: typed(event, &event_value),
            },
            Specialization::Merge {
                value: typed(context, &context_value),
            },
            Specialization::Zip {
                left: typed(context, &context_value),
                right: typed(clock.result_type(), &result_value),
            },
        ],
    };
    let encoded_package = serde_json::to_vec(&package).unwrap();
    let decoded_package = ProtocolSourcePackage::decode(&encoded_package).unwrap();
    let source = crate::protocol_source::PreparedProtocolSource::prepare(decoded_package).unwrap();
    let expanded = source.expand("bme280-autonomous").unwrap();
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
        "state/seeded/flow/until",
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
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    use crate::i2c_base::{
        installation::{I2cNativeIdentity, ReadyI2cBase},
        owner::I2cAttachment,
    };
    use crate::monotonic_clock::installation::{ClockNativeIdentity, ReadyClockBase};
    // SAFETY: callers retain only scripted or inert fixture providers, with no hardware access.
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
            provider,
        )
    }
    .unwrap();
    // SAFETY: callers retain only a scripted or inert fixture clock, with no native timer access.
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
            clock_provider,
        )
    }
    .unwrap();
    bus.append_to_advertisement(&mut host, &i2c).unwrap();
    timer.append_to_advertisement(&mut host, &clock).unwrap();
    source.publish_pure_backs(&expanded, &mut host).unwrap();
    let before = host.clone();
    source.publish_pure_backs(&expanded, &mut host).unwrap();
    assert_eq!(
        host, before,
        "republication retains the exact prepared offers"
    );
    let mut substituted = before.clone();
    let pure = &source.capabilities[0];
    substituted
        .capabilities
        .iter_mut()
        .find(|offer| offer.capability_id == pure.capability_id)
        .unwrap()
        .limits
        .max_queue_bytes += 1;
    let before_refusal = substituted.clone();
    assert!(
        source
            .publish_pure_backs(&expanded, &mut substituted)
            .is_err()
    );
    assert_eq!(
        substituted, before_refusal,
        "conflicting identity cannot partially publish offers"
    );
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
    let limits = source.queue_limits(&expanded, &hosts, &placements).unwrap();
    let boundaries = limits.boundaries;
    let connection_limits = limits.connections;
    let mut excessive = connection_limits.clone();
    let clock_cord = expanded
        .expanded
        .connections
        .iter()
        .find(|cord| {
            expanded.expanded.gears.iter().any(|gear| {
                gear.gear_id == cord.source_gear_id && gear.kind_id.as_str() == "machine/clock/at"
            })
        })
        .unwrap();
    excessive
        .get_mut(&(
            clock_cord.source_gear_id.clone(),
            clock_cord.source_port_id.clone(),
            clock_cord.sink_gear_id.clone(),
            clock_cord.sink_port_id.clone(),
        ))
        .unwrap()
        .byte_capacity = crate::monotonic_clock::contract::CLOCK_MAXIMUM_BYTES + 1;
    let options = PlanningOptions {
        connection_bases: &BTreeMap::new(),
        line_candidates: &BTreeMap::new(),
        connection_item_capacity: 1,
        connection_byte_capacity: 512,
        authority_grants: &grants,
        protected_resource_grants: &[],
        line_offers: &[],
    };
    assert!(matches!(
        conduit_planner::plan_expanded_authoring_with_connection_limits(
            &expanded,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            options,
            &excessive,
            &boundaries
        ),
        Err(conduit_planner::PlannerError::QueueRequirementAboveHostLimit(_))
    ));
    let artifact = source
        .plan_artifact(
            &expanded,
            ArtifactId::from("fixture/reviewed-source-package@1"),
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            options,
        )
        .unwrap();
    let plan = &artifact.artifact().definition().internal_plan;
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(plan.fragments[0].placements.len(), 52);
    assert_eq!(
        plan.fragments[0]
            .placements
            .iter()
            .filter(|gear| gear.base.is_some())
            .count(),
        2
    );
    let (table, handle, claim) = super::automatic_admission::possession(plan, false);
    let (clock_table, clock_handle, clock_claim) =
        super::automatic_admission::possession(plan, true);
    let identity = crate::protocol_artifact::ProtocolArtifactIdentity {
        source: plan.source_document_id.clone(),
        checked: plan.checked_plot_id.clone(),
        expanded: plan.expanded_plot_id.clone(),
        artifact: ArtifactId::from("fixture/reviewed-source-package@1"),
    };
    for altered in 0..4 {
        let mut stale = identity.clone();
        match altered {
            0 => stale.source = SourceDocumentId::from("stale/source"),
            1 => stale.checked = CheckedPlotId::from("stale/checked"),
            2 => stale.expanded = ExpandedPlotId::from("stale/expanded"),
            _ => stale.artifact = ArtifactId::from(""),
        }
        assert!(
            crate::protocol_artifact::AdmittedProtocolArtifact::admit(stale, plan.clone()).is_err()
        );
    }
    assert_eq!(artifact.artifact().identity(), &identity);
    let play = artifact
        .prepare_timed(
            crate::protocol_play::I2cAdmission {
                ready: bus,
                table,
                handle,
                claim,
            },
            crate::protocol_play::ClockAdmission {
                ready: timer,
                table: clock_table,
                handle: clock_handle,
                claim: clock_claim,
            },
        )
        .unwrap();
    (
        play,
        schema("BmeProtocolBegin").clone(),
        schema("BmeProtocolFailure").clone(),
    )
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
