//! Root-owned exact Mini PiTFT SPI, digital contacts and calibrated deadline clock.
//! Peripheral commands and waits remain in checked Source.
use super::authority::{CLAIMED, OperationAuthority, authority};
use alloc::{format, vec::Vec};
use conduit_core::*;
use conduitos::{
    cryptographic_entropy::CryptographicEntropyBase,
    digital_base::{
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
use core::sync::atomic::Ordering;

pub type NativeOwners = NativePeripheralOwners<super::providers::Clock>;

/// Actual retained input contacts, reserved for a subsequent ordinary protocol.
/// Admission does not read their level or implement button policy.
pub struct RemainingButtons {
    pub _button_a: super::providers::Contact,
    pub _button_b: super::providers::Contact,
}

/// Admit a single Boot's immutable physical owners before Source planning.
/// `digital_gears` names exactly [backlight, command/data] in that order.
///
/// # Safety
/// The administrator authorizes this exact B+ attachment. Linux line handles
/// exclusively reserve GPIO; the fixed SPI device is selected independently
/// of Source. Other native code must honor the attachment lock. This is a
/// cooperative ownership boundary, not confinement of hostile native code.
pub unsafe fn admit(
    host: &HostId,
    boot: &BootId,
    artifact: &str,
    digital_gears: [GearId; 2],
) -> Result<(NativeOwners, RemainingButtons), &'static str> {
    let (owners, buttons) =
        unsafe { admit_contacts::<4>(host, boot, artifact, digital_gears, None) }?;
    Ok((owners, buttons.ok_or("minipitft-input-retention-missing")?))
}

unsafe fn admit_contacts<const ENTROPY_REQUESTS: u32>(
    host: &HostId,
    boot: &BootId,
    artifact: &str,
    digital_gears: [GearId; 2],
    button_gears: Option<[GearId; 2]>,
) -> Result<(NativeOwners, Option<RemainingButtons>), &'static str> {
    if host.as_str().is_empty()
        || boot.as_str().is_empty()
        || artifact.is_empty()
        || digital_gears.iter().any(|gear| gear.as_str().is_empty())
        || digital_gears[0] == digital_gears[1]
    {
        return Err("minipitft-owner-identity-invalid");
    }
    let mut selected = alloc::collections::BTreeSet::new();
    for gear in digital_gears.iter().chain(button_gears.iter().flatten()) {
        if gear.as_str().is_empty() || !selected.insert(gear) {
            return Err("minipitft-owner-identity-invalid");
        }
    }
    CLAIMED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "protocol-resources-already-reserved")?;
    let physical = super::providers::admit().map_err(|_| "minipitft-physical-admission-refused")?;
    let clock_provider = super::providers::Clock::new();
    let source = super::providers::Entropy::new().map_err(|_| "minipitft-entropy-unavailable")?;
    let mut entropy = CryptographicEntropyBase::<_, ENTROPY_REQUESTS>::admit(source)
        .map_err(|_| "minipitft-entropy-refused")?;
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
    let mut contacts = Vec::with_capacity(if button_gears.is_some() { 4 } else { 2 });
    contacts.extend([
        ("backlight", physical.backlight, digital_gears[0].clone()),
        (
            "command-data",
            physical.command_data,
            digital_gears[1].clone(),
        ),
    ]);
    let buttons = if let Some(gears) = button_gears {
        contacts.extend([
            ("button-a", physical.button_a, gears[0].clone()),
            ("button-b", physical.button_b, gears[1].clone()),
        ]);
        None
    } else {
        Some(RemainingButtons {
            _button_a: physical.button_a,
            _button_b: physical.button_b,
        })
    };
    let mut digital = Vec::with_capacity(contacts.len());
    for (name, provider, gear) in contacts {
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
            sign_storage: conduit_composite::KernelCompositeSignStorage::default(),
            spi,
            spi_issuer,
            digital,
            clock,
            clock_issuer,
        },
        buttons,
    ))
}
