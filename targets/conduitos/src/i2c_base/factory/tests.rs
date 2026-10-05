//! Checked register topology is planned against actual retained native offers.
use super::*;
use crate::{
    expression_host_call,
    i2c_base::{
        I2cDisposition, I2cProvider, I2cTransaction,
        installation::{I2cNativeIdentity, ReadyI2cBase},
        owner::I2cAttachment,
    },
};
use alloc::{collections::BTreeMap, vec};
use conduit_planner::{
    PlanningOptions, default_expanded_placements, plan_expanded_authoring_with_connection_limits,
};
use conduit_plot::{
    PortableExpressionProgram, check_syntax_document, expand_canonical_plot_for_authoring,
    parse_syntax_document,
};

struct Provider;
impl I2cProvider for Provider {
    fn transact(&mut self, _: &I2cTransaction<'_>, _: &mut [u8]) -> Result<usize, I2cDisposition> {
        panic!("preparation cannot issue bus effects")
    }
    fn revoke(&mut self) {}
}

fn planned<P: I2cProvider>(provider: P) -> (Plan, ReadyI2cBase<P>, I2cNativeIdentity) {
    planned_source(
        provider,
        include_str!("../../../../../plots/device-protocols/i2c-register.conduit"),
    )
}

fn planned_source<P: I2cProvider>(
    provider: P,
    source: &str,
) -> (Plan, ReadyI2cBase<P>, I2cNativeIdentity) {
    planned_named_source(provider, source, "i2c-register-read")
}

fn planned_named_source<P: I2cProvider>(
    provider: P,
    source: &str,
    name: &str,
) -> (Plan, ReadyI2cBase<P>, I2cNativeIdentity) {
    let (plan, ready, identity, _) = planned_named_source_with_joins(provider, source, name);
    (plan, ready, identity)
}

