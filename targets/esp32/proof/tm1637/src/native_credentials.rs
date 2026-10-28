//! Actual private input custody retained by Root before projecting availability.
#[path = "native_credential_journal.rs"]
pub mod journal;
#[path = "native_known_credentials.rs"]
mod known;
use alloc::boxed::Box;
use conduit_core as c;
use conduit_environment_http::network_setup::{
    NetworkSetupChannel, NetworkSetupReader, NetworkSetupRefusal, NetworkSetupWriter, Submission,
};
use conduit_network_join_call::{
    CredentialSource, CredentialSourceRefusal, credential_installation::ReadyCredentialBase,
    installation::NetworkNativeIdentity,
};
use static_cell::StaticCell;
static INPUT: StaticCell<NetworkSetupChannel> = StaticCell::new();
pub struct RootCredentialSource {
    reader: NetworkSetupReader<'static>,
    generation: u64,
    // One affine allocation moves between Root and selected execution owners.
    // Inline profile arrays would multiply every dormant owner-slot reservation.
    owner: Option<Box<known::Owner>>,
    chosen: Option<conduit_net::KnownProfileKey>,
    // A cached/retry selection never consumes a later AP submission.
    frozen_origin: Option<Submission>,
    // Preserve exact acknowledged correlation through unused-snapshot retirement.
    committed_pending: Option<(Submission, conduit_net::KnownProfileKey)>,
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
        if output.len() < selected.wire_bytes {
            return Err(CredentialSourceRefusal::OutputCapacity);
        }
        if let Some(origin) = self.frozen_origin {
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
            self.frozen_origin = None;
            self.committed_pending = None;
            Ok(transferred.wire_bytes)
        } else {
            // A new AP request belongs to the NEXT preparation. Do not lock,
            // consume or refuse it while delivering this cached/retry selection.
            let transferred = owner
                .take_selected(output)
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
    // Two explicit page revisions plus one retained Boot configuration admit
    // at most nine finite attempts within the service owner's 32 commands.
    // Counters never rewind; journal capacity independently refuses exhaustion.
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
                chosen: None,
                frozen_origin: None,
                committed_pending: None,
            },
        )
    }
    .expect("actual private source must prepare");
    (ready, writer)
}

