//! Actual owned capacities for the closed local startup recipe. This is a
//! retained-runtime measurement, NOT an IMAGE admission/Root peak receipt.
use super::{InstalledRemoteFragment, StartupBindingState};
use crate::installed_std::{body_kernel, InstalledBack, InstalledScheduler};
use conduit_core::*;
use core::mem::size_of;

struct Count(usize);
impl Count {
    fn add(&mut self, n: usize) -> Result<(), String> {
        self.0 = self
            .0
            .checked_add(n)
            .ok_or("remote backing size overflow")?;
        Ok(())
    }
    fn vector<T>(&mut self, v: &Vec<T>) -> Result<(), String> {
        self.add(
            v.capacity()
                .checked_mul(size_of::<T>())
                .ok_or("remote vector overflow")?,
        )
    }
    fn owned(&mut self, n: Option<usize>) -> Result<(), String> {
        self.add(n.ok_or("remote startup receipt does not support this live auxiliary owner")?)
    }
}
macro_rules! strings { ($n:expr, $($s:expr),+ $(,)?) => { $( $n.add($s.0.capacity())?; )+ }; }
macro_rules! empty { ($n:expr, $($v:expr),+ $(,)?) => { $(
    if !$v.is_empty() { return Err("remote startup backing does not support this descriptor/profile".into()); }
    $n.vector($v)?;
)+ }; }
fn placement(n: &mut Count, p: &PlannedGear) -> Result<(), String> {
    strings!(
        n,
        p.placement_id,
        p.gear_id,
        p.kind_id,
        p.kind_contract_revision,
        p.execution_profile_id,
        p.host_id,
        p.boot_id,
        p.capability_id,
        p.implementation_id,
        p.artifact_id
    );
    empty!(
        n,
        &p.configuration,
        &p.realization_characteristics,
        &p.realization_properties,
        &p.terminal_transductions,
        &p.pool_references,
        &p.semantic_contract.configuration
    );
    if let Some(base) = &p.base {
        strings!(
            n,
            base.base_id,
            base.provider_instance_id,
            base.implementation_id,
            base.mechanism_family
        );
    }
    n.vector(&p.inputs)?;
    n.vector(&p.outputs)?;
    for port in p.inputs.iter().chain(&p.outputs) {
        strings!(n, port.port_id, port.value_kind);
        if let Some(kind) = &port.abnormal_kind {
            strings!(n, kind);
        }
    }
    n.vector(&p.semantic_contract.laws)?;
    for law in &p.semantic_contract.laws {
        match law {
            KindSemanticLaw::Terminal(_) => {}
            KindSemanticLaw::ValueContracts(contracts) => {
                n.vector(contracts)?;
                for c in contracts {
                    match &c.location {
                        FrontValueLocation::Startup(s) => n.add(s.capacity())?,
                        FrontValueLocation::Input(id)
                        | FrontValueLocation::Output(id)
                        | FrontValueLocation::InputAbnormal(id)
                        | FrontValueLocation::OutputAbnormal(id) => {
                            strings!(n, id);
                        }
                    }
                    n.owned(c.contract.retained_backing_bytes())?;
                }
            }
            _ => return Err("remote startup backing lacks this semantic law accounting".into()),
        }
    }
    n.vector(&p.host_calls)?;
    for call in &p.host_calls {
        strings!(n, call.contract_id);
        if let Some(kind) = &call.target_kind {
            strings!(n, kind);
        }
    }
    n.vector(&p.resources)?;
    for r in &p.resources {
        if r.protected.is_some() || r.compute.is_some() || r.content.is_some() {
            return Err("remote startup backing lacks external resource accounting".into());
        }
        strings!(n, r.pool_id, r.class_id);
    }
    n.vector(&p.authority)?;
    for a in &p.authority {
        strings!(
            n,
            a.grant_id,
            a.contract_id,
            a.host_call_contract_id,
            a.subject_kind,
            a.host_id,
            a.boot_id,
            a.capability_id
        );
    }
    Ok(())
}
impl InstalledRemoteFragment {
    /// Newly owned heap payload of this prepared runtime only. Inline caller
    /// storage, reservations, preparation workspace, process RSS and transport
    /// stores are separate. Unsupported state refuses; no grant is inferred.
    pub fn retained_startup_profile_backing_bytes(&self) -> Result<usize, String> {
        if self.startup_state != StartupBindingState::Unbound
            || self.body_time_required
            || self.vision_tracker.is_some()
            || self.pending_body_context.is_some()
            || self.delivered_body_context.is_some()
            || self.startup_outputs.len() != 1
            || self
                .startup_outputs
                .iter()
                .any(|c| c.pending.is_some() || c.receipt.request.is_some())
        {
            return Err(
                "remote startup backing requires original unstarted closed preparation".into(),
            );
        }
        let mut source = 0;
        let mut cue = 0;
        let mut n = Count(size_of::<InstalledScheduler>());
        for driver in self.scheduler.drivers() {
            match driver {
                InstalledBack::Inactive => {}
                InstalledBack::StartupSource(_) => source += 1,
                InstalledBack::StartupChime(_) => cue += 1,
                _ => return Err("remote startup backing does not cover this Back".into()),
            }
            n.add(body_kernel::remote_driver_backing(driver)?)?;
        }
        if source != 1 || cue != 1 {
            return Err("remote startup backing requires the exact local source/cue recipe".into());
        }
        n.add(self.scheduler.values().reserved_storage_bytes())?;
        n.add(self.scheduler.signs().reserved_storage_bytes())?;
        n.add(body_kernel::remote_identity_backing(
            &self.lowered.identity,
        )?)?;
        n.vector(&self.placements)?;
        for p in &self.placements {
            placement(&mut n, p)?;
        }
        n.vector(&self.startup_outputs)?;
        for c in &self.startup_outputs {
            n.add(c.output.retained_backing_bytes())?;
        }
        n.owned(self.whisper_languages.empty_backing_bytes())?;
        n.owned(self.sessions.empty_backing_bytes())?;
        n.owned(self.worker_text_basis.retained_backing_bytes())?;
        n.owned(self.data_text_hosts.empty_backing_bytes())?;
        n.add(self.text_output_buffer.capacity())?;
        n.add(self.model_output_buffer.capacity())?;
        n.add(self.vision_run_id.capacity())?;
        n.add(self.vision_active_play_id.capacity())?;
        n.add(self.vision_clock_basis.capacity())?;
        macro_rules! absent_providers { ($($v:expr),+) => { $(
            if $v.iter().any(Option::is_some) {return Err("remote startup backing has an unsupported live provider".into());}
            n.vector($v)?;
        )+ }; }
        absent_providers!(
            &self.recognized_turn_commit_hosts,
            &self.speech_window_hosts,
            &self.speech_result_stream_hosts,
            &self.generated_speech_commit_hosts,
            &self.body_chat_prompt_hosts
        );
        let l = &self.lowered;
        empty!(
            n,
            &l.states,
            &l.fusions,
            &l.remote_endpoints,
            &l.fore_ports,
            &l.resource_cords,
            &l.shared_pools
        );
        n.vector(&l.nodes)?;
        for node in &l.nodes {
            strings!(n, node.placement_id);
            empty!(n, &node.terminal_transductions);
            n.vector(&node.inputs)?;
            n.vector(&node.outputs)?;
            for port in node.inputs.iter().chain(&node.outputs) {
                strings!(n, port.port_id, port.value_kind);
                if let Some(kind) = &port.abnormal_kind {
                    strings!(n, kind);
                }
            }
        }
        n.vector(&l.node_specs)?;
        n.vector(&l.cords)?;
        n.vector(&l.routes)?;
        for c in &l.cords {
            strings!(n, c.connection_id);
        }
        for r in &l.routes {
            n.vector(&r.targets)?;
        }
        n.vector(&l.host_calls)?;
        for c in &l.host_calls {
            strings!(n, c.contract_id);
            if let Some(k) = &c.target_kind {
                strings!(n, k);
            }
        }
        n.vector(&l.resources)?;
        n.vector(&l.signs)?;
        for s in &l.signs {
            // Numeric expectation is inline in the sign vector.
            match &s.expected {
                ExpectedSign::PlacementPrepared(id) | ExpectedSign::PlacementTerminal(id) => {
                    strings!(n, id);
                }
                ExpectedSign::ConnectionTerminal(id) => {
                    strings!(n, id);
                }
                ExpectedSign::PlanFragmentReceived | ExpectedSign::PlanTerminal => {}
            }
        }
        let fixed = super::fixed_workspace::derive()
            .ok_or("startup fixed workspace is unsupported or overflowed")?;
        if self.scheduler.values().reserved_storage_bytes() < fixed.values
            || self.scheduler.signs().reserved_storage_bytes() < fixed.signs
            || n.0 < fixed.retained_kernel
        {
            return Err("startup fixed constructor accounting drifted".into());
        }
        #[cfg(test)]
        eprintln!(
            "STARTUP_FIXED_WORKSPACE retained={} constructor_requests={}",
            fixed.retained_kernel, fixed.constructor_requests
        );
        // The complete preparation recipe must add all non-fixed owners; this
        // partial term is deliberately not used to enable IMAGE preflight.
        let _fixed_constructor_requests = fixed.constructor_requests;
        Ok(n.0)
    }
}
