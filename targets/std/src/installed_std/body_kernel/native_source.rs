//! An actual retained immediate native owner beneath the ordinary Body kernel.
use super::*;
use crate::selected_native_source::{InitializedNativeSource, PreparedNativeSource};
use conduit_body::{BodyPlan, BodyPlayIdentity, Wake};
use conduit_core::{BaseRegistry, HostAdvertisement};

pub(crate) struct NativeBodySource<'owner, 'context> {
    pub owner: &'owner mut InitializedNativeSource,
    pub registry: &'context BaseRegistry,
    pub host: &'context HostAdvertisement,
    pub wake: &'context Wake,
    pub plan: &'context BodyPlan,
    pub play: &'context BodyPlayIdentity,
}
pub(super) struct NativeSourceRoute<'a> {
    source: PreparedNativeSource<'a>,
    image_backed: bool,
    budget: super::super::factory::BackBudget,
}
impl NativeSourceRoute<'_> {
    pub(super) fn image_backing_bytes(&self) -> Option<usize> {
        if !self.image_backed {
            return None;
        }
        core::mem::size_of::<Option<Self>>().checked_add(self.source.retained_backing_bytes()?)
    }
    pub(super) fn node(&self) -> conduit_kernel::NodeId {
        self.source.node
    }
    pub(super) fn input_bytes(&self) -> u32 {
        self.source.input_bytes()
    }
    pub(super) fn budget(&self) -> &super::super::factory::BackBudget {
        &self.budget
    }
    pub(super) fn matches_call(
        &self,
        node: conduit_kernel::NodeId,
        call: conduit_kernel::HostCallId,
    ) -> bool {
        node == self.source.node && call == conduit_kernel::HostCallId(0)
    }
}
pub(super) fn prepare<'a>(
    native: Option<NativeBodySource<'a, '_>>,
    fragments: &[&PlanFragment],
    partitions: &[conduit_plan_lowering::lowering::LoweredPlanFragment],
) -> Result<Option<NativeSourceRoute<'a>>, String> {
    let Some(native) = native else {
        return Ok(None);
    };
    let selected = fragments
        .iter()
        .enumerate()
        .filter_map(|(index, fragment)| {
            fragment
                .placements
                .iter()
                .find(|gear| native.owner.matches(gear))
                .map(|gear| (index, *fragment, gear))
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok(None);
    }
    if selected.len() != 1 || !native.owner.is_immediate() {
        return Err("Body native Source requires one actual immediate owner".into());
    }
    let (index, fragment, gear) = selected[0];
    let budget = native.owner.budget();
    let image_backed = native.owner.has_image_backing();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
        .map_err(|e| format!("Body native lowering: {e:?}"))?;
    let mut source = native
        .owner
        .prepare_body(
            native.registry,
            native.host,
            fragment,
            &lowered,
            (native.wake, native.plan, native.play),
        )?
        .ok_or("Body native owner lost its selected placement")?;
    let map = &partitions[index].identity;
    if map.plan_id != fragment.plan_id || map.fragment_id != fragment.fragment_id {
        return Err("Body native partition differs from its selected fragment".into());
    }
    // Numeric remapping is owned by this verified combined lowering. The
    // operation retains its original fragment and containing Body Plan/Play.
    source.node = map
        .node_for_placement(&gear.placement_id)
        .ok_or("Body native node was not lowered")?;
    Ok(Some(NativeSourceRoute {
        source,
        budget,
        image_backed,
    }))
}
impl BodyKernel<'_> {
    pub(super) fn begin_native_source(&mut self, request: HostCallRequest) -> Result<bool, String> {
        let Some(native) = self
            .native_source
            .as_mut()
            .filter(|s| s.matches_call(request.node, request.call))
        else {
            return Ok(false);
        };
        let maximum = self
            .operations
            .iter()
            .find(|op| op.node == request.node && op.call == request.call)
            .ok_or_else(|| {
                diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("Body native request has no exact Host Call"),
                )
            })?
            .binding
            .maximum_output_bytes;
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|e| {
                diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("Body native input: {e:?}"),
                )
            })?;
        let bytes = native
            .source
            .read_immediate(request, input)
            .map_err(|error| {
                diagnostic::error(&mut self.error_diagnostic, format_args!("{error}"))
            })?;
        let value = self.scheduler.store_host_value(bytes).map_err(|e| {
            diagnostic::error(
                &mut self.error_diagnostic,
                format_args!("Body native output: {e:?}"),
            )
        })?;
        self.scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(BoundedValueRef::new(value, maximum).map_err(|e| {
                        diagnostic::error(
                            &mut self.error_diagnostic,
                            format_args!("Body native bound: {e:?}"),
                        )
                    })?),
                    failure: None,
                },
            )
            .map_err(|e| {
                diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("Body native completion: {e:?}"),
                )
            })?;
        Ok(true)
    }
    pub(super) fn cancel_native_source(
        &mut self,
        cancellation: conduit_kernel::scheduler::HostCallCancellation,
    ) -> Result<bool, String> {
        let Some(native) = self
            .native_source
            .as_mut()
            .filter(|s| s.matches_call(cancellation.node, cancellation.call))
        else {
            return Ok(false);
        };
        native.source.cancel();
        self.scheduler
            .complete_host_call(
                cancellation.node,
                cancellation.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Cancelled,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|e| {
                diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("Body native cancellation: {e:?}"),
                )
            })?;
        Ok(true)
    }
}
