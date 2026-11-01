//! One affine native request while its ordinary kernel/session remains resident.
use super::*;
use conduit_core::{HostAdvertisement, PlacementId};
use conduit_plan_lowering::{
    lowering::LoweredPlanFragment, selected_operation::SelectedOperationPlan,
};
use std::sync::Arc;
/// The ORIGINAL admitted reservation moves here for the worker's finite turn.
/// No Clone/Deserialize or constructor from claimed IDs is provided.
pub struct PreparedNativeWork {
    reservation: Option<KernelResourceReservation>,
    inventory: Arc<HostAdvertisement>,
    fragment: Arc<PlanFragment>,
    lowered: Arc<LoweredPlanFragment>,
    active: ActivePlayIdentity,
    placement: PlacementId,
    request: HostCallRequest,
    input: Vec<u8>,
    maximum_output_bytes: u32,
    started: bool,
}
impl PreparedNativeWork {
    pub fn selected(&self) -> SelectedOperationPlan<'_> {
        SelectedOperationPlan {
            fragment: &self.fragment,
            lowered: &self.lowered,
            active: &self.active,
            placement_id: &self.placement,
        }
    }
    pub fn request(&self) -> HostCallRequest {
        self.request
    }
    pub fn input(&self) -> &[u8] {
        &self.input
    }
    pub fn maximum_output_bytes(&self) -> u32 {
        self.maximum_output_bytes
    }
}
pub struct NativeWorkReturnRefusal {
    pub work: PreparedNativeWork,
    pub reason: String,
}
impl core::fmt::Debug for NativeWorkReturnRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NativeWorkReturnRefusal")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}
/// Quiesced native completion after the original reservation was reunited.
pub struct ReturnedNativeRequest {
    request: HostCallRequest,
    active: ActivePlayIdentity,
    maximum_output_bytes: u32,
}
impl ReturnedNativeRequest {
    /// Cancellation discards late bytes. No output becomes committed merely
    /// because the worker returned, and no transport ACK is invented here.
    pub fn complete(
        self,
        admitted: &mut AdmittedRemoteFragment,
        output: Option<&[u8]>,
    ) -> Result<bool, String> {
        if admitted.identity != self.active
            || admitted.native_work_request != Some(self.request)
            || admitted.reservation.is_none()
        {
            return Err("native completion lost its exact original admitted owner".into());
        }
        admitted.native_work_request = None;
        if !admitted.runtime.native_request_is_pending(self.request) {
            return Ok(false);
        }
        let outcome = match output {
            Some(bytes) => {
                if bytes.len() > self.maximum_output_bytes as usize {
                    return Err("native result exceeds the exact selected output bound".into());
                }
                let value = admitted.runtime.store_host_value(bytes)?;
                conduit_kernel::HostCallOutcome {
                    disposition: conduit_kernel::HostCallDisposition::Completed,
                    output: Some(value),
                    failure: None,
                }
            }
            None => conduit_kernel::HostCallOutcome {
                disposition: conduit_kernel::HostCallDisposition::Denied,
                output: None,
                failure: Some(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::HostCallDenied,
                    detail: 1,
                }),
            },
        };
        admitted.runtime.complete_host_call(self.request, outcome)?;
        Ok(true)
    }
}
impl StdHost {
    pub fn prepare_native_work(
        &self,
        admitted: &mut AdmittedRemoteFragment,
        request: HostCallRequest,
    ) -> Result<PreparedNativeWork, String> {
        let witness = self.admitted_native_request(admitted, request)?;
        let selected = witness.selected();
        let work = witness.work();
        let prepared = (
            selected.fragment.clone(),
            selected.lowered.clone(),
            selected.active.clone(),
            selected.placement_id.clone(),
            work.input.to_vec(),
            work.maximum_output_bytes,
        );
        let reservation = admitted
            .reservation
            .take()
            .ok_or("native work needs its original admitted reservation")?;
        admitted.native_work_request = Some(request);
        Ok(PreparedNativeWork {
            reservation: Some(reservation),
            inventory: Arc::new(self.advertisement.clone()),
            fragment: Arc::new(prepared.0),
            lowered: Arc::new(prepared.1),
            active: prepared.2,
            placement: prepared.3,
            request,
            input: prepared.4,
            maximum_output_bytes: prepared.5,
            started: false,
        })
    }
    pub fn validate_native_work(&self, work: &PreparedNativeWork) -> Result<(), String> {
        if work.reservation.is_none()
            || work.inventory.as_ref() != &self.advertisement
            || work.active.host_id != self.advertisement.host_id
            || work.active.boot_id != self.advertisement.boot_id
            || work.fragment.offer_generation != self.advertisement.offer_generation
            || !conduit_core::verify_plan_fragment(&work.fragment)
        {
            return Err(
                "native work does not own the current original Host/Boot/inventory lease".into(),
            );
        }
        Ok(())
    }
    /// Admit one native attempt while retaining the original affine lease.
    pub fn begin_native_work(&self, work: &mut PreparedNativeWork) -> Result<(), String> {
        self.validate_native_work(work)?;
        if work.started {
            return Err("native work was already started".into());
        }
        work.started = true;
        Ok(())
    }
    #[allow(clippy::result_large_err)]
    pub fn return_native_work(
        &self,
        admitted: &mut AdmittedRemoteFragment,
        mut work: PreparedNativeWork,
    ) -> Result<ReturnedNativeRequest, NativeWorkReturnRefusal> {
        let checked = self.validate_native_work(&work).and_then(|()| {
            if admitted.reservation.is_some()
                || admitted.native_work_request != Some(work.request)
                || admitted.identity != work.active
                || &admitted.source_fragment != work.fragment.as_ref()
            {
                return Err(
                    "native work returned to a different admitted fragment or duplicate lease"
                        .into(),
                );
            }
            Ok(())
        });
        if let Err(reason) = checked {
            return Err(NativeWorkReturnRefusal { work, reason });
        }
        admitted.reservation = work.reservation.take();
        Ok(ReturnedNativeRequest {
            request: work.request,
            active: work.active,
            maximum_output_bytes: work.maximum_output_bytes,
        })
    }
}

