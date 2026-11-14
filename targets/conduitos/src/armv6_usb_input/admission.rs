//! Only the containing native Root issues these hardware-backed scopes.
use crate::{
    arch,
    cryptographic_entropy::CryptographicEntropyBase,
    identity::{self, BootIdentities},
    make::EMBEDDED_MAKE,
    monotonic_clock::installation::*,
    protocol_source::NativeProtocolIssuer,
    usb_base::{
        control_contract::*, control_factory::*, endpoint_read_contract::*,
        endpoint_read_factory::*, installation::UsbNativeIdentity,
    },
};
use alloc::format;
use conduit_core::*;
use conduit_monotonic_wait::clock_at::{contract::*, owner::MAXIMUM_POLL_STEPS};
pub(super) type Entropy = CryptographicEntropyBase<arch::HardwareEntropy, 3>;
pub(super) fn usb_identity(ids: &BootIdentities, role: &str) -> UsbNativeIdentity {
    let boot = identity::hex(&ids.boot);
    UsbNativeIdentity {
        host_id: identity::hex(&ids.host).into(),
        boot_id: boot.clone().into(),
        base_id: identity::hex(&identity::derive_base(&ids.boot, "conduitos/usb/dwc2/0")).into(),
        provider_instance_id: format!("conduitos/dwc2/{boot}/{role}").into(),
        provider_generation: 1,
        resource_pool_id: format!("conduitos/dwc2/{role}").into(),
        resource_generation_id: ResourceGenerationId(format!("{boot}/{role}/1")),
        envelope_id: format!("conduitos/dwc2/{role}/bounded").into(),
        artifact_id: format!("{}/{role}", EMBEDDED_MAKE.build_id).into(),
    }
}
/// # Safety
/// Caller owns the original prepared controller, its electrical/address scope,
/// and actual reserved PIO storage; data and discovery cannot authorize this.
pub(super) unsafe fn usb_issuer(
    identity: &UsbNativeIdentity,
    endpoint: bool,
    entropy: &mut Entropy,
) -> Result<NativeProtocolIssuer, &'static str> {
    let (kind, call, capability, authority, operations) = if endpoint {
        (
            ENDPOINT_READ_KIND,
            ENDPOINT_READ_CALL,
            ENDPOINT_READ_IMPLEMENTATION,
            ENDPOINT_READ_AUTHORITY,
            4096,
        )
    } else {
        (
            CONTROL_KIND,
            CONTROL_CALL,
            CONTROL_IMPLEMENTATION,
            CONTROL_AUTHORITY,
            512,
        )
    };
    let scope = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: format!("{}/authority", identity.provider_instance_id.as_str()).into(),
            contract_id: authority.into(),
            host_call_contract_id: call.into(),
            subject_kind: kind.into(),
            host_id: identity.host_id.clone(),
            boot_id: identity.boot_id.clone(),
            capability_id: capability.into(),
        },
        base_instance_id: identity.provider_instance_id.clone(),
        base_provider_generation: identity.provider_generation,
        resource_pool_id: identity.resource_pool_id.clone(),
        resource_generation_id: identity.resource_generation_id.clone(),
        envelope_id: identity.envelope_id.clone(),
        operation_contract_id: call.into(),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: operations,
    };
    unsafe { NativeProtocolIssuer::admit(scope, entropy) }.map_err(|_| "usb-native-issuer-refused")
}
pub(super) fn clock_identity(ids: &BootIdentities) -> ClockNativeIdentity {
    let boot = identity::hex(&ids.boot);
    ClockNativeIdentity {
        host_id: identity::hex(&ids.host).into(),
        boot_id: boot.clone().into(),
        base_id: identity::hex(&identity::derive_base(
            &ids.boot,
            "conduitos/clock/counter/0",
        ))
        .into(),
        provider_instance_id: format!("conduitos/counter/{boot}").into(),
        provider_generation: 1,
        resource_pool_id: "conduitos/counter/waits".into(),
        resource_generation_id: ResourceGenerationId(boot),
        envelope_id: "conduitos/counter/bounded".into(),
        artifact_id: format!("{}/counter", EMBEDDED_MAKE.build_id).into(),
    }
}
/// # Safety
/// Root owns the actual counter's calibrated millisecond basis and native timer
/// wake path. The retained provider must prevent completion after revocation.
pub(super) unsafe fn clock_issuer(
    identity: &ClockNativeIdentity,
    entropy: &mut Entropy,
) -> Result<NativeProtocolIssuer, &'static str> {
    let scope = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: format!("{}/authority", identity.provider_instance_id.as_str()).into(),
            contract_id: CLOCK_AUTHORITY.into(),
            host_call_contract_id: CLOCK_CALL.into(),
            subject_kind: CLOCK_KIND.into(),
            host_id: identity.host_id.clone(),
            boot_id: identity.boot_id.clone(),
            capability_id: CLOCK_IMPLEMENTATION.into(),
        },
        base_instance_id: identity.provider_instance_id.clone(),
        base_provider_generation: 1,
        resource_pool_id: identity.resource_pool_id.clone(),
        resource_generation_id: identity.resource_generation_id.clone(),
        envelope_id: identity.envelope_id.clone(),
        operation_contract_id: CLOCK_CALL.into(),
        maximum_parameter_bytes: 512,
        maximum_result_bytes: 512,
        maximum_work_units: MAXIMUM_POLL_STEPS,
        maximum_in_flight: 1,
        maximum_operations: 512,
    };
    unsafe { NativeProtocolIssuer::admit(scope, entropy) }.map_err(|_| "usb-clock-issuer-refused")
}
