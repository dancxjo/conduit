//! One Root-owned native I2C/clock admission per Boot; no peripheral protocol.
use super::request::ProtocolBootRequest;
use crate::{
    arch,
    cryptographic_entropy::CryptographicEntropyBase,
    i2c_base::{
        i801::I801Controller,
        installation::{I2cNativeIdentity, ReadyI2cBase},
        owner::I2cAttachment,
    },
    monotonic_clock::installation::{ClockNativeIdentity, ReadyClockBase},
    protocol_source::{NativeProtocolIssuer, NativeProtocolOwners},
};
use alloc::format;
use conduit_core::*;
use core::sync::atomic::{AtomicBool, Ordering};

static CLAIMED: AtomicBool = AtomicBool::new(false);
pub type NativeOwners =
    NativeProtocolOwners<I801Controller<arch::I801PortWindow>, arch::NativeMonotonicDeadlineClock>;

/// # Safety
/// Called only by the sole native composition Root before other PCI users.
/// The local boot administrator must control the request module and have
/// independently released firmware use and approved the electrical attachment.
/// Root owns the selected function/window and calibrated clock for this Boot.
/// Source, discovery and a decoded request alone do not establish these facts.
/// The reservation is never reassigned during this Boot, including failed stop.
pub unsafe fn admit(
    request: &ProtocolBootRequest,
    host: &HostId,
    boot: &BootId,
    artifact: &str,
) -> Result<NativeOwners, &'static str> {
    CLAIMED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "protocol-resources-already-reserved")?;
    let bus_provider = unsafe {
        arch::admitted_i801_pci_ports(
            request.pci_bus,
            request.pci_device,
            request.pci_function,
            request.maximum_polls,
        )
    }
    .map_err(|_| "protocol-controller-unavailable")?;
    let clock_provider =
        unsafe { arch::admitted_monotonic_deadline_clock(request.clock_lifetime_millis) }
            .map_err(|_| "protocol-clock-unavailable")?;
    let prefix = format!(
        "conduitos/protocol/{}/{:02x}:{:02x}.{}",
        boot.as_str(),
        request.pci_bus,
        request.pci_device,
        request.pci_function
    );
    let bus_identity = I2cNativeIdentity {
        host_id: host.clone(),
        boot_id: boot.clone(),
        base_id: format!("{prefix}/bus").into(),
        provider_instance_id: format!("{prefix}/bus-provider").into(),
        provider_generation: 1,
        resource_pool_id: format!("{prefix}/attachment").into(),
        resource_generation_id: ResourceGenerationId(format!("{prefix}/attachment/1")),
        envelope_id: format!("{prefix}/bus-envelope").into(),
        artifact_id: artifact.into(),
    };
    let clock_identity = ClockNativeIdentity {
        host_id: host.clone(),
        boot_id: boot.clone(),
        base_id: format!("{prefix}/clock").into(),
        provider_instance_id: format!("{prefix}/clock-provider").into(),
        provider_generation: 1,
        resource_pool_id: format!("{prefix}/clock-basis").into(),
        resource_generation_id: ResourceGenerationId(format!("{prefix}/clock-basis/1")),
        envelope_id: format!("{prefix}/clock-envelope").into(),
        artifact_id: artifact.into(),
    };
    let bus_authority = authority(
        host,
        boot,
        "bus",
        (
            &bus_identity.provider_instance_id,
            &bus_identity.resource_pool_id,
            &bus_identity.resource_generation_id,
            &bus_identity.envelope_id,
        ),
        OperationAuthority {
            contract: crate::i2c_base::installation::I2C_AUTHORITY,
            call: crate::i2c_base::contract::I2C_CALL,
            kind: "machine/i2c/transact",
            capability: "conduitos/i2c-transaction@1",
            bytes: crate::i2c_base::contract::I2C_MAXIMUM_BYTES,
            work: 1,
            operations: request.maximum_bus_operations,
        },
    );
    let clock_authority = authority(
        host,
        boot,
        "clock",
        (
            &clock_identity.provider_instance_id,
            &clock_identity.resource_pool_id,
            &clock_identity.resource_generation_id,
            &clock_identity.envelope_id,
        ),
        OperationAuthority {
            contract: crate::monotonic_clock::installation::CLOCK_AUTHORITY,
            call: crate::monotonic_clock::contract::CLOCK_CALL,
            kind: "machine/clock/at",
            capability: "conduitos/monotonic-clock-at@1",
            bytes: crate::monotonic_clock::contract::CLOCK_MAXIMUM_BYTES,
            work: crate::monotonic_clock::owner::MAXIMUM_POLL_STEPS,
            operations: request.maximum_clock_operations,
        },
    );
    let bus = unsafe {
        ReadyI2cBase::new(
            bus_identity,
            I2cAttachment {
                generation: 1,
                minimum_address: request.minimum_address,
                maximum_address: request.maximum_address,
                resource_bytes: 32,
            },
            bus_provider,
        )
    }
    .map_err(|_| "protocol-bus-identity-invalid")?;
    let clock = unsafe { ReadyClockBase::new(clock_identity, clock_provider) }
        .map_err(|_| "protocol-clock-identity-invalid")?;
    let source = arch::RdrandEntropy::detect(1).map_err(|_| "protocol-entropy-unavailable")?;
    let mut entropy =
        CryptographicEntropyBase::<_, 2>::admit(source).map_err(|_| "protocol-entropy-refused")?;
    let bus_issuer = unsafe { NativeProtocolIssuer::admit(bus_authority, &mut entropy) }
        .map_err(|_| "protocol-bus-issuance-refused")?;
    let clock_issuer = unsafe { NativeProtocolIssuer::admit(clock_authority, &mut entropy) }
        .map_err(|_| "protocol-clock-issuance-refused")?;
    Ok(NativeProtocolOwners {
        bus,
        clock,
        bus_issuer,
        clock_issuer,
    })
}

struct OperationAuthority<'a> {
    contract: &'a str,
    call: &'a str,
    kind: &'a str,
    capability: &'a str,
    bytes: u32,
    work: u64,
    operations: u32,
}

// All facts here originate in the independently retained native owners, before
// Source planning. No selected placement or authored metadata is consulted.
fn authority(
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
