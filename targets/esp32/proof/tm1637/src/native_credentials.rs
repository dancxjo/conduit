//! Actual private input custody retained by Root before projecting availability.
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
pub struct RootCredentialSource(NetworkSetupReader<'static>);
impl CredentialSource for RootCredentialSource {
    fn take_acknowledged(&mut self, output: &mut [u8]) -> Result<usize, CredentialSourceRefusal> {
        self.0
            .take_acknowledged(output)
            .map_err(|error| match error {
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
            })
    }
}
pub fn prepare(
    host: &c::HostId,
    boot: &c::BootId,
) -> (
    ReadyCredentialBase<RootCredentialSource>,
    NetworkSetupWriter<'static>,
) {
    let (writer, reader) = INPUT.init(NetworkSetupChannel::new(1)).split();
    let identity = NetworkNativeIdentity {
        host: host.clone(),
        boot: boot.clone(),
        base: "base/esp32/private-network-input".into(),
        instance: "instance/esp32/private-network-input".into(),
        provider_generation: 1,
        resource: "pool/esp32/private-network-input".into(),
        resource_generation: c::ResourceGenerationId("generation/private-network-input/1".into()),
        envelope: "envelope/esp32/private-network-input".into(),
        artifact: "native/esp32-private-network-input-root@1".into(),
        implementation: "esp32/private-network-input@1".into(),
        base_implementation: "esp32/private-network-input-base@1".into(),
    };
    // SAFETY: Root initializes one actual fixed-capacity volatile mailbox and
    // retains its unique non-Clone reader here. The source only try-locks and
    // encodes at most MAXIMUM_JOIN_INPUT_BYTES; successful transfer consumes and
    // erases the input. No socket route or radio authority is inferred from it.
    let ready = unsafe { ReadyCredentialBase::new(identity, RootCredentialSource(reader)) }
        .expect("actual private source must prepare");
    (ready, writer)
}
