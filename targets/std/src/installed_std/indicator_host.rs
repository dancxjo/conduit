//! Exact acquired-resource validation and correlated indicator effect completion.
use crate::hosted_indicator::{
    HostedIndicatorAdapter, IndicatorBinding, IndicatorFailure, IndicatorRequest,
};
use conduit_core::{ActivePlayIdentity, HostAdvertisement, InfoBool, PlanFragment};
use conduit_kernel::{
    scheduler::HostOperationRequest, Failure, FailureCode, HostOperationDisposition,
    HostOperationOutcome, NodeId,
};
use conduit_plan_lowering::lowering::KernelIdentityMap;

pub(super) struct IndicatorHost<'a> {
    adapter: Option<&'a mut dyn HostedIndicatorAdapter>,
    selected: Option<(NodeId, IndicatorBinding)>,
    play: ActivePlayIdentity,
}

impl<'a> IndicatorHost<'a> {
    pub(super) fn prepare(
        adapter: Option<&'a mut dyn HostedIndicatorAdapter>,
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        identity: &KernelIdentityMap,
        play: &ActivePlayIdentity,
    ) -> Result<Self, String> {
        let mut selected = None;
        for placement in &fragment.placements {
            if placement.implementation_id.as_str()
                != conduit_std_offers::indicator_resource::IMPLEMENTATION
            {
                continue;
            }
            if selected.is_some() {
                return Err("one acquired indicator adapter cannot own multiple placements".into());
            }
            let provider = adapter
                .as_deref()
                .ok_or("planned indicator has no acquired Host adapter")?;
            let binding = provider.binding();
            if binding.host_id != advertisement.host_id
                || binding.boot_id != advertisement.boot_id
                || binding.offer_generation != advertisement.offer_generation
                || !placement.resources.iter().any(|resource| {
                    resource.pool_id == binding.pool_id
                        && resource.class_id.as_str()
                            == conduit_std_offers::indicator_resource::RESOURCE_CLASS
                        && resource.units == 1
                        && resource.protected.is_none()
                        && resource.compute.is_none()
                        && resource.content.is_none()
                })
            {
                return Err("acquired indicator Host/Boot/generation/resource binding is stale or mismatched".into());
            }
            let node = identity
                .node_for_placement(&placement.placement_id)
                .ok_or("indicator placement was not lowered")?;
            selected = Some((node, binding.clone()));
        }
        Ok(Self {
            adapter,
            selected,
            play: play.clone(),
        })
    }

    pub(super) fn present(
        &mut self,
        request: HostOperationRequest,
        input: &[u8],
    ) -> HostOperationOutcome {
        let result = (|| {
            let (node, binding) = self
                .selected
                .as_ref()
                .ok_or(IndicatorFailure::StaleIdentity)?;
            let adapter = self.adapter.as_deref_mut().ok_or(IndicatorFailure::Lost)?;
            if *node != request.node || adapter.binding() != binding {
                return Err(IndicatorFailure::StaleIdentity);
            }
            let state = InfoBool::decode(input).map_err(|_| IndicatorFailure::InvalidInput)?;
            adapter.present(IndicatorRequest {
                play: &self.play,
                request: request.request,
                state,
            })
        })();
        match result {
            Ok(()) => HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
            Err(error) => HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(Failure {
                    code: if error == IndicatorFailure::Cancelled {
                        FailureCode::Cancelled
                    } else {
                        FailureCode::HostOperationFailed
                    },
                    detail: error as u16,
                }),
            },
        }
    }
}
