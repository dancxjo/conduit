//! Root native installation below the authenticated owner and retained reservation.
//! Native calls and the ordinary kernel remain dormant until attributed start.
use super::NativeRoot;
use crate::native_owner_reservation::PreparationPermit;
use alloc::vec::Vec;
use conduit_core as c;
use conduit_environment_runtime::appliance::assignment_transfer as t;
use conduit_plan_lowering::selected_assigned_operation::AssignedOperationInput;
impl NativeRoot {
    pub fn authorize_native_preparation(
        &self,
        token: crate::native_owner_control::AuthenticatedRequest<'_>,
    ) -> Result<PreparationPermit, &'static str> {
        if self.native_bindings.is_some()
            || self.appliance_kernel.is_some()
            || self.execution.is_some()
            || self.worker.is_some()
        {
            return Err("already-native-prepared");
        }
        self.owner_reservation
            .as_ref()
            .ok_or("owner-unreserved")?
            .authorize_preparation(token)
    }
    pub fn native_possession_prepared(&self) -> bool {
        self.native_bindings.is_some()
            || self.execution.is_some()
            || self
                .worker
                .as_ref()
                .is_some_and(|worker| worker.status().native_live)
    }
    pub fn execution_context_prepared(&self) -> bool {
        self.execution.is_some()
            || self
                .worker
                .as_ref()
                .is_some_and(|w| w.status().phase != "retired")
    }
    pub fn execution_context_bytes(&self) -> Option<usize> {
        self.execution_context_prepared()
            .then_some(core::mem::size_of::<super::DeviceExecution>())
    }
    pub fn worker_status(&self) -> Option<crate::native_task::Status> {
        self.worker.as_ref().map(|worker| worker.status())
    }
    pub async fn prepare_native_assignment(
        &mut self,
        permit: PreparationPermit,
        wire: &[u8],
    ) -> Result<(), &'static str> {
        if self.native_bindings.is_some()
            || self.appliance_kernel.is_some()
            || self.execution.is_some()
            || self.worker.is_some()
        {
            return Err("already-native-prepared");
        }
        let reservation = self.owner_reservation.as_ref().ok_or("owner-unreserved")?;
        if !permit.matches(reservation)
            || c::assigned_plan_payload_digest(wire) != reservation.assignment_seal
            || self.assignment_seal != Some(reservation.assignment_seal)
        {
            return Err("assignment-seal");
        }
        let assignment = self.assignment.as_ref().ok_or("assignment-unavailable")?;
        let resident = crate::native_software::resident().map_err(|_| "provisioning")?;
        let stats = esp_alloc::HEAP.stats();
        // Finite record views plus preparation-only capability/binding storage.
        let staging = t::MAXIMUM_RECORDS * core::mem::size_of::<c::AssignedRecord<'_>>() + 8192;
        if stats.size.saturating_sub(stats.current_usage) < staging {
            return Err("native-preparation-capacity");
        }
        let bindings = {
            let mut slots = Vec::new();
            slots
                .try_reserve_exact(t::MAXIMUM_RECORDS)
                .map_err(|_| "record-capacity")?;
            slots.resize(
                t::MAXIMUM_RECORDS,
                c::AssignedRecord {
                    tag: 0,
                    payload: &[],
                },
            );
            let decoded = t::decode(
                wire,
                resident.source,
                crate::native_software::computation_records(),
                &mut slots,
            )
            .map_err(|_| "transfer")?;
            let requirements = crate::appliance_layout::requirements(
                c::AssignedIdentity::from_text(self.host.as_str()),
                c::AssignedIdentity::from_text(self.boot.as_str()),
            );
            self.issuer.bind_assigned_native(
                &mut self.pulse,
                &mut self.wait,
                crate::native_authority::AssignedNativeInput {
                    assignment,
                    resident: &resident,
                    input: AssignedOperationInput::Records {
                        header: decoded.header,
                        records: decoded.records,
                    },
                    maxima: c::AssignedPlanMaxima {
                        encoded_bytes: 48 * 1024,
                        runtime_state_bytes: 32 * 1024,
                        counts: resident.layout.counts,
                    },
                    requirements,
                    fragment: &reservation.fragment,
                    active: &reservation.active,
                    contract: &self.contract,
                },
            )?
        }; // Release borrowed record staging before allocating the kernel.
        self.native_bindings = Some(bindings);
        // Refusal retains the native owners, with no dispatch or claim of rollback.
        // The diagnostic installer cannot bypass this reservation's custody.
        self.appliance_kernel = Some(crate::native_kernel::prepare(
            &mut self.software,
            &mut self.assignment,
        )?);
        self.assignment_seal = None;
        let stats = esp_alloc::HEAP.stats();
        let needed = core::mem::size_of::<super::DeviceExecution>() + 8192;
        if stats.size.saturating_sub(stats.current_usage) < needed {
            return Err("execution-context-capacity");
        }
        self.execution = Some(crate::native_worker::Execution::boxed_retained(
            &mut self.appliance_kernel,
            &mut self.native_bindings,
        )?);
        let spawner = self.worker_spawner.ok_or("worker-executor-unavailable")?;
        self.worker = Some(crate::native_task::prepare(&mut self.execution, spawner)?);
        let worker = self.worker.as_ref().expect("registered worker handle");
        if embassy_time::with_timeout(embassy_time::Duration::from_secs(2), worker.wait_ready())
            .await
            .is_err()
        {
            worker.abort_preparation();
            return Err("worker-ready-timeout");
        }
        if worker.status().phase != "ready-held" {
            worker.abort_preparation();
            return Err("worker-ownership-unavailable");
        }
        Ok(())
    }
}
