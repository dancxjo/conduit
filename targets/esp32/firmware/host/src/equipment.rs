//! Actual peripheral custody. These descriptive identities confer no authority.
use alloc::format;
use conduit_core::{HostAdvertisement, ResourceGenerationId};
use conduit_esp32_electrical::{Esp32PulseCapture, Esp32Tm1637, PULSE_LIMITS};
use conduit_mcu_host_runtime::recording_equipment::AppendNativeIdentity;
use conduit_pulse_capture::{
    installation::{PulseNativeIdentity, ReadyPulseBase},
    owner::PulseAttachment,
};
use conduit_tm1637_wire::{
    installation::{ReadyWriteBase, WriteNativeIdentity},
    owner::WriteAttachment,
};
pub const SOURCE_WAIT_SLOTS: usize = crate::workset::sampling::CLOCKS.len();
pub type RecordingStorage = conduit_esp32_snapshot_nor::hardware::Esp32SnapshotNor<'static>;
pub struct Equipment {
    pub activity: conduit_mcu_host_runtime::native_runner::NativeActivity,
    pub recording: conduit_mcu_host_runtime::recording_equipment::Equipment<RecordingStorage>,
    pub renderer: conduit_mcu_host_runtime::execution_mask::ReadyRenderer<crate::renderer::Clock>,
    pub pulse: Option<ReadyPulseBase<Capture>>,
    pub waits: [Option<
        conduit_monotonic_wait::ReadyMonotonicWaitBase<crate::runtime::MonotonicTimer>,
    >; SOURCE_WAIT_SLOTS],
    pub display: Option<ReadyWriteBase<Esp32Tm1637<'static>>>,
}
macro_rules! identity {
    ($kind:ident,$actual:ident,$slot:literal,$implementation:literal) => {
        $kind {
            host_id: $actual.host_id.clone(),
            boot_id: $actual.boot_id.clone(),
            base_id: concat!("esp32/", $slot).into(),
            provider_instance_id: format!("{}/{}", $actual.host_id.as_str(), $slot).into(),
            provider_generation: 1,
            resource_pool_id: concat!("pool/esp32/", $slot).into(),
            resource_generation_id: ResourceGenerationId(format!(
                "{}/{}",
                $actual.boot_id.as_str(),
                $slot
            )),
            envelope_id: concat!("envelope/esp32/", $slot).into(),
            artifact_id: crate::config::IMAGE_ID.into(),
            implementation_id: $implementation.into(),
            base_implementation_id: $implementation.into(),
        }
    };
}
impl Equipment {
    /// Publish only actual retained providers, with no decoded capability graph.
    /// All containing-vector and descriptive backing allocations are funded.
    pub fn publish(
        &self,
        actual: &mut HostAdvertisement,
    ) -> Result<(), conduit_mcu_host_runtime::native_images::Refusal> {
        use conduit_mcu_host_runtime::native_images::{Publication, Refusal};
        let remaining = crate::config::HEAP_BYTES
            .checked_sub(crate::runtime::allocator_used())
            .ok_or(Refusal::Capacity)?;
        let mut publication = Publication::new(
            actual,
            3 + self.waits.iter().flatten().count() + usize::from(self.recording.selected()),
            remaining,
        )?;
        publication.pulse(self.pulse.as_ref().ok_or(Refusal::Capacity)?)?;
        publication.write(self.display.as_ref().ok_or(Refusal::Capacity)?)?;
        publication.renderer(&self.renderer)?;
        for wait in self.waits.iter().flatten() {
            publication.wait(wait)?;
        }
        self.recording.publish(&mut publication)?;
        publication.finish()
    }

