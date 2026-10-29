//! Root provisioning policy, never a received grant or a fixture issuer. One
//! acknowledged private setup may read once and attempt one selected join.
#[path = "native_bindings.rs"]
mod bindings;
#[path = "native_display_authority.rs"]
mod display;
#[cfg(test)]
#[path = "../tests/authority_fixture.rs"]
mod fixture;
#[cfg(feature = "mask-root")]
#[path = "native_mask_authority.rs"]
mod mask;
#[cfg(feature = "mask-root")]
#[path = "native_mask_lease.rs"]
mod mask_lease;
#[path = "native_authority_network_verified.rs"]
mod network_verified;
#[cfg(feature = "mask-root")]
pub use mask_lease::{MaskLeaseIssuer, MaskScopeGuard};
pub use network_verified::LinkedNetworkPreparation;
#[path = "native_pulse_authority.rs"]
mod pulse;
#[path = "native_verified_bindings.rs"]
mod verified_bindings;
#[path = "native_wait_authority.rs"]
mod wait;
pub use bindings::{AssignedNativeInput, NativeBindings};
use conduit_core as c;
use conduit_network_join_call::installation::NetworkNativeIdentity;
use sha2::{Digest, Sha256};
#[cfg(feature = "target-mask-memory-check")]
pub use verified_bindings::DirectNativeInput;
pub use verified_bindings::VerifiedNativeInput;
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
    reserved_display_play: Option<alloc::boxed::Box<c::ActivePlayIdentity>>,
    issued_display_play: Option<alloc::boxed::Box<c::ActivePlayIdentity>>,
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
    #[cfg(target_arch = "xtensa")]
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
            reserved_display_play: None,
            issued_display_play: None,
            display_generation: 0,
            pulse_generation: 0,
            wait_generation: 0,
            last_play_sequence: 0,
            reserved_play: None,
            native_issued_play: None,
        })
    }
    /// Borrow the actual outstanding scope; observation grants no operation.
    pub(crate) fn native_issued_active(&self) -> Option<&c::ActivePlayIdentity> {
        self.native_issued_play.as_deref()
    }
    pub(crate) fn mask_issued_active(&self) -> Option<&c::ActivePlayIdentity> {
        self.issued_display_play.as_deref()
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
        let g = self.grant_ref(native, operation)?;
        Ok(c::AuthorityGrant {
            grant_id: g.grant_id.into(),
            contract_id: g.contract_id.into(),
            host_call_contract_id: g.host_call_contract_id.into(),
            subject_kind: g.subject_kind.into(),
            host_id: g.host_id.into(),
            boot_id: g.boot_id.into(),
            capability_id: g.capability_id.into(),
        })
    }
    pub fn grant_ref<'a>(
        &self,
        native: &'a NetworkNativeIdentity,
        operation: Operation,
    ) -> Result<c::AuthorityGrantRef<'a>, ()> {
        if native.host != self.host || native.boot != self.boot {
            return Err(());
        }
        let (id, authority, call, subject, cap, _, _, _) = operation.facts();
        Ok(c::AuthorityGrantRef {
            grant_id: id,
            contract_id: authority,
            host_call_contract_id: call,
            subject_kind: subject,
            host_id: native.host.as_str(),
            boot_id: native.boot.as_str(),
            capability_id: cap,
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
        if !c::verify_plan_fragment(fragment) {
            return Err(());
        }
        self.issue_network_checked(native, operation, fragment, active)
    }
    fn issue_network_checked(
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
        self.issue_network_scope(
            native,
            operation,
            fragment.plan_id.as_str(),
            fragment.host_id.as_str(),
            fragment.boot_id.as_str(),
            active,
        )
    }
    fn issue_network_scope(
        &mut self,
        native: &NetworkNativeIdentity,
        operation: Operation,
        plan: &str,
        host: &str,
        boot: &str,
        active: &c::ActivePlayIdentity,
    ) -> Result<
        (
            c::BaseCapabilityTable,
            c::BaseCapabilityHandle,
            c::BaseOperationClaim,
        ),
        (),
    > {
        if plan != active.plan_id.as_str()
            || host != self.host.as_str()
            || boot != self.boot.as_str()
        {
            return Err(());
        }
        let scope = self.network_scope_for(native, operation, active)?;
        let (mut table, authority, input, index) =
            self.network_issuer_for(native, operation, &scope, active)?;
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
        let handle = table.issue_borrowed(scope, authority).map_err(|_| ())?;
        self.issued[index] = true;
        Ok((table, handle, claim))
    }
    pub(crate) fn reserved_network_active(&self) -> Option<&c::ActivePlayIdentity> {
        self.reserved_play.as_deref()
    }
    /// Descriptive only; neither this scope nor matching IDs issue possession.
    fn network_scope_for(
        &self,
        native: &NetworkNativeIdentity,
        operation: Operation,
        active: &c::ActivePlayIdentity,
    ) -> Result<c::BaseCapabilityScope, ()> {
        let grant = self.grant_ref(native, operation)?;
        let (_, _, call, subject, cap, input, output, _) = operation.facts();
        let scope = c::BaseCapabilityScope {
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            base_instance_id: native.instance.clone(),
            base_provider_generation: native.provider_generation,
            plan_id: active.plan_id.clone(),
            active_play_id: active.active_play_id.clone(),
            authority_grant_id: grant.grant_id.into(),
            authority_contract_id: grant.contract_id.into(),
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
        Ok(scope)
    }
    /// One actual Root policy/key/table entrance for retained and staged paths.
    /// The scope is checked against independently retained Root/native facts;
    /// descriptive scope data can never choose its own authority or envelope.
    fn network_issuer_for<'native>(
        &self,
        native: &'native NetworkNativeIdentity,
        operation: Operation,
        scope: &c::BaseCapabilityScope,
        active: &c::ActivePlayIdentity,
    ) -> Result<
        (
            c::BaseCapabilityTable,
            c::BaseCapabilityAuthorityRef<'native>,
            u32,
            usize,
        ),
        (),
    > {
        let expected = self.grant_ref(native, operation)?;
        let (_, _, call, subject, cap, input, output, index) = operation.facts();
        if self.issued[index]
            || self.reserved_play.as_deref() != Some(active)
            || !active.is_bound_to(
                &active.plan_id,
                &self.host,
                &self.boot,
                active.play_sequence,
            )
            || scope.host_id != self.host
            || scope.boot_id != self.boot
            || scope.base_instance_id != native.instance
            || scope.base_provider_generation != native.provider_generation
            || scope.plan_id != active.plan_id
            || scope.active_play_id != active.active_play_id
            || scope.authority_grant_id.as_str() != expected.grant_id
            || scope.authority_contract_id.as_str() != expected.contract_id
            || scope.capability_id.as_str() != cap
            || scope.implementation_id != native.implementation
            || scope.operation_contract_id.as_str() != call
            || scope.subject_kind.as_str() != subject
            || scope.resource_pool_id != native.resource
            || scope.resource_generation_id != native.resource_generation
            || scope.envelope_id != native.envelope
            || scope.maximum_parameter_bytes != input
            || scope.maximum_result_bytes != output
            || scope.maximum_work_units != 1
            || scope.maximum_in_flight != 1
            || scope.maximum_operations != 1
        {
            return Err(());
        }
        let mut hash = Sha256::new();
        hash.update(self.key);
        hash.update(native.instance.as_str().as_bytes());
        let key: [u8; 32] = hash.finalize().into();
        let table = c::BaseCapabilityTable::new(
            self.host.clone(),
            self.boot.clone(),
            native.instance.clone(),
            native.provider_generation,
            key,
            1,
        )
        .map_err(|_| ())?;
        let authority = c::BaseCapabilityAuthorityRef {
            grant: expected,
            base_instance_id: native.instance.as_str(),
            base_provider_generation: native.provider_generation,
            resource_pool_id: native.resource.as_str(),
            resource_generation_id: native.resource_generation.0.as_str(),
            operation_contract_id: call,
            envelope_id: native.envelope.as_str(),
            maximum_parameter_bytes: input,
            maximum_result_bytes: output,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        };
        Ok((table, authority, input, index))
    }
}

impl RootIssuer {
    /// Finish only the exact returned network owners. No other native worker's
    /// scope, sequence, generation or issued state changes at this boundary.
    pub fn finish_network_play(
        &mut self,
        retired: &crate::native_play::RetiredNetworkControl,
    ) -> Result<(), ()> {
        let active = retired.active();
        if !retired.evidence_is_committed()
            || self.issued != [true, true]
            || self.reserved_play.as_deref() != Some(active)
            || active.host_id != self.host
            || active.boot_id != self.boot
            || !retired.matches_active(active)
        {
            return Err(());
        }
        self.issued = [false, false];
        self.reserved_play = None;
        Ok(())
    }
}
