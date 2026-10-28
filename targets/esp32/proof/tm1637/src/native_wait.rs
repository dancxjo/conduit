//! One retained software timer slot over Root's initialized RTOS monotonic clock.
//! This projects readiness only; no wait is dispatched without selected custody.
use conduit_monotonic_wait::{
    MonotonicWaitProvider, ReadyMonotonicWaitBase, WaitAttachment, WaitInstallationRefusal,
    WaitNativeIdentity, WaitProviderFailure,
};
const MAXIMUM_DURATION_MS: u64 = 60_000;
pub struct EspWait {
    #[cfg(feature = "local-appliance")]
    maintenance: Option<crate::native_flash_domain::Lane>,
}
impl MonotonicWaitProvider for EspWait {
    async fn wait(&mut self, duration_ms: u64) -> Result<(), WaitProviderFailure> {
        if duration_ms > MAXIMUM_DURATION_MS {
            return Err(WaitProviderFailure::ProviderUnavailable);
        }
        #[cfg(feature = "local-appliance")]
        if let Some(lane) = self.maintenance.as_mut() {
            return lane
                .wait(duration_ms, crate::native_flash_domain::advance)
                .await
                .map_err(|_| WaitProviderFailure::ProviderUnavailable);
        }
        embassy_time::Timer::after(embassy_time::Duration::from_millis(duration_ms)).await;
        Ok(())
    }
}
/// # Safety
/// Root must have initialized `esp_rtos::start` with its actual hardware timer
/// and retain one finite pending-wait slot in this Boot. The complete task/clock
/// and kernel memory/work closure must still be admitted before starting Play.
pub unsafe fn prepare(
    boot: &crate::boot::ObservedBoot,
) -> Result<ReadyMonotonicWaitBase<EspWait>, WaitInstallationRefusal> {
    let provider = EspWait {
        #[cfg(feature = "local-appliance")]
        maintenance: None,
    };
    // SAFETY: forward the actual initialized-clock/one-slot caller contract.
    unsafe { prepare_provider(boot, provider) }
}
/// # Safety
/// Same initialized RTOS clock and unique wait-slot custody as `prepare`.
/// The lane must come from this Boot's actual private arbitration domain.
#[cfg(feature = "local-appliance")]
pub unsafe fn prepare_with_maintenance(
    boot: &crate::boot::ObservedBoot,
    lane: crate::native_flash_domain::Lane,
) -> Result<ReadyMonotonicWaitBase<EspWait>, WaitInstallationRefusal> {
    // SAFETY: same actual-clock contract, with the uniquely returned endpoint.
    unsafe {
        prepare_provider(
            boot,
            EspWait {
                maintenance: Some(lane),
            },
        )
    }
}
unsafe fn prepare_provider(
    boot: &crate::boot::ObservedBoot,
    mut provider: EspWait,
) -> Result<ReadyMonotonicWaitBase<EspWait>, WaitInstallationRefusal> {
    let (host, boot) = boot.ids();
    let identity = WaitNativeIdentity {
        host_id: host.into(),
        boot_id: boot.into(),
        base_id: "base/esp32/monotonic-wait".into(),
        provider_instance_id: "instance/esp32/monotonic-wait".into(),
        provider_generation: 1,
        resource_pool_id: "pool/esp32/monotonic-wait".into(),
        resource_generation_id: conduit_core::ResourceGenerationId(
            "generation/monotonic-wait/1".into(),
        ),
        envelope_id: "envelope/esp32/monotonic-wait".into(),
        artifact_id: "native/esp32-monotonic-wait@1".into(),
        implementation_id: "esp32/monotonic-wait@1".into(),
        base_implementation_id: "esp32/monotonic-wait-base@1".into(),
    };
    // Constructing/dropping an unpolled future starts no timer. Retain the exact
    // future-state storage requirement, not a descriptor-derived byte count.
    let resource_bytes = {
        let pending = provider.wait(MAXIMUM_DURATION_MS);
        core::mem::size_of_val(&pending) as u64
    };
    // SAFETY: caller retains actual RTOS initialization and this unique slot;
    // the concrete provider has no allocation or detached task, and its dropped
    // future cannot produce a completion under a revoked capability.
    unsafe {
        ReadyMonotonicWaitBase::new(
            identity,
            WaitAttachment {
                generation: 1,
                resource_bytes,
                maximum_duration_ms: MAXIMUM_DURATION_MS,
            },
            provider,
        )
    }
}
