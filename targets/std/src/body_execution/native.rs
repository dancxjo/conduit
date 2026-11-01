//! Local native effects borrow no ambient authority: the original Host and
//! partition reservation move together through the independently installed Back.
use super::*;
use crate::kernel_preparation::KernelResourceReservation;
use crate::PreparedNativeWork;
use conduit_core::{ActivePlayIdentity, HostAdvertisement, PlacementId, PlanFragment};
use conduit_plan_lowering::lowering::LoweredPlanFragment;
use std::sync::Arc;

/// Finite diagnostic vocabulary. No provider text, endpoint or payload is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeFailureCategory {
    Unknown,
    Acquisition,
    Authorization,
    Capacity,
    TransportTimeout,
    TransportIo,
    ConnectionRefused,
    ProviderSpawn,
    ProviderProtocol,
    ProviderRefused,
    MalformedResponse,
    SourceIdentity,
    MalformedObservation,
    SourceContinuity,
    Retired,
}
impl NativeFailureCategory {
    pub(crate) fn denied(self) -> bool {
        matches!(self, Self::Unknown | Self::Authorization | Self::Retired)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct NativeFailureDiagnostic {
    pub category: NativeFailureCategory,
    pub node: u32,
    pub call: u32,
    pub request: u32,
}

/// Cooperative native adapter for one finite admitted effect. Implementations
/// must return the SAME Host and work, including on provider refusal. Their
/// independently selected deadlines bound IO; this is not preemptive containment.
pub trait BodyNativeProvider {
    /// Category for the immediately returned refusal; never grants execution authority.
    fn failure_category(&self) -> NativeFailureCategory {
        NativeFailureCategory::Unknown
    }
    fn execute(
        &mut self,
        host: StdHost,
        work: PreparedNativeWork,
    ) -> (StdHost, PreparedNativeWork, Result<Vec<u8>, String>);
}

/// Prepared descriptors are inert until joined to the ORIGINAL reserved lease.
pub(crate) struct LocalNativeDescriptor {
    pub inventory: Arc<HostAdvertisement>,
    pub fragment: Arc<PlanFragment>,
    pub lowered: Arc<LoweredPlanFragment>,
    pub active: ActivePlayIdentity,
    pub placement: PlacementId,
    pub input: Vec<u8>,
    pub maximum_output_bytes: u32,
}

pub(crate) struct NativeExecution<'a> {
    host: Option<StdHost>,
    reservations: Vec<Option<KernelResourceReservation>>,
    provider: &'a mut dyn BodyNativeProvider,
    // A mismatched affine return cannot be released as if it were our lease.
    // Preserve it through teardown while the returned Host remains non-idle.
    quarantined: Option<PreparedNativeWork>,
    pub(crate) last_failure: Option<NativeFailureDiagnostic>,
}
impl NativeExecution<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn execute(
        &mut self,
        partition: usize,
        fragment: &PlanFragment,
        active: &ActivePlayIdentity,
        placement: &PlacementId,
        request: HostCallRequest,
        input: &[u8],
        maximum: u32,
        descriptor: LocalNativeDescriptor,
    ) -> Result<(LocalNativeDescriptor, Result<Vec<u8>, String>), String> {
        let reservation = self
            .reservations
            .get_mut(partition)
            .and_then(Option::take)
            .ok_or("native request lost its original partition reservation")?;
        let host = self
            .host
            .take()
            .expect("native runner owns Host between effects");
        let work = PreparedNativeWork::from_local(reservation, descriptor, request, input);
        let (host, work, result) = self.provider.execute(host, work);
        let was_started = work.was_started();
        let reunited = work.reunite_local(&host, fragment, active, request, placement);
        self.host = Some(host);
        let descriptor = match reunited {
            Ok((reservation, descriptor)) => {
                self.reservations[partition] = Some(reservation);
                descriptor
            }
            Err(refusal) => {
                self.last_failure = Some(NativeFailureDiagnostic {
                    category: NativeFailureCategory::Authorization,
                    node: u32::from(request.node.0),
                    call: u32::from(request.call.0),
                    request: request.request.0,
                });
                self.quarantined = Some(refusal.work);
                return Err(refusal.reason);
            }
        };
        let mut category = self.provider.failure_category();
        let result = if result.is_ok() && !was_started {
            category = NativeFailureCategory::Authorization;
            Err("native output arrived without the admitted attempt starting".into())
        } else if result
            .as_ref()
            .is_ok_and(|bytes| bytes.len() > maximum as usize)
        {
            category = NativeFailureCategory::Capacity;
            Err("native output exceeds exact selected bound".into())
        } else {
            result
        };
        self.last_failure = result.as_ref().err().map(|_| NativeFailureDiagnostic {
            category,
            node: u32::from(request.node.0),
            call: request.call.0 as u32,
            request: request.request.0,
        });
        Ok((descriptor, result))
    }
}

