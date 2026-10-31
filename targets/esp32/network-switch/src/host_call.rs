//! Concrete SDK provider under the selected portable network Host Call owner.
use crate::{AccessPointServices, Esp32ApStationSwitch, Esp32Deadline};
use conduit_net::{
    AccessPointSwitchOperations, NetworkAttachmentInfo, NetworkJoinError, NetworkJoinRequest,
    OperationDeadline, StationAttemptFailure, StationFailure, StationOperations, StationPhase,
};
use conduit_network_join_call::{AttachmentRecord, NetworkJoinProvider};
use embassy_time::{Duration, Instant};
use esp_radio::wifi::{AuthenticationMethod, Config};

#[derive(Debug, PartialEq, Eq)]
pub enum Esp32JoinPreparationRefusal {
    OwnerUnavailable,
    Configuration,
    AttemptBudget,
    CleanupBudget,
    Attachment(NetworkJoinError),
}
/// One configuration attempt. Actual radio/stacks/services remain retained
/// through success, cancellation and Root-supervised restoration.
pub struct Esp32NetworkJoinProvider<'a, 'd, S> {
    radio: Esp32ApStationSwitch<'a, 'd, S>,
    attempt_budget: Duration,
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
    /// Root admits a finite duration now. Its single absolute deadline starts at
    /// the first authorized invocation, so planning time consumes no radio budget.
    /// Cancellation still spends the attempt; another invocation cannot restart it.
    /// The Root ownership slot is consumed only on success. Every preparation
    /// refusal leaves its controller, private configuration and service handle
    /// in that slot, without a radio operation or an implicit controller drop.
    ///
    /// # Safety
    /// Root must own the exact initialized controller, stacks and service task,
    /// independently admit native SDK memory/work and the attempt/cleanup
    /// budgets, and reserve this current Host/Boot/resource attachment generation.
    /// SDK allocation, cancellation and synchronous-call work must satisfy the
    /// provider contract. A cross-target build alone proves none of those bounds.
    pub unsafe fn prepare(
        radio_owner: &mut Option<Esp32ApStationSwitch<'a, 'd, S>>,
        attempt_budget: Duration,
        cleanup_budget: Duration,
        attachment: NetworkAttachmentInfo<'_>,
    ) -> Result<Self, Esp32JoinPreparationRefusal> {
        let radio = radio_owner
            .as_ref()
            .ok_or(Esp32JoinPreparationRefusal::OwnerUnavailable)?;
        let Config::Station(config) = &radio.station_config else {
            return Err(Esp32JoinPreparationRefusal::Configuration);
        };
        if crate::credentials::validate(NetworkJoinRequest {
            ssid: config.ssid().as_str().as_bytes(),
            credential: config.password().as_bytes(),
        })
        .is_err()
            || config.ssid().as_str().len() != config.ssid().len()
            || config.auth_method() != AuthenticationMethod::Wpa2Personal
            || config.failure_retry_cnt() != 0
        {
            return Err(Esp32JoinPreparationRefusal::Configuration);
        }
        if attempt_budget.as_ticks() == 0 {
            return Err(Esp32JoinPreparationRefusal::AttemptBudget);
        }
        if cleanup_budget.as_ticks() == 0 {
            return Err(Esp32JoinPreparationRefusal::CleanupBudget);
        }
        let attachment = AttachmentRecord::encode(attachment)
            .map_err(Esp32JoinPreparationRefusal::Attachment)?;
        // All fallible preparation precedes the unique ownership transfer. No
        // allocation, clone, pinning or self-reference is introduced by it.
        let radio = radio_owner
            .take()
            .ok_or(Esp32JoinPreparationRefusal::OwnerUnavailable)?;
        Ok(Self {
            radio,
            attempt_budget,
            cleanup_budget,
            cleanup_deadline: None,
            attachment: Some(attachment),
            invoked: false,
        })
    }
    pub fn station_inspection(&self) -> crate::StationInspection {
        self.radio.inspection()
    }
    fn matches(&self, request: &NetworkJoinRequest<'_>) -> bool {
        let Config::Station(config) = &self.radio.station_config else {
            return false;
        };
        config.ssid().as_str().as_bytes() == request.ssid
            && config.password().as_bytes() == request.credential
    }
    /// Observation of this actual retained interface, not attachment evidence.
    /// HTTP service readiness remains separately acknowledged by the owner.
    pub fn station_link_and_address_up(&self) -> bool {
        self.radio.station_stack.is_link_up() && self.radio.station_stack.config_v4().is_some()
    }
    /// Current address of this actual interface; never an attachment receipt or
    /// a claim that application services or a Body peer have been verified.
    pub fn station_address(&self) -> Option<[u8; 4]> {
        self.radio.station_stack.is_link_up().then(|| ())?;
        self.radio
            .station_stack
            .config_v4()
            .map(|config| config.address.address().octets())
    }
    /// Return restored AP ownership without rearming this spent attempt.
    /// Root must retire selected possession before calling this on a provider
    /// reclaimed through NetworkJoinHostCall::retire_retained, then prepare a
    /// fresh provider/attachment generation for any subsequent selected join.
    pub fn into_restored_radio(self) -> Result<Esp32ApStationSwitch<'a, 'd, S>, Self> {
        if self.radio.cleanup_required() {
            Err(self)
        } else {
            Ok(self.radio)
        }
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
        // Fresh authorized invocation; returned radio ownership may carry an old
        // frozen observation, which never belongs to the new attempt.
        radio.begin_inspection();
        let deadline =
            Instant::now()
                .checked_add(self.attempt_budget)
                .ok_or(StationAttemptFailure {
                    phase: StationPhase::WithdrawAccessPoint,
                    reason: StationFailure::Deadline,
                })?;
        let mut attempt = Esp32Deadline(deadline);
        let mut phase = StationPhase::WithdrawAccessPoint;
        let result = attempt
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
        // Snapshot before separately owned cleanup clears station state.
        radio.capture_inspection();
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
        // Also handles cancellation of the enclosing join future. Preserve the
        // first terminal snapshot before restoration changes link/address state.
        self.radio.capture_inspection();
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
