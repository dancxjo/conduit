//! Actual private input custody retained by Root before projecting availability.
#[path = "native_credential_journal.rs"]
pub mod journal;
use conduit_core as c;
use conduit_environment_http::network_setup::{
    NetworkSetupChannel, NetworkSetupReadRefusal, NetworkSetupReader, NetworkSetupRefusal,
    NetworkSetupWriter,
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
}
impl RootCredentialSource {
    pub fn generation(&self) -> u64 {
        self.generation
    }
}
impl CredentialSource for RootCredentialSource {
    fn take_acknowledged(&mut self, output: &mut [u8]) -> Result<usize, CredentialSourceRefusal> {
        match self.reader.take_acknowledged(output) {
            Ok(length) => Ok(length),
            Err(NetworkSetupReadRefusal::Input(NetworkSetupRefusal::NoSubmission)) => {
                read_committed(output)
            }
            Err(error) => Err(match error {
                NetworkSetupReadRefusal::Busy => CredentialSourceRefusal::Unavailable,
                NetworkSetupReadRefusal::Input(NetworkSetupRefusal::NotAcknowledged) => {
                    CredentialSourceRefusal::Unacknowledged
                }
                NetworkSetupReadRefusal::Input(NetworkSetupRefusal::NoSubmission) => {
                    CredentialSourceRefusal::Unavailable
                }
                NetworkSetupReadRefusal::Input(NetworkSetupRefusal::Wire(_)) => {
                    CredentialSourceRefusal::OutputCapacity
                }
                _ => CredentialSourceRefusal::Invalid,
            }),
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
struct PrivateWire([u8; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize]);
impl Drop for PrivateWire {
    fn drop(&mut self) {
        self.0.fill(0);
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

fn read_committed(output: &mut [u8]) -> Result<usize, CredentialSourceRefusal> {
    #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
    {
        let owner = durable().ok_or(CredentialSourceRefusal::Unavailable)?;
        let (_, length) = owner
            .lock(|journal| journal.borrow_mut().read(output))
            .map_err(|_| CredentialSourceRefusal::Unavailable)?;
        conduit_net::decode_network_join_request(&output[..length])
            .map_err(|_| CredentialSourceRefusal::Invalid)?;
        Ok(length)
    }
    #[cfg(not(any(feature = "local-evidence-store", feature = "network-credential-store")))]
    {
        let _ = output;
        Err(CredentialSourceRefusal::Unavailable)
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
/// SDK strings and durable private custody are prepared before a selected radio
/// attempt. Only flushed input can replace the journal. Committed input may be
/// reused by a new selected attempt/Boot; its old Play is never resurrected.
pub fn prepare_configuration(
    writer: &NetworkSetupWriter<'static>,
) -> Result<esp_radio::wifi::sta::StationConfig, &'static str> {
    let mailbox = writer.try_lock().map_err(|_| "credential-mailbox-busy")?;
    let acknowledged = mailbox.prepare_acknowledged(|request| {
        let configuration = conduit_esp32_network_switch::prepare_station_configuration(
            conduit_net::NetworkJoinRequest {
                ssid: request.ssid,
                credential: request.credential,
            },
        )
        .map_err(|_| "credential-configuration")?;
        #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
        {
            let mut wire = PrivateWire([0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize]);
            let length = conduit_net::encode_network_join_request(request, &mut wire.0)
                .map_err(|_| "credential-wire")?;
            let journal = durable().ok_or("credential-journal-unavailable")?;
            journal
                .lock(|journal| journal.borrow_mut().commit(&wire.0[..length]))
                .map_err(|_| "credential-journal-commit")?;
        }
        Ok(configuration)
    });
    match acknowledged {
        Ok(prepared) => return prepared,
        Err(NetworkSetupRefusal::NoSubmission) => (),
        Err(_) => return Err("credential-not-acknowledged"),
    }
    drop(mailbox);
    let mut wire = PrivateWire([0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize]);
    let length = read_committed(&mut wire.0).map_err(|_| "credential-unavailable")?;
    let request = conduit_net::decode_network_join_request(&wire.0[..length])
        .map_err(|_| "credential-wire")?;
    conduit_esp32_network_switch::prepare_station_configuration(request)
        .map_err(|_| "credential-configuration")
}
