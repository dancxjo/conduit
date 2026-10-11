//! Root-owned admission identity shared by transport implementations.
use alloc::format;
use conduit_core::*;
use core::sync::atomic::AtomicBool;

pub(super) static CLAIMED: AtomicBool = AtomicBool::new(false);

pub(super) struct OperationAuthority<'a> {
    pub(super) contract: &'a str,
    pub(super) call: &'a str,
    pub(super) kind: &'a str,
    pub(super) capability: &'a str,
    pub(super) bytes: u32,
    pub(super) work: u64,
    pub(super) operations: u32,
}

// All facts here originate in the independently retained native owners, before
// Source planning. No selected placement or authored metadata is consulted.
pub(super) fn authority(
    host: &HostId,
    boot: &BootId,
    name: &str,
    identity: (
        &BaseInstanceId,
        &ResourcePoolId,
        &ResourceGenerationId,
        &CapabilityEnvelopeId,
    ),
    operation: OperationAuthority<'_>,
) -> BaseCapabilityAuthority {
    let (provider, pool, generation, envelope) = identity;
    BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: format!("conduitos/protocol/{}/{name}/grant", boot.as_str()).into(),
            contract_id: operation.contract.into(),
            host_call_contract_id: operation.call.into(),
            subject_kind: operation.kind.into(),
            host_id: host.clone(),
            boot_id: boot.clone(),
            capability_id: operation.capability.into(),
        },
        base_instance_id: provider.clone(),
        base_provider_generation: 1,
        resource_pool_id: pool.clone(),
        resource_generation_id: generation.clone(),
        operation_contract_id: operation.call.into(),
        envelope_id: envelope.clone(),
        maximum_parameter_bytes: operation.bytes,
        maximum_result_bytes: operation.bytes,
        maximum_work_units: operation.work,
        maximum_in_flight: 1,
        maximum_operations: operation.operations,
    }
}