impl PreparedNativeWork {
    pub(crate) fn was_started(&self) -> bool {
        self.started
    }

    /// Move preallocated, inert preparation into the original affine lease.
    pub(crate) fn from_local(
        reservation: KernelResourceReservation,
        mut descriptor: crate::body_execution::native::LocalNativeDescriptor,
        request: HostCallRequest,
        input: &[u8],
    ) -> Self {
        // The route checks this before moving any affine owner.
        assert!(input.len() <= descriptor.input.capacity());
        descriptor.input.clear();
        descriptor.input.extend_from_slice(input);
        Self {
            reservation: Some(reservation),
            inventory: descriptor.inventory,
            fragment: descriptor.fragment,
            lowered: descriptor.lowered,
            active: descriptor.active,
            placement: descriptor.placement,
            request,
            input: descriptor.input,
            maximum_output_bytes: descriptor.maximum_output_bytes,
            started: false,
        }
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn reunite_local(
        mut self,
        host: &StdHost,
        fragment: &PlanFragment,
        active: &ActivePlayIdentity,
        request: HostCallRequest,
        placement: &PlacementId,
    ) -> Result<
        (
            KernelResourceReservation,
            crate::body_execution::native::LocalNativeDescriptor,
        ),
        NativeWorkReturnRefusal,
    > {
        let checked = host.validate_native_work(&self).and_then(|()| {
            if self.fragment.as_ref() != fragment
                || &self.active != active
                || self.request != request
                || &self.placement != placement
            {
                return Err("native work returned to a different local Body request".into());
            }
            Ok(())
        });
        if let Err(reason) = checked {
            return Err(NativeWorkReturnRefusal { work: self, reason });
        }
        let lease = self
            .reservation
            .take()
            .expect("validated original reservation");
        Ok((
            lease,
            crate::body_execution::native::LocalNativeDescriptor {
                inventory: self.inventory,
                fragment: self.fragment,
                lowered: self.lowered,
                active: self.active,
                placement: self.placement,
                input: self.input,
                maximum_output_bytes: self.maximum_output_bytes,
            },
        ))
    }
}
