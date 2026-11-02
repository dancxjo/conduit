//! Closed cooperative tick-only receipt. Shared immutable Body/workset inputs
//! retain their existing owner; this accounts newly owned execution backing.
use super::*;
use crate::body_execution::BodyRunReport;
use conduit_body::{BodyPlayIdentity, Wake, WakeLifecycleEvent};
use conduit_core::SignIdentity;

use crate::kernel_preparation::BodyMemoryReceipt;

fn add(total: &mut usize, value: usize) -> Result<(), String> {
    *total = total
        .checked_add(value)
        .ok_or("Body managed backing overflow")?;
    Ok(())
}
fn vector<T>(values: &Vec<T>) -> Result<usize, String> {
    values
        .capacity()
        .checked_mul(core::mem::size_of::<T>())
        .ok_or_else(|| "Body vector backing overflow".into())
}
fn identities(map: &KernelIdentityMap) -> Result<usize, String> {
    if !map.remote_endpoints.is_empty() {
        return Err("IMAGE tick memory does not support external Fore/Line maps".into());
    }
    let mut bytes = map.plan_id.0.capacity();
    add(&mut bytes, map.fragment_id.0.capacity())?;
    for count in [
        vector(&map.placements)?,
        vector(&map.ports)?,
        vector(&map.connections)?,
        vector(&map.remote_endpoints)?,
        vector(&map.fore_endpoints)?,
        vector(&map.host_calls)?,
        vector(&map.resources)?,
        vector(&map.resource_connections)?,
    ] {
        add(&mut bytes, count)?;
    }
    for endpoint in &map.fore_endpoints {
        add(&mut bytes, endpoint.front_port_id.0.capacity())?;
        add(&mut bytes, endpoint.value_kind.0.capacity())?;
    }
    for (_, id) in &map.placements {
        add(&mut bytes, id.0.capacity())?;
    }
    for port in &map.ports {
        add(&mut bytes, port.port_id.0.capacity())?;
    }
    for (_, id) in &map.connections {
        add(&mut bytes, id.0.capacity())?;
    }
    for (_, _, id) in &map.host_calls {
        add(&mut bytes, id.0.capacity())?;
    }
    for (_, _, binding) in &map.resources {
        if binding.compute.is_some() || binding.content.is_some() || binding.protected.is_some() {
            return Err("IMAGE tick memory does not support external resource payloads".into());
        }
        add(&mut bytes, binding.pool_id.0.capacity())?;
        add(&mut bytes, binding.class_id.0.capacity())?;
    }
    for (_, _, id) in &map.resource_connections {
        add(&mut bytes, id.0.capacity())?;
    }
    Ok(bytes)
}
pub(crate) fn lifecycle_bytes(
    play: &BodyPlayIdentity,
    wake: &Wake,
    sign: &SignIdentity,
) -> Result<usize, String> {
    let mut bytes = wake
        .workset
        .retained_backing_bytes()
        .ok_or("Body immutable workset backing overflow")?;
    for count in [
        play.active_play_id.0.capacity(),
        play.body_id.retained_backing_bytes(),
        play.wake_id.retained_backing_bytes(),
        play.plan_id.0.capacity(),
        wake.wake_id.retained_backing_bytes(),
        wake.body_id.retained_backing_bytes(),
        sign.sign_id.0.capacity(),
        sign.host_id.0.capacity(),
        sign.boot_id.0.capacity(),
        sign.active_play_id.as_ref().map_or(0, |id| id.0.capacity()),
        vector(&wake.plans)?,
        vector(&wake.rejections)?,
        vector(&wake.sign_ids)?,
        vector(&wake.events)?,
    ] {
        add(&mut bytes, count)?;
    }
    for plan in &wake.plans {
        if plan.hold.is_some() {
            return Err("IMAGE tick result does not support held-plan metadata".into());
        }
        add(&mut bytes, plan.plan_id.0.capacity())?;
        add(
            &mut bytes,
            plan.active_play_id.as_ref().map_or(0, |id| id.0.capacity()),
        )?;
    }
    for id in &wake.sign_ids {
        add(&mut bytes, id.0.capacity())?;
    }
    for rejection in &wake.rejections {
        for count in [
            rejection.reason_code.capacity(),
            rejection.category.capacity(),
            rejection.stage.capacity(),
            rejection.resource.capacity(),
            rejection.host_id.0.capacity(),
            rejection.boot_id.0.capacity(),
            rejection.plan_id.as_ref().map_or(0, |id| id.0.capacity()),
            vector(&rejection.checked_plot_ids)?,
        ] {
            add(&mut bytes, count)?;
        }
        for id in &rejection.checked_plot_ids {
            add(&mut bytes, id.0.capacity())?;
        }
    }
    for event in &wake.events {
        add(&mut bytes, event.sign_id().0.capacity())?;
        match event {
            WakeLifecycleEvent::Woke { .. }
            | WakeLifecycleEvent::Lulled { .. }
            | WakeLifecycleEvent::Failed { .. } => {}
            WakeLifecycleEvent::PlanReady { plan_id, .. }
            | WakeLifecycleEvent::BecameUnsatisfied { plan_id, .. }
            | WakeLifecycleEvent::SamePlanObserved { plan_id, .. } => {
                add(&mut bytes, plan_id.0.capacity())?
            }
            WakeLifecycleEvent::PlayStarted {
                plan_id,
                active_play_id,
                ..
            }
            | WakeLifecycleEvent::HeldPlanReleased {
                plan_id,
                active_play_id,
                ..
            } => {
                add(&mut bytes, plan_id.0.capacity())?;
                add(&mut bytes, active_play_id.0.capacity())?;
            }
            WakeLifecycleEvent::PlanHeld {
                prior_plan_id,
                plan_id,
                basis_sign_ids,
                ..
            } => {
                add(
                    &mut bytes,
                    prior_plan_id.as_ref().map_or(0, |id| id.0.capacity()),
                )?;
                add(&mut bytes, plan_id.0.capacity())?;
                add(&mut bytes, vector(basis_sign_ids)?)?;
                for id in basis_sign_ids {
                    add(&mut bytes, id.0.capacity())?;
                }
            }
            WakeLifecycleEvent::HeldPlanInvalidated {
                plan_id,
                current_basis_sign_ids,
                ..
            } => {
                add(&mut bytes, plan_id.0.capacity())?;
                add(&mut bytes, vector(current_basis_sign_ids)?)?;
                for id in current_basis_sign_ids {
                    add(&mut bytes, id.0.capacity())?;
                }
            }
            WakeLifecycleEvent::WorkloadChanged {
                prior_plan_id,
                prior_workset,
                replacement_workset,
                ..
            } => {
                add(&mut bytes, prior_plan_id.0.capacity())?;
                add(
                    &mut bytes,
                    prior_workset
                        .retained_backing_bytes()
                        .ok_or("prior workset backing overflow")?,
                )?;
                add(
                    &mut bytes,
                    replacement_workset
                        .retained_backing_bytes()
                        .ok_or("replacement workset backing overflow")?,
                )?;
            }
            WakeLifecycleEvent::Replanned {
                prior_plan_id,
                replacement_plan_id,
                ..
            } => {
                add(&mut bytes, prior_plan_id.0.capacity())?;
                add(&mut bytes, replacement_plan_id.0.capacity())?;
            }
        }
    }
    Ok(bytes)
}
fn back_bytes(driver: &InstalledBack) -> Result<usize, String> {
    match driver {
        InstalledBack::Tick(tick) => tick
            .retained_backing_bytes()
            .ok_or_else(|| "tick backing overflow".into()),
        InstalledBack::Inactive => Ok(0),
        _ => Err("IMAGE memory receipt lacks a complete owned Back accounting".into()),
    }
}
impl BodyKernel<'_> {
    pub(crate) fn attach_image_memory(
        &mut self,
        lease: &crate::kernel_preparation::ImageMemoryLease,
    ) {
        self.fore.attach_image_memory(lease);
    }
    pub(crate) fn prepare_memory<T: TimerAdapter>(
        &mut self,
        timer: &mut T,
        host: &conduit_core::HostId,
        boot: &conduit_core::BootId,
        lifecycle: usize,
    ) -> Result<BodyMemoryReceipt, String> {
        if self.creche.is_some()
            || self.input_origin.is_some()
            || self.parsec_map.is_some()
            || self.parsec_terminal.is_some()
            || self.todo_checkpoint.is_some()
            || self.todo_checkpoint_read.is_some()
            || self.typed_record_hosts.iter().any(Option::is_some)
            || self.image_text_hosts.iter().any(Option::is_some)
            || self.image_text_record_hosts.iter().any(Option::is_some)
            || self.text_state_hosts.iter().any(Option::is_some)
            || self.input_keymaps.iter().any(Option::is_some)
            || self.face_labels.iter().any(Option::is_some)
        {
            return Err("IMAGE memory receipt supports direct tick only; provider/input/activation cases are unsupported".into());
        }
        let mut bytes = core::mem::size_of::<InstalledScheduler>();
        add(
            &mut bytes,
            crate::kernel_preparation::ImageMemoryLease::owned_backing_bytes(),
        )?;
        let mut ticks = 0;
        for driver in self.scheduler.drivers() {
            if matches!(driver, InstalledBack::Tick(_)) {
                ticks += 1;
            }
            add(&mut bytes, back_bytes(driver)?)?;
        }
        if ticks == 0 {
            return Err("IMAGE memory receipt requires actual tick Back".into());
        }
        add(
            &mut bytes,
            self.fore
                .tick_backing_bytes()
                .ok_or("IMAGE memory receipt does not support this Fore/input/live contract")?,
        )?;
        for host in &self.data_text_hosts {
            add(
                &mut bytes,
                host.empty_backing_bytes()
                    .ok_or("IMAGE memory receipt does not support data residence")?,
            )?;
        }
        for activation in &self.activations {
            if !activation.entries.is_empty() {
                return Err("IMAGE memory receipt does not support activation ownership".into());
            }
            add(&mut bytes, activation.source_plan_id.0.capacity())?;
            add(&mut bytes, activation.fragment_id.0.capacity())?;
            add(&mut bytes, vector(&activation.entries)?)?;
        }
        for map in &self.partitions {
            add(&mut bytes, identities(map)?)?;
        }
        for operation in &self.operations {
            add(&mut bytes, operation.contract_id.0.capacity())?;
            add(
                &mut bytes,
                operation
                    .target_kind
                    .as_ref()
                    .map_or(0, |id| id.0.capacity()),
            )?;
        }
        for count in [
            vector(&self.activations)?,
            vector(&self.partitions)?,
            vector(&self.operations)?,
            vector(&self.face_labels)?,
            vector(&self.typed_record_hosts)?,
            vector(&self.image_text_hosts)?,
            vector(&self.image_text_record_hosts)?,
            vector(&self.text_state_hosts)?,
            vector(&self.data_text_hosts)?,
            vector(&self.requests)?,
            self.scheduler.values().reserved_storage_bytes(),
            self.scheduler.signs().reserved_storage_bytes(),
            lifecycle,
        ] {
            add(&mut bytes, count)?;
        }
        self.error_diagnostic = Some(diagnostic::Diagnostic::prepare());
        self.cleanup_diagnostic = Some(diagnostic::Diagnostic::prepare());
        add(
            &mut bytes,
            self.error_diagnostic.as_ref().unwrap().backing_bytes(),
        )?;
        add(
            &mut bytes,
            self.cleanup_diagnostic.as_ref().unwrap().backing_bytes(),
        )?;
        let events = self.clock_observations.event_capacity();
        self.terminal_events = Some(Vec::with_capacity(events));
        self.clock_observations.prepare_bounded(timer, host, boot)?;
        add(&mut bytes, vector(self.terminal_events.as_ref().unwrap())?)?;
        add(
            &mut bytes,
            self.clock_observations
                .retained_backing_bytes()
                .ok_or("prepared clock backing overflow")?,
        )?;
        Ok(BodyMemoryReceipt::prepared_tick(bytes))
    }
}

