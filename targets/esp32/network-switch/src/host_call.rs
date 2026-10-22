//! Concrete SDK provider under the selected portable network Host Call owner.
use crate::{AccessPointServices, Esp32ApStationSwitch, Esp32Deadline};
use conduit_net::{
    AccessPointSwitchOperations, NetworkAttachmentInfo, NetworkJoinError, NetworkJoinRequest,
    OperationDeadline, StationAttemptFailure, StationFailure, StationOperations, StationPhase,
};
use conduit_network_join_call::{AttachmentRecord, NetworkJoinProvider};
use embassy_time::{Duration, Instant};
use esp_radio::wifi::Config;

#[derive(Debug, PartialEq, Eq)]
pub enum Esp32JoinPreparationRefusal {
    Configuration,
    CleanupBudget,
    Attachment(NetworkJoinError),
}
/// One configuration attempt. Actual radio/stacks/services remain retained
/// through success, cancellation and Root-supervised restoration.
pub struct Esp32NetworkJoinProvider<'a, 'd, S> {
    radio: Esp32ApStationSwitch<'a, 'd, S>,
    attempt: Esp32Deadline,
    cleanup_budget: Duration,
    cleanup_deadline: Option<Instant>,
    attachment: Option<AttachmentRecord>,
    invoked: bool,
}
impl<'a, 'd, S: AccessPointServices> Esp32NetworkJoinProvider<'a, 'd, S> {
    /// Prepare after private credential admission, before starting the control
    /// Play. SDK configuration owns its already allocated credential buffer;
    /// this provider compares the runtime Host Call request without copying it.
    /// Encoding reserved attachment metadata here establishes no physical truth.
    ///
    /// # Safety
    /// Root must own the exact initialized controller, stacks and service task,
    /// independently admit native SDK memory/work and the attempt/cleanup
    /// budgets, and reserve this current Host/Boot/resource attachment generation.
    /// SDK allocation, cancellation and synchronous-call work must satisfy the
    /// provider contract. A cross-target build alone proves none of those bounds.
    pub unsafe fn prepare(
        radio: Esp32ApStationSwitch<'a, 'd, S>,
        attempt: Instant,
        cleanup_budget: Duration,
        attachment: NetworkAttachmentInfo<'_>,
    ) -> Result<Self, Esp32JoinPreparationRefusal> {
        let Config::Station(config) = &radio.station_config else {
            return Err(Esp32JoinPreparationRefusal::Configuration);
        };
        if config.ssid().is_empty() || config.password().is_empty() || config.password().len() > 64
        {
            return Err(Esp32JoinPreparationRefusal::Configuration);
        }
        if cleanup_budget.as_ticks() == 0 {
            return Err(Esp32JoinPreparationRefusal::CleanupBudget);
        }
        Ok(Self {
            radio,
            attempt: Esp32Deadline(attempt),
            cleanup_budget,
            cleanup_deadline: None,
            attachment: Some(
                AttachmentRecord::encode(attachment)
                    .map_err(Esp32JoinPreparationRefusal::Attachment)?,
            ),
            invoked: false,
        })
    }
    fn matches(&self, request: &NetworkJoinRequest<'_>) -> bool {
        let Config::Station(config) = &self.radio.station_config else {
            return false;
        };
        config.ssid().as_str().as_bytes() == request.ssid
            && config.password().as_bytes() == request.credential
    }
}
impl<S: AccessPointServices> NetworkJoinProvider for Esp32NetworkJoinProvider<'_, '_, S> {
    async fn join(
        &mut self,
        request: NetworkJoinRequest<'_>,
    ) -> Result<AttachmentRecord, StationAttemptFailure> {
        if self.invoked || !self.matches(&request) {
            return Err(StationAttemptFailure {
                phase: StationPhase::WithdrawAccessPoint,
                reason: StationFailure::Rejected,
            });
        }
        self.invoked = true;
        let radio = &mut self.radio;
        let deadline = self.attempt.0;
        let mut phase = StationPhase::WithdrawAccessPoint;
        let result = self
            .attempt
            .run(async {
                conduit_net::guard_operation_deadline(radio.withdraw_access_point(), || {
                    Instant::now() >= deadline
                })
                .await
                .map_err(|_| StationFailure::Deadline)??;
                phase = StationPhase::Associate;
                conduit_net::guard_operation_deadline(radio.associate(), || {
                    Instant::now() >= deadline
                })
                .await
                .map_err(|_| StationFailure::Deadline)??;
                phase = StationPhase::ConfigureAddress;
                conduit_net::guard_operation_deadline(radio.configure_address(), || {
                    Instant::now() >= deadline
                })
                .await
                .map_err(|_| StationFailure::Deadline)?
            })
            .await;
        match result {
            Ok(Ok(())) => self.attachment.take().ok_or(StationAttemptFailure {
                phase,
                reason: StationFailure::ProviderUnavailable,
            }),
            Ok(Err(reason)) => Err(StationAttemptFailure { phase, reason }),
            Err(_) => Err(StationAttemptFailure {
                phase,
                reason: StationFailure::Deadline,
            }),
        }
    }
    async fn cleanup(&mut self) -> Result<(), StationFailure> {
        // The first cleanup may be much later than a successful join. Its one
        // Root-admitted budget starts now; cancellation/re-poll cannot reset it.
        let deadline = match self.cleanup_deadline {
            Some(deadline) => deadline,
            None => {
                let deadline = Instant::now()
                    .checked_add(self.cleanup_budget)
                    .ok_or(StationFailure::Deadline)?;
                self.cleanup_deadline = Some(deadline);
                deadline
            }
        };
        conduit_net::restore_access_point(&mut self.radio, &mut Esp32Deadline(deadline)).await
    }
}
