//! One independently initialized native Source in an installed Host.
//!
//! This entrance does not turn an offer, callback or observation into authority.
//! The retained Base, native definition and grant precede planning; possession is
//! issued only for the current registry entry and the actual installed Play.
mod definition;
mod image;
mod immediate;
mod installation;
mod preparation;
mod selection;
use definition::validate_definition;
pub(crate) use image::InitialNativeSource;
pub use preparation::ImmediateSourcePreparationBudget;
mod worker;
use conduit_core::*;
use conduit_kernel::NodeId;
pub use worker::{
    NativeReadContext, NativeSourceWorkerProvider, NativeWorkerBudget, WorkerBackingObservation,
    WorkerFailure, NATIVE_SOURCE_WORKER_PROFILE,
};

/// Independently retained definition of a single-call native Source.
/// All fields describe the initialized provider, never received Plan bytes.
pub struct NativeSourceDefinition {
    pub kind: Kind,
    pub base: BaseProviderEntry,
    pub grant: AuthorityGrant,
    pub resource_generation: ResourceGenerationId,
    pub configuration: Vec<ConfigurationEntry>,
    pub input_bytes: u32,
    pub output_bytes: u32,
    pub resource_bytes: u64,
}

/// Native correspondence boundary for an immediate bounded CPU Source.
///
/// # Safety
/// Implementations must represent the actual independently initialized owner
/// described by `NativeSourceDefinition`, retain continuity across child Plays,
/// and return within its admitted finite native execution budget without IO,
/// sleeping, allocation, unwinding, background work or retained borrows. Physical IO must
/// use a separately admitted, cancellable worker; this immediate entrance is
/// deliberately insufficient for a blocking USB sensor capture.
pub unsafe trait ImmediateNativeSource: Send {
    fn read(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, String>;
}

pub(crate) struct InitializedNativeSource {
    definition: NativeSourceDefinition,
    provider: NativeProvider,
    _image_memory: Option<crate::kernel_preparation::ImageMemoryLease>,
    _metadata_memory: Option<crate::kernel_preparation::ImageMemoryLease>,
    pub(super) backing: Option<crate::kernel_preparation::BaseBackingReservation>,
}
enum NativeProvider {
    Immediate(Box<dyn ImmediateNativeSource>),
    Worker(worker::NativeWorker),
}

impl crate::StdHost {
    /// Borrow the original grant retained by independently authorized Root
    /// construction. An advertisement, checked Source or received Plan cannot
    /// supply this private native owner. Retirement removes this grant too.
    /// This is a planner input, not proof of current execution possession.
    pub fn native_source_authority_grant(&self) -> Option<&AuthorityGrant> {
        self.native_source
            .as_ref()
            .map(|source| &source.definition.grant)
    }