pub(crate) fn result_bytes(report: &BodyRunReport) -> Result<usize, String> {
    let mut bytes = lifecycle_bytes(&report.play, &report.wake_at_start, &report.terminal_sign)?;
    for map in &report.partitions {
        add(&mut bytes, identities(map)?)?;
    }
    add(
        &mut bytes,
        crate::kernel_preparation::ImageMemoryLease::owned_backing_bytes(),
    )?;
    add(&mut bytes, vector(&report.fore_deliveries)?)?;
    for delivery in &report.fore_deliveries {
        add(
            &mut bytes,
            delivery
                .retained_backing_bytes()
                .ok_or("Fore result backing overflow")?,
        )?;
    }
    if let Some(pending) = &report.fore_pending_output {
        add(
            &mut bytes,
            pending
                .delivery
                .retained_backing_bytes()
                .ok_or("pending Fore backing overflow")?,
        )?;
    }
    for count in [
        vector(&report.partitions)?,
        vector(&report.requests)?,
        vector(&report.kernel_events)?,
        vector(&report.clock_observations)?,
        report.failure.as_ref().map_or(0, String::capacity),
        report.cleanup_failure.as_ref().map_or(0, String::capacity),
    ] {
        add(&mut bytes, count)?;
    }
    for observation in &report.clock_observations {
        add(
            &mut bytes,
            observation
                .time
                .local()
                .clock()
                .retained_backing_bytes()
                .ok_or("result clock backing overflow")?,
        )?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_real_deadline_back_is_not_misreported_as_zero_bytes() {
        let value = conduit_kernel::ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 8,
        };
        let back = InstalledBack::TimeDeadline(conduit_time::CancellationDeadlineBack::prepare(
            value, value,
        ));
        assert!(back_bytes(&back).is_err());
        assert_eq!(back_bytes(&InstalledBack::Inactive).unwrap(), 0);
    }
}
