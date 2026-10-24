//! Root provisioning policy, never a received grant or a fixture issuer. One
//! acknowledged private setup may read once and attempt one selected join.
#[path = "native_bindings.rs"]
mod bindings;
#[path = "native_display_authority.rs"]
mod display;
#[path = "native_pulse_authority.rs"]
mod pulse;
#[path = "native_wait_authority.rs"]
mod wait;
pub use bindings::{AssignedNativeInput, NativeBindings};
use conduit_core as c;
use conduit_network_join_call::installation::NetworkNativeIdentity;
use sha2::{Digest, Sha256};
#[derive(Clone, Copy)]
pub enum Operation {
    Credentials,
    Join,
}
impl Operation {
    fn facts(
        self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        u32,
        u32,
        usize,
    ) {
        use conduit_net::*;
        match self {
            Self::Credentials => (
                "grant/esp32/setup-credentials/1",
                NETWORK_CREDENTIALS_AUTHORITY,
                NETWORK_CREDENTIALS_HOST_CALL,
                NETWORK_CREDENTIALS_SUBJECT,
                conduit_network_join_call::credential_installation::CREDENTIAL_CAPABILITY,
                1,
                MAXIMUM_JOIN_INPUT_BYTES,
                0,
            ),
            Self::Join => (
                "grant/esp32/setup-network/1",
                NETWORK_CONFIG_AUTHORITY,
                NETWORK_JOIN_HOST_CALL,
                NETWORK_CONFIG_SUBJECT,
                conduit_network_join_call::installation::JOIN_CAPABILITY,
                MAXIMUM_JOIN_INPUT_BYTES,
                MAXIMUM_JOIN_OUTPUT_BYTES,
                1,
            ),
        }
    }
}
pub struct RootIssuer {
    host: c::HostId,
    boot: c::BootId,
    key: [u8; 32],
    issued: [bool; 2],
    wait_issued: bool,
    pulse_issued: bool,
    display_issued: bool,
    display_generation: u64,
    pulse_generation: u64,
    wait_generation: u64,
    last_play_sequence: u64,
    // Prepared control identities live in admitted Root heap headroom, keeping
    // the permanent executor task frame small. No allocation occurs in Play.
    reserved_play: Option<alloc::boxed::Box<c::ActivePlayIdentity>>,
    native_issued_play: Option<alloc::boxed::Box<c::ActivePlayIdentity>>,
}
impl RootIssuer {
    pub fn observe(boot: &crate::boot::ObservedBoot) -> Result<Self, ()> {
        let mut key = [0; 32];
        esp_hal::rng::Trng::try_new()
            .map_err(|_| ())?
            .read(&mut key);
        if key == [0; 32] {
            return Err(());
        }
        let (host, boot) = boot.ids();
        Ok(Self {
            host: host.into(),
            boot: boot.into(),
            key,
            issued: [false; 2],
            wait_issued: false,
            pulse_issued: false,
            display_issued: false,
            display_generation: 0,
            pulse_generation: 0,
            wait_generation: 0,
            last_play_sequence: 0,
            reserved_play: None,
            native_issued_play: None,
        })
    }
    /// Observation only; this neither reserves nor authorizes the next identity.
    pub fn next_play_sequence(&self) -> Option<u64> {
        self.last_play_sequence.checked_add(1)
    }
    /// Allocate in this actual Host/Boot, independently of any Body-local
    /// counter. Reservation is identity custody only: it neither starts Play,
    /// issues a capability nor retires an earlier active provider.
    pub fn reserve_play(
        &mut self,
        fragment: &c::PlanFragment,
    ) -> Result<c::ActivePlayIdentity, ()> {
        if fragment.host_id != self.host
            || fragment.boot_id != self.boot
            || !c::verify_plan_fragment(fragment)
        {
            return Err(());
        }
        self.reserve_plan(&fragment.plan_id)
    }
    /// Correlate full identities with already validated bounded tables. The
    /// header's truncated identities cannot reconstruct full Plan/fragment IDs.
    /// This local reservation is neither authenticated Body installation nor
    /// permission to start: native possession and complete admission remain
    /// separate gates. Do not expose this method as an unsigned start command.
    pub fn reserve_assigned_play(
        &mut self,
        assignment: &conduit_environment_runtime::appliance::assigned::ValidatedAssignedAppliance,
        plan: &c::PlanId,
        fragment: &c::FragmentId,
    ) -> Result<c::ActivePlayIdentity, ()> {
        let identity = assignment.identity();
        if identity.host != c::AssignedIdentity::from_text(self.host.as_str())
            || identity.boot != c::AssignedIdentity::from_text(self.boot.as_str())
            || identity.plan != c::AssignedIdentity::from_text(plan.as_str())
            || identity.fragment != c::AssignedIdentity::from_text(fragment.as_str())
        {
            return Err(());
        }
        self.reserve_plan(plan)
    }
    /// Expected identity is checked before consuming Root's global sequence.
    /// The caller has separately authenticated owner intent and assignment seal.
    pub fn reserve_assigned_play_matching(
        &mut self,
        assignment: &conduit_environment_runtime::appliance::assigned::ValidatedAssignedAppliance,
        plan: &c::PlanId,
        fragment: &c::FragmentId,
        expected: &str,
    ) -> Result<c::ActivePlayIdentity, ()> {
        let next = self.next_play_sequence().ok_or(())?;
        let candidate = c::bind_active_play(plan, &self.host, &self.boot, next);
        if candidate.active_play_id.as_str() != expected {
            return Err(());
        }
        self.reserve_assigned_play(assignment, plan, fragment)
    }
    /// Actual returned custody is non-deserializable and carries the exact
    /// retired Play scope. Finishing never rewinds sequence or provider generations.
    pub fn finish_native_play<P, W>(
        &mut self,
        active: &c::ActivePlayIdentity,
        retired: &crate::native_retirement::RetiredProviders<P, W>,
    ) -> Result<(), ()> {
        if self.native_issued_play.as_deref() != Some(active)
            || !self.pulse_issued
            || !self.wait_issued
            || !retired.matches_active(active)
            || retired.provider_generations() != [self.pulse_generation, self.wait_generation]
        {
            return Err(());
        }
        self.pulse_issued = false;
        self.wait_issued = false;
        self.native_issued_play = None;
        if self.reserved_play.as_deref() == Some(active) {
            self.reserved_play = None;
        }
        Ok(())
    }
    fn reserve_plan(&mut self, plan: &c::PlanId) -> Result<c::ActivePlayIdentity, ()> {
        let sequence = self.last_play_sequence.checked_add(1).ok_or(())?;
        let active = c::bind_active_play(plan, &self.host, &self.boot, sequence);
        self.reserved_play = Some(alloc::boxed::Box::new(active.clone()));
        self.last_play_sequence = sequence;
        Ok(active)
    }
    pub fn grant(
        &self,
        native: &NetworkNativeIdentity,
        operation: Operation,
    ) -> Result<c::AuthorityGrant, ()> {
        if native.host != self.host || native.boot != self.boot {
            return Err(());
        }
        let (id, authority, call, subject, cap, _, _, _) = operation.facts();
        Ok(c::AuthorityGrant {
            grant_id: id.into(),
            contract_id: authority.into(),
            host_call_contract_id: call.into(),
            subject_kind: subject.into(),
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            capability_id: cap.into(),
        })
    }
    pub fn issue(
        &mut self,
        native: &NetworkNativeIdentity,
        operation: Operation,
        fragment: &c::PlanFragment,
        active: &c::ActivePlayIdentity,
    ) -> Result<
        (
            c::BaseCapabilityTable,
            c::BaseCapabilityHandle,
            c::BaseOperationClaim,
        ),
        (),
    > {
        let grant = self.grant(native, operation)?;
        let (_, _, call, subject, cap, input, output, index) = operation.facts();
        if self.issued[index]
            || fragment.host_id != self.host
            || fragment.boot_id != self.boot
            || !c::verify_plan_fragment(fragment)
            || self.reserved_play.as_deref() != Some(active)
            || *active
                != c::bind_active_play(
                    &fragment.plan_id,
                    &self.host,
                    &self.boot,
                    active.play_sequence,
                )
        {
            return Err(());
        }
        let mut hash = Sha256::new();
        hash.update(self.key);
        hash.update(native.instance.as_str().as_bytes());
        let key: [u8; 32] = hash.finalize().into();
        let mut table = c::BaseCapabilityTable::new(
            self.host.clone(),
            self.boot.clone(),
            native.instance.clone(),
            native.provider_generation,
            key,
            1,
        )
        .map_err(|_| ())?;
        let scope = c::BaseCapabilityScope {
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            base_instance_id: native.instance.clone(),
            base_provider_generation: native.provider_generation,
            plan_id: fragment.plan_id.clone(),
            active_play_id: active.active_play_id.clone(),
            authority_grant_id: grant.grant_id.clone(),
            authority_contract_id: grant.contract_id.clone(),
            capability_id: cap.into(),
            implementation_id: native.implementation.clone(),
            operation_contract_id: call.into(),
            subject_kind: subject.into(),
            resource_pool_id: native.resource.clone(),
            resource_generation_id: native.resource_generation.clone(),
            envelope_id: native.envelope.clone(),
            maximum_parameter_bytes: input,
            maximum_result_bytes: output,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        };
        let claim = c::BaseOperationClaim {
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            plan_id: scope.plan_id.clone(),
            active_play_id: scope.active_play_id.clone(),
            implementation_id: scope.implementation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            envelope_id: scope.envelope_id.clone(),
            parameter_bytes: input,
            work_units: 1,
        };
        let authority = c::BaseCapabilityAuthority {
            grant,
            base_instance_id: native.instance.clone(),
            base_provider_generation: native.provider_generation,
            resource_pool_id: native.resource.clone(),
            resource_generation_id: native.resource_generation.clone(),
            operation_contract_id: call.into(),
            envelope_id: native.envelope.clone(),
            maximum_parameter_bytes: input,
            maximum_result_bytes: output,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        };
        let handle = table
            .issue(c::CapabilityIssueRequest { scope, authority })
            .map_err(|_| ())?;
        self.issued[index] = true;
        Ok((table, handle, claim))
    }
}