    /// Install only while the actual finite resource ledger is idle. Refusal
    /// returns the affine native owner unchanged; replacing an existing session
    /// requires its separately implemented quiescent retirement path.
    ///
    /// # Safety
    /// The caller must establish correspondence between this exact definition
    /// and the actual initialized native provider, including its owned finite
    /// memory/execution budget and retained source continuity. The grant must
    /// come from independent current Host authorization, not an incoming Plan,
    /// observer receipt, or callback. These native facts cannot be inferred
    /// from the descriptive IDs in `definition`.
    #[allow(clippy::result_large_err)]
    pub unsafe fn install_immediate_native_source(
        &mut self,
        definition: NativeSourceDefinition,
        provider: Box<dyn ImmediateNativeSource>,
    ) -> Result<
        (),
        (
            NativeSourceDefinition,
            Box<dyn ImmediateNativeSource>,
            String,
        ),
    > {
        let prepared = self.stage_native_source(&definition, None);
        match prepared {
            Err(reason) => Err((definition, provider, reason)),
            Ok((registry, advertisement, ledger)) => {
                self.base_registry = registry;
                self.advertisement = advertisement;
                self.kernel_resources = ledger;
                self.native_source = Some(InitializedNativeSource {
                    definition,
                    provider: NativeProvider::Immediate(provider),
                    backing: None,
                    _image_memory: None,
                    _metadata_memory: None,
                });
                Ok(())
            }
        }
    }
}

pub(crate) struct PreparedNativeSource<'a> {
    pub(crate) node: NodeId,
    operation: BoundBaseOperation,
    provider: &'a mut NativeProvider,
    output: Box<[u8]>,
    next_request: u32,
    pending: Option<NativePending>,
    cancelled: bool,
    cancellation_started: Option<std::time::Instant>,
}
struct NativePending {
    request: conduit_kernel::scheduler::HostCallRequest,
    generation: u64,
    lease: BaseOperationLease,
}
impl PreparedNativeSource<'_> {
    /// Newly owned operation/result payload; the borrowed provider retains its
    /// separately reserved Base backing. This observation grants no authority.
    pub(crate) fn retained_backing_bytes(&self) -> Option<usize> {
        self.operation
            .retained_backing_bytes()?
            .checked_add(self.output.len())
    }
    pub(crate) fn input_bytes(&self) -> u32 {
        self.operation.inspection().scope.maximum_parameter_bytes
    }
    pub(crate) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(crate) fn cancel(&mut self) {
        self.cancelled = true;
        self.cancellation_started
            .get_or_insert_with(std::time::Instant::now);
        if let NativeProvider::Worker(worker) = self.provider {
            worker.cancel();
        }
    }
    pub(crate) fn read(
        &mut self,
        request: conduit_kernel::scheduler::HostCallRequest,
        input: &[u8],
    ) -> Result<Option<&[u8]>, String> {
        if matches!(self.provider, NativeProvider::Immediate(_)) {
            return self
                .read_immediate(request, input)
                .map(Some)
                .map_err(|error| format!("{error}"));
        }
        if self.cancelled
            || self.pending.is_some()
            || request.node != self.node
            || request.call != conduit_kernel::HostCallId(0)
            || request.request != conduit_kernel::RequestId(self.next_request)
            || input.len() > self.operation.inspection().scope.maximum_parameter_bytes as usize
        {
            return Err("native Source request differs from selected call".into());
        }
        let lease = self
            .operation
            .authorize()
            .map_err(|e| format!("native Source authorization: {e:?}"))?;
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or("native Source request sequence exhausted")?;
        match self.provider {
            NativeProvider::Immediate(_) => unreachable!("immediate read dispatched above"),
            NativeProvider::Worker(worker) => match worker.submit(input) {
                Ok(generation) => {
                    self.pending = Some(NativePending {
                        request,
                        generation,
                        lease,
                    });
                    Ok(None)
                }
                Err(error) => {
                    self.operation
                        .complete(lease, 0)
                        .map_err(|e| format!("native Source unsubmitted completion: {e:?}"))?;
                    self.operation
                        .revoke()
                        .map_err(|e| format!("native Source unsubmitted revoke: {e:?}"))?;
                    Err(format!("native Source submit: {error:?}"))
                }
            },
        }
    }
    pub(crate) fn poll(
        &mut self,
    ) -> Result<Option<(conduit_kernel::scheduler::HostCallRequest, &[u8])>, String> {
        let Some(pending) = self.pending.as_ref() else {
            return Ok(None);
        };
        let NativeProvider::Worker(worker) = self.provider else {
            return Err("immediate Source cannot own pending IO".into());
        };
        if self
            .cancellation_started
            .is_some_and(|started| started.elapsed() > worker.cancellation_timeout())
        {
            worker.mark_cancellation_overdue();
        }
        let result = match worker.poll(pending.generation, &mut self.output) {
            Ok(None) => return Ok(None),
            Ok(Some(result)) => result,
            Err(error) => {
                if !worker.is_quiescent() {
                    worker.cancel();
                    return Ok(None);
                }
                Err(error)
            }
        };
        let pending = self.pending.take().expect("observed pending owner");
        let bytes = if self.cancelled {
            0
        } else {
            result.as_ref().copied().unwrap_or(0)
        };
        self.operation
            .complete(pending.lease, bytes as u32)
            .map_err(|e| format!("native worker completion: {e:?}"))?;
        if self.cancelled || result.is_err() {
            self.operation
                .revoke()
                .map_err(|e| format!("native worker revoke: {e:?}"))?;
            return if self.cancelled {
                Ok(None)
            } else {
                Err(format!("native worker read: {:?}", result.err().unwrap()))
            };
        }
        Ok(Some((pending.request, &self.output[..bytes])))
    }
}
impl Drop for PreparedNativeSource<'_> {
    fn drop(&mut self) {
        if self.pending.is_none() {
            return;
        }
        self.cancel();
        // Error/terminal cleanup cannot detach a live native read or free its
        // lease. The cooperative cancellation bound is observable, not proof
        // of quiescence: a stubborn provider keeps this ownership retained.
        while self.pending.is_some() {
            let _ = self.poll();
            if self.pending.is_some() {
                std::thread::park_timeout(std::time::Duration::from_millis(1));
            }
        }
    }
}

/// Read-only evidence of this Host's actual retained native worker backing.
/// It is not permission to allocate a worker or invoke a Source.
#[derive(Clone, Copy, Debug)]
pub struct NativeWorkerInspection {
    pub backing: WorkerBackingObservation,
    pub reserved_bytes: u32,
    pub idle_backing_is_reserved: bool,
    pub cancellation_overdue: bool,
}
pub struct RetiredNativeSource {
    pub definition: NativeSourceDefinition,
    /// None means the native provider was lost; no replacement is invented.
    pub provider: Option<Box<dyn NativeSourceWorkerProvider>>,
}
