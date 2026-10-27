use super::*;
use crate::digital_base::{
    DigitalDisposition, DigitalProvider, DigitalRequest,
    installation::{DigitalNativeIdentity, ReadyDigitalBase},
    owner::DigitalAttachment,
};
use alloc::{collections::BTreeMap, vec};
use conduit_planner::{
    PlanningOptions, default_expanded_placements, plan_expanded_authoring_with_connection_limits,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};
struct Provider;
impl DigitalProvider for Provider {
    fn access(&mut self, _: DigitalRequest) -> Result<bool, DigitalDisposition> {
        panic!("planning cannot touch a contact");
    }
    fn revoke(&mut self) {}
}
#[test]
fn ordinary_digital_source_plans_only_against_ready_native_possession_and_explicit_grant() {
    let contract = DigitalContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../../plots/device-protocols/digital.conduit"
        )),
        &startup,
    )
    .unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "digital-access", &profile).unwrap();
    let identity = DigitalNativeIdentity {
        capability_id: CapabilityId::from("conduitos/digital-access@1"),
        host_id: HostId::from("host/native"),
        boot_id: BootId::from("boot/native"),
        base_id: HostBaseId::from("base/native-digital"),
        provider_instance_id: BaseInstanceId::from("provider/native-digital"),
        provider_generation: 1,
        resource_pool_id: ResourcePoolId::from("resource/native-digital"),
        resource_generation_id: ResourceGenerationId("generation/one".into()),
        envelope_id: CapabilityEnvelopeId::from("envelope/native-digital"),
        artifact_id: ArtifactId::from("fixture/native-controller"),
    };
    // SAFETY: this provider is inert and panics if preparation invokes hardware.
    let ready = unsafe {
        ReadyDigitalBase::new(
            identity.clone(),
            DigitalAttachment {
                generation: 1,
                resource_bytes: 1,
            },
            Provider,
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
    let grants = [AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/native-digital"),
        contract_id: AuthorityContractId::from(DIGITAL_AUTHORITY),
        host_call_contract_id: HostCallContractId::from(DIGITAL_CALL),
        subject_kind: contract.kind().kind_id.clone(),
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        capability_id: CapabilityId::from("conduitos/digital-access@1"),
    }];
    let hosts = [host];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let limits = crate::protocol_kernel_fixture::queue_limits(
        &authoring,
        &hosts,
        &placements,
        DIGITAL_MAXIMUM_BYTES,
    );
    let empty = BTreeMap::new();
    let plan = |grants: &[AuthorityGrant]| {
        plan_expanded_authoring_with_connection_limits(
            &authoring,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &empty,
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: DIGITAL_MAXIMUM_BYTES,
                authority_grants: grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &limits.connections,
            &limits.boundaries,
        )
    };
    assert!(plan(&[]).is_err());
    let planned = plan(&grants).unwrap();
    assert!(verify_plan(&planned));
    let [gear] = planned.fragments[0].placements.as_slice() else {
        panic!("one DIGITAL operation");
    };
    let factory =
        DigitalOperationFactory::prepare_contract(vec![identity.capability_id.clone()]).unwrap();
    assert_eq!(
        factory.budget(gear).unwrap().maximum_value_bytes,
        DIGITAL_MAXIMUM_BYTES
    );
    for forgery in 0..4 {
        let mut forged = gear.clone();
        match forgery {
            0 => forged.authority.clear(),
            1 => forged.host_calls[0].maximum_output_bytes += 1,
            2 => forged.base = None,
            _ => forged.resources.clear(),
        }
        assert!(factory.budget(&forged).is_err());
    }
}

#[test]
fn admitted_contact_inventory_is_finite_unique_and_nonempty() {
    assert!(DigitalOperationFactory::prepare_contract(vec![]).is_err());
    assert!(DigitalOperationFactory::prepare_contract(vec![CapabilityId::from("")]).is_err());
    let id = CapabilityId::from("contact/one");
    assert!(DigitalOperationFactory::prepare_contract(vec![id.clone(), id]).is_err());
    assert!(
        DigitalOperationFactory::prepare_contract(
            (0..65)
                .map(|i| CapabilityId::from(alloc::format!("contact/{i}")))
                .collect()
        )
        .is_err()
    );
}
