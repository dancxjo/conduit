use core::{
    future::{poll_fn, Future},
    marker::PhantomData,
};
use std::{cell::RefCell, task::Poll};
#[derive(Default)]
pub struct Sdk {
    pub now: u64,
    pub ap_link: bool,
    pub ap_address: Option<StaticConfigV4>,
    pub station_address: Option<StaticConfigV4>,
    pub ready_generation: Option<u32>,
    pub restore_ack: bool,
    pub restore_failed: bool,
    pub commands: u32,
    pub controller_drops: u32,
    pub configurations: u32,
    pub ap_configurations: u32,
    pub scan_starts: u32,
    pub associations: u32,
    pub stops: u32,
    pub list_clears: u32,
    pub terminal: Option<wifi::bounded_scan::Completion>,
}
thread_local! {pub static SDK:RefCell<Sdk>=RefCell::new(Sdk::default());}
pub fn reset() {
    SDK.with_borrow_mut(|s| {
        *s = Sdk {
            ap_link: true,
            ap_address: Some(StaticConfigV4(42)),
            ready_generation: Some(0),
            restore_ack: true,
            ..Sdk::default()
        }
    });
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant(u64);
impl Instant {
    pub fn now() -> Self {
        Self(SDK.with_borrow(|s| s.now))
    }
    pub fn from_millis(ms: u64) -> Self {
        Self(ms)
    }
    pub fn as_millis(self) -> u64 {
        self.0
    }
}
pub struct Duration(u64);
impl Duration {
    pub fn from_millis(ms: u64) -> Self {
        Self(ms)
    }
}
pub struct Timer;
impl Timer {
    pub async fn after(duration: Duration) {
        let until = Instant::now().0 + duration.0;
        poll_fn(|_| {
            if Instant::now().0 >= until {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await
    }
}
pub async fn with_deadline<T>(at: Instant, work: impl Future<Output = T>) -> Result<T, ()> {
    let mut work = core::pin::pin!(work);
    poll_fn(|cx| {
        if Instant::now() >= at {
            Poll::Ready(Err(()))
        } else {
            work.as_mut().poll(cx).map(Ok)
        }
    })
    .await
}
pub async fn guard_operation_deadline<T>(
    work: impl Future<Output = T>,
    mut expired: impl FnMut() -> bool,
) -> Result<T, ()> {
    let mut work = core::pin::pin!(work);
    poll_fn(|cx| {
        if expired() {
            return Poll::Ready(Err(()));
        }
        let result = work.as_mut().poll(cx);
        if expired() {
            Poll::Ready(Err(()))
        } else {
            result.map(Ok)
        }
    })
    .await
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticConfigV4(pub u32);
pub enum ConfigV4 {
    None,
    Static(StaticConfigV4),
}
#[derive(Clone, Copy)]
pub struct Stack<'a>(pub u8, pub PhantomData<&'a ()>);
impl Stack<'_> {
    pub fn is_link_up(&self) -> bool {
        SDK.with_borrow(|s| s.ap_link)
    }
    pub fn config_v4(&self) -> Option<StaticConfigV4> {
        SDK.with_borrow(|s| {
            if self.0 == 0 {
                s.ap_address.clone()
            } else {
                s.station_address.clone()
            }
        })
    }
    pub fn set_config_v4(&self, config: ConfigV4) {
        SDK.with_borrow_mut(|s| {
            let value = match config {
                ConfigV4::None => None,
                ConfigV4::Static(value) => Some(value),
            };
            if self.0 == 0 {
                s.ap_address = value
            } else {
                s.station_address = value
            }
        })
    }
    pub async fn wait_link_up(&self) {
        poll_fn(|_| {
            if self.is_link_up() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await
    }
}
#[derive(Clone, Copy)]
pub struct StationInspection;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationFailure {
    Pressure,
    Rejected,
    Deadline,
    ProviderUnavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanFailure {
    Malformed,
    Pressure,
    Deadline,
    Rejected,
    Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownSecurity {
    Wpa2Personal,
    Wpa3Personal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservedNetworkSecurity {
    Known(KnownSecurity),
    Unsupported,
    Unknown,
}
#[derive(Clone, Copy)]
pub struct ScanObservation {
    pub security: ObservedNetworkSecurity,
    pub observed_ms: u64,
}
impl ScanObservation {
    pub const EMPTY: Self = Self {
        security: ObservedNetworkSecurity::Unknown,
        observed_ms: 0,
    };
    pub fn new(
        _: &[u8],
        security: ObservedNetworkSecurity,
        _: i16,
        observed_ms: u64,
    ) -> Result<Self, ScanFailure> {
        Ok(Self {
            security,
            observed_ms,
        })
    }
}
#[derive(Clone, Copy)]
pub struct NetworkScanRequest {
    pub generation: u64,
    pub scan_deadline_ms: u64,
    pub cleanup_deadline_ms: u64,
}
impl NetworkScanRequest {
    pub fn validate(self) -> Result<Self, ScanFailure> {
        if self.generation == 0 || self.cleanup_deadline_ms <= self.scan_deadline_ms {
            Err(ScanFailure::Malformed)
        } else {
            Ok(self)
        }
    }
}
pub struct ScanCompletion {
    pub generation: u64,
    pub completed_ms: u64,
    pub count: u8,
    pub total: u16,
}
pub mod scan {
    use super::*;
    pub trait NetworkScanProvider {
        fn now_ms(&self) -> u64;
        async fn scan(
            &mut self,
            request: NetworkScanRequest,
            output: &mut [ScanObservation; 16],
        ) -> Result<ScanCompletion, ScanFailure>;
        async fn cleanup(&mut self, at: u64) -> Result<(), ScanFailure>;
    }
}
pub trait AccessPointServices {
    async fn withdraw(&mut self) -> Result<(), StationFailure>;
    async fn restore(&mut self) -> Result<(), StationFailure>;
    async fn station(&mut self) -> Result<(), StationFailure>;
}
pub struct Services {
    pub generation: u32,
    pub maximum: u32,
}
impl AccessPointServices for Services {
    async fn withdraw(&mut self) -> Result<(), StationFailure> {
        self.generation += 1;
        SDK.with_borrow_mut(|s| {
            s.commands += 1;
            s.ready_generation = None
        });
        Ok(())
    }
    async fn restore(&mut self) -> Result<(), StationFailure> {
        self.generation += 1;
        let generation = self.generation;
        SDK.with_borrow_mut(|s| s.commands += 1);
        poll_fn(|_| {
            SDK.with_borrow_mut(|s| {
                if s.restore_failed {
                    return Poll::Ready(Err(StationFailure::ProviderUnavailable));
                }
                if s.restore_ack {
                    s.ready_generation = Some(generation);
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
        })
        .await
    }
    async fn station(&mut self) -> Result<(), StationFailure> {
        panic!("scan must never start station service")
    }
}
impl crate::adapter::RestoredAccessPointServices for Services {
    fn access_point_ready(&self) -> bool {
        SDK.with_borrow(|s| s.ready_generation == Some(self.generation))
    }
    fn remaining_service_commands(&self) -> u32 {
        self.maximum - self.generation
    }
}
pub struct Esp32ApStationSwitch<'a, 'd, S> {
    pub controller: wifi::WifiController<'d>,
    pub ap_stack: Stack<'a>,
    pub station_stack: Stack<'a>,
    pub ap_address: StaticConfigV4,
    pub ap_config: wifi::Config,
    pub station_config: wifi::Config,
    pub services: S,
    pub cleanup_required: bool,
    pub inspection: StationInspection,
}
pub fn switch() -> Esp32ApStationSwitch<'static, 'static, Services> {
    Esp32ApStationSwitch {
        controller: wifi::WifiController(PhantomData),
        ap_stack: Stack(0, PhantomData),
        station_stack: Stack(1, PhantomData),
        ap_address: StaticConfigV4(42),
        ap_config: wifi::Config::AccessPoint(String::from("ap")),
        station_config: wifi::Config::Station(String::from("private")),
        services: Services {
            generation: 0,
            maximum: 2,
        },
        cleanup_required: false,
        inspection: StationInspection,
    }
}
pub fn station_failure(_: wifi::WifiError) -> StationFailure {
    StationFailure::ProviderUnavailable
}
pub mod wifi {
    use super::*;
    pub enum Config {
        AccessPoint(String),
        Station(String),
    }
    impl Config {
        pub fn secret_ptr(&self) -> *const u8 {
            match self {
                Self::AccessPoint(s) | Self::Station(s) => s.as_ptr(),
            }
        }
    }
    pub struct WifiError;
    pub struct WifiController<'d>(pub PhantomData<&'d ()>);
    impl WifiController<'_> {
        pub fn set_config(&mut self, config: &Config) -> Result<(), WifiError> {
            SDK.with_borrow_mut(|s| {
                s.configurations += 1;
                if matches!(config, Config::AccessPoint(_)) {
                    s.ap_configurations += 1
                }
            });
            Ok(())
        }
    }
    impl Drop for WifiController<'_> {
        fn drop(&mut self) {
            SDK.with_borrow_mut(|s| s.controller_drops += 1)
        }
    }
    pub mod scan {
        #[derive(Default)]
        pub struct ScanConfig;
    }
    pub mod bounded_scan {
        use super::*;
        #[derive(Clone, Copy, Debug)]
        pub struct Completion {
            pub status: u32,
            pub number: u8,
            pub scan_id: u8,
        }
        #[derive(Clone, Copy, Debug)]
        pub struct Report {
            pub completion: Completion,
            pub total: u16,
            pub copied: u8,
            pub truncated: bool,
            pub stop_requested: bool,
        }
        pub struct Record {
            pub ssid: [u8; 32],
            pub ssid_len: u8,
            pub rssi: i8,
            pub authentication: u32,
        }
        pub enum Error {
            Sdk(i32),
            Capacity,
            Subscribers,
            MappingRefused { written: u8 },
            InvalidRecord,
            Quarantined,
        }
        pub struct OwnedScan<'d> {
            controller: Option<WifiController<'d>>,
            active: bool,
            stopped: bool,
            terminal: Option<Completion>,
        }
        impl<'d> OwnedScan<'d> {
            pub unsafe fn from_quiescent(controller: WifiController<'d>) -> Self {
                Self {
                    controller: Some(controller),
                    active: false,
                    stopped: false,
                    terminal: None,
                }
            }
            pub fn start(&mut self, _: &scan::ScanConfig, _: usize) -> Result<(), Error> {
                self.active = true;
                SDK.with_borrow_mut(|s| s.scan_starts += 1);
                Ok(())
            }
            pub fn request_stop(&mut self) -> Result<(), Error> {
                if self.active && self.terminal.is_none() && !self.stopped {
                    SDK.with_borrow_mut(|s| s.stops += 1);
                    self.stopped = true;
                }
                Ok(())
            }
            pub async fn wait_terminal(&mut self) -> Result<Completion, Error> {
                if let Some(event) = self.terminal {
                    return Ok(event);
                }
                let event = poll_fn(|_| {
                    SDK.with_borrow(|s| match s.terminal {
                        Some(event) => Poll::Ready(event),
                        None => Poll::Pending,
                    })
                })
                .await;
                self.terminal = Some(event);
                Ok(event)
            }
            pub async fn finish_mapped<T>(
                &mut self,
                out: &mut [T],
                mut map: impl FnMut(Record) -> Option<T>,
            ) -> Result<Report, Error> {
                let completion = self.wait_terminal().await?;
                let total = 30;
                for (at, slot) in out.iter_mut().enumerate() {
                    *slot = map(Record {
                        ssid: [1; 32],
                        ssid_len: 32,
                        rssi: -42,
                        authentication: if at == 0 { u32::MAX } else { 3 },
                    })
                    .unwrap();
                }
                SDK.with_borrow_mut(|s| s.list_clears += 1);
                self.active = false;
                Ok(Report {
                    completion,
                    total,
                    copied: 16,
                    truncated: true,
                    stop_requested: self.stopped,
                })
            }
            pub async fn cleanup(&mut self) -> Result<(), Error> {
                if !self.active {
                    return Ok(());
                }
                self.request_stop()?;
                self.wait_terminal().await?;
                SDK.with_borrow_mut(|s| s.list_clears += 1);
                self.active = false;
                Ok(())
            }
            pub fn try_into_controller(mut self) -> Result<WifiController<'d>, Self> {
                if self.active {
                    Err(self)
                } else {
                    Ok(self.controller.take().unwrap())
                }
            }
        }
        impl Drop for OwnedScan<'_> {
            fn drop(&mut self) {
                if self.active {
                    if let Some(controller) = self.controller.take() {
                        core::mem::forget(controller)
                    }
                }
            }
        }
    }
}
