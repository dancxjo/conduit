use super::*;
use crate::spi_base::{
    SpiDisposition, SpiProvider, SpiTransaction,
    installation::{ReadySpiBase, SpiNativeIdentity},
    owner::SpiAttachment,
};
use alloc::{collections::BTreeMap, vec};
use conduit_planner::{
    PlanningOptions, default_expanded_placements, plan_expanded_authoring_with_connection_limits,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};
struct Provider;
impl SpiProvider for Provider {
    fn exchange(&mut self, _: &SpiTransaction<'_>, _: &mut [u8]) -> Result<(), SpiDisposition> {
        panic!("planning cannot touch SPI");
    }
    fn revoke(&mut self) {}
}
fn planned<P: SpiProvider>(
    provider: P,
) -> (Plan, ReadySpiBase<P>, SpiNativeIdentity, CapabilityOffer) {
    let contract = SpiContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../../plots/device-protocols/spi.conduit"
        )),
        &startup,
    )
    .unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "spi-exchange", &profile).unwrap();
    let identity = SpiNativeIdentity {
        host_id: HostId::from("host/native"),
        boot_id: BootId::from("boot/native"),
        base_id: HostBaseId::from("base/native-spi"),
        provider_instance_id: BaseInstanceId::from("provider/native-spi"),
        provider_generation: 1,
        resource_pool_id: ResourcePoolId::from("resource/native-spi"),
        resource_generation_id: ResourceGenerationId("generation/one".into()),
        envelope_id: CapabilityEnvelopeId::from("envelope/native-spi"),
        artifact_id: ArtifactId::from("fixture/native-controller"),
    };
    // SAFETY: callers supply inert or scripted providers without hardware access.
    let ready = unsafe {
        ReadySpiBase::new(
            identity.clone(),
            SpiAttachment {
                generation: 1,
                resource_bytes: 512,
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
    let grants = [AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/native-spi"),
        contract_id: AuthorityContractId::from(SPI_AUTHORITY),
        host_call_contract_id: HostCallContractId::from(SPI_CALL),
        subject_kind: contract.kind().kind_id.clone(),
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        capability_id: CapabilityId::from("conduitos/spi-exchange@1"),
    }];
    let external = host.capabilities[0].clone();
    let hosts = [host];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let limits = crate::protocol_kernel_fixture::queue_limits(
        &authoring,
        &hosts,
        &placements,
        SPI_MAXIMUM_BYTES,
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
                connection_byte_capacity: SPI_MAXIMUM_BYTES,
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
    (planned, ready, identity, external)
}

#[test]
fn ordinary_spi_source_plans_only_against_ready_native_possession_and_explicit_grant() {
    let (planned, _, _, _) = planned(Provider);
    let [gear] = planned.fragments[0].placements.as_slice() else {
        panic!("one SPI operation");
    };
    let factory = SpiOperationFactory::prepare_contract().unwrap();
    assert_eq!(
        factory.budget(gear).unwrap().maximum_value_bytes,
        SPI_MAXIMUM_BYTES
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

mod execution;
