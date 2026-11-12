//! Closed cooperative tick, one-shot Source, finite projection and Crèche receipts. Shared immutable Body/workset inputs
//! retain their existing owner; this accounts newly owned execution backing.
use super::*;

use crate::kernel_preparation::BodyMemoryReceipt;
mod lifecycle;
pub(crate) use lifecycle::{lifecycle_bytes, result_bytes};

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
        return Err("local IMAGE memory does not support external Fore/Line maps".into());
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
        if binding.compute.is_some() || binding.protected.is_some() {
            return Err("local IMAGE memory does not support external resource payloads".into());
        }
        if let Some(content) = &binding.content {
            for text in [
                &content.contract.content_profile.0,
                &content.owner_host.0,
                &content.owner_boot.0,
                &content.base_id.0,
                &content.residence_profile.0,
            ] {
                add(&mut bytes, text.capacity())?;
            }
        }
        add(&mut bytes, binding.pool_id.0.capacity())?;
        add(&mut bytes, binding.class_id.0.capacity())?;
    }
    for (_, _, id) in &map.resource_connections {
        add(&mut bytes, id.0.capacity())?;
    }
    Ok(bytes)
}
fn back_bytes(driver: &InstalledBack) -> Result<usize, String> {
    match driver {
        InstalledBack::CombineLatest(back) => back
            .retained_backing_bytes()
            .ok_or_else(|| "latest-value backing overflow".into()),
        InstalledBack::MeasurementChartConstructor(back) => back
            .retained_backing_bytes()
            .checked_add(back.workspace_bytes() as usize)
            .ok_or_else(|| "chart constructor backing overflow".into()),
        InstalledBack::Tick(tick) => tick
            .retained_backing_bytes()
            .ok_or_else(|| "tick backing overflow".into()),
        InstalledBack::MeasurementPlot(back) => back
            .retained_backing_bytes()
            .checked_add(crate::hosted_measurement::measurement_plot_workspace_bytes() as usize)
            .ok_or_else(|| "measurement projection prepared workspace overflow".into()),
        InstalledBack::MeasurementChartFace(back) => Ok(back.retained_backing_bytes()),
        InstalledBack::MeasurementChartFlowFace(back) => Ok(back.retained_backing_bytes()),
        InstalledBack::Creche(back) => Ok(back.retained_backing_bytes()),
        // These Backs retain only inline state; the complete receipt separately
        // requires and counts the actual selected native/file route owner.
        InstalledBack::Inactive
        | InstalledBack::SelectedNativeSource(_)
        | InstalledBack::DataSaveText(_)
        | InstalledBack::DataLoadText(_) => Ok(0),
        _ => Err("IMAGE memory receipt lacks a complete owned Back accounting".into()),
    }
}
impl BodyKernel<'_> {
    pub(crate) fn attach_image_memory(
        &mut self,
        lease: &crate::kernel_preparation::ImageMemoryLease,
    ) {
        self.fore.attach_image_memory(lease);
        if let Some(route) = &mut self.chart_face {
            route.attach_image_memory(lease);
        }
    }
    pub(crate) fn escaped_face_backing_bytes(&self) -> Result<usize, String> {
        self.chart_face
            .as_ref()
            .map_or(Ok(0), |route| route.escaped_backing_bytes())
    }
    pub(crate) fn prepare_memory<T: TimerAdapter>(
        &mut self,
        timer: &mut T,
        host: &conduit_core::HostId,
        boot: &conduit_core::BootId,
        lifecycle: usize,
    ) -> Result<BodyMemoryReceipt, String> {
        if self.input_origin.is_some()
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
            return Err(
                "IMAGE memory receipt does not support this provider/input/activation profile"
                    .into(),
            );
        }
        let mut bytes = core::mem::size_of::<InstalledScheduler>();
        add(
            &mut bytes,
            crate::kernel_preparation::ImageMemoryLease::owned_backing_bytes(),
        )?;
        let mut ticks = 0;
        let mut measurements = 0;
        let mut sources = 0;
        let mut creches = 0;
        let mut chart_faces = 0;
        let mut files = 0;
        for (index, driver) in self.scheduler.drivers().iter().enumerate() {
            if matches!(driver, InstalledBack::SelectedNativeSource(_)) {
                let route = self
                    .native_source
                    .as_ref()
                    .filter(|route| usize::from(route.node().0) == index)
                    .ok_or("IMAGE Source driver has no exact retained owner")?;
                add(
                    &mut bytes,
                    route
                        .image_backing_bytes()
                        .ok_or("IMAGE Source lacks independently charged provider backing")?,
                )?;
                sources += 1;
            }
            if matches!(driver, InstalledBack::Creche(_)) {
                let route = self
                    .creche
                    .as_ref()
                    .ok_or("IMAGE Crèche Back lacks its exact prepared route")?;
                add(
                    &mut bytes,
                    route
                        .prepared_backing_bytes(index)
                        .ok_or("IMAGE Crèche route is dispatched or names another Back")?,
                )?;
                creches += 1;
            }
            if matches!(
                driver,
                InstalledBack::MeasurementChartFace(_) | InstalledBack::MeasurementChartFlowFace(_)
            ) {
                chart_faces += 1;
            }
            if matches!(driver, InstalledBack::MeasurementPlot(_)) {
                measurements += 1;
            }
            if matches!(driver, InstalledBack::Tick(_)) {
                ticks += 1;
            }
            if matches!(
                driver,
                InstalledBack::DataSaveText(_) | InstalledBack::DataLoadText(_)
            ) {
                let node = conduit_kernel::NodeId(
                    index.try_into().map_err(|_| "file node index overflow")?,
                );
                let route = self
                    .data_text_hosts
                    .iter()
                    .find(|route| route.file_node() == Some(node))
                    .ok_or("IMAGE data Back lacks its exact selected file owner")?;
                route
                    .image_backing_bytes()
                    .ok_or("IMAGE file owner lacks original funded backing")?;
                files += 1;
            }
            add(&mut bytes, back_bytes(driver)?)?;
        }
        if chart_faces > 1
            || (self.chart_face.is_some() != (chart_faces == 1))
            || (chart_faces == 1 && (creches != 0 || sources != 0 || ticks != 0))
            || sources > 1
            || creches > 1
            || files > 1
            || (files == 1
                && (ticks != 0
                    || sources != 0
                    || creches != 0
                    || measurements != 0
                    || chart_faces != 0))
            || (creches == 1 && (ticks != 0 || sources != 0))
            || measurements > 1
            || (measurements == 1 && (ticks != 0 || sources != 0 || creches != 0))
            || (ticks == 0
                && sources == 0
                && creches == 0
                && measurements == 0
                && chart_faces == 0
                && files == 0)
            || (self.creche.is_some() != (creches == 1))
            || (self.native_source.is_some() != (sources == 1))
        {
            return Err(
                "IMAGE memory receipt requires exact tick, one-shot Source, single projection, chart, Crèche or funded file ownership".into(),
            );
        }
        add(
            &mut bytes,
            if sources == 1 || creches == 1 || measurements == 1 || chart_faces == 1 || files == 1 {
                self.fore
                    .finite_one_shot_backing_bytes()
                    .or_else(|| self.fore.image_live_backing_bytes())
            } else {
                self.fore.tick_backing_bytes()
            }
            .ok_or("IMAGE memory receipt does not support this Fore/input/live contract")?,
        )?;
        if let Some(route) = &self.chart_face {
            add(&mut bytes, route.additional_backing_bytes()?)?;
            add(&mut bytes, route.escaped_backing_bytes()?)?;
        }
        for host in &self.data_text_hosts {
            add(
                &mut bytes,
                host.image_backing_bytes()
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
        Ok(BodyMemoryReceipt::prepared_local(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creche_back_counts_only_its_own_inline_state_not_worker_or_receipts() {
        use conduit_body_make::deployment_runtime::{CrecheBack, CrecheRequest};
        let deferred =
            InstalledBack::Creche(CrecheBack::for_admitted_host(conduit_kernel::HostCallId(0)));
        let pinned = InstalledBack::Creche(CrecheBack::new(
            conduit_kernel::HostCallId(0),
            CrecheRequest([7; 32]),
        ));
        assert_eq!(back_bytes(&deferred).unwrap(), 0);
        assert_eq!(back_bytes(&pinned).unwrap(), 0);
        // The complete kernel receipt still rejects its separate Crèche owner.
        // A zero Back charge must never be treated as an entire Play receipt.
    }
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

#[cfg(test)]
#[path = "memory/creche_tests.rs"]
mod creche_tests;
