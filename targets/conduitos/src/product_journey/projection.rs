use super::*;

impl ProductJourney {
    pub fn projection(&self) -> JourneyProjection {
        let wake_sign_id = self.current_wake().and_then(|wake| {
            wake.events.iter().find_map(|event| match event {
                WakeLifecycleEvent::Woke { sign_id } => Some(sign_id.clone()),
                _ => None,
            })
        });
        let plan_sign_id = self.current_wake().and_then(|wake| {
            wake.events.iter().rev().find_map(|event| match event {
                WakeLifecycleEvent::PlanReady { plan_id, sign_id }
                    if Some(plan_id) == self.current_plan().map(|plan| &plan.plan_id) =>
                {
                    Some(sign_id.clone())
                }
                WakeLifecycleEvent::Replanned {
                    replacement_plan_id,
                    sign_id,
                    ..
                } if Some(replacement_plan_id) == self.current_plan().map(|plan| &plan.plan_id) => {
                    Some(sign_id.clone())
                }
                _ => None,
            })
        });
        let play_sign_id = self.current_wake().and_then(|wake| {
            wake.events.iter().rev().find_map(|event| match event {
                WakeLifecycleEvent::PlayStarted {
                    active_play_id,
                    sign_id,
                    ..
                } if Some(active_play_id)
                    == self.current_play().map(|play| &play.active_play_id) =>
                {
                    Some(sign_id.clone())
                }
                _ => None,
            })
        });
        JourneyProjection {
            status: self.status,
            revision: self.revision,
            source_document_id: self
                .plot
                .as_ref()
                .map(|plot| plot.source_document_id.clone()),
            checked_plot_id: self.plot.as_ref().map(|plot| plot.checked_plot_id.clone()),
            expanded_plot_id: self.plot.as_ref().map(|plot| plot.expanded_plot_id.clone()),
            host_id: self.host_id.clone(),
            boot_id: self.boot_id.clone(),
            offer_generation: self.offer_generation,
            body_id: self.body().map(|body| body.body_id.clone()),
            born_sign_id: self.body().and_then(|body| {
                body.events.iter().find_map(|event| match event {
                    conduit_body::BodyLifecycleEvent::Born { sign_id, .. } => Some(sign_id.clone()),
                    _ => None,
                })
            }),
            fulfilled_sign_id: self.body().and_then(|body| match &body.state {
                BodyState::Fulfilled { sign_id } => Some(sign_id.clone()),
                _ => None,
            }),
            workload_revision: self.body().map(|body| body.workload_revision),
            workload_sign_id: self.body().and_then(|body| {
                body.events.iter().rev().find_map(|event| match event {
                    conduit_body::BodyLifecycleEvent::PlotAdmitted { sign_id, .. } => {
                        Some(sign_id.clone())
                    }
                    _ => None,
                })
            }),
            lull_sign_id: self.body().and_then(|body| {
                body.events.iter().rev().find_map(|event| match event {
                    conduit_body::BodyLifecycleEvent::LullRetained { sign_id, .. } => {
                        Some(sign_id.clone())
                    }
                    _ => None,
                })
            }),
            workload_capacity_available: self
                .body()
                .is_some_and(|body| body.workset.len() < native_workset::NATIVE_PLOT_CAPACITY),
            friendly_name: self
                .biography()
                .map(|evidence| evidence.friendly_name.clone()),
            part_id: self.biography().and_then(|evidence| {
                evidence
                    .membership
                    .parts
                    .iter()
                    .find(|part| {
                        part.current.as_ref().is_some_and(|current| {
                            current.host_id == self.host_id && current.boot_id == self.boot_id
                        })
                    })
                    .map(|part| part.part_id.clone())
            }),
            wake_id: self.current_wake().map(|wake| wake.wake_id.clone()),
            wake_sign_id,
            plan_id: self.current_plan().map(|plan| plan.plan_id.clone()),
            plan_sign_id,
            active_play_id: self.current_play().map(|play| play.active_play_id.clone()),
            play_sign_id,
            gear_ids: self
                .current_plan()
                .into_iter()
                .flat_map(|plan| &plan.plots)
                .flat_map(|plot| &plot.plan.fragments)
                .flat_map(|fragment| &fragment.placements)
                .map(|placement| placement.gear_id.as_str().to_owned())
                .collect(),
            port_ids: self
                .current_plan()
                .into_iter()
                .flat_map(|plan| &plan.plots)
                .flat_map(|plot| &plot.plan.fragments)
                .flat_map(|fragment| &fragment.connections)
                .flat_map(|connection| {
                    [
                        format!(
                            "{}.{}",
                            connection.source_placement_id.as_str(),
                            connection.source_port_id.as_str()
                        ),
                        format!(
                            "{}.{}",
                            connection.sink_placement_id.as_str(),
                            connection.sink_port_id.as_str()
                        ),
                    ]
                })
                .collect(),
            cord_ids: self
                .current_plan()
                .into_iter()
                .flat_map(|plan| &plan.plots)
                .flat_map(|plot| &plot.plan.fragments)
                .flat_map(|fragment| &fragment.connections)
                .map(|connection| connection.connection_id.as_str().to_owned())
                .collect(),
            input_sign_id: self.input_sign_id.clone(),
            loss_kind: self.loss_kind,
            loss_sign_id: self.loss_sign_id.clone(),
            result_sign_id: self.results[self.foreground_index()].sign.clone(),
            result: self.foreground_result().map(|text| text.into()),
            result_omitted_bytes: self.results[self.foreground_index()]
                .omitted_bytes(self.plots[self.foreground_index()]),
            input_count: self.input_count,
            kernel_sign_gap: self
                .kernel
                .as_ref()
                .and_then(|kernel| kernel.sign_retention_gap())
                .or(self.retained_kernel_sign_gap),
            last_request_id: self.last_request_id.clone(),
            mask: self
                .mask_control
                .as_ref()
                .filter(|_| self.current_play().is_some())
                .and_then(|control| control.mask_evidence().cloned()),
        }
    }
}
