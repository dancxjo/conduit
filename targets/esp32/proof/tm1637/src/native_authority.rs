//! Root provisioning policy, never a received grant or a fixture issuer. One
//! acknowledged private setup may read once and attempt one selected join.
#[path = "native_wait_authority.rs"]
mod wait;
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
    last_play_sequence: u64,
    reserved_play: Option<c::ActivePlayIdentity>,
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
            last_play_sequence: 0,
            reserved_play: None,
        })
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
        let sequence = self.last_play_sequence.checked_add(1).ok_or(())?;
        let active = c::bind_active_play(&fragment.plan_id, &self.host, &self.boot, sequence);
        self.reserved_play = Some(active.clone());
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
            || self.reserved_play.as_ref() != Some(active)
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
