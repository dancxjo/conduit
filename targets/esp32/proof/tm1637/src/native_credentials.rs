//! Actual private input custody retained by Root before projecting availability.
#[path = "native_credential_backing.rs"]
mod backing;
#[path = "native_credential_configuration.rs"]
mod configuration;
#[path = "native_credential_failover.rs"]
mod failover;
#[path = "native_credential_journal.rs"]
pub mod journal;
#[path = "native_known_credentials.rs"]
mod known;
#[path = "native_credential_selection.rs"]
mod selection;
use alloc::boxed::Box;
use conduit_core as c;
use conduit_environment_http::network_setup::{
    NetworkSetupChannel, NetworkSetupReader, NetworkSetupRefusal, NetworkSetupWriter, Submission,
};
use conduit_network_join_call::{
    credential_installation::ReadyCredentialBase, installation::NetworkNativeIdentity,
    CredentialSource, CredentialSourceRefusal,
};
use static_cell::StaticCell;
static INPUT: StaticCell<NetworkSetupChannel> = StaticCell::new();
pub struct RootCredentialSource {
    reader: NetworkSetupReader<'static>,
    generation: u64,
    // One affine allocation moves between Root and selected execution owners.
    // Inline profile arrays would multiply every dormant owner-slot reservation.
    owner: Option<Box<PrivateProfiles>>,
}
// Correlations travel with the same affine allocation as the private profiles.
// Dormant execution slots retain only its pointer, not another copy of them.
struct PrivateProfiles {
    profiles: known::Owner,
    chosen: Option<conduit_net::KnownProfileKey>,
    fallback: failover::Fallback,
    // A cached/retry selection never consumes a later AP submission.
    frozen_origin: Option<Submission>,
    // Preserve exact acknowledged correlation through unused-snapshot retirement.
    committed_pending: Option<(Submission, conduit_net::KnownProfileKey)>,
}
impl PrivateProfiles {
    fn new(profiles: known::Owner) -> Self {
        Self {
            profiles,
            chosen: None,
            fallback: failover::Fallback::new(),
            frozen_origin: None,
            committed_pending: None,
        }
    }
}
impl core::ops::Deref for PrivateProfiles {
    type Target = known::Owner;
    fn deref(&self) -> &Self::Target {
        &self.profiles
    }
}
impl core::ops::DerefMut for PrivateProfiles {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.profiles
    }
}
impl RootCredentialSource {
    pub fn generation(&self) -> u64 {
        self.generation
    }
}
impl CredentialSource for RootCredentialSource {
    fn take_acknowledged(&mut self, output: &mut [u8]) -> Result<usize, CredentialSourceRefusal> {
        let owner = self
            .owner
            .as_mut()
            .ok_or(CredentialSourceRefusal::Unavailable)?;
        let selected = owner
            .selection()
            .ok_or(CredentialSourceRefusal::Unavailable)?;
        if !owner.fallback.validates(selected, self.generation) {
            return Err(CredentialSourceRefusal::Invalid);
        }
        if output.len() < selected.wire_bytes {
            return Err(CredentialSourceRefusal::OutputCapacity);
        }
        if let Some(origin) = owner.frozen_origin {
            let mut mailbox = self
                .reader
                .try_lock()
                .map_err(|_| CredentialSourceRefusal::Unavailable)?;
            if mailbox
                .acknowledged_submission()
                .map_err(|_| CredentialSourceRefusal::Unacknowledged)?
                != origin
            {
                return Err(CredentialSourceRefusal::Invalid);
            }
            let matches = mailbox
                .prepare_acknowledged(|raw| {
                    owner.with_selected_request(|frozen| {
                        raw.ssid == frozen.ssid && raw.credential == frozen.credential
                    })
                })
                .map_err(|_| CredentialSourceRefusal::Unacknowledged)?
                .map_err(|_| CredentialSourceRefusal::Invalid)?;
            if !matches {
                return Err(CredentialSourceRefusal::Invalid);
            }
            // Exact token, output capacity and immutable selected content were
            // checked while holding this same mailbox guard. No intervening
            // writer can replace/cancel this acknowledged request.
            let mut expected = PrivateWire([0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize]);
            let expected_len = owner
                .with_selected_request(|request| {
                    conduit_net::encode_network_join_request(request, &mut expected.0)
                })
                .map_err(|_| CredentialSourceRefusal::Invalid)?
                .map_err(|_| CredentialSourceRefusal::Invalid)?;
            if expected_len != selected.wire_bytes {
                return Err(CredentialSourceRefusal::Invalid);
            }
            let mut consumed = PrivateWire([0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize]);
            let length = mailbox
                .take_acknowledged(&mut consumed.0)
                .map_err(|_| CredentialSourceRefusal::Invalid)?;
            // Invariant after exact preflight under exclusive mailbox custody.
            assert!(length == expected_len && consumed.0[..length] == expected.0[..length]);
            let transferred = owner
                .take_selected(output)
                .expect("preflighted selected private transfer");
            owner.frozen_origin = None;
            owner.committed_pending = None;
            owner
                .fallback
                .taken(transferred)
                .map_err(|_| CredentialSourceRefusal::Invalid)?;
            Ok(transferred.wire_bytes)
        } else {
            // A new AP request belongs to the NEXT preparation. Do not lock,
            // consume or refuse it while delivering this cached/retry selection.
            let transferred = owner
                .take_selected(output)
                .map_err(|_| CredentialSourceRefusal::Invalid)?;
            owner
                .fallback
                .taken(transferred)
                .map_err(|_| CredentialSourceRefusal::Invalid)?;
            Ok(transferred.wire_bytes)
        }
    }
}
pub fn prepare(
    host: &c::HostId,
    boot: &c::BootId,
) -> (
    ReadyCredentialBase<RootCredentialSource>,
    NetworkSetupWriter<'static>,
) {
    // Mailbox revisions, saved-profile policy, mandatory journal capacity and
    // the service owner's 32 commands impose independent finite bounds.
    // No profile choice renews a grant, command budget or recovery generation.
    let (writer, reader) = INPUT.init(NetworkSetupChannel::new(2)).split();
    let identity = identity(host, boot, 1);
    // SAFETY: Root initializes one actual fixed-capacity volatile mailbox and
    // retains its unique non-Clone reader here. The source only try-locks and
    // encodes at most MAXIMUM_JOIN_INPUT_BYTES; successful mailbox transfer
    // consumes and erases that input. With durable custody enabled, a fresh
    // selected read may replay the committed configuration under this Boot's
    // actual source owner. No socket route or radio authority is inferred.
    let ready = unsafe {
        ReadyCredentialBase::new(
            identity,
            RootCredentialSource {
                reader,
                generation: 1,
                owner: None,
            },
        )
    }
    .expect("actual private source must prepare");
    (ready, writer)
}

