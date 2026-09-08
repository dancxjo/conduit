//! Allocation-free protection-domain capability handles for ConduitOS.
//!
//! This table is kernel-owned realization state. It does not replace the
//! semantic capability authority in `conduit-core`; it seals the already
//! selected scope into a small domain-local handle suitable for a trap gate.

pub const MAXIMUM_DOMAIN_CAPABILITIES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectionDomainId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelCapabilityHandle(u64);

impl KernelCapabilityHandle {
    #[cfg(feature = "conduitos-isolation-proof")]
    pub(crate) const fn raw_for_domain(self) -> u64 {
        self.0
    }

    #[cfg(feature = "conduitos-isolation-proof")]
    pub(crate) const fn from_untrusted(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelCapabilityScope {
    pub host: [u8; 32],
    pub boot: [u8; 32],
    pub plan: [u8; 32],
    pub play: [u8; 32],
    pub implementation: [u8; 32],
    pub base: [u8; 32],
    pub base_generation: u32,
    pub resource: [u8; 32],
    pub resource_generation: u32,
    pub operation: u32,
    pub subject: [u8; 32],
    pub authority: [u8; 32],
    pub maximum_parameter_bytes: u32,
    pub maximum_work_units: u32,
    pub maximum_in_flight: u16,
    pub maximum_operations: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelOperationClaim {
    pub boot: [u8; 32],
    pub plan: [u8; 32],
    pub play: [u8; 32],
    pub base_generation: u32,
    pub resource_generation: u32,
    pub operation: u32,
    pub parameter_bytes: u32,
    pub work_units: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelOperationLease {
    slot: u16,
    table_generation: u32,
    sequence: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelCapabilityRefusal {
    InvalidTable,
    InvalidScope,
    TableFull,
    UnknownHandle,
    WrongDomain,
    WrongScope,
    ParameterEnvelope,
    WorkEnvelope,
    InFlightFull,
    Exhausted,
    Revoked,
    StaleLease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelRevocationCause {
    PlayCancelled,
    PlayCompleted,
    PlanReplaced,
    AuthorityRevoked,
    ResourceReplaced,
    BaseReplaced,
    BootReplaced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelRevocationReceipt {
    pub cause: KernelRevocationCause,
    pub revoked_handles: u16,
}

#[derive(Clone, Copy)]
struct Entry {
    occupied: bool,
    domain: ProtectionDomainId,
    handle: KernelCapabilityHandle,
    scope: KernelCapabilityScope,
    generation: u32,
    next_sequence: u32,
    active_sequence: u32,
    completed: u32,
    in_flight: u16,
    revoked: bool,
}

const EMPTY_SCOPE: KernelCapabilityScope = KernelCapabilityScope {
    host: [0; 32],
    boot: [0; 32],
    plan: [0; 32],
    play: [0; 32],
    implementation: [0; 32],
    base: [0; 32],
    base_generation: 0,
    resource: [0; 32],
    resource_generation: 0,
    operation: 0,
    subject: [0; 32],
    authority: [0; 32],
    maximum_parameter_bytes: 0,
    maximum_work_units: 0,
    maximum_in_flight: 0,
    maximum_operations: 0,
};

const EMPTY_ENTRY: Entry = Entry {
    occupied: false,
    domain: ProtectionDomainId(0),
    handle: KernelCapabilityHandle(0),
    scope: EMPTY_SCOPE,
    generation: 0,
    next_sequence: 0,
    active_sequence: 0,
    completed: 0,
    in_flight: 0,
    revoked: false,
};

pub struct KernelCapabilityTable {
    secret: u64,
    next_issuance: u32,
    entries: [Entry; MAXIMUM_DOMAIN_CAPABILITIES],
}

impl KernelCapabilityTable {
    pub fn new(secret: u64) -> Result<Self, KernelCapabilityRefusal> {
        if secret == 0 {
            return Err(KernelCapabilityRefusal::InvalidTable);
        }
        Ok(Self {
            secret,
            next_issuance: 1,
            entries: [EMPTY_ENTRY; MAXIMUM_DOMAIN_CAPABILITIES],
        })
    }

    pub fn issue(
        &mut self,
        domain: ProtectionDomainId,
        scope: KernelCapabilityScope,
    ) -> Result<KernelCapabilityHandle, KernelCapabilityRefusal> {
        validate_scope(domain, &scope)?;
        let slot = self
            .entries
            .iter()
            .position(|entry| !entry.occupied)
            .ok_or(KernelCapabilityRefusal::TableFull)?;
        let issuance = self.next_issuance;
        self.next_issuance = self.next_issuance.wrapping_add(1).max(1);
        let handle =
            KernelCapabilityHandle(mix_handle(self.secret, domain, slot, issuance, &scope));
        self.entries[slot] = Entry {
            occupied: true,
            domain,
            handle,
            scope,
            generation: issuance,
            next_sequence: 1,
            active_sequence: 0,
            completed: 0,
            in_flight: 0,
            revoked: false,
        };
        Ok(handle)
    }

    pub fn authorize(
        &mut self,
        domain: ProtectionDomainId,
        handle: KernelCapabilityHandle,
        claim: KernelOperationClaim,
    ) -> Result<KernelOperationLease, KernelCapabilityRefusal> {
        let (slot, entry) = self
            .entries
            .iter_mut()
            .enumerate()
            .find(|(_, entry)| entry.occupied && constant_time_equal(entry.handle.0, handle.0))
            .ok_or(KernelCapabilityRefusal::UnknownHandle)?;
        if entry.domain != domain {
            return Err(KernelCapabilityRefusal::WrongDomain);
        }
        if entry.revoked {
            return Err(KernelCapabilityRefusal::Revoked);
        }
        if claim.boot != entry.scope.boot
            || claim.plan != entry.scope.plan
            || claim.play != entry.scope.play
            || claim.base_generation != entry.scope.base_generation
            || claim.resource_generation != entry.scope.resource_generation
            || claim.operation != entry.scope.operation
        {
            return Err(KernelCapabilityRefusal::WrongScope);
        }
        if claim.parameter_bytes > entry.scope.maximum_parameter_bytes {
            return Err(KernelCapabilityRefusal::ParameterEnvelope);
        }
        if claim.work_units > entry.scope.maximum_work_units {
            return Err(KernelCapabilityRefusal::WorkEnvelope);
        }
        if entry.in_flight == entry.scope.maximum_in_flight {
            return Err(KernelCapabilityRefusal::InFlightFull);
        }
        if entry.completed + u32::from(entry.in_flight) >= entry.scope.maximum_operations {
            return Err(KernelCapabilityRefusal::Exhausted);
        }
        let sequence = entry.next_sequence;
        entry.next_sequence = entry.next_sequence.wrapping_add(1).max(1);
        entry.active_sequence = sequence;
        entry.in_flight += 1;
        Ok(KernelOperationLease {
            slot: slot as u16,
            table_generation: entry.generation,
            sequence,
        })
    }

    pub fn complete(&mut self, lease: KernelOperationLease) -> Result<(), KernelCapabilityRefusal> {
        let entry = self
            .entries
            .get_mut(usize::from(lease.slot))
            .ok_or(KernelCapabilityRefusal::StaleLease)?;
        if !entry.occupied
            || entry.revoked
            || entry.generation != lease.table_generation
            || entry.active_sequence != lease.sequence
            || entry.in_flight != 1
        {
            return Err(KernelCapabilityRefusal::StaleLease);
        }
        entry.active_sequence = 0;
        entry.in_flight = 0;
        entry.completed += 1;
        Ok(())
    }

    pub fn revoke_handle(
        &mut self,
        domain: ProtectionDomainId,
        handle: KernelCapabilityHandle,
        cause: KernelRevocationCause,
    ) -> Result<KernelRevocationReceipt, KernelCapabilityRefusal> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.occupied && constant_time_equal(entry.handle.0, handle.0))
            .ok_or(KernelCapabilityRefusal::UnknownHandle)?;
        if entry.domain != domain {
            return Err(KernelCapabilityRefusal::WrongDomain);
        }
        entry.revoked = true;
        entry.in_flight = 0;
        entry.active_sequence = 0;
        entry.generation = entry.generation.wrapping_add(1).max(1);
        Ok(KernelRevocationReceipt {
            cause,
            revoked_handles: 1,
        })
    }

    pub fn revoke_domain(
        &mut self,
        domain: ProtectionDomainId,
        cause: KernelRevocationCause,
    ) -> KernelRevocationReceipt {
        let mut revoked = 0u16;
        for entry in &mut self.entries {
            if entry.occupied && entry.domain == domain && !entry.revoked {
                entry.revoked = true;
                entry.in_flight = 0;
                entry.active_sequence = 0;
                entry.generation = entry.generation.wrapping_add(1).max(1);
                revoked += 1;
            }
        }
        KernelRevocationReceipt {
            cause,
            revoked_handles: revoked,
        }
    }
}

fn validate_scope(
    domain: ProtectionDomainId,
    scope: &KernelCapabilityScope,
) -> Result<(), KernelCapabilityRefusal> {
    if domain.0 == 0
        || scope.host == [0; 32]
        || scope.boot == [0; 32]
        || scope.plan == [0; 32]
        || scope.play == [0; 32]
        || scope.implementation == [0; 32]
        || scope.base == [0; 32]
        || scope.base_generation == 0
        || scope.resource == [0; 32]
        || scope.resource_generation == 0
        || scope.operation == 0
        || scope.subject == [0; 32]
        || scope.authority == [0; 32]
        || scope.maximum_parameter_bytes == 0
        || scope.maximum_work_units == 0
        || scope.maximum_in_flight != 1
        || scope.maximum_operations == 0
    {
        return Err(KernelCapabilityRefusal::InvalidScope);
    }
    Ok(())
}

fn mix_handle(
    secret: u64,
    domain: ProtectionDomainId,
    slot: usize,
    issuance: u32,
    scope: &KernelCapabilityScope,
) -> u64 {
    let mut value =
        secret ^ (u64::from(domain.0) << 32) ^ issuance as u64 ^ (slot as u64).rotate_left(19);
    for byte in scope
        .boot
        .iter()
        .chain(scope.plan.iter())
        .chain(scope.play.iter())
        .chain(scope.base.iter())
        .chain(scope.resource.iter())
        .chain(scope.authority.iter())
    {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x100_0000_01b3).rotate_left(11);
    }
    value | 1
}

fn constant_time_equal(left: u64, right: u64) -> bool {
    (left ^ right).count_ones() == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(seed: u8) -> KernelCapabilityScope {
        KernelCapabilityScope {
            host: [seed; 32],
            boot: [seed.wrapping_add(1); 32],
            plan: [seed.wrapping_add(2); 32],
            play: [seed.wrapping_add(3); 32],
            implementation: [seed.wrapping_add(4); 32],
            base: [seed.wrapping_add(5); 32],
            base_generation: 1,
            resource: [seed.wrapping_add(6); 32],
            resource_generation: 1,
            operation: 7,
            subject: [seed.wrapping_add(7); 32],
            authority: [seed.wrapping_add(8); 32],
            maximum_parameter_bytes: 8,
            maximum_work_units: 2,
            maximum_in_flight: 1,
            maximum_operations: 2,
        }
    }

    fn claim(scope: &KernelCapabilityScope) -> KernelOperationClaim {
        KernelOperationClaim {
            boot: scope.boot,
            plan: scope.plan,
            play: scope.play,
            base_generation: scope.base_generation,
            resource_generation: scope.resource_generation,
            operation: scope.operation,
            parameter_bytes: 8,
            work_units: 2,
        }
    }

    #[test]
    fn handle_is_domain_local_exact_bounded_and_finite() {
        let owner = ProtectionDomainId(3);
        let sibling = ProtectionDomainId(4);
        let exact = scope(1);
        let mut table = KernelCapabilityTable::new(0x1234_5678).unwrap();
        let handle = table.issue(owner, exact).unwrap();
        assert_eq!(
            table.authorize(sibling, handle, claim(&exact)),
            Err(KernelCapabilityRefusal::WrongDomain)
        );
        let mut wrong = claim(&exact);
        wrong.operation += 1;
        assert_eq!(
            table.authorize(owner, handle, wrong),
            Err(KernelCapabilityRefusal::WrongScope)
        );
        let first = table.authorize(owner, handle, claim(&exact)).unwrap();
        assert_eq!(
            table.authorize(owner, handle, claim(&exact)),
            Err(KernelCapabilityRefusal::InFlightFull)
        );
        table.complete(first).unwrap();
        let second = table.authorize(owner, handle, claim(&exact)).unwrap();
        table.complete(second).unwrap();
        assert_eq!(
            table.authorize(owner, handle, claim(&exact)),
            Err(KernelCapabilityRefusal::Exhausted)
        );
    }

    #[test]
    fn arbitrary_stolen_and_stale_values_never_gain_authority() {
        let owner = ProtectionDomainId(9);
        let sibling = ProtectionDomainId(10);
        let exact = scope(10);
        let mut table = KernelCapabilityTable::new(0xfeed_beef).unwrap();
        let handle = table.issue(owner, exact).unwrap();
        assert_eq!(
            table.authorize(owner, KernelCapabilityHandle(17), claim(&exact)),
            Err(KernelCapabilityRefusal::UnknownHandle)
        );
        assert_eq!(
            table.authorize(sibling, handle, claim(&exact)),
            Err(KernelCapabilityRefusal::WrongDomain)
        );
        let lease = table.authorize(owner, handle, claim(&exact)).unwrap();
        table
            .revoke_handle(owner, handle, KernelRevocationCause::AuthorityRevoked)
            .unwrap();
        assert_eq!(
            table.complete(lease),
            Err(KernelCapabilityRefusal::StaleLease)
        );
        assert_eq!(
            table.authorize(owner, handle, claim(&exact)),
            Err(KernelCapabilityRefusal::Revoked)
        );
    }

    #[test]
    fn lifecycle_replacement_revokes_every_domain_handle() {
        let domain = ProtectionDomainId(5);
        let other = ProtectionDomainId(6);
        let mut table = KernelCapabilityTable::new(99).unwrap();
        let first_scope = scope(21);
        let second_scope = scope(41);
        let first = table.issue(domain, first_scope).unwrap();
        let second = table.issue(domain, second_scope).unwrap();
        let other_handle = table.issue(other, scope(61)).unwrap();
        assert_eq!(
            table.revoke_domain(domain, KernelRevocationCause::PlanReplaced),
            KernelRevocationReceipt {
                cause: KernelRevocationCause::PlanReplaced,
                revoked_handles: 2,
            }
        );
        assert_eq!(
            table.authorize(domain, first, claim(&first_scope)),
            Err(KernelCapabilityRefusal::Revoked)
        );
        assert_eq!(
            table.authorize(domain, second, claim(&second_scope)),
            Err(KernelCapabilityRefusal::Revoked)
        );
        assert!(
            table
                .authorize(other, other_handle, claim(&scope(61)))
                .is_ok()
        );
    }
}
