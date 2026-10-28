//! Portable private profile custody and one immutable selected CNJ1 snapshot.
//! No radio, scan, policy, Play, grant or Body authority is established here.
//! Not wired into native_credentials yet: Root must retain this SAME owner across
//! selected executions and only access it through actual affine source custody.
use conduit_net as n;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Wire(n::KnownNetworkWireRefusal),
    Profile(n::KnownNetworkRefusal),
    Revision,
    UpdateLimit,
    Commit,
    Pending,
    NoSelection,
    StaleSelection,
    Capacity,
    Exhausted,
}
impl From<n::KnownNetworkWireRefusal> for Refusal {
    fn from(value: n::KnownNetworkWireRefusal) -> Self {
        Self::Wire(value)
    }
}
impl From<n::KnownNetworkRefusal> for Refusal {
    fn from(value: n::KnownNetworkRefusal) -> Self {
        Self::Profile(value)
    }
}
struct Secret<const N: usize>([u8; N]);
impl<const N: usize> Drop for Secret<N> {
    fn drop(&mut self) {
        for byte in &mut self.0 {
            // SAFETY: exclusively owned initialized private bytes, never escaped.
            unsafe { core::ptr::write_volatile(byte, 0) }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub profile: n::KnownProfileKey,
    pub security: n::KnownSecurity,
    pub store_revision: u64,
    pub sequence: u64,
    pub wire_bytes: usize,
}
struct Frozen {
    info: Selection,
    wire: Secret<{ n::MAXIMUM_JOIN_INPUT_BYTES as usize }>,
}
/// Neither Clone/Debug/Serialize nor ambient shared access to secret custody.
/// Runtime profile keys reconstructed on load are correlations, never grants.
pub struct Owner {
    profiles: n::KnownNetworkProfiles,
    store_revision: u64,
    updates: u32,
    maximum_updates: u32,
    next_selection: u64,
    frozen: Option<Frozen>,
}
impl Owner {
    /// First-Boot empty storage only; do not recreate to reset update/selection
    /// counters or policy. Root must independently preserve mailbox limits.
    pub fn empty(maximum_updates: u32) -> Self {
        Self {
            profiles: n::KnownNetworkProfiles::new(),
            store_revision: 0,
            updates: 0,
            maximum_updates,
            next_selection: 0,
            frozen: None,
        }
    }
    /// Load one readback-committed CKN set or exact legacy CNJ1 record ONCE at
    /// Boot. Legacy migration is in RAM; the next explicit update writes CKN.
    /// Caller owns/wipes input backing; a decode refusal publishes no owner.
    pub fn load(
        bytes: &[u8],
        store_revision: u64,
        maximum_updates: u32,
    ) -> Result<Self, Refusal> {
        if store_revision == 0 {
            return Err(Refusal::Revision);
        }
        let profiles = n::decode_private_known_networks_or_legacy(bytes)?;
        Ok(Self {
            profiles,
            store_revision,
            updates: 0,
            maximum_updates,
            next_selection: 0,
            frozen: None,
        })
    }
    pub fn profiles(&self) -> &n::KnownNetworkProfiles {
        &self.profiles
    }
    pub fn store_revision(&self) -> u64 {
        self.store_revision
    }
    pub fn accepted_updates(&self) -> u32 {
        self.updates
    }
    pub fn selection(&self) -> Option<Selection> {
        self.frozen.as_ref().map(|f| f.info)
    }
    /// CALLER ACKNOWLEDGEMENT BOUNDARY: construct input only within the real
    /// mailbox's acknowledged preparation callback; this method cannot prove
    /// HTTP flush. It neither consumes nor resets that external mailbox.
    /// Commit callback must retain these EXACT complete bytes in its private
    /// store and return the next store revision. Persistent adapters must write
    /// and read back committed media. Legacy volatile adapters retain only RAM
    /// custody: their store revision MUST NOT be projected as durable custody.
    /// This callback must not publish execution owners or authorize effects.
    /// On refusal the prior in-memory set/revision/counters remain unchanged;
    /// ambiguous storage failure recovery remains the durable adapter's duty.
    /// Callback's borrow cannot outlive private scratch. All temporary candidates
    /// and input erase on refusal. Pending selection forbids profile mutation.
    pub fn upsert_after_acknowledgement(
        &mut self,
        input: n::PrivateNetworkInput,
        commit: impl FnOnce(&[u8]) -> Result<u64, ()>,
    ) -> Result<n::KnownProfileKey, Refusal> {
        if self.frozen.is_some() {
            return Err(Refusal::Pending);
        }
        if self.updates >= self.maximum_updates {
            return Err(Refusal::UpdateLimit);
        }
        let revision = self
            .store_revision
            .checked_add(1)
            .ok_or(Refusal::Exhausted)?;
        let updates = self.updates.checked_add(1).ok_or(Refusal::Exhausted)?;
        // Explicit bounded copy through the private codec; no secret Clone API.
        let mut scratch = Secret([0; n::MAXIMUM_KNOWN_NETWORK_PRIVATE_BYTES]);
        let length = n::encode_private_known_networks(&self.profiles, &mut scratch.0)?;
        let mut candidate = n::decode_private_known_networks(&scratch.0[..length])?;
        // Candidate is only an encoding scratch, NEVER the live replacement:
        // decoding normalizes keys and would reset unchanged policy generations.
        // Preflight the original revision and rebuild only the changed input for
        // infallible publication into the ORIGINAL set after durable commit.
        if self
            .profiles
            .entries()
            .any(|p| p.key.generation == u64::MAX)
        {
            return Err(Refusal::Exhausted);
        }
        let selected = candidate.upsert(input)?;
        let info = candidate.info(selected)?;
        let publication = candidate.with_request(selected, |request| {
            n::PrivateNetworkInput::new(
                request.ssid,
                request.credential,
                info.security,
                info.priority,
            )
        })??;
        let length = n::encode_private_known_networks(&candidate, &mut scratch.0)?;
        if commit(&scratch.0[..length]).map_err(|_| Refusal::Commit)? != revision {
            return Err(Refusal::Revision);
        }
        // Same membership/capacity and validated input as the candidate; original
        // revision exhaustion was checked before invoking external durability.
        let key = self
            .profiles
            .upsert(publication)
            .expect("preflighted private publication");
        self.store_revision = revision;
        self.updates = updates;
        Ok(key)
    }
    /// Freeze before SDK preparation/actual source transfer. A second freeze
    /// cannot replace a pending snapshot. Selection/policy authority is external.
    pub fn freeze(&mut self, key: n::KnownProfileKey) -> Result<Selection, Refusal> {
        if self.frozen.is_some() {
            return Err(Refusal::Pending);
        }
        let sequence = self
            .next_selection
            .checked_add(1)
            .ok_or(Refusal::Exhausted)?;
        let mut wire = Secret([0; n::MAXIMUM_JOIN_INPUT_BYTES as usize]);
        let length = self
            .profiles
            .with_request(key, |request| {
                n::encode_network_join_request(request, &mut wire.0)
            })?
            .map_err(|_| Refusal::Capacity)?;
        let info = Selection {
            profile: key,
            security: self.profiles.info(key)?.security,
            store_revision: self.store_revision,
            sequence,
            wire_bytes: length,
        };
        self.frozen = Some(Frozen { info, wire });
        self.next_selection = sequence;
        Ok(info)
    }
    /// Construct the SDK configuration from this exact immutable snapshot,
    /// rather than re-reading a mailbox or a mutable/newer profile set.
    pub fn with_selected_request<T>(
        &self,
        prepare: impl FnOnce(n::NetworkJoinRequest<'_>) -> T,
    ) -> Result<T, Refusal> {
        let frozen = self.frozen.as_ref().ok_or(Refusal::NoSelection)?;
        let request = n::decode_network_join_request(&frozen.wire.0[..frozen.info.wire_bytes])
            .map_err(|_| Refusal::Capacity)?;
        Ok(prepare(request))
    }
    /// CALLER RETIREMENT BOUNDARY: settle/return the actual native preparation
    /// or source owner and mandatory evidence BEFORE this explicit discard.
    /// Exact Selection correlation is not authority or proof of that retirement.
    /// Stale correlation cannot erase a different pending snapshot. Counters,
    /// private profiles and durable update custody never rewind.
    pub fn retire_unconsumed(&mut self, expected: Selection) -> Result<(), Refusal> {
        let frozen = self.frozen.as_ref().ok_or(Refusal::NoSelection)?;
        if frozen.info != expected {
            return Err(Refusal::StaleSelection);
        }
        drop(self.frozen.take());
        Ok(())
    }
    /// Only the actually selected CredentialSource may call this. Output pressure
    /// leaves snapshot/counters untouched. Success transfers exactly one CNJ1,
    /// then volatile-wipes local backing; caller must wipe its output separately.
    /// This is private data transfer, not permission to invoke the radio.
    pub fn take_selected(&mut self, out: &mut [u8]) -> Result<Selection, Refusal> {
        let frozen = self.frozen.as_ref().ok_or(Refusal::NoSelection)?;
        if out.len() < frozen.info.wire_bytes {
            return Err(Refusal::Capacity);
        }
        out[..frozen.info.wire_bytes].copy_from_slice(&frozen.wire.0[..frozen.info.wire_bytes]);
        let info = frozen.info;
        drop(self.frozen.take());
        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(name: &[u8], pass: &[u8], priority: i8) -> n::PrivateNetworkInput {
        n::PrivateNetworkInput::new(name, pass, n::KnownSecurity::Wpa2Personal, priority).unwrap()
    }
    fn insert(owner: &mut Owner, name: &[u8], pass: &[u8]) -> n::KnownProfileKey {
        let revision = owner.store_revision() + 1;
        owner
            .upsert_after_acknowledgement(input(name, pass, 0), |wire| {
                let decoded = n::decode_private_known_networks(wire).unwrap();
                assert!(!decoded.is_empty());
                Ok(revision)
            })
            .unwrap()
    }
    #[test]
    fn acknowledged_transaction_preserves_other_keys_counts_and_old_state_on_failure() {
        let mut owner = Owner::empty(4);
        let a = insert(&mut owner, b"a", b"public-first-pass");
        let b = insert(&mut owner, b"b", b"public-second-pass");
        assert_eq!(owner.profiles().len(), 2);
        assert_eq!(owner.accepted_updates(), 2);
        assert_eq!(owner.store_revision(), 2);
        assert_eq!(
            owner.upsert_after_acknowledgement(input(b"a", b"public-new-pass", 2), |_| Err(())),
            Err(Refusal::Commit)
        );
        assert_eq!(owner.accepted_updates(), 2);
        assert_eq!(owner.store_revision(), 2);
        assert!(owner.profiles().info(a).is_ok());
        assert!(owner.profiles().info(b).is_ok());
        owner
            .profiles()
            .with_request(a, |r| assert_eq!(r.credential, b"public-first-pass"))
            .unwrap();
        assert_eq!(
            owner.upsert_after_acknowledgement(input(b"a", b"public-new-pass", 2), |_| Ok(99)),
            Err(Refusal::Revision)
        );
        let updated = owner
            .upsert_after_acknowledgement(input(b"a", b"public-new-pass", 2), |wire| {
                let profiles = n::decode_private_known_networks(wire).unwrap();
                assert_eq!(profiles.len(), 2);
                Ok(3)
            })
            .unwrap();
        assert_eq!(updated.slot, a.slot);
        assert!(updated.generation > b.generation);
        assert!(owner.profiles().info(a).is_err());
        assert!(owner.profiles().info(b).is_ok());
        assert_eq!(owner.accepted_updates(), 3);
        assert_eq!(owner.store_revision(), 3);
        let next = owner
            .upsert_after_acknowledgement(input(b"b", b"public-another-pass", 1), |_| Ok(4))
            .unwrap();
        assert_eq!(next.generation, 4);
        assert!(owner.profiles().info(updated).is_ok());
        assert_eq!(owner.accepted_updates(), 4);
    }
    #[test]
    fn exact_frozen_cnj1_drives_sdk_callback_transfers_once_and_pressure_preserves() {
        let mut owner = Owner::empty(2);
        let a = insert(&mut owner, b"a", b"public-first-pass");
        let b = insert(&mut owner, b"b", b"public-second-pass");
        let frozen = owner.freeze(b).unwrap();
        assert_eq!(frozen.profile, b);
        assert_eq!(frozen.store_revision, 2);
        assert_eq!(owner.freeze(a), Err(Refusal::Pending));
        let mut called = false;
        assert_eq!(
            owner.upsert_after_acknowledgement(input(b"a", b"public-new-pass", 0), |_| {
                called = true;
                Ok(3)
            }),
            Err(Refusal::Pending)
        );
        assert!(!called);
        owner
            .with_selected_request(|r| {
                assert_eq!(r.ssid, b"b");
                assert_eq!(r.credential, b"public-second-pass");
            })
            .unwrap();
        let mut small = [0xa5; 1];
        assert_eq!(owner.take_selected(&mut small), Err(Refusal::Capacity));
        assert_eq!(small, [0xa5]);
        assert_eq!(owner.selection(), Some(frozen));
        let refused_preparation = owner.with_selected_request(|_| Err::<(), ()>(())).unwrap();
        assert!(refused_preparation.is_err());
        assert_eq!(owner.selection(), Some(frozen));
        let mut out = Secret([0; 167]);
        assert_eq!(owner.take_selected(&mut out.0).unwrap(), frozen);
        let request = n::decode_network_join_request(&out.0[..frozen.wire_bytes]).unwrap();
        assert_eq!(request.ssid, b"b");
        assert_eq!(request.credential, b"public-second-pass");
        assert_eq!(owner.take_selected(&mut out.0), Err(Refusal::NoSelection));
        let next = owner.freeze(a).unwrap();
        assert_eq!(next.sequence, frozen.sequence + 1);
        assert_eq!(owner.accepted_updates(), 2);
    }
    #[test]
    fn actual_cnj1_migration_ckn_reload_limits_and_stale_keys() {
        let mut wire = Secret([0; 401]);
        let length = n::encode_network_join_request(
            n::NetworkJoinRequest {
                ssid: b"legacy",
                credential: b"public-legacy-pass",
            },
            &mut wire.0,
        )
        .unwrap();
        let mut owner = Owner::load(&wire.0[..length], 7, 1).unwrap();
        assert_eq!(owner.profiles().len(), 1);
        let old = owner.profiles().entries().next().unwrap();
        assert_eq!(old.security, n::KnownSecurity::Wpa2Personal);
        let mut committed = Secret([0; 401]);
        let mut committed_len = 0;
        owner
            .upsert_after_acknowledgement(input(b"second", b"public-second-pass", 0), |wire| {
                assert_eq!(&wire[..3], b"CKN");
                committed.0[..wire.len()].copy_from_slice(wire);
                committed_len = wire.len();
                Ok(8)
            })
            .unwrap();
        assert!(owner.profiles().info(old.key).is_ok());
        let reloaded = Owner::load(&committed.0[..committed_len], 8, 1).unwrap();
        assert_eq!(reloaded.profiles().len(), 2);
        assert_eq!(reloaded.accepted_updates(), 0);
        let mut called = false;
        assert_eq!(
            owner.upsert_after_acknowledgement(input(b"third", b"public-third-pass", 0), |_| {
                called = true;
                Ok(9)
            }),
            Err(Refusal::UpdateLimit)
        );
        assert!(!called);
        assert!(Owner::load(&committed.0[..committed_len], 0, 1).is_err());
        assert!(Owner::load(b"unknown", 1, 1).is_err());
    }
    #[test]
    fn four_profile_capacity_and_counter_exhaustion_do_not_call_commit() {
        let mut owner = Owner::empty(5);
        let a = insert(&mut owner, b"a", b"public-test-pass");
        for name in [b"b", b"c", b"d"] {
            insert(&mut owner, name, b"public-test-pass");
        }
        let mut called = false;
        assert!(
            owner
                .upsert_after_acknowledgement(input(b"e", b"public-test-pass", 0), |_| {
                    called = true;
                    Ok(5)
                })
                .is_err()
        );
        assert!(!called);
        assert_eq!(owner.accepted_updates(), 4);
        assert!(owner.profiles().info(a).is_ok());
        owner.next_selection = u64::MAX;
        assert_eq!(owner.freeze(a), Err(Refusal::Exhausted));
        assert!(owner.selection().is_none());
        owner.store_revision = u64::MAX;
        assert_eq!(
            owner.upsert_after_acknowledgement(input(b"a", b"public-new-pass", 0), |_| {
                called = true;
                Ok(0)
            }),
            Err(Refusal::Exhausted)
        );
        assert!(!called);
    }
    #[test]
    fn exact_unconsumed_retirement_keeps_sequence_profile_and_update_counters() {
        let mut owner = Owner::empty(2);
        let key = insert(&mut owner, b"known", b"public-test-pass");
        let first = owner.freeze(key).unwrap();
        let mut stale = first;
        stale.sequence += 1;
        assert_eq!(owner.retire_unconsumed(stale), Err(Refusal::StaleSelection));
        assert_eq!(owner.selection(), Some(first));
        owner.retire_unconsumed(first).unwrap();
        assert!(owner.selection().is_none());
        assert_eq!(owner.accepted_updates(), 1);
        assert_eq!(owner.store_revision(), 1);
        assert!(owner.profiles().info(key).is_ok());
        let next = owner.freeze(key).unwrap();
        assert_eq!(next.sequence, first.sequence + 1);
        assert_eq!(owner.retire_unconsumed(first), Err(Refusal::StaleSelection));
        assert_eq!(owner.selection(), Some(next));
        owner.retire_unconsumed(next).unwrap();
        assert_eq!(owner.retire_unconsumed(next), Err(Refusal::NoSelection));
    }
}
