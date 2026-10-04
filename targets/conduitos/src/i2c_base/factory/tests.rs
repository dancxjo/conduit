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
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions, default_expanded_placements,
    plan_expanded_authoring_with_options,
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
    let contract = I2cContract::prepare().unwrap();
    let (startup, mut profile) = contract.catalogs();
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
                    byte_capacity: I2C_MAXIMUM_BYTES,
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
            connection_bases: &empty,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: I2C_MAXIMUM_BYTES,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .unwrap();
    (plan, ready, identity)
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

mod execution;

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
