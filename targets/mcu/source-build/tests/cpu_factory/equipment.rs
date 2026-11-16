//! Finite CPU providers; all effects require original authenticated Start.
use conduit_core::{HostAdvertisement, ResourceGenerationId};
use conduit_mcu_host_runtime::recording_service_equipment as recording;
use conduit_pulse_capture::{
    installation::{PulseNativeIdentity, ReadyPulseBase},
    owner::PulseAttachment,
};
use conduit_tm1637_wire::{
    installation::{ReadyWriteBase, WriteNativeIdentity},
    owner::WriteAttachment,
};
use recording::AppendNativeIdentity;
const SOURCE_WAIT_SLOTS: usize = crate::workset::sampling::CLOCKS.len();
const PULSE_LIMITS: conduit_pulse_capture::PulseReadLimits =
    conduit_pulse_capture::PulseReadLimits {
        maximum_hold_low_us: 50_000,
        maximum_skipped_pulses: 4,
        minimum_sample_offset_us: 10,
        maximum_sample_offset_us: 200,
        maximum_octets: 8,
        maximum_capture_timeout_us: 50_000,
    };

pub const WAIT_SLOTS: usize = crate::workset::sampling::CLOCKS.len();
pub use crate::runtime::MonotonicTimer;
pub struct Capture;
impl conduit_pulse_capture::owner::PulseReadProvider for Capture {
    async fn read(
        &mut self,
        request: conduit_pulse_capture::CheckedPulseRead,
    ) -> Result<conduit_pulse_capture::PulseReadCapture, conduit_pulse_capture::PulseReadRefusal>
    {
        Ok(crate::effects::capture(request))
    }
}
pub struct Wire;
impl conduit_tm1637_wire::OpenDrain for Wire {
    type Error = ();
    fn begin_transaction(&mut self) -> Result<(), ()> {
        crate::effects::begin_wire();
        Ok(())
    }
    fn clock(&mut self, released: bool) -> Result<(), ()> {
        crate::effects::clock(released);
        Ok(())
    }
    fn data(&mut self, released: bool) -> Result<(), ()> {
        crate::effects::data(released);
        Ok(())
    }
    fn read_data(&mut self) -> Result<bool, ()> {
        Ok(crate::effects::ack())
    }
    fn half_period(&mut self) -> Result<(), ()> {
        crate::effects::require();
        Ok(())
    }
}

pub struct RecordingStorage {
    bytes: Vec<u8>,
    retain: bool,
}
impl RecordingStorage {
    pub fn from_retained(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            retain: false,
        }
    }
    pub fn new() -> Self {
        let storage = Self {
            retain: true,
            bytes: vec![
                255;
                crate::config::RECORDING_PREFIX_BYTES as usize
                    + crate::config::RECORDING_SLOTS as usize * 512
            ],
        };
        crate::accounting::exclude_media(storage.bytes.capacity());
        storage
    }
}
impl Drop for RecordingStorage {
    fn drop(&mut self) {
        if self.retain && crate::effects::enabled() {
            crate::effects::save_media(std::mem::take(&mut self.bytes));
        } else {
            crate::accounting::restore_media(self.bytes.capacity());
        }
    }
}
impl conduit_resource_snapshot::nor::NorStorage for RecordingStorage {
    fn capacity(&self) -> u32 {
        self.bytes.len() as u32
    }
    fn program_unit(&self) -> u32 {
        256
    }
    fn read(
        &mut self,
        offset: u32,
        out: &mut [u8],
    ) -> Result<(), conduit_resource_snapshot::nor::NorRefusal> {
        let bytes = self
            .bytes
            .get(offset as usize..offset as usize + out.len())
            .ok_or(conduit_resource_snapshot::nor::NorRefusal::Capacity)?;
        out.copy_from_slice(bytes);
        Ok(())
    }
    fn program(
        &mut self,
        offset: u32,
        bytes: &[u8],
    ) -> Result<(), conduit_resource_snapshot::nor::NorRefusal> {
        use conduit_resource_snapshot::nor::NorRefusal;
        crate::effects::require();
        if offset % 256 != 0 || bytes.len() != 256 {
            return Err(NorRefusal::Geometry);
        }
        let target = self
            .bytes
            .get_mut(offset as usize..offset as usize + bytes.len())
            .ok_or(NorRefusal::Capacity)?;
        if target.iter().zip(bytes).any(|(old, new)| old & new != *new) {
            return Err(NorRefusal::Occupied);
        }
        for (old, new) in target.iter_mut().zip(bytes) {
            *old &= *new;
        }
        crate::effects::PROGRAMS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}
pub type RecordingReady =
    conduit_resource_snapshot::native_service::ReadyRecordService<RecordingStorage>;
pub struct Equipment {
    pub activity: conduit_mcu_host_runtime::native_runner::NativeActivity,
    pub recording:
        conduit_mcu_host_runtime::recording_service_equipment::Equipment<RecordingStorage>,
    pub renderer: conduit_mcu_host_runtime::execution_mask::ReadyRenderer<crate::renderer::Clock>,
    pub pulse: Option<conduit_pulse_capture::installation::ReadyPulseBase<Capture>>,
    pub waits: [Option<conduit_monotonic_wait::ReadyMonotonicWaitBase<MonotonicTimer>>; WAIT_SLOTS],
    pub display: Option<conduit_tm1637_wire::installation::ReadyWriteBase<Wire>>,
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
fn recording_identity(actual: &HostAdvertisement) -> AppendNativeIdentity {
    #[cfg(record_service)]
    {
        identity!(
            AppendNativeIdentity,
            actual,
            "record-service",
            "esp32/nor-record-service@1"
        )
    }
    #[cfg(not(record_service))]
    {
        identity!(
            AppendNativeIdentity,
            actual,
            "record-append",
            "esp32/nor-record-append@1"
        )
    }
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
        body: &conduit_body::BodyId,
        pulse: Capture,
        display: Wire,
        recording_storage: RecordingStorage,
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
                recording: recording::Equipment::new(
                    recording_storage,
                    recording_identity(actual),
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
                        crate::runtime::MonotonicTimer {slot},
                    ).expect("actual initialized clock slot"))
                }),
                pulse: Some(
                    ReadyPulseBase::new(
                        identity!(PulseNativeIdentity, actual, "gpio-pulse", "esp32/dht11@1"),
                        PulseAttachment {
                            generation: 1,
                            // Fund both the physical provider and its bounded capture result.
                            resource_bytes: (core::mem::size_of::<Capture>()
                                + core::mem::size_of::<conduit_pulse_capture::PulseReadCapture>())
                                as u64,
                            limits: PULSE_LIMITS,
                        },
                        pulse,
                    )
                    .expect("actual pulse attachment"),
                ),
                display: Some(
                    ReadyWriteBase::new(
                        identity!(WriteNativeIdentity, actual, "tm1637", "esp32/tm1637@1"),
                        WriteAttachment {
                            generation: 1,
                            resource_bytes: WriteAttachment::retained_bytes_for_provider::<Wire>(),
                        },
                        display,
                    )
                    .expect("actual display attachment"),
                ),
            }
        }
    }
}
