//! Selected scan adapter: the existing affine radio/context moves, never clones.
#[path = "scan/prepare.rs"]
mod prepare;
use crate::{AccessPointServices, Esp32ApStationSwitch, StationInspection};
use conduit_net::{
    KnownSecurity, NetworkScanRequest, ObservedNetworkSecurity, ScanCompletion, ScanFailure,
    ScanObservation, StationFailure,
};
use conduit_network_join_call::scan::NetworkScanProvider;
use embassy_net::{ConfigV4, Stack, StaticConfigV4};
use embassy_time::{with_deadline, Duration, Instant, Timer};
use esp_radio::wifi::{
    bounded_scan::{Completion, Error as SdkScanError, OwnedScan, Report},
    scan::ScanConfig,
    Config, WifiController,
};

/// Read only actual observations correlated with this service owner's current
/// command generation. A stale mailbox Ready cannot establish this predicate.
pub trait RestoredAccessPointServices: AccessPointServices {
    fn access_point_ready(&self) -> bool;
    fn remaining_service_commands(&self) -> u32;
}
#[derive(Debug, PartialEq, Eq)]
pub enum Esp32ScanPreparationRefusal {
    OwnerUnavailable,
    CleanupPending,
    Configuration,
    Interface,
    Services,
    ServiceCapacity,
}
#[derive(Clone, Copy, Debug)]
pub struct Esp32ScanInspection {
    pub terminal: Option<Completion>,
    pub observed_ms: Option<u64>,
    pub report: Option<Report>,
    pub sdk_error: Option<i32>,
}
struct Context<'a, S> {
    ap_stack: Stack<'a>,
    station_stack: Stack<'a>,
    ap_address: StaticConfigV4,
    ap_config: Config,
    station_config: Config,
    services: S,
    inspection: StationInspection,
}
enum Radio<'d> {
    Controller(WifiController<'d>),
    Scan(OwnedScan<'d>),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Prepared,
    Withdrawing,
    Scanning,
    Restoring,
    RestoringLink,
    RestoringServices { before: u32 },
    Restored,
}
/// One selected invocation. Cancellation keeps every context field and cleanup
/// phase here; no borrowed future contains the only radio or service owner.
pub struct Esp32NetworkScanProvider<'a, 'd, S> {
    context: Option<Context<'a, S>>,
    radio: Option<Radio<'d>>,
    phase: Phase,
    invoked: bool,
    request: Option<NetworkScanRequest>,
    inspection: Esp32ScanInspection,
}
impl<'a, 'd, S: RestoredAccessPointServices> Esp32NetworkScanProvider<'a, 'd, S> {
    /// Pure preparation. All refusal checks precede slot.take; no SDK operation,
    /// command, allocation, configuration clone or credential copy occurs here.
    pub fn prepare_from_restored(
        slot: &mut Option<Esp32ApStationSwitch<'a, 'd, S>>,
    ) -> Result<Self, Esp32ScanPreparationRefusal> {
        use Esp32ScanPreparationRefusal as R;
        let radio = prepare::take_checked(slot, R::OwnerUnavailable, |radio| {
            if radio.cleanup_required {
                return Err(R::CleanupPending);
            }
            if !matches!(radio.ap_config, Config::AccessPoint(_))
                || !matches!(radio.station_config, Config::Station(_))
            {
                return Err(R::Configuration);
            }
            if !radio.ap_stack.is_link_up()
                || radio.ap_stack.config_v4().as_ref() != Some(&radio.ap_address)
                || radio.station_stack.config_v4().is_some()
            {
                return Err(R::Interface);
            }
            if !radio.services.access_point_ready() {
                return Err(R::Services);
            }
            if radio.services.remaining_service_commands() < 2 {
                return Err(R::ServiceCapacity);
            }
            Ok(())
        })?;
        let Esp32ApStationSwitch {
            controller,
            ap_stack,
            station_stack,
            ap_address,
            ap_config,
            station_config,
            services,
            inspection,
            ..
        } = radio;
        Ok(Self {
            context: Some(Context {
                ap_stack,
                station_stack,
                ap_address,
                ap_config,
                station_config,
                services,
                inspection,
            }),
            radio: Some(Radio::Controller(controller)),
            phase: Phase::Prepared,
            invoked: false,
            request: None,
            inspection: Esp32ScanInspection {
                terminal: None,
                observed_ms: None,
                report: None,
                sdk_error: None,
            },
        })
    }
    pub fn inspection(&self) -> Esp32ScanInspection {
        self.inspection
    }
    /// After selected-operation retirement, return exact configuration/context
    /// only if this actual AP interface and current service ACK are restored.
    pub fn try_into_restored(mut self) -> Result<Esp32ApStationSwitch<'a, 'd, S>, Self> {
        let context = self.context.as_ref().expect("owned context");
        if !matches!(self.phase, Phase::Prepared | Phase::Restored)
            || !context.ap_stack.is_link_up()
            || context.ap_stack.config_v4().as_ref() != Some(&context.ap_address)
            || !context.services.access_point_ready()
            || !matches!(self.radio, Some(Radio::Controller(_)))
        {
            return Err(self);
        }
        let Some(Radio::Controller(controller)) = self.radio.take() else {
            unreachable!()
        };
        let Context {
            ap_stack,
            station_stack,
            ap_address,
            ap_config,
            station_config,
            services,
            inspection,
        } = self.context.take().unwrap();
        Ok(Esp32ApStationSwitch {
            controller,
            ap_stack,
            station_stack,
            ap_address,
            ap_config,
            station_config,
            services,
            inspection,
            cleanup_required: false,
        })
    }
    fn sdk_failure(&mut self, error: SdkScanError) -> ScanFailure {
        match error {
            SdkScanError::Sdk(code) => {
                self.inspection.sdk_error = Some(code);
                if code == 0x101 {
                    ScanFailure::Pressure
                } else {
                    ScanFailure::Unavailable
                }
            }
            SdkScanError::Capacity | SdkScanError::Subscribers => ScanFailure::Pressure,
            SdkScanError::MappingRefused { .. } | SdkScanError::InvalidRecord => {
                ScanFailure::Malformed
            }
            _ => ScanFailure::Unavailable,
        }
    }
    async fn scan_once(
        &mut self,
        request: NetworkScanRequest,
        output: &mut [ScanObservation; 16],
    ) -> Result<ScanCompletion, ScanFailure> {
        self.phase = Phase::Withdrawing;
        self.context
            .as_mut()
            .unwrap()
            .services
            .withdraw()
            .await
            .map_err(station_failure)?;
        if Instant::now().as_millis() >= request.scan_deadline_ms {
            return Err(ScanFailure::Deadline);
        }
        let context = self.context.as_mut().unwrap();
        context.ap_stack.set_config_v4(ConfigV4::None);
        context.station_stack.set_config_v4(ConfigV4::None);
        let Some(Radio::Controller(controller)) = self.radio.as_mut() else {
            return Err(ScanFailure::Unavailable);
        };
        controller
            .set_config(&context.station_config)
            .map_err(|error| station_failure(crate::station_failure(error)))?;
        let Some(Radio::Controller(controller)) = self.radio.take() else {
            unreachable!()
        };
        // SAFETY: preparation checked actual restored AP ownership; this adapter
        // exclusively owns its controller and never associates. No scan existed
        // before this selected boundary. SDK start, wait and cleanup retain it.
        self.radio = Some(Radio::Scan(unsafe {
            OwnedScan::from_quiescent(controller)
        }));
        self.phase = Phase::Scanning;
        let Some(Radio::Scan(scan)) = self.radio.as_mut() else {
            unreachable!()
        };
        if let Err(error) = scan.start(&ScanConfig::default(), 16) {
            return Err(self.sdk_failure(error));
        }
        let terminal = match scan.wait_terminal().await {
            Ok(event) => event,
            Err(error) => return Err(self.sdk_failure(error)),
        };
        let observed_ms = Instant::now().as_millis();
        self.inspection.terminal = Some(terminal);
        self.inspection.observed_ms = Some(observed_ms);
        let report = match scan
            .finish_mapped(output, |raw| {
                ScanObservation::new(
                    &raw.ssid[..usize::from(raw.ssid_len)],
                    security(raw.authentication),
                    i16::from(raw.rssi),
                    observed_ms,
                )
                .ok()
            })
            .await
        {
            Ok(report) => report,
            Err(error) => return Err(self.sdk_failure(error)),
        };
        self.inspection.report = Some(report);
        if report.completion.status != 0 || report.stop_requested {
            return Err(ScanFailure::Rejected);
        }
        if Instant::now().as_millis() >= request.scan_deadline_ms {
            return Err(ScanFailure::Deadline);
        }
        Ok(ScanCompletion {
            generation: request.generation,
            completed_ms: observed_ms,
            count: report.copied,
            total: report.total,
        })
    }
    #[inline(never)]
    fn reclaim_quiescent_controller(&mut self) -> Result<(), ScanFailure> {
        if !matches!(self.radio, Some(Radio::Scan(_))) {
            return Ok(());
        }
        let Some(Radio::Scan(scan)) = self.radio.take() else {
            unreachable!()
        };
        match scan.try_into_controller() {
            Ok(controller) => {
                self.radio = Some(Radio::Controller(controller));
                Ok(())
            }
            Err(scan) => {
                self.radio = Some(Radio::Scan(scan));
                Err(ScanFailure::Unavailable)
            }
        }
    }
    async fn restore_once(&mut self) -> Result<(), ScanFailure> {
        if matches!(self.phase, Phase::Prepared | Phase::Restored) {
            return Ok(());
        }
        // finish_mapped may already have reconciled terminal/list custody. Its
        // actual owner-return gate is stronger than waiting for a second event.
        let _ = self.reclaim_quiescent_controller();
        if let Some(Radio::Scan(scan)) = self.radio.as_mut() {
            if let Err(error) = scan.request_stop() {
                return Err(self.sdk_failure(error));
            }
            let terminal = match scan.wait_terminal().await {
                Ok(event) => event,
                Err(error) => return Err(self.sdk_failure(error)),
            };
            if self.inspection.terminal.is_none() {
                self.inspection.terminal = Some(terminal);
                self.inspection.observed_ms = Some(Instant::now().as_millis());
            }
            if let Err(error) = scan.cleanup().await {
                return Err(self.sdk_failure(error));
            }
            self.reclaim_quiescent_controller()?;
        }
        if !matches!(
            self.phase,
            Phase::RestoringLink | Phase::RestoringServices { .. }
        ) {
            self.phase = Phase::Restoring;
            let context = self.context.as_mut().unwrap();
            context.station_stack.set_config_v4(ConfigV4::None);
            let Some(Radio::Controller(controller)) = self.radio.as_mut() else {
                return Err(ScanFailure::Unavailable);
            };
            controller
                .set_config(&context.ap_config)
                .map_err(|error| station_failure(crate::station_failure(error)))?;
            context
                .ap_stack
                .set_config_v4(ConfigV4::Static(context.ap_address.clone()));
            self.phase = Phase::RestoringLink;
        }
        if self.phase == Phase::RestoringLink {
            let context = self.context.as_mut().unwrap();
            context.ap_stack.wait_link_up().await;
            let before = context.services.remaining_service_commands();
            if before == 0 {
                return Err(ScanFailure::Unavailable);
            }
            self.phase = Phase::RestoringServices { before };
        }
        let Phase::RestoringServices { before } = self.phase else {
            unreachable!()
        };
        let context = self.context.as_mut().unwrap();
        if context.services.remaining_service_commands() == before {
            context.services.restore().await.map_err(station_failure)?;
        }
        // If restoration's future was canceled after issuing its command, wait
        // for that exact generation's actual ACK, without spending another one.
        while !context.services.access_point_ready() {
            Timer::after(Duration::from_millis(1)).await;
        }
        if !context.ap_stack.is_link_up()
            || context.ap_stack.config_v4().as_ref() != Some(&context.ap_address)
        {
            return Err(ScanFailure::Unavailable);
        }
        self.phase = Phase::Restored;
        Ok(())
    }
}
impl<S: RestoredAccessPointServices> NetworkScanProvider for Esp32NetworkScanProvider<'_, '_, S> {
    fn now_ms(&self) -> u64 {
        Instant::now().as_millis()
    }
    async fn scan(
        &mut self,
        request: NetworkScanRequest,
        output: &mut [ScanObservation; 16],
    ) -> Result<ScanCompletion, ScanFailure> {
        if self.invoked {
            return Err(ScanFailure::Rejected);
        }
        self.invoked = true;
        request.validate()?;
        self.request = Some(request);
        if !prepare::budgets(
            Instant::now().as_millis(),
            request.scan_deadline_ms,
            request.cleanup_deadline_ms,
        ) {
            return Err(ScanFailure::Deadline);
        }
        let deadline = Instant::from_millis(request.scan_deadline_ms);
        with_deadline(
            deadline,
            conduit_net::guard_operation_deadline(self.scan_once(request, output), || {
                Instant::now() >= deadline
            }),
        )
        .await
        .map_err(|_| ScanFailure::Deadline)?
        .map_err(|_| ScanFailure::Deadline)?
    }
    async fn cleanup(&mut self, absolute_deadline_ms: u64) -> Result<(), ScanFailure> {
        if self.request.is_none() && self.phase == Phase::Prepared {
            return Ok(());
        }
        if self.request.map(|r| r.cleanup_deadline_ms) != Some(absolute_deadline_ms)
            || Instant::now().as_millis() >= absolute_deadline_ms
        {
            return Err(ScanFailure::Deadline);
        }
        with_deadline(
            Instant::from_millis(absolute_deadline_ms),
            conduit_net::guard_operation_deadline(self.restore_once(), || {
                Instant::now().as_millis() >= absolute_deadline_ms
            }),
        )
        .await
        .map_err(|_| ScanFailure::Deadline)?
        .map_err(|_| ScanFailure::Deadline)?
    }
}
impl<S> Drop for Esp32NetworkScanProvider<'_, '_, S> {
    fn drop(&mut self) {
        if !matches!(self.phase, Phase::Prepared | Phase::Restored) {
            // Abandoned custody must not invoke an unproved SDK deinit. Active
            // OwnedScan has the same quarantine rule in its own Drop.
            if let Some(Radio::Controller(controller)) = self.radio.take() {
                core::mem::forget(controller);
            }
        }
    }
}
fn station_failure(failure: StationFailure) -> ScanFailure {
    match failure {
        StationFailure::Pressure => ScanFailure::Pressure,
        StationFailure::Rejected => ScanFailure::Rejected,
        StationFailure::Deadline => ScanFailure::Deadline,
        _ => ScanFailure::Unavailable,
    }
}
fn security(raw: u32) -> ObservedNetworkSecurity {
    // Exact ESP-IDF5.5.3 wifi_auth_mode_t values. Mixed suites are not narrowed
    // to a weaker suite; unrecognized raw values remain Unknown, never Open.
    match raw {
        3 => ObservedNetworkSecurity::Known(KnownSecurity::Wpa2Personal),
        6 => ObservedNetworkSecurity::Known(KnownSecurity::Wpa3Personal),
        0 | 1 | 2 | 5 | 8 | 9 | 10 | 13 | 14 | 15 | 16 => ObservedNetworkSecurity::Unsupported,
        _ => ObservedNetworkSecurity::Unknown,
    }
}
