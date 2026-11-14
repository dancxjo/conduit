//! Initialized RP2040 providers. Hardware leases retain the one physical owner;
//! describing these objects does not authorize an operation or reserve a Play.
pub use crate::pulse_owner::CaptureLease as Capture;
pub use crate::tm1637_owner::DisplayLease as Wire;
use alloc::format;
use conduit_core::{HostAdvertisement, ResourceGenerationId};
#[cfg(not(record_service))]
use conduit_mcu_host_runtime::recording_equipment as recording;
#[cfg(record_service)]
use conduit_mcu_host_runtime::recording_service_equipment as recording;
use conduit_monotonic_wait::{ReadyMonotonicWaitBase, WaitAttachment, WaitNativeIdentity};
use conduit_pulse_capture::{
    installation::{PulseNativeIdentity, ReadyPulseBase},
    owner::PulseAttachment,
};
use conduit_tm1637_wire::{
    installation::{ReadyWriteBase, WriteNativeIdentity},
    owner::WriteAttachment,
};
use recording::AppendNativeIdentity;
pub const WAIT_SLOTS: usize = crate::workset::generated::sampling::CLOCKS.len();
pub type RecordingStorage = conduit_rp2040_snapshot_nor::SnapshotNor;
#[cfg(not(record_service))]
pub type RecordingReady =
    conduit_resource_snapshot::native_append::ReadyRecordAppender<RecordingStorage>;
#[cfg(record_service)]
pub type RecordingReady =
    conduit_resource_snapshot::native_service::ReadyRecordService<RecordingStorage>;