fn identity(host: &c::HostId, boot: &c::BootId, generation: u64) -> NetworkNativeIdentity {
    crate::native_network_identity::prepare(
        crate::native_network_identity::Role::Credentials,
        host,
        boot,
        generation,
    )
    .expect("initial Root credential identity backing")
}

/// Reissue readiness only for the actual returned private source, advancing its
/// native generation without resetting the mailbox or its submission counter.
pub fn reprepare(
    mut source: RootCredentialSource,
    host: &c::HostId,
    boot: &c::BootId,
) -> Result<ReadyCredentialBase<RootCredentialSource>, RootCredentialSource> {
    let Some(generation) = source.generation.checked_add(1) else {
        return Err(source);
    };
    if host.as_str().is_empty() || boot.as_str().is_empty() {
        return Err(source);
    }
    let identity = match crate::native_network_identity::prepare(
        crate::native_network_identity::Role::Credentials,
        host,
        boot,
        generation,
    ) {
        Ok(identity) => identity,
        Err(()) => return Err(source),
    };
    // This entrance is called only after actual provider/source return and
    // mandatory evidence/issuer retirement in finish_network_retirement.
    if let Some(owner) = source.owner.as_mut() {
        if let Some(selection) = owner.selection() {
            if owner.retire_unconsumed(selection).is_err() {
                return Err(source);
            }
        }
        // Retain the exact pending token/key; unused preparation retry must not
        // commit or count this acknowledged input twice.
        owner.frozen_origin = None;
        owner.chosen = None;
    }
    source.generation = generation;
    // SAFETY: source is the actual non-Clone reader returned after capability
    // revocation. Root retains its backing and separately admits fresh selection.
    Ok(unsafe { ReadyCredentialBase::new(identity, source) }.expect("checked current identity"))
}

#[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
type Durable = embassy_sync::blocking_mutex::Mutex<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    core::cell::RefCell<journal::Journal<crate::native_evidence_region::CredentialRegion>>,
>;
#[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
static DURABLE: StaticCell<Durable> = StaticCell::new();
#[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
static DURABLE_PTR: core::sync::atomic::AtomicPtr<Durable> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());
struct PrivateWire<const N: usize>([u8; N]);
impl<const N: usize> Drop for PrivateWire<N> {
    fn drop(&mut self) {
        for byte in &mut self.0 {
            // SAFETY: exclusively owned initialized private scratch.
            unsafe { core::ptr::write_volatile(byte, 0) }
        }
    }
}
#[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
pub fn initialize_journal(
    region: crate::native_evidence_region::CredentialRegion,
) -> Result<(), &'static str> {
    let journal = journal::Journal::open(region).map_err(|_| "credential-journal-open")?;
    let owner = DURABLE
        .try_init(Durable::new(core::cell::RefCell::new(journal)))
        .ok_or("credential-journal-already-owned")?;
    DURABLE_PTR.store(owner, core::sync::atomic::Ordering::Release);
    Ok(())
}
#[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
fn durable() -> Option<&'static Durable> {
    let pointer = DURABLE_PTR.load(core::sync::atomic::Ordering::Acquire);
    if pointer.is_null() {
        None
    } else {
        // SAFETY: pointer is published once after StaticCell initialization and
        // remains valid forever; all access to the journal uses its mutex.
        Some(unsafe { &*pointer })
    }
}

