//! Explicit CPU substitutes for hardware entropy, Boot and radio. These are
//! never linked into firmware and establish no physical Root or attachment.
use super::*;
#[derive(Debug)]
pub struct EntropyUnavailable;
thread_local! { static ENTROPY_MODE:Cell<u8>=const {Cell::new(0)}; }
pub(crate) fn entropy_mode(mode: u8) {
    ENTROPY_MODE.with(|v| v.set(mode));
}
pub struct Trng;
impl Trng {
    pub fn try_new() -> Result<Self, EntropyUnavailable> {
        if ENTROPY_MODE.with(|v| v.get()) == 1 {
            Err(EntropyUnavailable)
        } else {
            Ok(Self)
        }
    }
    pub fn read(&self, b: &mut [u8]) {
        b.fill(if ENTROPY_MODE.with(|v| v.get()) == 2 {
            0
        } else {
            0x55
        });
    }
}
thread_local! { static SCOPE_MODE:Cell<u8>=const {Cell::new(0)}; }
pub(crate) fn scope_mode(mode: u8) {
    SCOPE_MODE.with(|value| value.set(mode));
}
pub struct ObservedBoot;
impl ObservedBoot {
    pub fn ids(&self) -> (&str, &str) {
        match SCOPE_MODE.with(Cell::get) {
            1 => ("host/foreign", "boot/current"),
            2 => ("host/network", "boot/stale"),
            _ => ("host/network", "boot/current"),
        }
    }
}
#[derive(Default)]
pub(crate) struct State {
    pub reads: Cell<u32>,
    pub joins: Cell<u32>,
    pub cleanups: Cell<u32>,
    pub failure: Cell<Option<net::StationFailure>>,
    pub cleanup_failure: Cell<bool>,
}
pub(crate) struct Provider(pub Rc<State>);
impl n::NetworkJoinProvider for Provider {
    async fn join(
        &mut self,
        request: net::NetworkJoinRequest<'_>,
    ) -> Result<n::AttachmentRecord, net::StationAttemptFailure> {
        assert_eq!(request.ssid, b"fixture-network");
        assert_eq!(request.credential, b"fixture-password");
        self.0.joins.set(self.0.joins.get() + 1);
        if let Some(reason) = self.0.failure.get() {
            return Err(net::StationAttemptFailure {
                phase: net::StationPhase::Associate,
                reason,
            });
        }
        Ok(n::AttachmentRecord::encode(net::NetworkAttachmentInfo {
            attachment_id: "fixture/attachment",
            host_id: "host/network",
            boot_id: "boot/current",
            resource_pool_id: "pool/radio",
            generation: 1,
        })
        .unwrap())
    }
    async fn cleanup(&mut self) -> Result<(), net::StationFailure> {
        self.0.cleanups.set(self.0.cleanups.get() + 1);
        if self.0.cleanup_failure.get() {
            Err(net::StationFailure::ProviderUnavailable)
        } else {
            Ok(())
        }
    }
}
pub(crate) struct RootCredentialSource {
    pub mailbox: NetworkSetupMailbox,
    pub state: Rc<State>,
}
impl n::CredentialSource for RootCredentialSource {
    fn take_acknowledged(&mut self, out: &mut [u8]) -> Result<usize, n::CredentialSourceRefusal> {
        self.state.reads.set(self.state.reads.get() + 1);
        self.mailbox
            .take_acknowledged(out)
            .map_err(|_| n::CredentialSourceRefusal::Unacknowledged)
    }
}
pub(crate) fn identity(join: bool) -> NetworkNativeIdentity {
    NetworkNativeIdentity {
        host: "host/network".into(),
        boot: "boot/current".into(),
        base: if join { "base/join" } else { "base/source" }.into(),
        instance: if join {
            "instance/join"
        } else {
            "instance/source"
        }
        .into(),
        provider_generation: 1,
        resource: if join { "pool/radio" } else { "pool/input" }.into(),
        resource_generation: c::ResourceGenerationId("generation/1".into()),
        envelope: "envelope/1".into(),
        artifact: if join {
            "artifact/join"
        } else {
            "artifact/source"
        }
        .into(),
        implementation: if join { "impl/join" } else { "impl/source" }.into(),
        base_implementation: if join { "base/join@1" } else { "base/source@1" }.into(),
    }
}
pub(crate) fn ready(
    state: Rc<State>,
    ack: bool,
) -> (
    ReadyCredentialBase<RootCredentialSource>,
    ReadyNetworkJoinBase<Provider>,
) {
    let mut mailbox = NetworkSetupMailbox::new(1);
    let id = mailbox
        .submit(
            NetworkSetupInput::from_form(
                b"ssid=fixture-network&password=fixture-password",
                NetworkSetupLimits {
                    minimum_credential_bytes: 8,
                    maximum_credential_bytes: 63,
                },
            )
            .unwrap(),
        )
        .unwrap();
    if ack {
        mailbox.acknowledge(id).unwrap();
    }
    // SAFETY: CPU fixtures only, with the real bounded private mailbox. Hardware
    // initialization and Root authority are explicitly substitutes in this test.
    unsafe {
        (
            ReadyCredentialBase::new(
                identity(false),
                RootCredentialSource {
                    mailbox,
                    state: state.clone(),
                },
            )
            .unwrap(),
            ReadyNetworkJoinBase::new(identity(true), 1, Provider(state)).unwrap(),
        )
    }
}
pub(crate) fn host() -> c::HostAdvertisement {
    c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "host/network".into(),
        boot_id: "boot/current".into(),
        offer_generation: c::OfferGeneration(2),
        profile: "fixture/root-control".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
