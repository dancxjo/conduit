//! Root-owned exact Mini PiTFT SPI, digital contacts and calibrated deadline clock.
//! Peripheral commands and waits remain in checked Source.
use super::authority::{CLAIMED, OperationAuthority, authority};
use crate::{
    arch,
    cryptographic_entropy::CryptographicEntropyBase,
    digital_base::{
        bcm2835::Pin,
        contract::{DIGITAL_CALL, DIGITAL_KIND, DIGITAL_MAXIMUM_BYTES},
        installation::{DIGITAL_AUTHORITY, DigitalNativeIdentity, ReadyDigitalBase},
        owner::DigitalAttachment,
    },
    monotonic_clock::{
        contract::{CLOCK_CALL, CLOCK_MAXIMUM_BYTES},
        installation::{CLOCK_AUTHORITY, ClockNativeIdentity, ReadyClockBase},
        owner::MAXIMUM_POLL_STEPS,
    },
    protocol_source::{NativeDigitalOwner, NativePeripheralOwners, NativeProtocolIssuer},
    spi_base::{
        contract::{SPI_CALL, SPI_KIND, SPI_MAXIMUM_BYTES},
        installation::{ReadySpiBase, SPI_AUTHORITY, SpiNativeIdentity},
        owner::SpiAttachment,
        transaction::MAXIMUM_TRANSACTION_BYTES,
    },
};
use alloc::{format, vec::Vec};
use conduit_core::*;
use core::sync::atomic::Ordering;

pub type NativeOwners =
    NativePeripheralOwners<arch::deadline::DeadlineClock<arch::deadline::SystemTimer>>;

/// Actual retained input contacts, reserved for a subsequent ordinary protocol.
/// Admission does not read their level or implement button policy.
pub struct RemainingButtons {
    pub button_a: Pin<arch::minipitft::GpioRegisters>,
    pub button_b: Pin<arch::minipitft::GpioRegisters>,
}