impl RootCredentialSource {
    fn ensure_owner(&mut self) -> Result<(), &'static str> {
        if self.owner.is_some() {
            return Ok(());
        }
        #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
        {
            let journal = durable().ok_or("credential-journal-unavailable")?;
            let mut wire = PrivateWire([0; conduit_net::MAXIMUM_KNOWN_NETWORK_PRIVATE_BYTES]);
            let loaded = journal.lock(|journal| journal.borrow_mut().read(&mut wire.0));
            self.owner = Some(
                backing::try_box(PrivateProfiles::new(match loaded {
                    Ok((revision, length)) => known::Owner::load(&wire.0[..length], revision, 2)
                        .map_err(|_| "credential-profile-wire")?,
                    Err(journal::Refusal::Empty) => known::Owner::empty(2),
                    Err(_) => return Err("credential-journal-read"),
                }))
                .map_err(|_| "credential-owner-capacity")?,
            );
            Ok(())
        }
        #[cfg(not(any(feature = "local-evidence-store", feature = "network-credential-store")))]
        {
            // Existing nonpersistent feature: one retained volatile private
            // owner, finite mailbox/update limits, no flash/persistence claim.
            self.owner = Some(
                backing::try_box(PrivateProfiles::new(known::Owner::empty(2)))
                    .map_err(|_| "credential-owner-capacity")?,
            );
            Ok(())
        }
    }
}
pub fn committed_revision() -> u64 {
    #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
    {
        durable().map_or(0, |owner| owner.lock(|journal| journal.borrow().revision()))
    }
    #[cfg(not(any(feature = "local-evidence-store", feature = "network-credential-store")))]
    {
        0
    }
}
pub fn has_committed_configuration() -> bool {
    #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
    {
        durable().is_some_and(|owner| owner.lock(|journal| journal.borrow().revision() != 0))
    }
    #[cfg(not(any(feature = "local-evidence-store", feature = "network-credential-store")))]
    {
        false
    }
}
/// Temporarily unpack ONLY the actual unbound Ready owner. Restore the exact
/// same native identity/source on every preparation success/refusal. No new
/// generation, grant, mailbox, policy or attempt budget is manufactured here.
pub fn prepare_configuration(
    slot: &mut Option<ReadyCredentialBase<RootCredentialSource>>,
) -> Result<esp_radio::wifi::sta::StationConfig, &'static str> {
    with_unbound_source(slot, RootCredentialSource::prepare_configuration)
}
/// Prepare only the retained saved set after actual unselected owner return.
/// A later AP submission is neither locked nor consumed here.
pub fn prepare_retry(
    slot: &mut Option<ReadyCredentialBase<RootCredentialSource>>,
) -> Result<esp_radio::wifi::sta::StationConfig, &'static str> {
    with_unbound_source(slot, RootCredentialSource::prepare_saved_retry)
}
/// Root calls only after actual AP reclaim and finish_scan_play. Preserve the
/// same unbound private source, all generations and the original pending input.
#[cfg(feature = "network-scan")]
pub(crate) fn renew_after_scan(
    slot: &mut Option<ReadyCredentialBase<RootCredentialSource>>,
    committed: &conduit_network_join_call::scan::CommittedScanEvidence,
    now: u64,
) -> Result<(), &'static str> {
    let native = slot
        .as_ref()
        .ok_or("credential-owner-unavailable")?
        .native_identity();
    let scope = &committed.evidence().inspection().scope;
    if scope.host_id != native.host || scope.boot_id != native.boot {
        return Err("credential-scan-foreign-scope");
    }
    with_unbound_source(slot, |source| {
        let owner = source
            .owner
            .as_deref_mut()
            .ok_or("credential-owner-unavailable")?;
        if owner.selection().is_some()
            || owner.frozen_origin.is_some()
            || owner.committed_pending.is_some()
        {
            return Err("credential-scan-selection-pending");
        }
        let key = owner
            .fallback
            .renew_committed_scan(owner.profiles.profiles(), committed, now)?;
        owner.chosen = Some(key);
        Ok(())
    })
}
fn with_unbound_source<T>(
    slot: &mut Option<ReadyCredentialBase<RootCredentialSource>>,
    prepare: impl FnOnce(&mut RootCredentialSource) -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    let ready = slot.take().ok_or("credential-owner-unavailable")?;
    let (identity, mut source) = ready.into_unbound_parts();
    let result = prepare(&mut source);
    // SAFETY: exact initialized unbound owner and identity just unpacked, with
    // actual private custody and all counters retained, before selected binding.
    *slot = Some(
        unsafe { ReadyCredentialBase::new(identity, source) }
            .expect("same unbound credential identity"),
    );
    result
}

impl RootCredentialSource {
    pub(crate) fn record_retired_outcome(
        &mut self,
        revision: u64,
        outcome: Option<(bool, u64)>,
        now: u64,
    ) -> Result<(), &'static str> {
        let owner = self.owner.as_mut().ok_or("credential-owner-unavailable")?;
        owner
            .fallback
            .settled(self.generation, revision, outcome, now)
    }
}
