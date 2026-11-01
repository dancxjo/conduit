// Explicit CPU hardware / absent-network boundary. No simulated owner can be
// produced by received identities; actual capability issuance remains native_authority.
extern crate self as conduit_esp32_pulse_capture;
extern crate self as esp_alloc;
extern crate self as esp_hal;
extern crate std as esp_println;
pub mod gpio {
    pub trait Pin {}
    pub struct CpuPin;
    impl Pin for CpuPin {}
}
pub struct Heap;
pub static HEAP: Heap = Heap;
pub struct RegionStats {
    pub free: usize,
}
pub struct Stats {
    pub size: usize,
    pub current_usage: usize,
    pub max_usage: usize,
    pub region_stats: [Option<RegionStats>; 3],
}
impl Heap {
    pub fn free(&self) -> usize {
        let stats = self.stats();
        stats.size.saturating_sub(stats.current_usage)
    }
    pub fn stats(&self) -> Stats {
        Stats {
            size: 2 * 1024 * 1024,
            current_usage: 0,
            max_usage: 0,
            region_stats: [None, None, None],
        }
    }
}
mod sdk_memory {
    pub fn stats() -> super::Stats {
        super::HEAP.stats()
    }
}
mod boot {
    pub struct ObservedBoot {
        pub host: &'static str,
        pub boot: &'static str,
    }
    impl ObservedBoot {
        pub fn ids(&self) -> (&str, &str) {
            (self.host, self.boot)
        }
    }
}
pub const LIMITS: conduit_pulse_capture::PulseReadLimits = conduit_pulse_capture::PulseReadLimits {
    maximum_hold_low_us: 50_000,
    maximum_skipped_pulses: 4,
    minimum_sample_offset_us: 10,
    maximum_sample_offset_us: 200,
    maximum_octets: 8,
    maximum_capture_timeout_us: 10_000,
};
pub struct Esp32PulseCapture<'a> {
    pub effects: std::rc::Rc<std::cell::Cell<usize>>,
    pub lifetime: core::marker::PhantomData<&'a ()>,
}
impl conduit_pulse_capture::owner::PulseReadProvider for Esp32PulseCapture<'_> {
    async fn read(
        &mut self,
        request: conduit_pulse_capture::CheckedPulseRead,
    ) -> Result<conduit_pulse_capture::PulseReadCapture, conduit_pulse_capture::PulseReadRefusal>
    {
        assert_eq!(request.request().octets, 5);
        self.effects.set(self.effects.get() + 1);
        Ok(
            conduit_pulse_capture::PulseReadCapture::from_complete_octets(
                conduit_pulse_capture::PulseReadDisposition::Complete,
                &[56, 0, 19, 8, 83],
            )
            .unwrap(),
        )
    }
}
mod electrical {
    pub struct DisplayPins<'a> {
        pub edges: usize,
        lifetime: core::marker::PhantomData<&'a ()>,
    }
    impl<'a> DisplayPins<'a> {
        pub fn new(_: impl super::gpio::Pin + 'a, _: impl super::gpio::Pin + 'a) -> Self {
            Self {
                edges: 0,
                lifetime: core::marker::PhantomData,
            }
        }
    }
    impl conduit_tm1637_wire::OpenDrain for DisplayPins<'_> {
        type Error = core::convert::Infallible;
        fn clock(&mut self, _: bool) -> Result<(), Self::Error> {
            self.edges += 1;
            Ok(())
        }
        fn data(&mut self, _: bool) -> Result<(), Self::Error> {
            self.edges += 1;
            Ok(())
        }
        fn read_data(&mut self) -> Result<bool, Self::Error> {
            Ok(false)
        }
        fn half_period(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }
}
mod native_network {
    pub struct NativeNetwork;
    #[derive(serde::Serialize)]
    pub struct Inspection;
    impl NativeNetwork {
        pub fn append_to_advertisement(
            &self,
            _: &mut conduit_core::HostAdvertisement,
        ) -> Result<(), ()> {
            Ok(())
        }
        pub fn inspect(&self) -> Inspection {
            Inspection
        }
    }
}
mod native_play {
    pub struct NativePlay;
    impl NativePlay {
        pub fn append_offer(&self, _: &mut conduit_core::HostAdvertisement) {}
        pub fn station_address(&self) -> Option<[u8; 4]> {
            None
        }
    }
    pub struct RetiredNetworkControl;
    impl RetiredNetworkControl {
        pub fn active(&self) -> &conduit_core::ActivePlayIdentity {
            panic!("no initialized CPU Wi-Fi owner")
        }
        pub fn evidence_is_committed(&self) -> bool {
            false
        }
        pub fn matches_active(&self, _: &conduit_core::ActivePlayIdentity) -> bool {
            false
        }
    }
}
mod network_planning {
    pub enum PreparedVerifiedNetworkSetup {}
    pub enum NetworkPlanningRefusal {}
}
mod native_credentials {
    pub enum RootCredentialSource {}
    impl conduit_network_join_call::CredentialSource for RootCredentialSource {
        fn take_acknowledged(
            &mut self,
            _: &mut [u8],
        ) -> Result<usize, conduit_network_join_call::CredentialSourceRefusal> {
            match *self {}
        }
    }
}
mod native_body_image {
    pub fn load(
        _: &mut super::native_root::NativeRoot,
    ) -> Result<Box<conduit_body::SealedBodyLifecycleSession>, &'static str> {
        Err("CPU bootstrap is explicit")
    }
}
mod native_evidence_region {
    use std::sync::{Arc, Mutex};
    pub struct Region {
        pub media: Arc<Mutex<Vec<u8>>>,
        start: usize,
        length: usize,
    }
    impl Region {
        pub fn erased(size: usize) -> Self {
            Self {
                media: Arc::new(Mutex::new(vec![255; size])),
                start: 0,
                length: size,
            }
        }
        pub fn seal_prefix(mut self, length: u32) -> Result<Self, Self> {
            let length = length as usize;
            if !length.is_multiple_of(4) || length >= self.length {
                return Err(self);
            }
            self.start += length;
            self.length -= length;
            Ok(self)
        }
        fn bound(
            &self,
            at: u32,
            n: usize,
        ) -> Result<usize, conduit_environment_runtime::display_evidence_journal::Refusal> {
            let at = at as usize;
            if !at.is_multiple_of(4)
                || !n.is_multiple_of(4)
                || at.checked_add(n).is_none_or(|end| end > self.length)
            {
                return Err(
                    conduit_environment_runtime::display_evidence_journal::Refusal::Geometry,
                );
            }
            Ok(self.start + at)
        }
    }
    impl conduit_environment_runtime::display_evidence_journal::Storage for Region {
        fn capacity(&self) -> u32 {
            self.length as u32
        }
        fn read(
            &mut self,
            at: u32,
            out: &mut [u8],
        ) -> Result<(), conduit_environment_runtime::display_evidence_journal::Refusal> {
            let at = self.bound(at, out.len())?;
            let media = self.media.lock().unwrap();
            out.copy_from_slice(
                media
                    .get(at..at + out.len())
                    .ok_or(conduit_environment_runtime::display_evidence_journal::Refusal::Media)?,
            );
            Ok(())
        }
        fn program(
            &mut self,
            at: u32,
            input: &[u8],
        ) -> Result<(), conduit_environment_runtime::display_evidence_journal::Refusal> {
            let at = self.bound(at, input.len())?;
            let mut media = self.media.lock().unwrap();
            let old = media
                .get_mut(at..at + input.len())
                .ok_or(conduit_environment_runtime::display_evidence_journal::Refusal::Media)?;
            for (old, new) in old.iter_mut().zip(input) {
                if *old & new != *new {
                    return Err(
                        conduit_environment_runtime::display_evidence_journal::Refusal::Media,
                    );
                }
                *old &= new;
            }
            Ok(())
        }
    }
    #[derive(Clone)]
    pub struct BodyRegion {
        #[allow(clippy::type_complexity)] // Bytes, write enable, bounded commit fault counter.
        pub media: Arc<Mutex<(Vec<u8>, bool, Option<u32>)>>,
    }
    pub fn test_empty() -> BodyRegion {
        BodyRegion {
            media: Arc::new(Mutex::new((vec![255; 65536], true, None))),
        }
    }
    impl super::native_body_checkpoint::Storage for BodyRegion {
        fn capacity(&self) -> u32 {
            65536
        }
        fn read(
            &mut self,
            at: u32,
            out: &mut [u8],
        ) -> Result<(), super::native_body_checkpoint::Refusal> {
            if !at.is_multiple_of(4)
                || !out.len().is_multiple_of(4)
                || at as usize + out.len() > 65536
            {
                return Err(super::native_body_checkpoint::Refusal::Geometry);
            }
            let m = self.media.lock().unwrap();
            out.copy_from_slice(&m.0[at as usize..at as usize + out.len()]);
            crate::CHECKPOINT_DECODE_FAULT.with(|fault| {
                use sha2::Digest;
                let (mode, mut headers, mut payloads) = fault.get();
                if mode == 0 {
                    return;
                }
                if at.is_multiple_of(32768) && out.len() == 160 {
                    headers += 1;
                    payloads = 0;
                    if mode == 1 && headers == 2 {
                        // A completely authenticated second source with a
                        // different private seed must still fail the root pin.
                        out[56..88].fill(62);
                        let length = u32::from_le_bytes(out[16..20].try_into().unwrap()) as usize;
                        let mut hash = sha2::Sha256::new();
                        hash.update(&out[..120]);
                        hash.update(&m.0[at as usize + 160..at as usize + 160 + length]);
                        out[120..152].copy_from_slice(&hash.finalize());
                    }
                } else if mode == 2 && headers == 2 && at % 32768 >= 160 {
                    payloads += 1;
                    let base = (at / 32768 * 32768) as usize;
                    let length =
                        u32::from_le_bytes(m.0[base + 16..base + 20].try_into().unwrap()) as usize;
                    if payloads == length.div_ceil(128) + 1 && !out.is_empty() {
                        out[0] ^= 1;
                    }
                }
                fault.set((mode, headers, payloads));
            });

            Ok(())
        }
        fn program(
            &mut self,
            at: u32,
            input: &[u8],
        ) -> Result<(), super::native_body_checkpoint::Refusal> {
            if !at.is_multiple_of(4)
                || !input.len().is_multiple_of(4)
                || at as usize + input.len() > 65536
            {
                return Err(super::native_body_checkpoint::Refusal::Geometry);
            }
            let mut state = self.media.lock().unwrap();
            if !state.1 || state.2 == Some(0) {
                return Err(super::native_body_checkpoint::Refusal::Storage);
            }
            for (old, new) in state.0[at as usize..at as usize + input.len()]
                .iter_mut()
                .zip(input)
            {
                assert_eq!(*old & new, *new);
                *old &= new;
            }
            // Fault injection observes actual commit-last writes; it never
            // creates a marker, validates a payload or acknowledges an archive.
            if at % 32768 == 152 && input == b"done" {
                if let Some(left) = state.2.as_mut() {
                    *left -= 1;
                }
            }
            Ok(())
        }
        fn erase_bank(&mut self, bank: u8) -> Result<(), super::native_body_checkpoint::Refusal> {
            let mut state = self.media.lock().unwrap();
            if !state.1 || state.2 == Some(0) {
                return Err(super::native_body_checkpoint::Refusal::Storage);
            }
            state.0[usize::from(bank) * 32768..(usize::from(bank) + 1) * 32768].fill(255);
            Ok(())
        }
    }
}

// Native Wait interface with an explicitly initialized CPU monotonic timer.
// We do not call the ESP-only RTOS initialization constructor on a host.
mod native_wait {
    pub struct EspWait;
    impl conduit_monotonic_wait::MonotonicWaitProvider for EspWait {
        async fn wait(
            &mut self,
            duration_ms: u64,
        ) -> Result<(), conduit_monotonic_wait::WaitProviderFailure> {
            if duration_ms > 60_000 {
                return Err(conduit_monotonic_wait::WaitProviderFailure::ProviderUnavailable);
            }
            embassy_time::Timer::after_millis(duration_ms).await;
            Ok(())
        }
    }
}

mod native_carrier {
    pub struct Carrier<'a>(pub core::marker::PhantomData<&'a mut ()>);
}
mod native_control {
    pub async fn write_all(_: &mut super::native_carrier::Carrier<'_>, _: &[u8]) -> Result<(), ()> {
        Err(())
    }
}

// No HTTP join provider is installed by this renewal fixture. Its diagnostics
// honestly report no retained HTTP response; membership is proved separately.
mod native_body_join_http {
    pub fn completed_storage_usage() -> Option<[usize; 2]> {
        None
    }
}