/// Admit a single Boot's immutable physical owners before Source planning.
/// `digital_gears` names exactly [backlight, command/data] in that order.
///
/// # Safety
/// Trusted native Root must independently authorize this exact B+ v1.2 physical
/// attachment and exclusive SPI0/GPIO/RNG use, including against firmware, DMA
/// and interrupts. IRQs must be disabled and peripheral MMIO correctly mapped.
/// Root must establish the fixed 250 MHz core clock and BCM2835 system timer's
/// one-microsecond calibration. The RNG quality is a hardware trust assumption.
/// Source gear identities select already approved contacts; they grant no
/// physical authority. This reservation is never reassigned within this Boot.
pub unsafe fn admit(
    host: &HostId,
    boot: &BootId,
    artifact: &str,
    digital_gears: [GearId; 2],
) -> Result<(NativeOwners, RemainingButtons), &'static str> {
    if host.as_str().is_empty()
        || boot.as_str().is_empty()
        || artifact.is_empty()
        || digital_gears.iter().any(|gear| gear.as_str().is_empty())
        || digital_gears[0] == digital_gears[1]
    {
        return Err("minipitft-owner-identity-invalid");
    }
    CLAIMED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "protocol-resources-already-reserved")?;
    let physical =
        unsafe { arch::minipitft::admit() }.map_err(|_| "minipitft-physical-admission-refused")?;
    let clock_provider = unsafe { arch::deadline::SystemTimer::admit() };
    let source =
        unsafe { arch::entropy::detect(1) }.map_err(|_| "minipitft-entropy-unavailable")?;
    let mut entropy =
        CryptographicEntropyBase::<_, 4>::admit(source).map_err(|_| "minipitft-entropy-refused")?;
    let prefix = format!("conduitos/protocol/{}/minipitft", boot.as_str());
    let spi_identity = SpiNativeIdentity {
        host_id: host.clone(),
        boot_id: boot.clone(),
        base_id: format!("{prefix}/spi").into(),
        provider_instance_id: format!("{prefix}/spi-provider").into(),
        provider_generation: 1,
        resource_pool_id: format!("{prefix}/spi-attachment").into(),
        resource_generation_id: ResourceGenerationId(format!("{prefix}/spi-attachment/1")),
        envelope_id: format!("{prefix}/spi-envelope").into(),
        artifact_id: artifact.into(),
    };
    let spi_authority = authority(
        host,
        boot,
        "minipitft-spi",
        (
            &spi_identity.provider_instance_id,
            &spi_identity.resource_pool_id,
            &spi_identity.resource_generation_id,
            &spi_identity.envelope_id,
        ),
        OperationAuthority {
            contract: SPI_AUTHORITY,
            call: SPI_CALL,
            kind: SPI_KIND,
            capability: "conduitos/spi-exchange@1",
            bytes: SPI_MAXIMUM_BYTES,
            work: 1,
            operations: 4096,
        },
    );
    let spi = unsafe {
        ReadySpiBase::new(
            spi_identity,
            SpiAttachment {
                generation: 1,
                resource_bytes: MAXIMUM_TRANSACTION_BYTES as u64,
            },
            physical.spi,
        )
    }
    .map_err(|_| "minipitft-spi-identity-invalid")?
    .erase();
    let spi_issuer = unsafe { NativeProtocolIssuer::admit(spi_authority, &mut entropy) }
        .map_err(|_| "minipitft-spi-issuance-refused")?;
    let mut digital = Vec::with_capacity(2);
    for ((name, provider), gear) in [
        ("backlight", physical.backlight),
        ("command-data", physical.command_data),
    ]
    .into_iter()
    .zip(digital_gears)
    {
        let capability = format!("{prefix}/{name}/access");
        let identity = DigitalNativeIdentity {
            capability_id: capability.clone().into(),
            host_id: host.clone(),
            boot_id: boot.clone(),
            base_id: format!("{prefix}/{name}").into(),
            provider_instance_id: format!("{prefix}/{name}-provider").into(),
            provider_generation: 1,
            resource_pool_id: format!("{prefix}/{name}-attachment").into(),
            resource_generation_id: ResourceGenerationId(format!("{prefix}/{name}-attachment/1")),
            envelope_id: format!("{prefix}/{name}-envelope").into(),
            artifact_id: artifact.into(),
        };
        let grant = authority(
            host,
            boot,
            &format!("minipitft-{name}"),
            (
                &identity.provider_instance_id,
                &identity.resource_pool_id,
                &identity.resource_generation_id,
                &identity.envelope_id,
            ),
            OperationAuthority {
                contract: DIGITAL_AUTHORITY,
                call: DIGITAL_CALL,
                kind: DIGITAL_KIND,
                capability: &capability,
                bytes: DIGITAL_MAXIMUM_BYTES,
                work: 1,
                operations: 4096,
            },
        );
        let ready = unsafe {
            ReadyDigitalBase::new(
                identity,
                DigitalAttachment {
                    generation: 1,
                    resource_bytes: 1,
                },
                provider,
            )
        }
        .map_err(|_| "minipitft-digital-identity-invalid")?
        .erase();
        let issuer = unsafe { NativeProtocolIssuer::admit(grant, &mut entropy) }
            .map_err(|_| "minipitft-digital-issuance-refused")?;
        digital.push(NativeDigitalOwner {
            gear,
            ready,
            issuer,
        });
    }
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
    let clock_authority = authority(
        host,
        boot,
        "minipitft-clock",
        (
            &clock_identity.provider_instance_id,
            &clock_identity.resource_pool_id,
            &clock_identity.resource_generation_id,
            &clock_identity.envelope_id,
        ),
        OperationAuthority {
            contract: CLOCK_AUTHORITY,
            call: CLOCK_CALL,
            kind: "machine/clock/at",
            capability: "conduitos/monotonic-clock-at@1",
            bytes: CLOCK_MAXIMUM_BYTES,
            work: MAXIMUM_POLL_STEPS,
            operations: 512,
        },
    );
    let clock = unsafe { ReadyClockBase::new(clock_identity, clock_provider) }
        .map_err(|_| "minipitft-clock-identity-invalid")?;
    let clock_issuer = unsafe { NativeProtocolIssuer::admit(clock_authority, &mut entropy) }
        .map_err(|_| "minipitft-clock-issuance-refused")?;
    Ok((
        NativePeripheralOwners {
            spi,
            spi_issuer,
            digital,
            clock,
            clock_issuer,
        },
        RemainingButtons {
            button_a: physical.button_a,
            button_b: physical.button_b,
        },
    ))
}
