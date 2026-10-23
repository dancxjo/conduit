//! Root-owned control Play through the production fixed kernel. No direct
//! radio method is available; only exact selected owners dispatch Host Calls.
use crate::native_authority::{Operation, RootIssuer};
use conduit_core as c;
use conduit_kernel::{self as k, scheduler::SchedulerStatus};
use conduit_network_join_call::{
    self as n, credential_installation::ReadyCredentialBase, installation::ReadyNetworkJoinBase,
    PreparedNetworkKernel,
};
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;
type Source = crate::native_credentials::RootCredentialSource;
type Provider = crate::native_network::Provider;
const CAP: &str = "conduit/esp32/network-attachment-receipt@1";
const IMPLEMENTATION: &str = "esp32/network-attachment-receipt@1";
const ARTIFACT: &str = "native/esp32-network-attachment-receipt@1";
pub struct NativePlay {
    steps: u16,
    dispatches: [u8; 3],
    attempted: bool,
    phase: &'static str,
    prepared: Option<PreparedNetworkKernel>,
    source: Option<n::NetworkCredentialsHostCall<Source>>,
    join: Option<n::NetworkJoinHostCall<Provider>>,
    active: Option<c::ActivePlayIdentity>,
    expected: [u8; 167],
    expected_len: usize,
    receipt: [u8; 167],
    receipt_len: usize,
    failure: Option<k::Failure>,
    native_failure: Option<conduit_net::StationAttemptFailure>,
    cleanup_failure: Option<conduit_net::StationFailure>,
}
impl NativePlay {
    pub fn new() -> Self {
        Self {
            steps: 0,
            dispatches: [0; 3],
            attempted: false,
            phase: "uninstalled",
            prepared: None,
            source: None,
            join: None,
            active: None,
            expected: [0; 167],
            expected_len: 0,
            receipt: [0; 167],
            receipt_len: 0,
            failure: None,
            native_failure: None,
            cleanup_failure: None,
        }
    }
    pub fn progress(&self) -> (u16, [u8; 3]) {
        (self.steps, self.dispatches)
    }
    pub fn active(&self) -> Option<&c::ActivePlayIdentity> {
        self.active.as_ref()
    }
    pub fn receipt_received(&self) -> bool {
        self.receipt_len > 0
    }
    pub fn attempted(&self) -> bool {
        self.attempted
    }
    pub fn installed(&self) -> bool {
        self.prepared.is_some() && self.source.is_some() && self.join.is_some()
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
    /// Every fallible transfer uses retained slots. A partial installation stays
    /// owned here on refusal and never starts a partially selected kernel.
    pub fn install(
        &mut self,
        prepared: &crate::network_planning::PreparedNetworkSetup,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
    ) -> Result<(), ()> {
        if self.attempted {
            return Err(());
        }
        self.attempted = true;
        self.phase = "installation-refused";
        let fragment = &prepared.plan().fragments[0];
        let lowered = prepared.lowered();
        let kernel = PreparedNetworkKernel::prepare(fragment, lowered).map_err(|_| ())?;
        let role = kernel.roles;
        let sign = &fragment
            .placements
            .iter()
            .find(|g| {
                g.placement_id == lowered.nodes[usize::from(role.attachment_sign.0)].placement_id
            })
            .ok_or(())?;
        if sign.capability_id.as_str() != CAP
            || sign.implementation_id.as_str() != IMPLEMENTATION
            || sign.artifact_id.as_str() != ARTIFACT
            || sign.execution_profile_id.as_str() != conduit_net::NETWORK_ATTACHMENT_LOCAL_PROFILE
            || sign.host_calls.len() != 1
            || sign.host_calls[0].contract_id.as_str()
                != conduit_net::NETWORK_ATTACHMENT_SIGN_HOST_CALL
        {
            return Err(());
        }
        let active = issuer.reserve_play(fragment)?;
        self.prepared = Some(kernel);
        let (table, handle, claim) = issuer.issue(
            source.as_ref().ok_or(())?.native_identity(),
            Operation::Credentials,
            fragment,
            &active,
        )?;
        self.source = Some(
            ReadyCredentialBase::bind_selected_retained(
                source,
                table,
                handle,
                claim,
                &n::PreparedCredentialContract::prepare(),
                SelectedOperationPlan {
                    fragment,
                    lowered,
                    active: &active,
                    placement_id: &lowered.nodes[usize::from(role.credentials.0)].placement_id,
                },
            )
            .map_err(|_| ())?,
        );
        let (table, handle, claim) = issuer.issue(
            join.as_ref().ok_or(())?.native_identity(),
            Operation::Join,
            fragment,
            &active,
        )?;
        self.join = Some(
            ReadyNetworkJoinBase::bind_selected_retained(
                join,
                table,
                handle,
                claim,
                &n::PreparedJoinContract::prepare(),
                SelectedOperationPlan {
                    fragment,
                    lowered,
                    active: &active,
                    placement_id: &lowered.nodes[usize::from(role.join.0)].placement_id,
                },
            )
            .map_err(|_| ())?,
        );
        self.active = Some(active);
        self.phase = "installed";
        Ok(())
    }
    /// One admitted control invocation, at most 100 kernel steps and three
    /// sequential Host Calls. SDK native storage/time budgets remain owned by
    /// the selected join provider. Draining is not semantic completion.
    pub async fn run(&mut self) {
        if self.phase != "installed" || !self.installed() {
            return;
        }
        self.phase = "running";
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
                            self.expected[..bytes.len()].copy_from_slice(bytes);
                            self.expected_len = bytes.len();
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
                    && self.receipt_len == 0
                    && self.expected_len > 0
                    && input == &self.expected[..self.expected_len]
                {
                    self.dispatches[2] += 1;
                    self.receipt[..input.len()].copy_from_slice(input);
                    self.receipt_len = input.len();
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
                Ok(SchedulerStatus::Drained) if self.receipt_len > 0 => {
                    self.phase = "attached";
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
