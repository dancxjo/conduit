#![no_std]
//! ESP32 mode/address mechanics for an independently admitted network Host Call.
//! No offer, credential policy, retry, attachment or Body lifecycle is minted.
mod credentials;
mod host_call;
use conduit_net::{
    AccessPointSwitchOperations, DeadlineExpired, OperationDeadline, StationFailure,
    StationOperations,
};
use core::future::Future;
pub use credentials::{prepare_station_configuration, StationConfigurationRefusal};
use embassy_net::{ConfigV4, Stack, StaticConfigV4};
use embassy_time::{with_deadline, Instant};
use esp_radio::wifi::{
    ap::AccessPointConfig, sta::StationConfig, Config, WifiController, WifiError,
};
pub use host_call::{Esp32JoinPreparationRefusal, Esp32NetworkJoinProvider};

/// Services remain owned by Root. Withdrawal must retire existing AP requests;
/// restoration returns only after the actual DHCP/HTTP services are ready.
/// Dropping either future must preserve exclusive ownership and permit cleanup.
pub trait AccessPointServices {
    fn withdraw(&mut self) -> impl Future<Output = Result<(), StationFailure>>;
    fn restore(&mut self) -> impl Future<Output = Result<(), StationFailure>>;
    /// Start station listeners only after link/address readiness. Completion is
    /// actual service evidence, within the same attempt deadline; not membership.
    fn station(&mut self) -> impl Future<Output = Result<(), StationFailure>>;
}

/// One absolute deadline, shared across withdrawal, association and DHCP.
/// A separate instance must be admitted for restoration. No deadline is reset
/// by an intermediate success or by polling this object again.
pub struct Esp32Deadline(pub Instant);
impl OperationDeadline for Esp32Deadline {
    async fn run<T>(&mut self, work: impl Future<Output = T>) -> Result<T, DeadlineExpired> {
        let deadline = self.0;
        with_deadline(
            deadline,
            conduit_net::guard_operation_deadline(work, || Instant::now() >= deadline),
        )
        .await
        .map_err(|_| DeadlineExpired)?
    }
}

/// Own the actual controller and service handle, retaining both independently
/// running interfaces. No sibling Root field is borrowed by the resulting owner.
/// Configurations and their SDK-owned strings are prepared before execution;
/// these methods neither clone credentials nor construct growable buffers.
pub struct Esp32ApStationSwitch<'a, 'd, S> {
    controller: WifiController<'d>,
    ap_stack: Stack<'a>,
    station_stack: Stack<'a>,
    ap_address: StaticConfigV4,
    ap_config: Config,
    station_config: Config,
    services: S,
    cleanup_required: bool,
}
impl<'a, 'd, S: AccessPointServices> Esp32ApStationSwitch<'a, 'd, S> {
    /// # Safety
    /// Root must retain the actual distinct AP/station interface runners and
    /// exclusive controller, admit the SDK's finite memory/work, and select this
    /// operation through the generic Host Call boundary. Services must describe
    /// these same AP resources. Root must revoke any old attachment before mode
    /// changes and complete separately bounded restoration after cancellation.
    /// Construction itself establishes none of those admission facts.
    pub unsafe fn new(
        controller: WifiController<'d>,
        stacks: (Stack<'a>, Stack<'a>),
        ap_address: StaticConfigV4,
        configs: (AccessPointConfig, StationConfig),
        services: S,
    ) -> Self {
        Self {
            controller,
            ap_stack: stacks.0,
            station_stack: stacks.1,
            ap_address,
            ap_config: Config::AccessPoint(configs.0),
            station_config: Config::Station(configs.1),
            services,
            cleanup_required: false,
        }
    }
    /// Cancellation leaves this true. Only completed restoration clears it;
    /// synchronous radio fallback is not an AP-service readiness claim.
    pub fn cleanup_required(&self) -> bool {
        self.cleanup_required
    }
}
impl<S: AccessPointServices> StationOperations for Esp32ApStationSwitch<'_, '_, S> {
    async fn associate(&mut self) -> Result<(), StationFailure> {
        let mut guard = Association {
            controller: &mut self.controller,
            fallback: &self.ap_config,
            armed: true,
        };
        let result = guard.controller.connect_async().await;
        if result.is_ok() {
            guard.armed = false;
        }
        result.map(|_| ()).map_err(station_failure)
    }
    async fn configure_address(&mut self) -> Result<(), StationFailure> {
        self.station_stack.wait_config_up().await;
        if !self.station_stack.is_link_up() || self.station_stack.config_v4().is_none() {
            return Err(StationFailure::ProviderUnavailable);
        }
        self.services.station().await
    }
}
impl<S: AccessPointServices> AccessPointSwitchOperations for Esp32ApStationSwitch<'_, '_, S> {
    async fn withdraw_access_point(&mut self) -> Result<(), StationFailure> {
        self.cleanup_required = true;
        self.services.withdraw().await?;
        self.ap_stack.set_config_v4(ConfigV4::None);
        // Invalidate any previous station address before the new association.
        self.station_stack.set_config_v4(ConfigV4::None);
        self.controller
            .set_config(&self.station_config)
            .map_err(station_failure)?;
        self.station_stack
            .set_config_v4(ConfigV4::Dhcp(Default::default()));
        Ok(())
    }
    async fn restore_access_point(&mut self) -> Result<(), StationFailure> {
        self.cleanup_required = true;
        self.station_stack.set_config_v4(ConfigV4::None);
        self.controller
            .set_config(&self.ap_config)
            .map_err(station_failure)?;
        self.ap_stack
            .set_config_v4(ConfigV4::Static(self.ap_address.clone()));
        self.ap_stack.wait_link_up().await;
        self.services.restore().await?;
        self.cleanup_required = false;
        Ok(())
    }
}

fn station_failure(error: WifiError) -> StationFailure {
    match error {
        WifiError::OutOfMemory => StationFailure::Pressure,
        WifiError::Disconnected(_) | WifiError::InvalidSsid | WifiError::InvalidPassword => {
            StationFailure::Rejected
        }
        _ => StationFailure::ProviderUnavailable,
    }
}

/// The SDK's connect future does not disconnect on drop. Resetting mode on
/// cancellation/error stops that pending station operation while keeping the
/// controller owned. Root still owes full bounded AP-service restoration.
struct Association<'a, 'd> {
    controller: &'a mut WifiController<'d>,
    fallback: &'a Config,
    armed: bool,
}
impl Drop for Association<'_, '_> {
    fn drop(&mut self) {
        if self.armed {
            // Failure leaves cleanup_required set in the owning operation.
            // No attachment or service readiness is published from Drop.
            let _ = self.controller.set_config(self.fallback);
        }
    }
}
