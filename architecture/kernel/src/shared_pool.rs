use crate::NodeId;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PoolId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemberKey(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemberPlacement {
    pub node: NodeId,
    pub realization: u16,
    pub play: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemberIdentity {
    pub pool: PoolId,
    pub key: MemberKey,
    pub slot: u16,
    pub epoch: u32,
    pub placement: MemberPlacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberState {
    Empty,
    Preparing,
    Active,
    Releasing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolSignReason {
    Admitted,
    PlayStarted,
    ReleaseRequested,
    Released,
    PoolFull,
    DuplicateKey,
    AuthorityDenied,
    RealizationDenied,
    PreparationFailed,
    MemberFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolSign {
    pub sequence: u32,
    pub pool: PoolId,
    pub key: MemberKey,
    pub slot: Option<u16>,
    pub epoch: Option<u32>,
    pub placement: Option<MemberPlacement>,
    pub from: MemberState,
    pub to: MemberState,
    pub reason: PoolSignReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolError {
    InvalidContract,
    SignExhausted,
    SequenceOverflow,
    PoolFull,
    DuplicateKey,
    AuthorityDenied,
    RealizationDenied,
    UnknownKey,
    StaleMember,
    InvalidLifecycle,
    SnapshotTooSmall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MemberSlot {
    key: MemberKey,
    epoch: u32,
    state: MemberState,
    placement: MemberPlacement,
}

impl MemberSlot {
    const EMPTY: Self = Self {
        key: MemberKey([0; 32]),
        epoch: 0,
        state: MemberState::Empty,
        placement: MemberPlacement {
            node: NodeId(u16::MAX),
            realization: u16::MAX,
            play: u16::MAX,
        },
    };

    fn identity(self, pool: PoolId, slot: u16) -> MemberIdentity {
        MemberIdentity {
            pool,
            key: self.key,
            slot,
            epoch: self.epoch,
            placement: self.placement,
        }
    }
}

/// Fixed-storage membership truth owned by the execution kernel. It allocates
/// no member Gears and schedules no work; it admits exact already-lowered
/// member placements and protects their occupation epochs.
pub struct FixedSharedPool<const SLOTS: usize, const SIGN: usize> {
    pool: PoolId,
    maximum_members: u16,
    authority: u16,
    realization_count: u16,
    slots: [MemberSlot; SLOTS],
    signs: [Option<PoolSign>; SIGN],
    sign_len: u16,
    next_sequence: u32,
}

impl<const SLOTS: usize, const SIGN: usize> FixedSharedPool<SLOTS, SIGN> {
    pub fn new(
        pool: PoolId,
        maximum_members: u16,
        authority: u16,
        realization_count: u16,
    ) -> Result<Self, PoolError> {
        if maximum_members == 0
            || usize::from(maximum_members) > SLOTS
            || authority == u16::MAX
            || realization_count == 0
            || SIGN == 0
            || SIGN > usize::from(u16::MAX)
        {
            return Err(PoolError::InvalidContract);
        }
        Ok(Self {
            pool,
            maximum_members,
            authority,
            realization_count,
            slots: [MemberSlot::EMPTY; SLOTS],
            signs: [None; SIGN],
            sign_len: 0,
            next_sequence: 0,
        })
    }

    pub fn population(&self) -> u16 {
        self.slots[..usize::from(self.maximum_members)]
            .iter()
            .filter(|slot| slot.state != MemberState::Empty)
            .count() as u16
    }

    pub fn active_population(&self) -> u16 {
        self.slots[..usize::from(self.maximum_members)]
            .iter()
            .filter(|slot| slot.state == MemberState::Active)
            .count() as u16
    }

    pub fn signs(&self) -> impl Iterator<Item = PoolSign> + '_ {
        self.signs.iter().copied().flatten()
    }

    pub fn admit(
        &mut self,
        key: MemberKey,
        placement: MemberPlacement,
        authority: u16,
    ) -> Result<MemberIdentity, PoolError> {
        if authority != self.authority {
            self.record_denial(key, PoolSignReason::AuthorityDenied)?;
            return Err(PoolError::AuthorityDenied);
        }
        if placement.realization >= self.realization_count {
            self.record_denial(key, PoolSignReason::RealizationDenied)?;
            return Err(PoolError::RealizationDenied);
        }
        if self.slots[..usize::from(self.maximum_members)]
            .iter()
            .any(|slot| slot.state != MemberState::Empty && slot.key == key)
        {
            self.record_denial(key, PoolSignReason::DuplicateKey)?;
            return Err(PoolError::DuplicateKey);
        }
        let Some(index) = self.slots[..usize::from(self.maximum_members)]
            .iter()
            .position(|slot| slot.state == MemberState::Empty)
        else {
            self.record_denial(key, PoolSignReason::PoolFull)?;
            return Err(PoolError::PoolFull);
        };
        let epoch = self.slots[index]
            .epoch
            .checked_add(1)
            .ok_or(PoolError::InvalidLifecycle)?;
        self.record(
            key,
            Some(index as u16),
            Some(epoch),
            Some(placement),
            MemberState::Empty,
            MemberState::Preparing,
            PoolSignReason::Admitted,
        )?;
        self.slots[index] = MemberSlot {
            key,
            epoch,
            state: MemberState::Preparing,
            placement,
        };
        Ok(self.slots[index].identity(self.pool, index as u16))
    }

    pub fn trigger(&mut self, member: MemberIdentity) -> Result<(), PoolError> {
        self.transition(
            member,
            MemberState::Preparing,
            MemberState::Active,
            PoolSignReason::PlayStarted,
        )
    }

    pub fn fail_preparation(&mut self, member: MemberIdentity) -> Result<(), PoolError> {
        self.transition(
            member,
            MemberState::Preparing,
            MemberState::Empty,
            PoolSignReason::PreparationFailed,
        )
    }

    pub fn request_release(&mut self, member: MemberIdentity) -> Result<(), PoolError> {
        self.transition(
            member,
            MemberState::Active,
            MemberState::Releasing,
            PoolSignReason::ReleaseRequested,
        )
    }

    pub fn fail_member(&mut self, member: MemberIdentity) -> Result<(), PoolError> {
        self.transition(
            member,
            MemberState::Active,
            MemberState::Releasing,
            PoolSignReason::MemberFailed,
        )
    }

    pub fn complete_release(&mut self, member: MemberIdentity) -> Result<(), PoolError> {
        self.transition(
            member,
            MemberState::Releasing,
            MemberState::Empty,
            PoolSignReason::Released,
        )
    }

    pub fn member_for_key(&self, key: MemberKey) -> Result<MemberIdentity, PoolError> {
        let Some((index, slot)) = self.slots[..usize::from(self.maximum_members)]
            .iter()
            .enumerate()
            .find(|(_, slot)| slot.key == key)
        else {
            return Err(PoolError::UnknownKey);
        };
        if slot.state != MemberState::Active {
            return Err(PoolError::StaleMember);
        }
        Ok(slot.identity(self.pool, index as u16))
    }

    pub fn validate_active(&self, member: MemberIdentity) -> Result<(), PoolError> {
        let slot = self.exact_slot(member)?;
        if slot.state != MemberState::Active {
            return Err(PoolError::InvalidLifecycle);
        }
        Ok(())
    }

    /// Capture the current active population once in deterministic key order.
    pub fn snapshot_active(&self, output: &mut [MemberIdentity]) -> Result<usize, PoolError> {
        let required = usize::from(self.active_population());
        if output.len() < required {
            return Err(PoolError::SnapshotTooSmall);
        }
        let mut len = 0;
        for (index, slot) in self.slots[..usize::from(self.maximum_members)]
            .iter()
            .enumerate()
        {
            if slot.state != MemberState::Active {
                continue;
            }
            let member = slot.identity(self.pool, index as u16);
            let insertion = output[..len]
                .iter()
                .position(|prior| prior.key > member.key)
                .unwrap_or(len);
            output.copy_within(insertion..len, insertion + 1);
            output[insertion] = member;
            len += 1;
        }
        Ok(len)
    }

    fn exact_slot(&self, member: MemberIdentity) -> Result<MemberSlot, PoolError> {
        if member.pool != self.pool || usize::from(member.slot) >= usize::from(self.maximum_members)
        {
            return Err(PoolError::StaleMember);
        }
        let slot = self.slots[usize::from(member.slot)];
        if slot.epoch != member.epoch
            || slot.key != member.key
            || slot.placement != member.placement
        {
            return Err(PoolError::StaleMember);
        }
        Ok(slot)
    }

    fn transition(
        &mut self,
        member: MemberIdentity,
        from: MemberState,
        to: MemberState,
        reason: PoolSignReason,
    ) -> Result<(), PoolError> {
        let slot = self.exact_slot(member)?;
        if slot.state != from {
            return Err(PoolError::InvalidLifecycle);
        }
        self.record(
            member.key,
            Some(member.slot),
            Some(member.epoch),
            Some(member.placement),
            from,
            to,
            reason,
        )?;
        self.slots[usize::from(member.slot)].state = to;
        Ok(())
    }

    fn record_denial(&mut self, key: MemberKey, reason: PoolSignReason) -> Result<(), PoolError> {
        self.record(
            key,
            None,
            None,
            None,
            MemberState::Empty,
            MemberState::Empty,
            reason,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &mut self,
        key: MemberKey,
        slot: Option<u16>,
        epoch: Option<u32>,
        placement: Option<MemberPlacement>,
        from: MemberState,
        to: MemberState,
        reason: PoolSignReason,
    ) -> Result<(), PoolError> {
        if usize::from(self.sign_len) >= SIGN {
            return Err(PoolError::SignExhausted);
        }
        let sequence = self.next_sequence;
        let next = sequence.checked_add(1).ok_or(PoolError::SequenceOverflow)?;
        self.signs[usize::from(self.sign_len)] = Some(PoolSign {
            sequence,
            pool: self.pool,
            key,
            slot,
            epoch,
            placement,
            from,
            to,
            reason,
        });
        self.sign_len += 1;
        self.next_sequence = next;
        Ok(())
    }
}