impl StdHost {
    /// Run a finite local Body with independently retained native equipment.
    /// The ordinary kernel owns ordering, cancellation and typed Fore custody.
    /// No remote Line or extra reservation is created for a local effect.
    /// Returns the actual Host even when preparation, execution or cleanup fails.
    #[allow(clippy::too_many_arguments)]
    pub fn run_body_plan_owned_native_to<W: Write, T: TimerAdapter, F>(
        self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        provider: &mut dyn BodyNativeProvider,
        started: F,
    ) -> (Self, Result<BodyRunReport, String>)
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.run_body_plan_owned_native_fore_to(
            request,
            inputs,
            false,
            false,
            fore_output,
            output,
            timer,
            provider,
            started,
        )
    }

    /// Stream typed outputs to a synchronous borrowed consumer. Accepted bytes
    /// are owned by that consumer's admitted storage; the report retains Signs,
    /// not an accumulating copy of each output. Queue capacity is unchanged.
    #[allow(clippy::too_many_arguments)]
    pub fn run_body_plan_owned_native_borrowed_to<W: Write, T: TimerAdapter, F>(
        self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        fore_output: &mut dyn BodyBorrowedForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        provider: &mut dyn BodyNativeProvider,
        started: F,
    ) -> (Self, Result<BodyRunReport, String>)
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.run_body_plan_owned_native_fore_to(
            request,
            inputs,
            false,
            true,
            &mut super::borrowed_fore::BorrowedAdapter(fore_output),
            output,
            timer,
            provider,
            started,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_body_plan_owned_native_fore_to<W: Write, T: TimerAdapter, F>(
        mut self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        sequential: bool,
        borrowed_stream: bool,
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        provider: &mut dyn BodyNativeProvider,
        mut started: F,
    ) -> (Self, Result<BodyRunReport, String>)
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        let prepared = (|| {
            request
                .plan
                .validate_for(request.wake)
                .map_err(|e| format!("Body Plan validation: {e:?}"))?;
            if request.plan.body_time_requirement.is_some() {
                return Err(
                    "native Body runner requires an explicit BodyTime admission entrance".into(),
                );
            }
            // A native binder expects unshifted canonical fragment node IDs.
            // Until the provider API carries explicit remapping evidence, refuse
            // combined shapes rather than interpreting global IDs as local IDs.
            if request.plan.plots.len() != 1 || request.plan.plots[0].plan.fragments.len() != 1 {
                return Err("native local Body requires one exact local Plot fragment".into());
            }
            let play = BodyPlayIdentity::bind(request.plan, self.next_kernel_play_sequence);
            let next = self
                .next_kernel_play_sequence
                .checked_add(1)
                .ok_or("Body Play sequence exhausted")?;
            let mut kernel = BodyKernel::prepare_with_live(
                &request.plan.plots,
                request.keyboard.is_some(),
                &play.active_play_id,
                inputs,
                sequential,
                true,
                None,
                None,
                #[cfg(target_os = "linux")]
                None,
                &self.retained_native_calls,
            )?;
            if borrowed_stream {
                kernel.prepare_borrowed_fore()?;
            }
            kernel.prepare_native_slots(&self.advertisement, play.play_sequence)?;
            kernel.prepare_input_origin(&self.advertisement, request.plan, &play)?;
            kernel.require_supported_execution()?;
            let reservations = self.kernel_resources.prepare_and_reserve_plans(
                &self.advertisement,
                &request
                    .plan
                    .plots
                    .iter()
                    .map(|part| (&part.plan, false))
                    .collect::<Vec<_>>(),
            )?;
            let reservations = reservations.into_iter().map(Some).collect::<Vec<_>>();
            Ok((kernel, play, next, reservations))
        })();
        let (mut kernel, play, next, reservations) = match prepared {
            Ok(prepared) => prepared,
            Err(reason) => return (self, Err(reason)),
        };
        self.next_kernel_play_sequence = next;
        let sign = |sequence| {
            bind_sign(
                &self.advertisement.host_id,
                &self.advertisement.boot_id,
                Some(&play.active_play_id),
                sequence,
            )
        };
        let lifecycle = request
            .wake
            .body_plan_ready(request.plan, sign(0).sign_id)
            .and_then(|wake| wake.body_play_started(request.plan, &play, sign(1).sign_id))
            .map_err(|e| format!("Body start lifecycle: {e:?}"));
        let wake_at_start = match lifecycle.and_then(|wake| {
            kernel.confirm_input_play(&self.advertisement, &play)?;
            started(&play, &wake)?;
            Ok(wake)
        }) {
            Ok(wake) => wake,
            Err(reason) => {
                let errors = reservations
                    .into_iter()
                    .flatten()
                    .filter_map(|lease| self.kernel_resources.release(lease).err())
                    .collect();
                return (self, finish_body_release(Err(reason), errors));
            }
        };
        let terminal_sign = sign(2);
        let host_id = self.advertisement.host_id.clone();
        let boot_id = self.advertisement.boot_id.clone();
        let mut execution = NativeExecution {
            host: Some(self),
            reservations,
            provider,
            quarantined: None,
            last_failure: None,
        };
        let result = kernel.run_with_native(
            output,
            timer,
            request.keyboard,
            Some(fore_output),
            request.control,
            &host_id,
            &boot_id,
            None,
            None,
            None,
            None,
            Some(&mut execution),
        );
        let mut host = execution
            .host
            .take()
            .expect("native provider returned actual Host");
        let errors = execution
            .reservations
            .into_iter()
            .flatten()
            .filter_map(|lease| host.kernel_resources.release(lease).err())
            .collect();
        let report = report_from_kernel(play, wake_at_start, terminal_sign, result);
        (host, finish_body_release(Ok(report), errors))
    }
}

fn report_from_kernel(
    play: BodyPlayIdentity,
    wake_at_start: Wake,
    terminal_sign: SignIdentity,
    result: crate::installed_std::body_kernel::BodyKernelResult,
) -> BodyRunReport {
    BodyRunReport {
        play,
        wake_at_start,
        terminal: result.terminal,
        failure: result.failure,
        kernel_failure: result.kernel_failure,
        native_failure: result.native_failure,
        cleanup_failure: result.cleanup_failure,
        terminal_sign,
        partitions: result.partitions,
        requests: result.requests,
        kernel_events: result.events,
        scan_child_signs: result.scan_child_signs,
        scan_cancellation_failed: result.scan_cancellation_failed,
        scan_output_completion_failed: result.scan_output_completion_failed,
        each_child_signs: result.each_child_signs,
        each_cancellation_failed: result.each_cancellation_failed,
        each_output_completion_failed: result.each_output_completion_failed,
        fore_deliveries: result.fore_deliveries,
        fore_pending_output: result.fore_pending_output,
        live_fore_status: None,
        clock_observations: result.clock_observations,
        clock_quality: result.clock_quality,
        clock_execution_bounds: result.clock_execution_bounds,
    }
}