pub struct Equipment {
    pub activity: conduit_mcu_host_runtime::native_runner::NativeActivity,
    pub recording: recording::Equipment<RecordingStorage>,
    pub renderer: conduit_mcu_host_runtime::execution_mask::ReadyRenderer<crate::renderer::Clock>,
    pub pulse: Option<ReadyPulseBase<crate::pulse_owner::CaptureLease>>,
    pub display: Option<ReadyWriteBase<crate::tm1637_owner::DisplayLease>>,
    pub waits: [Option<ReadyMonotonicWaitBase<MonotonicTimer>>; WAIT_SLOTS],
}
pub struct MonotonicTimer {
    _slot: usize,
}
impl conduit_monotonic_wait::MonotonicWaitProvider for MonotonicTimer {
    async fn wait(
        &mut self,
        duration_ms: u64,
    ) -> Result<(), conduit_monotonic_wait::WaitProviderFailure> {
        embassy_time::Timer::after_millis(duration_ms).await;
        Ok(())
    }
}
macro_rules! identity {
    ($kind:ident,$actual:ident,$slot:literal,$implementation:literal) => {
        $kind {
            host_id: $actual.host_id.clone(),
            boot_id: $actual.boot_id.clone(),
            base_id: concat!("rp2040/", $slot).into(),
            provider_instance_id: format!("{}/{}", $actual.host_id.as_str(), $slot).into(),
            provider_generation: 1,
            resource_pool_id: concat!("pool/rp2040/", $slot).into(),
            resource_generation_id: ResourceGenerationId(format!(
                "{}/{}",
                $actual.boot_id.as_str(),
                $slot
            )),
            envelope_id: concat!("envelope/rp2040/", $slot).into(),
            artifact_id: crate::config::IMAGE_ID.into(),
            implementation_id: $implementation.into(),
            base_implementation_id: $implementation.into(),
        }
    };
}
fn recording_identity(actual: &HostAdvertisement) -> AppendNativeIdentity {
    #[cfg(record_service)]
    {
        identity!(
            AppendNativeIdentity,
            actual,
            "record-service",
            "rp2040/nor-record-service@1"
        )
    }
    #[cfg(not(record_service))]
    {
        identity!(
            AppendNativeIdentity,
            actual,
            "record-append",
            "rp2040/nor-record-append@1"
        )
    }
}
impl Equipment {
    pub fn publish(
        &self,
        actual: &mut HostAdvertisement,
    ) -> Result<(), conduit_mcu_host_runtime::native_images::Refusal> {
        use conduit_mcu_host_runtime::native_images::{Publication, Refusal};
        let remaining = crate::config::HEAP_BYTES
            .checked_sub(crate::heap::usage().0)
            .ok_or(Refusal::Capacity)?;
        let mut publication = Publication::new(
            actual,
            3 + self.waits.iter().flatten().count() + usize::from(self.recording.selected()),
            remaining,
        )?;
        publication.pulse(self.pulse.as_ref().ok_or(Refusal::Context)?)?;
        publication.write(self.display.as_ref().ok_or(Refusal::Context)?)?;
        publication.renderer(&self.renderer)?;
        for wait in self.waits.iter().flatten() {
            publication.wait(wait)?;
        }
        self.recording.publish(&mut publication)?;
        publication.finish()
    }
    pub fn new(
        actual: &HostAdvertisement,
        body: &conduit_body::BodyId,
        flash: crate::credential_source::Device,
    ) -> Option<Self> {
        if WAIT_SLOTS == 0 || WAIT_SLOTS >= crate::config::TIMER_SLOTS {
            return None;
        }
        let pulse = crate::pulse_owner::try_lease()?;
        let display = crate::tm1637_owner::try_lease()?;
        // SAFETY: exact checked hardware has been initialized once and both
        // affine leases above own its only physical access. Timer slots are
        // finite initialized Embassy providers under this Host's checked limit.
        unsafe {
            Some(Self {
                activity: conduit_mcu_host_runtime::native_runner::NativeActivity::new(),
                // No electrical tasks exist yet; this is a bounded read-only
                // recovery using the sole checked DURABLE peripheral owner.
                recording: recording::Equipment::new(
                    conduit_rp2040_snapshot_nor::SnapshotNor::new(
                        flash,
                        conduit_rp2040_snapshot_nor::Slot::new(
                            0,
                            conduit_rp2040_flash_layout::DURABLE.bytes(),
                        )
                        .ok()?,
                    ),
                    recording_identity(actual),
                    body,
                    crate::workset::generated::sampling::NATIVE_KINDS,
                    crate::config::RECORDING_PREFIX_BYTES,
                    crate::config::RECORDING_SLOTS,
                ),
                renderer: crate::renderer::initialize(actual)?,
                pulse: Some(
                    ReadyPulseBase::new(
                        identity!(PulseNativeIdentity, actual, "gpio-pulse", "rp2040/dht11@1"),
                        PulseAttachment {
                            generation: 1,
                            // Fund both the physical provider and its bounded capture result.
                            resource_bytes: (core::mem::size_of::<crate::pulse_owner::Capture>()
                                + core::mem::size_of::<conduit_pulse_capture::PulseReadCapture>())
                                as u64,
                            limits: conduit_rp2040_pulse_capture::LIMITS,
                        },
                        pulse,
                    )
                    .ok()?,
                ),
                display: Some(
                    ReadyWriteBase::new(
                        identity!(WriteNativeIdentity, actual, "tm1637", "rp2040/tm1637@1"),
                        WriteAttachment {
                            generation: 1,
                            resource_bytes: WriteAttachment::retained_bytes_for_provider::<
                                crate::tm1637_owner::Display,
                            >(),
                        },
                        display,
                    )
                    .ok()?,
                ),
                waits: core::array::from_fn(|slot| {
                    let suffix = format!("wait/{slot}");
                    let period = crate::workset::generated::sampling::CLOCKS[slot].1;
                    Some(
                        ReadyMonotonicWaitBase::new(
                            WaitNativeIdentity {
                                host_id: actual.host_id.clone(),
                                boot_id: actual.boot_id.clone(),
                                base_id: format!("rp2040/{suffix}").into(),
                                provider_instance_id: format!(
                                    "{}/{suffix}",
                                    actual.host_id.as_str()
                                )
                                .into(),
                                provider_generation: 1,
                                resource_pool_id: format!("pool/rp2040/{suffix}").into(),
                                resource_generation_id: ResourceGenerationId(format!(
                                    "{}/{suffix}",
                                    actual.boot_id.as_str()
                                )),
                                envelope_id: format!("envelope/rp2040/{suffix}").into(),
                                artifact_id: crate::config::IMAGE_ID.into(),
                                implementation_id: "rp2040/monotonic-clock@1".into(),
                                base_implementation_id: "rp2040/monotonic-clock@1".into(),
                            },
                            WaitAttachment {
                                generation: 1,
                                resource_bytes: core::mem::size_of::<MonotonicTimer>() as u64,
                                maximum_duration_ms: period,
                            },
                            MonotonicTimer { _slot: slot + 1 },
                        )
                        .expect("checked finite Source clock provider"),
                    )
                }),
            })
        }
    }
}
