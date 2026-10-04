use super::*;
use alloc::{collections::BTreeMap, vec};
use conduit_composite::*;
use conduit_planner::{
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions, default_expanded_placements,
    plan_expanded_authoring_with_options,
};
use conduit_plot::{
    ProfileCatalog, StartupCatalog, check_syntax_document, expand_canonical_plot_for_authoring,
    parse_syntax_document,
};

fn planned() -> (Plan, SeededStateOperationFactory, CapabilityOffer) {
    let schema = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let value = CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![]).unwrap();
    planned_source(
        StartupCatalog::new(),
        ProfileCatalog::new(),
        &value,
        &schema,
        "plot seeded (\n >> seed: Boolean...| <= 1B\n >> next: Boolean...| <= 1B\n current: $Boolean <= 1B >>\n) {\n cell: state/seeded/finite\n seed >> cell.seed\n next >> cell.next\n cell.current >> current\n}\n",
        "seeded",
    )
}

fn planned_source(
    mut startup: StartupCatalog,
    mut profile: ProfileCatalog,
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
    source: &str,
    name: &str,
) -> (Plan, SeededStateOperationFactory, CapabilityOffer) {
    conduit_semantic_catalog::install_seeded_state_kind(value, schema, &mut startup, &mut profile)
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&checked, name, &profile).unwrap();
    let mut factory = SeededStateOperationFactory::default();
    let offer = factory.install(value, schema).unwrap();
    let mut capabilities = vec![offer.clone()];
    for gear in &authoring.expanded.gears {
        if let [entry] = gear.configuration.as_slice()
            && entry.key == "program"
        {
            let ConfigurationValue::Text(encoded) = &entry.value else {
                panic!("program")
            };
            capabilities.push(
                crate::expression_host_call::offer(
                    &conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
                    PortTemporal::Flow { closes: true },
                )
                .unwrap(),
            );
        }
    }
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("fixture/native"),
        boot_id: BootId::from("fixture/boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/native@1"),
        bases: vec![],
        resources: vec![],
        capabilities,
        planner_capabilities: vec![],
    }];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundary_limits = authoring
        .input_bindings
        .iter()
        .map(|binding| (PortDirection::Input, binding))
        .chain(
            authoring
                .output_bindings
                .iter()
                .map(|binding| (PortDirection::Output, binding)),
        )
        .map(|(direction, binding)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: value.maximum_bytes.max(1),
                },
            )
        })
        .collect();
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: value.maximum_bytes.max(1),
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .unwrap();
    (plan, factory, offer)
}

#[test]
fn checked_source_selects_retained_schema_and_rejects_forged_native_realizations() {
    let (plan, factory, _) = planned();
    factory.validate_plan(&plan).unwrap();
    assert!(
        SeededStateOperationFactory::default()
            .validate_plan(&plan)
            .is_err()
    );
    let gear = &plan.fragments[0].placements[0];
    let budget = factory.budget(gear).unwrap();
    assert_eq!(
        (budget.value_items, budget.value_bytes, budget.host_requests),
        (1, 1, 0)
    );
    let mut artifact = gear.clone();
    artifact.artifact_id = ArtifactId::from("forged/state");
    let mut bounds = gear.clone();
    bounds.limits.max_queue_bytes += 1;
    let mut fore = gear.clone();
    fore.outputs[0].temporal = PortTemporal::Flow { closes: true };
    let mut configuration = gear.clone();
    configuration.configuration.push(ConfigurationEntry {
        key: "default".into(),
        value: ConfigurationValue::Bool(false),
    });
    for forged in [artifact, bounds, fore, configuration] {
        assert!(factory.budget(&forged).is_err());
    }
}

