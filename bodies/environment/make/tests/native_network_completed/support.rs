use crate::{
    native_authority::{Operation, RootIssuer},
    native_play::NativePlay,
};
use conduit_core as c;
use conduit_network_join_call::{self as n, credential_installation::*, installation::*};
use std::{cell::Cell, rc::Rc};
#[derive(Default)]
pub struct Observations {
    pub calls: Cell<[usize; 3]>,
    pub join_failure: Cell<bool>,
    pub cleanup_failure: Cell<bool>,
    pub link: Cell<bool>,
    pub profile_updates: Cell<usize>,
}
pub struct Provider(pub Rc<Observations>);
impl Provider {
    pub fn generation(&self) -> u64 {
        1
    }
    pub fn record_retired_outcome(
        &mut self,
        _: u64,
        _: Option<(bool, u64)>,
        _: u64,
    ) -> Result<(), &'static str> {
        self.0.profile_updates.set(self.0.profile_updates.get() + 1);
        Ok(())
    }
    pub fn station_link_and_address_up(&self) -> bool {
        self.0.link.get()
    }
    pub fn station_address(&self) -> Option<[u8; 4]> {
        self.0.link.get().then_some([127, 0, 0, 1])
    }
}
impl n::CredentialSource for Provider {
    fn take_acknowledged(&mut self, out: &mut [u8]) -> Result<usize, n::CredentialSourceRefusal> {
        let mut calls = self.0.calls.get();
        calls[0] += 1;
        self.0.calls.set(calls);
        conduit_net::encode_network_join_request(
            conduit_net::NetworkJoinRequest {
                ssid: b"fixture-network",
                credential: b"fixture-only",
            },
            out,
        )
        .map_err(|_| n::CredentialSourceRefusal::OutputCapacity)
    }
}
impl n::NetworkJoinProvider for Provider {
    async fn join(
        &mut self,
        request: conduit_net::NetworkJoinRequest<'_>,
    ) -> Result<n::AttachmentRecord, conduit_net::StationAttemptFailure> {
        assert_eq!(request.ssid, b"fixture-network");
        let mut calls = self.0.calls.get();
        calls[1] += 1;
        self.0.calls.set(calls);
        if self.0.join_failure.get() {
            return Err(conduit_net::StationAttemptFailure {
                phase: conduit_net::StationPhase::Associate,
                reason: conduit_net::StationFailure::Rejected,
            });
        }
        self.0.link.set(true);
        Ok(
            n::AttachmentRecord::encode(conduit_net::NetworkAttachmentInfo {
                attachment_id: "fixture/attached",
                host_id: "fixture/host",
                boot_id: "fixture/boot",
                resource_pool_id: "fixture/resource/join",
                generation: 1,
            })
            .unwrap(),
        )
    }
    async fn cleanup(&mut self) -> Result<(), conduit_net::StationFailure> {
        let mut calls = self.0.calls.get();
        calls[2] += 1;
        self.0.calls.set(calls);
        if self.0.cleanup_failure.get() {
            return Err(conduit_net::StationFailure::Rejected);
        }
        self.0.link.set(false);
        Ok(())
    }
}
fn identity(name: &str) -> NetworkNativeIdentity {
    NetworkNativeIdentity {
        host: "fixture/host".into(),
        boot: "fixture/boot".into(),
        base: format!("fixture/base/{name}").into(),
        instance: format!("fixture/instance/{name}").into(),
        provider_generation: 1,
        resource: format!("fixture/resource/{name}").into(),
        resource_generation: c::ResourceGenerationId(format!("fixture/generation/{name}")),
        envelope: format!("fixture/envelope/{name}").into(),
        artifact: format!("fixture/artifact/{name}").into(),
        implementation: format!("fixture/implementation/{name}").into(),
        base_implementation: format!("fixture/base-implementation/{name}").into(),
    }
}
pub fn fixture() -> (NativePlay, Rc<Observations>, RootIssuer) {
    let state = Rc::new(Observations::default());
    let mut play = NativePlay::new();
    // SAFETY: uniquely owned initialized inert CPU providers; no device offer.
    let (mut source, mut join) = unsafe {
        (
            Some(
                ReadyCredentialBase::new(identity("credentials"), Provider(state.clone())).unwrap(),
            ),
            Some(ReadyNetworkJoinBase::new(identity("join"), 1, Provider(state.clone())).unwrap()),
        )
    };
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: identity("join").host,
        boot_id: identity("join").boot,
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/network".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    source
        .as_ref()
        .unwrap()
        .append_to_advertisement(&mut host)
        .unwrap();
    join.as_ref()
        .unwrap()
        .append_to_advertisement(&mut host)
        .unwrap();
    play.append_offer(&mut host);
    let mut issuer = RootIssuer::fixture(host.host_id.clone(), host.boot_id.clone());
    let grants = [
        issuer
            .grant(
                source.as_ref().unwrap().native_identity(),
                Operation::Credentials,
            )
            .unwrap(),
        issuer
            .grant(join.as_ref().unwrap().native_identity(), Operation::Join)
            .unwrap(),
    ];
    let bases = [
        "conduit.base/local@1".into(),
        identity("join").base_implementation,
        identity("credentials").base_implementation,
    ];
    let plot = conduit_environment_make::compile_network_setup().unwrap();
    let pins = c::PlotIdentity {
        source_document_id: plot.source_document_id.clone(),
        checked_plot_id: plot.checked_plot_id.clone(),
        expanded_plot_id: plot.expanded_plot_id.clone(),
    };
    let bytes = conduit_plot::flat_image::indexed::encode(&plot).unwrap();
    // SAFETY: this exact byte image was produced from the freshly checked Source.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes) }
            .unwrap();
    let prepared = crate::network_planning::PreparedNetworkSetup::from_indexed_for_identity(
        &pins, &image, &host, &bases, &grants,
    )
    .unwrap();
    play.install(&prepared, &mut source, &mut join, &mut issuer)
        .unwrap();
    assert!(source.is_none() && join.is_none());
    (play, state, issuer)
}
pub fn immediate<T>(future: impl core::future::Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let mut context = core::task::Context::from_waker(core::task::Waker::noop());
    match future.as_mut().poll(&mut context) {
        core::task::Poll::Ready(value) => value,
        core::task::Poll::Pending => panic!("CPU provider unexpectedly pending"),
    }
}
