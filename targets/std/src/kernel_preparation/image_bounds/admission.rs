//! Selected IMAGE admission limits, distinct from Linux allocator/RSS limits.
use super::*;
use conduit_host_make::HostBounds;

#[derive(Default)]
pub(super) struct Shape {
    pub(super) instances: u64,
    pub(super) operations: u64,
    pub(super) queue_items: u64,
    pub(super) buffered_bytes: u64,
    pub(super) timer_slots: u64,
    pub(super) line_sessions: u64,
    pub(super) evidence_items: u64,
}
fn add(total: &mut u64, value: u64) -> Result<(), String> {
    *total = total
        .checked_add(value)
        .ok_or("selected IMAGE workset shape overflow")?;
    Ok(())
}
impl Shape {
    fn include(&mut self, fragment: &PlanFragment) -> Result<(), String> {
        add(&mut self.instances, fragment.placements.len() as u64)?;
        add(
            &mut self.evidence_items,
            u64::from(fragment.sign_storage_budget.item_capacity),
        )?;
        for placement in &fragment.placements {
            add(&mut self.operations, placement.host_calls.len() as u64)?;
            for resource in &placement.resources {
                if matches!(
                    resource.class_id.as_str(),
                    conduit_core::TIMER_RESOURCE_CLASS
                        | conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS
                ) {
                    add(&mut self.timer_slots, u64::from(resource.units))?;
                }
            }
        }
        for connection in &fragment.connections {
            add(&mut self.queue_items, u64::from(connection.item_capacity))?;
            add(
                &mut self.buffered_bytes,
                u64::from(connection.byte_capacity),
            )?;
            add(
                &mut self.line_sessions,
                u64::from(connection.selected_line.is_some()),
            )?;
        }
        for port in &fragment.fore_ports {
            add(&mut self.queue_items, u64::from(port.item_capacity))?;
            add(&mut self.buffered_bytes, u64::from(port.byte_capacity))?;
            add(
                &mut self.line_sessions,
                u64::from(port.selected_line.is_some()),
            )?;
        }
        Ok(())
    }
    pub(super) fn check(&self, bounds: &HostBounds) -> Result<(), String> {
        for (field, required, available) in [
            (
                "active_instances",
                self.instances,
                u64::from(bounds.active_instances),
            ),
            (
                "operation_slots",
                self.operations,
                u64::from(bounds.operation_slots),
            ),
            (
                "queue_items",
                self.queue_items,
                u64::from(bounds.queue_items),
            ),
            ("buffered_bytes", self.buffered_bytes, bounds.buffered_bytes),
            (
                "timer_slots",
                self.timer_slots,
                u64::from(bounds.timer_slots),
            ),
            (
                "line_sessions",
                self.line_sessions,
                u64::from(bounds.line_sessions),
            ),
            (
                "evidence_items",
                self.evidence_items,
                u64::from(bounds.evidence_items),
            ),
        ] {
            if required > available {
                return Err(format!(
                    "selected IMAGE aggregate {field} requires {required}, admitted {available}"
                ));
            }
        }
        Ok(())
    }
}

impl KernelResourceLedger {
    #[cfg(test)]
    pub(crate) fn admitted_image_bounds(&self) -> Option<&HostBounds> {
        self.image_bounds.as_ref()
    }

    /// Initial construction only. Retain the exact declared capacities without
    /// changing initialized pools or claiming an OS-wide allocator restriction.
    pub(crate) fn bind_image_bounds(&mut self, bounds: &HostBounds) -> Result<(), String> {
        if self.image_bounds.is_some() || !self.is_idle() {
            return Err("selected IMAGE bounds require one fresh idle resource ledger".into());
        }
        self.image_bounds = Some(bounds.clone());
        Ok(())
    }

    /// A different offer/provider set requires another checked IMAGE/Boot.
    /// Reference profiles retain their existing explicit attachment lifecycle.
    pub(crate) fn require_reference_reconfiguration(&self) -> Result<(), String> {
        if self.image_bounds.is_some() {
            return Err(
                "selected IMAGE reconfiguration requires fresh checked construction".into(),
            );
        }
        Ok(())
    }
    pub(crate) fn replacement_for(
        &self,
        advertisement: &HostAdvertisement,
    ) -> Result<Self, String> {
        self.require_reference_reconfiguration()?;
        Self::new(advertisement)
    }

    /// Check the complete proposed workload before constructing its kernel or
    /// staging resource counters. These scalar checks are necessary, not a
    /// complete managed-memory receipt. In particular, slots are never bytes.
    pub(crate) fn preflight_image_workset<'a>(
        &self,
        fragments: impl IntoIterator<Item = &'a PlanFragment>,
    ) -> Result<(), String> {
        self.require_available()?;
        let Some(bounds) = &self.image_bounds else {
            return Ok(());
        };
        let mut shape = Shape::default();
        for fragment in fragments {
            if (
                &fragment.host_id,
                &fragment.boot_id,
                fragment.offer_generation,
            ) != (&self.identity.0, &self.identity.1, self.identity.2)
            {
                return Err("selected IMAGE workset targets stale Host/Boot/offer identity".into());
            }
            shape.include(fragment)?;
        }
        shape.check(bounds)?;
        // Ordinary installed Backs do not yet provide complete owned backing
        // receipts. Region receipts cannot be borrowed for a cooperative Body
        // kernel. Boot/current offers remain independent of Play admission.
        Err("selected IMAGE workset lacks complete ordinary managed-memory accounting; no Play admitted".into())
    }
}