#[test]
fn native_prepared_specializations_have_a_finite_quota_and_fail_without_mutation() {
    let schema = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let mut factory = SeededStateOperationFactory::default();
    for maximum in 1..=MAXIMUM_SPECIALIZATIONS as u32 {
        let value = CheckedValueContract::new(kind_id(BOOL_INFO_ID), maximum, vec![]).unwrap();
        factory.install(&value, &schema).unwrap();
        assert!(factory.install(&value, &schema).is_err());
        assert_eq!(factory.offers().count(), maximum as usize);
    }
    let value = CheckedValueContract::new(kind_id(BOOL_INFO_ID), 17, vec![]).unwrap();
    assert!(factory.install(&value, &schema).is_err());
    assert_eq!(factory.offers().count(), MAXIMUM_SPECIALIZATIONS);
}

fn prepared_kernel() -> KernelCompositeHost {
    let (plan, factory, external) = planned();
    kernel(plan, factory, external)
}

fn kernel(
    plan: Plan,
    factory: SeededStateOperationFactory,
    external: CapabilityOffer,
) -> KernelCompositeHost {
    let definition = crate::protocol_kernel_fixture::definition(plan, external);
    let mut registry = KernelOperationRegistry::new();
    registry.install(factory).unwrap();
    registry
        .install(crate::expression_host_call::ExpressionOperationFactory::default())
        .unwrap();
    KernelCompositeHost::prepare(definition, &registry).unwrap()
}

mod source;

#[test]
fn source_seed_and_replacements_retire_pool_cells_and_complete_within_the_sign_profile() {
    let mut play = prepared_kernel();
    let values = [false, true].map(|value| ValuePayload {
        value_kind: kind_id(BOOL_INFO_ID),
        encoded: vec![u8::from(value)],
    });
    let mut output = values[0].clone();
    play.start().unwrap();
    play.admit_input(&port_id("seed"), 0, &values[0]).unwrap();
    play.close_input(&port_id("seed")).unwrap();
    // Three remote cords admit 24 retained sign records. Five observations
    // exercise reuse of the smaller value pool and leave room for closure.
    for sequence in 0..5 {
        let expected = sequence % 2 == 1;
        if sequence > 0 {
            play.admit_input(
                &port_id("next"),
                sequence - 1,
                &values[usize::from(expected)],
            )
            .unwrap();
        }
        for _ in 0..16 {
            play.step().unwrap();
        }
        assert_eq!(
            play.output_into(&port_id("current"), &mut output).unwrap(),
            Some(sequence)
        );
        assert_eq!(output, values[usize::from(expected)]);
        play.complete_output(&port_id("current"), sequence)
            .unwrap_or_else(|error| panic!("replacement output {sequence}: {error:?}"));
    }
    play.close_input(&port_id("next")).unwrap();
    let mut status = KernelCompositeStatus::Active;
    for _ in 0..16 {
        status = play.step().unwrap();
    }
    assert_eq!(status, KernelCompositeStatus::Complete);
    assert_eq!(
        play.output_into(&port_id("current"), &mut output).unwrap(),
        None
    );
}

#[test]
fn exhausted_remote_transcript_is_a_typed_sign_failure_without_hidden_growth() {
    let mut play = prepared_kernel();
    let values = [false, true].map(|value| ValuePayload {
        value_kind: kind_id(BOOL_INFO_ID),
        encoded: vec![u8::from(value)],
    });
    let mut output = values[0].clone();
    play.start().unwrap();
    play.admit_input(&port_id("seed"), 0, &values[0]).unwrap();
    play.close_input(&port_id("seed")).unwrap();
    for sequence in 0..6 {
        if sequence > 0 {
            play.admit_input(
                &port_id("next"),
                sequence - 1,
                &values[(sequence % 2) as usize],
            )
            .unwrap();
        }
        for _ in 0..16 {
            play.step().unwrap();
        }
        assert_eq!(
            play.output_into(&port_id("current"), &mut output).unwrap(),
            Some(sequence)
        );
        let outcome = play.complete_output(&port_id("current"), sequence);
        if sequence == 5 {
            assert!(matches!(
                outcome,
                Err(KernelCompositeError::Execution {
                    reason: ChildExecutionError::Scheduler(
                        conduit_kernel::scheduler::SchedulerError::Sign(
                            conduit_kernel::SignError::RemoteItemCapacityExceeded
                        )
                    ),
                    ..
                })
            ));
        } else {
            outcome.unwrap();
        }
    }
}