fn planned_named_source_with_joins<P: I2cProvider>(
    provider: P,
    source: &str,
    name: &str,
) -> (
    Plan,
    ReadyI2cBase<P>,
    I2cNativeIdentity,
    crate::flow_zip::FlowZipOperationFactory,
) {
    let contract = I2cContract::prepare().unwrap();
    let (mut startup, mut profile) = contract.catalogs();
    crate::protocol_test_support::install_clock_result(&mut startup);
    let (join_offer, joins) = if source.contains("flow/zip/finite") {
        let (offer, joins) = join::install(&mut startup, &mut profile);
        (Some(offer), joins)
    } else {
        (None, crate::flow_zip::FlowZipOperationFactory::default())
    };
    let mut sampler_offer = None;
    if source.contains("current/sample") {
        let types = check_syntax_document(
            &parse_syntax_document(include_str!(
                "../../../../../plots/device-protocols/bme280-lifecycle.conduit"
            )),
            &startup,
        )
        .unwrap();
        let state = &types
            .native_types
            .iter()
            .find(|ty| ty.name == "BmeProtocolState")
            .unwrap()
            .value_type;
        let value = CheckedValueContract::new(
            state.profile().unwrap().value_kind().clone(),
            crate::current_sample::MAXIMUM_BYTES,
            vec![],
        )
        .unwrap();
        assert!(
            conduit_plot::maximum_prepared_canonical_value_bytes(state).unwrap()
                <= crate::current_sample::MAXIMUM_BYTES
        );
        let trigger = CheckedValueContract::new(kind_id("value/u64"), 8, vec![]).unwrap();
        conduit_semantic_catalog::install_current_sample_finite_kind(
            &value,
            &trigger,
            &mut startup,
            &mut profile,
        )
        .unwrap();
        sampler_offer = Some(crate::current_sample::offer(&value, &trigger).unwrap());
    }
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
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
    let authoring = expand_canonical_plot_for_authoring(&checked, name, &profile).unwrap();
    let expanded = &authoring.expanded;
    assert!(expanded.gears.len() >= 2);
    let identity = I2cNativeIdentity {
        host_id: HostId::from("host/native"),
        boot_id: BootId::from("boot/native"),
        base_id: HostBaseId::from("base/native-i2c"),
        provider_instance_id: BaseInstanceId::from("provider/native-i2c"),
        provider_generation: 1,
        resource_pool_id: ResourcePoolId::from("resource/native-i2c"),
        resource_generation_id: ResourceGenerationId("generation/one".into()),
        envelope_id: CapabilityEnvelopeId::from("envelope/native-i2c"),
        artifact_id: ArtifactId::from("fixture/native-controller"),
    };
    // SAFETY: callers retain only inert/scripted test providers; no hardware is accessible.
    let ready = unsafe {
        ReadyI2cBase::new(
            identity.clone(),
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
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/native@1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    ready.append_to_advertisement(&mut host, &contract).unwrap();
    host.capabilities.extend(join_offer);
    host.capabilities.extend(sampler_offer);
    for gear in &expanded.gears {
        if let [entry] = gear.configuration.as_slice()
            && let ConfigurationValue::Text(encoded) = &entry.value
        {
            let temporal = PortTemporal::Flow { closes: true };
            match entry.key.as_str() {
                "program" => host.capabilities.push(
                    expression_host_call::offer(
                        &PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
                        temporal,
                    )
                    .unwrap(),
                ),
                "selector" => host.capabilities.push(
                    crate::structured_selector_host_call::offer(
                        &StructuredSelector::from_canonical_hex(encoded).unwrap(),
                        temporal,
                    )
                    .unwrap(),
                ),
                _ => panic!("unsupported checked native configuration"),
            }
        }
    }
    let grants = [AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/native-i2c"),
        contract_id: AuthorityContractId::from(I2C_AUTHORITY),
        host_call_contract_id: HostCallContractId::from(I2C_CALL),
        subject_kind: contract.kind().kind_id.clone(),
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        capability_id: CapabilityId::from("conduitos/i2c-transaction@1"),
    }];
    let hosts = [host];
    let placements = default_expanded_placements(expanded, &hosts).unwrap();
    let empty = BTreeMap::new();
    let queue_bytes = if source.contains("flow/zip/finite") {
        crate::flow_zip::MAXIMUM_PAIR_BYTES
    } else {
        I2C_MAXIMUM_BYTES
    };
    let limits =
        crate::protocol_kernel_fixture::queue_limits(&authoring, &hosts, &placements, queue_bytes);
    let plan = plan_expanded_authoring_with_connection_limits(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: queue_bytes,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits.connections,
        &limits.boundaries,
    )
    .unwrap();
    (plan, ready, identity, joins)
}

#[test]
fn register_plot_selects_native_expression_and_capability_bound_i2c_backs() {
    let (plan, _, _) = planned(Provider);
    assert!(verify_plan(&plan));
    assert_eq!(plan.fragments.len(), 1);
    let factory = I2cOperationFactory::prepare_contract().unwrap();
    let mut count = 0;
    for gear in &plan.fragments[0].placements {
        if gear.implementation_id.as_str() == I2C_IMPLEMENTATION {
            assert_eq!(
                factory.budget(gear).unwrap().maximum_value_bytes,
                I2C_MAXIMUM_BYTES
            );
            let mut forged = gear.clone();
            forged.authority.clear();
            assert!(factory.budget(&forged).is_err());
            count += 1;
        } else {
            assert_eq!(
                gear.implementation_id.as_str(),
                expression_host_call::IMPLEMENTATION
            );
        }
    }
    assert_eq!(count, 1);
}

mod common;
mod execution;
mod join;

const BME_FIRST_CALL_SOURCE: &str = concat!(
    include_str!("../../../../../plots/device-protocols/bme280-lifecycle.conduit"),
    "\nplot bme280-first-call (\n query: BmeProtocolBegin...| >> result: I2cResult...|\n) {\n bus: machine/i2c/transact\n query >> bme280-protocol-initialize() >> bme280-protocol-action() >> select(BmeProtocolAction.transact, unmatched=drop) >> bus >> result\n}\n"
);

#[test]
fn bme280_source_initializer_and_action_plan_against_native_i2c() {
    let (plan, _, _) = planned_named_source(Provider, BME_FIRST_CALL_SOURCE, "bme280-first-call");
    assert!(verify_plan(&plan));
    assert_eq!(plan.fragments[0].placements.len(), 4);
}

#[test]
fn complete_bme280_transition_and_next_action_have_finite_native_admission() {
    use conduit_composite::KernelOperationFactory;
    let source = concat!(
        include_str!("../../../../../plots/device-protocols/bme280-lifecycle.conduit"),
        "\nplot bme280-next-call (\n transition: BmeProtocolTransition...| >> result: I2cResult...|\n) {\n bus: machine/i2c/transact\n transition >> bme280-protocol-transition() >> bme280-protocol-action() >> select(BmeProtocolAction.transact, unmatched=drop) >> bus >> result\n}\n"
    );
    let (plan, _, _) = planned_named_source(Provider, source, "bme280-next-call");
    assert!(verify_plan(&plan));
    let fragment = &plan.fragments[0];
    // All twelve Source transition stages remain ordinary expression nodes.
    // Action, selection and the one physical operation fit without expanding
    // the native child's sixteen-node profile.
    assert_eq!(fragment.placements.len(), 15);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let expressions = expression_host_call::ExpressionOperationFactory::default();
    let selectors = crate::structured_selector_host_call::SelectorOperationFactory::default();
    let bus = I2cOperationFactory::prepare_contract().unwrap();
    let mut expression_count = 0;
    for gear in &fragment.placements {
        let budget = match gear.implementation_id.as_str() {
            expression_host_call::IMPLEMENTATION => {
                expression_host_call::ExpressionHostCall::prepare(
                    fragment,
                    &lowered,
                    &active,
                    &gear.placement_id,
                )
                .unwrap();
                expression_count += 1;
                expressions.budget(gear).unwrap()
            }
            crate::structured_selector_host_call::IMPLEMENTATION => selectors.budget(gear).unwrap(),
            I2C_IMPLEMENTATION => bus.budget(gear).unwrap(),
            _ => panic!("unexpected native realization"),
        };
        assert!(budget.maximum_value_bytes <= I2C_MAXIMUM_BYTES);
        assert_eq!(budget.host_requests, 1);
    }
    assert_eq!(expression_count, 13);
}

mod state;
