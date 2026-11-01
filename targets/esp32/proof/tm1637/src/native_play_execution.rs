//! Actual native execution state and bounded Host Call dispatch.
use super::*;
pub struct NativePlay {
    #[cfg(feature = "flat-network-appliance")]
    pub(super) flat_refusal: flat::IssuedCustody,
    pub(super) steps: u16,
    pub(super) dispatches: [u8; 3],
    pub(super) attempted: bool,
    pub(super) phase: &'static str,
    pub(super) prepared: Option<alloc::boxed::Box<PreparedNetworkKernel>>,
    pub(super) completed: Option<completed::CompletedControl>,
    pub(super) source: Option<n::NetworkCredentialsHostCall<Source>>,
    pub(super) join: Option<n::NetworkJoinHostCall<Provider>>,
    pub(super) active: Option<c::ActivePlayIdentity>,
    pub(super) receipt_observed: bool,
    pub(super) failure: Option<k::Failure>,
    pub(super) native_failure: Option<conduit_net::StationAttemptFailure>,
    pub(super) station_outcome: station_outcome::StationOutcome,
    pub(super) cleanup_failure: Option<conduit_net::StationFailure>,
    pub(super) configuration_revision: u64,
}
impl NativePlay {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "flat-network-appliance")]
            flat_refusal: flat::IssuedCustody::Empty,
            steps: 0,
            dispatches: [0; 3],
            attempted: false,
            phase: "uninstalled",
            prepared: None,
            completed: None,
            source: None,
            join: None,
            active: None,
            receipt_observed: false,
            failure: None,
            native_failure: None,
            station_outcome: station_outcome::StationOutcome::default(),
            cleanup_failure: None,
            configuration_revision: 0,
        }
    }
    /// No issued bearer, selected call or active execution is retained here.
    pub fn permits_unselected_configuration(&self) -> bool {
        #[cfg(feature = "flat-network-appliance")]
        if !self.flat_refusal.is_empty() {
            return false;
        }
        !self.attempted
            && self.active.is_none()
            && self.prepared.is_none()
            && self.completed.is_none()
            && self.source.is_none()
            && self.join.is_none()
    }

    pub fn progress(&self) -> (u16, [u8; 3]) {
        (self.steps, self.dispatches)
    }
    pub fn active(&self) -> Option<&c::ActivePlayIdentity> {
        self.active.as_ref()
    }
    pub fn receipt_received(&self) -> bool {
        self.receipt_observed
    }
    /// Current native interface and acknowledged socket-task observation.
    /// The historical receipt alone cannot keep a lost attachment available.
    pub fn station_services_available(&self) -> bool {
        self.phase == "attached"
            && self.join.as_ref().is_some_and(|join| {
                join.observe_provider(|provider| provider.station_link_and_address_up())
            })
            && crate::services::MAILBOX.observed_state()
                == Some(conduit_environment_http::service_control::ServiceState::StationReady)
    }
    /// Observe loss only through this Play's retained provider and service owner.
    /// The observation travels with the exact Play through native retirement;
    /// it neither changes the original join Signs nor authorizes another attempt.
    pub fn observe_attachment_availability(&mut self, now: u64) -> bool {
        let available = self.station_services_available();
        if self.phase == "attached" && !available {
            self.station_outcome.detached(now);
        }
        available
    }
    // Actual SDK observation only; CPU transcript providers do not impersonate it.
    #[cfg(target_arch = "xtensa")]
    pub(crate) fn station_attempt_inspection(
        &self,
    ) -> Option<conduit_esp32_network_switch::StationInspection> {
        self.join
            .as_ref()
            .map(|join| join.observe_provider(|provider| provider.station_inspection()))
    }
    pub fn station_address(&self) -> Option<[u8; 4]> {
        self.join
            .as_ref()
            .and_then(|join| join.observe_provider(|provider| provider.station_address()))
    }
    pub fn attempted(&self) -> bool {
        self.attempted
    }
    pub fn installed(&self) -> bool {
        self.execution_is_installed() && self.source.is_some() && self.join.is_some()
    }
    pub fn status(&self) -> &'static str {
        self.phase
    }
    pub fn failure_reason(&self) -> &'static str {
        use conduit_net::StationFailure as F;
        match self
            .cleanup_failure
            .or(self.native_failure.map(|f| f.reason))
        {
            Some(F::Rejected) => "rejected",
            Some(F::Pressure) => "pressure",
            Some(F::ProviderUnavailable) => "provider-unavailable",
            Some(F::Deadline) => "deadline",
            None => self.failure.map_or("none", |f| f.code.as_str()),
        }
    }
    pub fn failure_phase(&self) -> &'static str {
        use conduit_net::StationPhase as P;
        if self.cleanup_failure.is_some() {
            return "restore-access-point";
        }
        match self.native_failure.map(|f| f.phase) {
            Some(P::WithdrawAccessPoint) => "withdraw-access-point",
            Some(P::Associate) => "associate",
            Some(P::ConfigureAddress) => "configure-address",
            None => "none",
        }
    }
    pub fn append_offer(&self, ad: &mut c::HostAdvertisement) {
        if !self.attempted {
            ad.capabilities
                .push(conduit_net::network_attachment_sign_local_offer(
                    CAP.into(),
                    IMPLEMENTATION.into(),
                    ARTIFACT.into(),
                ));
        }
    }
    /// One admitted control invocation, at most 100 kernel steps and three
    /// sequential Host Calls. SDK native storage/time budgets remain owned by
    /// the selected join provider. Draining is not semantic completion.
    pub async fn run(&mut self) {
        let complete = self.installed();
        if !install_gate::begin_run(&mut self.phase, complete) {
            return;
        }
        let prepared = self.prepared.as_mut().expect("installed fixed storage");
        let roles = prepared.roles;
        let kernel = &mut prepared.kernel;
        let mut buffer = [0; 167];
        for _ in 0..100 {
            if let Some(request) = kernel.next_host_request() {
                let mut failure = None;
                let mut output = None;
                let input = match kernel.host_value(request.input.value) {
                    Ok(value) => value,
                    Err(_) => {
                        self.failure = Some(fail(k::FailureCode::InvalidInput, 1));
                        break;
                    }
                };
                if request.node == roles.credentials {
                    self.dispatches[0] += 1;
                    match self.source.as_mut().expect("selected source").invoke(
                        request.node,
                        request.call,
                        request.request,
                        input,
                        &mut buffer,
                    ) {
                        Ok(len) => match kernel.store_host_value(&buffer[..len]) {
                            Ok(value) => {
                                output = Some(
                                    k::BoundedValueRef::new(value, 167)
                                        .expect("bounded stored result"),
                                )
                            }
                            Err(_) => failure = Some(fail(k::FailureCode::StorageExhausted, 2)),
                        },
                        Err(_) => failure = Some(fail(k::FailureCode::HostCallDenied, 3)),
                    }
                    buffer.fill(0);
                } else if request.node == roles.join {
                    self.dispatches[1] += 1;
                    match self
                        .join
                        .as_mut()
                        .expect("selected join")
                        .invoke(request.node, request.call, request.request, input)
                        .await
                    {
                        Ok(bytes) => {
                            self.station_outcome = station_outcome::StationOutcome::attempt(
                                Ok(()),
                                embassy_time::Instant::now().as_millis(),
                            );
                            match kernel.store_host_value(bytes) {
                                Ok(value) => {
                                    output = Some(
                                        k::BoundedValueRef::new(value, 167)
                                            .expect("bounded stored result"),
                                    )
                                }
                                Err(_) => failure = Some(fail(k::FailureCode::StorageExhausted, 4)),
                            }
                        }
                        Err(n::JoinCallRefusal::Provider(error)) => {
                            self.station_outcome = station_outcome::StationOutcome::attempt(
                                Err(error),
                                embassy_time::Instant::now().as_millis(),
                            );
                            self.native_failure = Some(error);
                            failure = Some(fail(
                                if error.reason == conduit_net::StationFailure::Pressure {
                                    k::FailureCode::StorageExhausted
                                } else {
                                    k::FailureCode::HostCallFailed
                                },
                                5,
                            ));
                        }
                        Err(_) => failure = Some(fail(k::FailureCode::HostCallDenied, 6)),
                    }
                } else if request.node == roles.attachment_sign
                    && request.call == k::HostCallId(0)
                    && request.request == k::RequestId(0)
                    && !self.receipt_observed
                    && self
                        .join
                        .as_ref()
                        .and_then(n::NetworkJoinHostCall::completed_output)
                        .is_some_and(|expected| !expected.is_empty() && input == expected)
                {
                    self.dispatches[2] += 1;
                    self.receipt_observed = true;
                } else {
                    failure = Some(fail(k::FailureCode::HostCallDenied, 7));
                }
                if let Some(f) = failure {
                    self.failure = Some(f);
                }
                if kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: if failure.is_some() {
                                k::HostCallDisposition::Failed
                            } else {
                                k::HostCallDisposition::Completed
                            },
                            output,
                            failure,
                        },
                    )
                    .is_err()
                {
                    self.failure = Some(fail(k::FailureCode::InvalidLifecycle, 8));
                    break;
                }
            }
            self.steps += 1;
            match kernel.step() {
                Ok(SchedulerStatus::Drained) if self.receipt_observed => {
                    self.phase = "attached";
                    // A receipt does not itself prove retirement. The actual
                    // scheduler gate may refuse, retaining its identical Box.
                    self.retire_completed_kernel();
                    return;
                }
                Ok(SchedulerStatus::Drained) => {
                    self.failure = Some(fail(k::FailureCode::InvalidLifecycle, 10));
                    break;
                }
                Err(k::scheduler::SchedulerError::BackFailed(error)) => {
                    if self.failure.is_none() {
                        self.failure = Some(error);
                    }
                    break;
                }
                Err(_) => {
                    self.failure = Some(fail(k::FailureCode::InvalidLifecycle, 11));
                    break;
                }
                Ok(SchedulerStatus::Cancelled) => {
                    self.failure = Some(fail(k::FailureCode::Cancelled, 12));
                    break;
                }
                _ => {}
            }
        }
        if self.failure.is_none() {
            self.failure = Some(fail(k::FailureCode::WorkBudgetExhausted, 9));
        }
        let _ = kernel.cancel();
        let _ = self.source.as_mut().expect("selected source").revoke();
        let join = self.join.as_mut().expect("selected join");
        let _ = join.revoke(roles.join, k::HostCallId(0));
        if !join.cleanup_required() {
            self.phase = "setup-denied";
            return;
        }
        self.phase = "restoring-access-point";
        match join.cleanup().await {
            Ok(()) => self.phase = "access-point-restored",
            Err(error) => {
                self.cleanup_failure = Some(error);
                self.phase = "cleanup-failed";
            }
        }
    }
}
const fn fail(code: k::FailureCode, detail: u16) -> k::Failure {
    k::Failure {
        code,
        detail: 2100 + detail,
    }
}
