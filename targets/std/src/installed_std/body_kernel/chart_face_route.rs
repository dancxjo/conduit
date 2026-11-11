//! One admitted semantic Face stream, not another UI service or renderer.
use super::*;
use crate::body_execution::{
    face_contribution::{
        FaceContributionContext, FaceContributionRefusal, PendingFaceContributionRoute,
    },
    BodyFaceContributionObserver,
};
use crate::measurement_chart_offer::MEASUREMENT_CHART_FACE_HOST_CALL;
use conduit_body::{BodyPlan, BodyPlayIdentity, Wake};
use conduit_core::HostAdvertisement;

pub(super) struct ChartFaceRoute {
    node: conduit_kernel::NodeId,
    call: conduit_kernel::HostCallId,
    title: String,
    maximum_updates: u32,
    route: PendingFaceContributionRoute,
    waiting: Option<HostCallRequest>,
}
impl ChartFaceRoute {
    pub(super) fn additional_backing_bytes(&self) -> Result<usize, String> {
        self.route
            .additional_backing_bytes()
            .and_then(|bytes| bytes.checked_add(self.title.capacity()))
            .and_then(|bytes| {
                bytes.checked_add(
                    super::super::measurement_chart_face_back::workspace_bytes() as usize
                )
            })
            .ok_or_else(|| "chart Face route backing overflow".into())
    }
    pub(super) fn escaped_backing_bytes(&self) -> Result<usize, String> {
        let per_frame = super::super::measurement_chart_face_back::maximum_fragment_backing_bytes()
            .checked_add(core::mem::size_of::<
                crate::body_execution::CommittedFaceContribution,
            >())
            .ok_or("chart Face escaped frame backing overflow")?;
        per_frame
            .checked_mul(self.maximum_updates as usize)
            .and_then(|bytes| bytes.checked_add(self.route.additional_backing_bytes()?))
            .ok_or_else(|| "chart Face finite escaped result backing overflow".into())
    }
    pub(super) fn attach_image_memory(
        &mut self,
        lease: &crate::kernel_preparation::ImageMemoryLease,
    ) {
        self.route.attach_image_memory(lease);
    }
}
impl BodyKernel<'_> {
    pub(crate) fn attach_chart_face(
        &mut self,
        wake: &Wake,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
        host: &HostAdvertisement,
        observer: bool,
    ) -> Result<(), String> {
        let mut routes = Vec::with_capacity(1);
        for (index, driver) in self.scheduler.drivers().iter().enumerate() {
            let InstalledBack::MeasurementChartFace(back) = driver else {
                continue;
            };
            if !observer || self.chart_face.is_some() || !routes.is_empty() {
                return Err("chart Face requires one exact prepared observer route".into());
            }
            let node = conduit_kernel::NodeId(index as u16);
            let partition = self
                .partitions
                .iter()
                .find(|partition| partition.placement_for_node(node).is_some())
                .ok_or("chart Face node lacks its actual partition")?;
            let body_plot = plan
                .plots
                .iter()
                .find(|p| p.plan.plan_id == partition.plan_id)
                .ok_or("chart Face partition lacks its resident Plot")?;
            let placement_id = partition
                .placement_for_node(node)
                .ok_or("chart Face placement missing")?;
            let placement = body_plot
                .plan
                .fragments
                .iter()
                .flat_map(|f| &f.placements)
                .find(|p| &p.placement_id == placement_id)
                .ok_or("chart Face Source placement missing")?;
            let (title, maximum_updates) =
                super::super::measurement_chart_face_back::configuration(placement)?;
            let operation = self
                .operations
                .iter()
                .find(|op| {
                    op.node == node && op.contract_id.as_str() == MEASUREMENT_CHART_FACE_HOST_CALL
                })
                .ok_or("chart Face has no exact Host Call")?;
            if operation.call != conduit_kernel::HostCallId(0)
                || operation.binding.maximum_output_bytes != 0
            {
                return Err("chart Face Host Call shape differs".into());
            }
            let route = PendingFaceContributionRoute::prepare(
                FaceContributionContext {
                    wake,
                    plan,
                    play,
                    host,
                    partition,
                    node,
                    call: operation.call,
                    contract: &operation.contract_id,
                },
                back.observation(),
            )
            .map_err(|e| format!("chart Face scope: {e:?}"))?;
            routes.push(ChartFaceRoute {
                node,
                call: operation.call,
                title: title.to_owned(),
                maximum_updates,
                route,
                waiting: None,
            });
        }
        self.chart_face = routes.pop();
        Ok(())
    }
    pub(super) fn chart_face_call(&mut self, request: HostCallRequest) -> Result<bool, String> {
        let Some(route) = self
            .chart_face
            .as_mut()
            .filter(|route| route.node == request.node && route.call == request.call)
        else {
            return Ok(false);
        };
        if route.waiting.is_some() {
            return Err("chart Face already retains a dispatched request".into());
        }
        route.waiting = Some(request);
        self.prepare_chart_face_call()?;
        Ok(true)
    }
    fn prepare_chart_face_call(&mut self) -> Result<(), String> {
        let Some(route) = self.chart_face.as_mut() else {
            return Ok(());
        };
        if route.route.is_pending() {
            return Ok(());
        }
        let Some(request) = route.waiting else {
            return Ok(());
        };
        if !self.scheduler.host_call_is_pending(request) {
            return Err("chart Face waiting request lost exact kernel custody".into());
        }
        let canonical = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|e| format!("chart Face input custody: {e:?}"))?;
        let value = conduit_core::validate_canonical_structured_value(canonical)
            .map_err(|e| format!("chart Face canonical input: {e:?}"))?;
        let raw = value
            .primitive_bytes(conduit_data::MEASUREMENT_CHART_INFO_ID)
            .map_err(|e| format!("chart Face intrinsic payload: {e:?}"))?;
        let chart = conduit_data::decode_measurement_chart(raw)
            .map_err(|e| format!("chart Face decode: {e:?}"))?;
        let series = chart
            .series()
            .iter()
            .map(|item| crate::measurement_face::MeasurementFaceSeries {
                label: item.label(),
                series: item.series(),
            })
            .collect::<Vec<_>>();
        let fragment = crate::measurement_face::measurement_plot_fragment(
            "measurement-chart",
            &route.title,
            &series,
            route.route.basis(),
        )
        .map_err(|e| format!("chart Face semantic fragment: {e:?}"))?;
        route
            .route
            .stage(self.scheduler.as_ref(), request, fragment)
            .map_err(|(_, e)| format!("chart Face pending contribution: {e:?}"))?;
        self.scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|e| format!("chart Face Host completion: {e:?}"))?;
        route.waiting = None;
        Ok(())
    }
    pub(super) fn poll_chart_face(
        &mut self,
        observer: &mut Option<&mut dyn BodyFaceContributionObserver>,
    ) -> Result<(), String> {
        let Some(route) = self.chart_face.as_mut() else {
            return Ok(());
        };
        let observer = observer
            .as_deref_mut()
            .ok_or("chart Face observer detached")?;
        match route.route.publish(observer) {
            Ok(_) | Err(FaceContributionRefusal::Pressure) => {}
            Err(error) => return Err(format!("chart Face contribution refused: {error:?}")),
        }
        self.prepare_chart_face_call()
    }
    pub(super) fn chart_face_pending(&self) -> bool {
        self.chart_face
            .as_ref()
            .is_some_and(|route| route.route.is_pending() || route.waiting.is_some())
    }
}