fn identity(host: &c::HostId, boot: &c::BootId, generation: u64) -> NetworkNativeIdentity {
    NetworkNativeIdentity {
        host: host.clone(),
        boot: boot.clone(),
        base: "base/esp32/private-network-input".into(),
        instance: "instance/esp32/private-network-input".into(),
        provider_generation: generation,
        resource: "pool/esp32/private-network-input".into(),
        resource_generation: c::ResourceGenerationId(alloc::format!(
            "generation/private-network-input/{generation}"
        )),
        envelope: "envelope/esp32/private-network-input".into(),
        artifact: "native/esp32-private-network-input-root@1".into(),
        implementation: "esp32/private-network-input@1".into(),
        base_implementation: "esp32/private-network-input-base@1".into(),
    }
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
    // This entrance is called only after actual provider/source return and
    // mandatory evidence/issuer retirement in finish_network_retirement.
    if let Some(owner) = source.owner.as_mut() {
        if let Some(selection) = owner.selection() {
            if owner.retire_unconsumed(selection).is_err() {
                return Err(source);
            }
        }
    }
    // The acknowledged input remains pending; committed_pending preserves its
    // exact token/key so unused preparation retry does not commit/count it twice.
    source.frozen_origin = None;
    source.generation = generation;
    // SAFETY: source is the actual non-Clone reader returned after capability
    // revocation. Root retains its backing and separately admits fresh selection.
    Ok(
        unsafe { ReadyCredentialBase::new(identity(host, boot, generation), source) }
            .expect("checked current identity"),
    )
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
            self.owner = Some(Box::new(match loaded {
                Ok((revision, length)) => known::Owner::load(&wire.0[..length], revision, 2)
                    .map_err(|_| "credential-profile-wire")?,
                Err(journal::Refusal::Empty) => known::Owner::empty(2),
                Err(_) => return Err("credential-journal-read"),
            }));
            Ok(())
        }
        #[cfg(not(any(feature = "local-evidence-store", feature = "network-credential-store")))]
        {
            // Existing nonpersistent feature: one retained volatile private
            // owner, finite mailbox/update limits, no flash/persistence claim.
            self.owner = Some(Box::new(known::Owner::empty(2)));
            Ok(())
        }
    }
    fn prepare_retry(&mut self) -> Result<(), &'static str> {
        let owner = self.owner.as_mut().ok_or("credential-owner-unavailable")?;
        if owner.selection().is_some() {
            return Ok(());
        }
        let key = self.chosen.ok_or("credential-selection")?;
        if owner
            .profiles()
            .info(key)
            .map_err(|_| "credential-profile-key")?
            .security
            != conduit_net::KnownSecurity::Wpa2Personal
        {
            return Err("credential-auth-unsupported");
        }
        // Preserve the SDK's SAME chosen profile. New AP input belongs to a
        // later AP-only configuration, never silently changes a Waiting retry.
        let origin = if let Some((token, pending_key)) = self
            .committed_pending
            .filter(|(_, pending_key)| *pending_key == key)
        {
            let _ = pending_key;
            let mailbox = self
                .reader
                .try_lock()
                .map_err(|_| "credential-mailbox-busy")?;
            (mailbox.acknowledged_submission().ok() == Some(token)).then_some(token)
        } else {
            None
        };
        owner.freeze(key).map_err(|_| "credential-selection")?;
        self.frozen_origin = origin;
        Ok(())
    }
    fn prepare_configuration(
        &mut self,
    ) -> Result<esp_radio::wifi::sta::StationConfig, &'static str> {
        self.ensure_owner()?;
        let owner = self.owner.as_mut().expect("loaded private owner");
        if owner.selection().is_none() {
            let mailbox = self
                .reader
                .try_lock()
                .map_err(|_| "credential-mailbox-busy")?;
            let (key, origin) = match mailbox.acknowledged_submission() {
                Ok(submission) => {
                    let key = if let Some((_, key)) = self
                        .committed_pending
                        .filter(|(token, _)| *token == submission)
                    {
                        key
                    } else {
                        let key = mailbox
                            .prepare_acknowledged(|request| {
                                let input = conduit_net::PrivateNetworkInput::new(
                                    request.ssid,
                                    request.credential,
                                    conduit_net::KnownSecurity::Wpa2Personal,
                                    0,
                                )
                                .map_err(|_| "credential-configuration")?;
                                #[cfg(not(any(
                                    feature = "local-evidence-store",
                                    feature = "network-credential-store"
                                )))]
                                let volatile_revision = owner
                                    .store_revision()
                                    .checked_add(1)
                                    .ok_or("credential-configuration-exhausted")?;
                                owner
                                    .upsert_after_acknowledgement(input, |wire| {
                                        #[cfg(any(
                                            feature = "local-evidence-store",
                                            feature = "network-credential-store"
                                        ))]
                                        {
                                            durable()
                                                .ok_or(())?
                                                .lock(|journal| journal.borrow_mut().commit(wire))
                                                .map_err(|_| ())
                                        }
                                        #[cfg(not(any(
                                            feature = "local-evidence-store",
                                            feature = "network-credential-store"
                                        )))]
                                        {
                                            // Finite within-Boot private acceptance only.
                                            // This feature does not persist or advertise
                                            // a committed flash configuration; public
                                            // committed_revision remains zero.
                                            let _ = wire;
                                            Ok(volatile_revision)
                                        }
                                    })
                                    .map_err(|_| "credential-journal-commit")
                            })
                            .map_err(|_| "credential-not-acknowledged")??;
                        self.committed_pending = Some((submission, key));
                        key
                    };
                    (key, Some(submission))
                }
                Err(NetworkSetupRefusal::NoSubmission) => {
                    // Until actual scan/policy integration, preserve the last
                    // chosen supported profile within this Boot; first Boot
                    // deterministically selects the first supported WPA2 entry.
                    // This is NOT a best-network/RSSI or Body-peer decision.
                    let key = self
                        .chosen
                        .filter(|key| {
                            owner.profiles().info(*key).is_ok_and(|p| {
                                p.security == conduit_net::KnownSecurity::Wpa2Personal
                            })
                        })
                        .or_else(|| {
                            owner
                                .profiles()
                                .entries()
                                .find(|p| p.security == conduit_net::KnownSecurity::Wpa2Personal)
                                .map(|p| p.key)
                        })
                        .ok_or("credential-unavailable")?;
                    (key, None)
                }
                Err(_) => return Err("credential-not-acknowledged"),
            };
            if owner
                .profiles()
                .info(key)
                .map_err(|_| "credential-profile-key")?
                .security
                != conduit_net::KnownSecurity::Wpa2Personal
            {
                return Err("credential-auth-unsupported");
            }
            owner.freeze(key).map_err(|_| "credential-selection")?;
            self.chosen = Some(key);
            self.frozen_origin = origin;
        }
        if owner.selection().ok_or("credential-selection")?.security
            != conduit_net::KnownSecurity::Wpa2Personal
        {
            return Err("credential-auth-unsupported");
        }
        // SDK refusal deliberately retains the frozen snapshot for explicit
        // returned-owner retirement/retry, never silently clears private custody.
        owner
            .with_selected_request(conduit_esp32_network_switch::prepare_station_configuration)
            .map_err(|_| "credential-selection")?
            .map_err(|_| "credential-configuration")
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
/// Re-freeze only the retained chosen input for the existing staged SDK config.
/// Called before reserving a Waiting retry, never merges a newly queued AP input.
pub fn prepare_retry(
    slot: &mut Option<ReadyCredentialBase<RootCredentialSource>>,
) -> Result<(), &'static str> {
    with_unbound_source(slot, RootCredentialSource::prepare_retry)
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