    pub fn new(
        actual: &HostAdvertisement,
        body: &conduit_core::BodyId,
        pulse: Esp32PulseCapture<'static>,
        display: Esp32Tm1637<'static>,
        recording_storage: conduit_esp32_snapshot_nor::hardware::Esp32SnapshotNor<'static>,
    ) -> Self {
        assert!(
            crate::workset::sampling::CLOCKS.len() + 1 <= crate::config::WAIT_SLOTS,
            "actual renderer and Source clocks exceed checked timer slot count"
        );
        assert!(
            crate::workset::sampling::CLOCKS
                .iter()
                .all(|clock| clock.1 <= crate::config::MAXIMUM_WAIT_MS),
            "Source cadence exceeds actual initialized timer duration"
        );
        // SAFETY: main consumed the exact checked GPIO resources, initialized
        // the physical providers and transfers exclusive ownership here. Both
        // providers terminate within finite electrical bounds and release lines
        // on cancellation/drop. No received assignment can construct this owner.
        unsafe {
            Self {
                activity: conduit_mcu_host_runtime::native_runner::NativeActivity::new(),
                // Sole checked owner; only selected immutable Source enables
                // the finite read-only recovery before any electrical task.
                recording: conduit_mcu_host_runtime::recording_equipment::Equipment::new(
                    recording_storage,
                    identity!(
                        AppendNativeIdentity,
                        actual,
                        "record-append",
                        "esp32/nor-record-append@1"
                    ),
                    body,
                    crate::workset::sampling::NATIVE_KINDS,
                    crate::config::RECORDING_PREFIX_BYTES,
                    crate::config::RECORDING_SLOTS,
                ),
                renderer: crate::renderer::initialize(actual),
                waits: core::array::from_fn(|index| {
                    // The renderer owns slot0; only actual Source clocks receive
                    // independent stateful wait owners, starting at physicalslot1.
                    let slot = index + 1;
                    let suffix = format!("wait/{slot}");
                    Some(conduit_monotonic_wait::ReadyMonotonicWaitBase::new(
                        conduit_monotonic_wait::WaitNativeIdentity {
                            host_id:actual.host_id.clone(),boot_id:actual.boot_id.clone(),
                            base_id:format!("esp32/{suffix}").into(),
                            provider_instance_id:format!("{}/{suffix}",actual.host_id.as_str()).into(),
                            provider_generation:1,resource_pool_id:format!("pool/esp32/{suffix}").into(),
                            resource_generation_id:ResourceGenerationId(format!("{}/{suffix}",actual.boot_id.as_str())),
                            envelope_id:format!("envelope/esp32/{suffix}").into(),
                            artifact_id:crate::config::IMAGE_ID.into(),
                            implementation_id:"esp32/monotonic-clock@1".into(),
                            base_implementation_id:"esp32/monotonic-clock@1".into(),
                        },
                        conduit_monotonic_wait::WaitAttachment {generation:1,resource_bytes:core::mem::size_of::<crate::runtime::MonotonicTimer>() as u64,
                            maximum_duration_ms:crate::config::MAXIMUM_WAIT_MS},
                        crate::runtime::MonotonicTimer::new(slot),
                    ).expect("actual initialized clock slot"))
                }),
                pulse: Some(
                    ReadyPulseBase::new(
                        identity!(PulseNativeIdentity, actual, "gpio-pulse", "esp32/dht11@1"),
                        PulseAttachment {
                            generation: 1,
                            // Fund both the physical provider and its bounded capture result.
                            resource_bytes: (core::mem::size_of::<Esp32PulseCapture>()
                                + core::mem::size_of::<conduit_pulse_capture::PulseReadCapture>())
                                as u64,
                            limits: PULSE_LIMITS,
                        },
                        Capture(pulse),
                    )
                    .expect("actual pulse attachment"),
                ),
                display: Some(
                    ReadyWriteBase::new(
                        identity!(WriteNativeIdentity, actual, "tm1637", "esp32/tm1637@1"),
                        WriteAttachment {
                            generation: 1,
                            resource_bytes: WriteAttachment::retained_bytes_for_provider::<
                                Esp32Tm1637,
                            >(),
                        },
                        display,
                    )
                    .expect("actual display attachment"),
                ),
            }
        }
    }
}

/// The electrical provider's class ceiling is narrowed by this checked physical
/// construction; Source remains responsible for decoding the returned octets.
pub struct Capture(Esp32PulseCapture<'static>);
impl conduit_pulse_capture::owner::PulseReadProvider for Capture {
    async fn read(
        &mut self,
        request: conduit_pulse_capture::CheckedPulseRead,
    ) -> Result<conduit_pulse_capture::PulseReadCapture, conduit_pulse_capture::PulseReadRefusal>
    {
        let geometry = request.request();
        if geometry.hold_low_us != crate::config::DHT_START_LOW_US {
            return Err(conduit_pulse_capture::PulseReadRefusal::DriveExtent);
        }
        if geometry
            .hold_low_us
            .saturating_add(geometry.capture_timeout_us)
            > crate::config::DHT_TRANSACTION_US
        {
            return Err(conduit_pulse_capture::PulseReadRefusal::DeadlineExtent);
        }
        self.0.read(request).await
    }
}

pub type Wire = Esp32Tm1637<'static>;
pub use crate::runtime::MonotonicTimer;
pub const WAIT_SLOTS: usize = SOURCE_WAIT_SLOTS;
