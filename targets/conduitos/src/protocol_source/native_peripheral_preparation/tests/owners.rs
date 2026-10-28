//! Shared inert Ready-owner/issuer admission for native protocol kernel fixtures.
use crate::{
    digital_base::{
        DigitalProvider,
        installation::{DIGITAL_AUTHORITY, DigitalNativeIdentity, ReadyDigitalBase},
        owner::DigitalAttachment,
    },
    monotonic_clock::{
        installation::{CLOCK_AUTHORITY, ClockNativeIdentity, ReadyClockBase},
        owner::{MAXIMUM_POLL_STEPS, MonotonicDeadlineProvider},
    },
    protocol_source::{
        NativeDigitalOwner, NativePeripheralOwners, NativeProtocolIssuer, PreparedProtocolEntry,
    },
    spi_base::{
        SpiProvider,
        installation::{ReadySpiBase, SPI_AUTHORITY, SpiNativeIdentity},
        owner::SpiAttachment,
    },
};
use alloc::{format, vec, vec::Vec};
use conduit_body::BodyLifecycleSession;
use conduit_core::*;
fn authority(
    name: &str,
    contract: &str,
    call: &str,
    kind: &str,
    capability: &str,
    bytes: u32,
    work: u64,
) -> BaseCapabilityAuthority {
    BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: format!("grant/{name}").into(),
            contract_id: contract.into(),
            host_call_contract_id: call.into(),
            subject_kind: kind.into(),
            host_id: "host/peripheral-fixture".into(),
            boot_id: "boot/peripheral-fixture".into(),
            capability_id: capability.into(),
        },
        base_instance_id: format!("provider/{name}").into(),
        base_provider_generation: 1,
        resource_pool_id: format!("resource/{name}").into(),
        resource_generation_id: ResourceGenerationId(format!("generation/{name}/1")),
        operation_contract_id: call.into(),
        envelope_id: format!("envelope/{name}").into(),
        maximum_parameter_bytes: bytes,
        maximum_result_bytes: bytes,
        maximum_work_units: work,
        maximum_in_flight: 1,
        maximum_operations: 4096,
    }
}
fn issuer(a: BaseCapabilityAuthority, key: u8) -> NativeProtocolIssuer {
    NativeProtocolIssuer::from_test_authority(a, [key; 32]).unwrap()
}
pub fn prepare<
    S: SpiProvider + 'static,
    D: DigitalProvider + 'static,
    C: MonotonicDeadlineProvider,
>(
    entry: &PreparedProtocolEntry,
    gears: [GearId; 2],
    spi_provider: S,
    digital_providers: [D; 2],
    clock_provider: C,
) -> (
    BodyLifecycleSession,
    HostAdvertisement,
    NativePeripheralOwners<C>,
) {
    let spi_a = authority(
        "spi",
        SPI_AUTHORITY,
        crate::spi_base::contract::SPI_CALL,
        "machine/spi/exchange",
        "conduitos/spi-exchange@1",
        4096,
        1,
    );
    let clock_a = authority(
        "clock",
        CLOCK_AUTHORITY,
        crate::monotonic_clock::contract::CLOCK_CALL,
        "machine/clock/at",
        "conduitos/monotonic-clock-at@1",
        512,
        MAXIMUM_POLL_STEPS,
    );
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: spi_a.grant.host_id.clone(),
        boot_id: spi_a.grant.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let expanded = &entry.expanded().expanded;
    let session = crate::protocol_test_support::protocol_body_for_resident(
        &conduit_body::ResidentPlot::new(
            expanded.source_document_id.clone(),
            expanded.checked_plot_id.clone(),
        ),
        &host.host_id,
        &host.boot_id,
    );
    let spi = unsafe {
        ReadySpiBase::new(
            SpiNativeIdentity {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                base_id: "base/spi".into(),
                provider_instance_id: spi_a.base_instance_id.clone(),
                provider_generation: 1,
                resource_pool_id: spi_a.resource_pool_id.clone(),
                resource_generation_id: spi_a.resource_generation_id.clone(),
                envelope_id: spi_a.envelope_id.clone(),
                artifact_id: "artifact/fixture".into(),
            },
            SpiAttachment {
                generation: 1,
                resource_bytes: 512,
            },
            spi_provider,
        )
    }
    .unwrap()
    .erase();
    let mut digital = Vec::new();
    for (index, (gear, provider)) in gears.iter().zip(digital_providers).enumerate() {
        let name = format!("contact-{index}");
        let capability = format!("capability/{name}");
        let a = authority(
            &name,
            DIGITAL_AUTHORITY,
            crate::digital_base::contract::DIGITAL_CALL,
            "machine/digital/access",
            &capability,
            512,
            1,
        );
        let ready = unsafe {
            ReadyDigitalBase::new(
                DigitalNativeIdentity {
                    capability_id: capability.into(),
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    base_id: format!("base/{name}").into(),
                    provider_instance_id: a.base_instance_id.clone(),
                    provider_generation: 1,
                    resource_pool_id: a.resource_pool_id.clone(),
                    resource_generation_id: a.resource_generation_id.clone(),
                    envelope_id: a.envelope_id.clone(),
                    artifact_id: "artifact/fixture".into(),
                },
                DigitalAttachment {
                    generation: 1,
                    resource_bytes: 1,
                },
                provider,
            )
        }
        .unwrap()
        .erase();
        digital.push(NativeDigitalOwner {
            gear: gear.clone(),
            ready,
            issuer: issuer(a, index as u8 + 2),
        });
    }
    let clock = unsafe {
        ReadyClockBase::new(
            ClockNativeIdentity {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                base_id: "base/clock".into(),
                provider_instance_id: clock_a.base_instance_id.clone(),
                provider_generation: 1,
                resource_pool_id: clock_a.resource_pool_id.clone(),
                resource_generation_id: clock_a.resource_generation_id.clone(),
                envelope_id: clock_a.envelope_id.clone(),
                artifact_id: "artifact/fixture".into(),
            },
            clock_provider,
        )
    }
    .unwrap();
    (
        session,
        host,
        NativePeripheralOwners {
            sign_storage: conduit_composite::KernelCompositeSignStorage::default(),
            spi,
            spi_issuer: issuer(spi_a, 1),
            digital,
            clock,
            clock_issuer: issuer(clock_a, 4),
        },
    )
}
