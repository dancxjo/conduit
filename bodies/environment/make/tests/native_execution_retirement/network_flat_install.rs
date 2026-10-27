//! Actual qualification-profile installer with inert SDK adapters. The complete
//! Source, ordinary planner, Root policy, selected owners and kernel are real;
//! this does not prove the target Root future, stack or physical radio.
use super::{identity, source_image, Inert};
use crate::{
    allocation_probe, c,
    native_authority::{Operation, RootIssuer},
};
use conduit_kernel as k;
use conduit_network_join_call::{
    self as n, credential_installation::ReadyCredentialBase, installation::ReadyNetworkJoinBase,
    PreparedNetworkKernel,
};
type Source = Inert;
type Provider = Inert;
const CAP: &str = "conduit/esp32/network-attachment-receipt@1";
const IMPLEMENTATION: &str = "esp32/network-attachment-receipt@1";
const ARTIFACT: &str = "native/esp32-network-attachment-receipt@1";
struct NativePlay {
    attempted: bool,
    configuration_revision: u64,
    phase: &'static str,
    active: Option<c::ActivePlayIdentity>,
    source: Option<n::NetworkCredentialsHostCall<Source>>,
    join: Option<n::NetworkJoinHostCall<Provider>>,
    prepared: Option<Box<PreparedNetworkKernel>>,
    flat_refusal: flat::IssuedCustody,
}
impl NativePlay {
    fn new() -> Self {
        Self {
            attempted: false,
            configuration_revision: 0,
            phase: "uninstalled",
            active: None,
            source: None,
            join: None,
            prepared: None,
            flat_refusal: flat::IssuedCustody::Empty,
        }
    }
    fn materialize_held(&mut self, parts: n::PreparedNetworkKernelParts) -> Result<(), ()> {
        self.prepared = Some(Box::new(parts.finish().map_err(|_| ())?));
        self.phase = "installed-held";
        Ok(())
    }
}
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_play_flat.rs"]
mod flat;
fn ready() -> (
    Option<ReadyCredentialBase<Source>>,
    Option<ReadyNetworkJoinBase<Provider>>,
) {
    // SAFETY: exclusive initialized transcript providers; no SDK readiness claim.
    unsafe {
        (
            Some(ReadyCredentialBase::new(identity("private-network-input"), Inert).unwrap()),
            Some(ReadyNetworkJoinBase::new(identity("wifi-station"), u64::MAX, Inert).unwrap()),
        )
    }
}
fn issuer() -> RootIssuer {
    RootIssuer::fixture(
        "esp32/b0a732286178".into(),
        "boot/0123456789abcdef0123456789abcdef".into(),
    )
}
#[test]
fn actual_flat_installer_retains_partial_issuance_and_never_overwrites_it() {
    let image = source_image();
    let pins = image.identity().unwrap();
    for mutation in 0..5 {
        let mut issuer = issuer();
        let (mut source, mut join) = ready();
        let mut expected = pins.clone();
        match mutation {
            1 => expected.source_document_id = "wrong/source".into(),
            2 => expected.checked_plot_id = "wrong/checked".into(),
            3 => expected.expanded_plot_id = "wrong/expanded".into(),
            4 => issuer.refuse_join_fixture(),
            _ => {}
        }
        let mut play = NativePlay::new();
        let result = play.install_linked_held(
            &image,
            &expected,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
        );
        assert!(play.attempted);
        assert_eq!(play.configuration_revision, 7);
        if mutation == 0 {
            assert!(result.is_ok());
            assert_eq!(play.phase, "installed-held");
            assert!(source.is_none() && join.is_none());
            assert!(play.source.is_some() && play.join.is_some());
            assert!(play.active.is_some());
            assert!(play.flat_refusal.is_empty());
            assert_eq!(
                k::SignSink::len(play.prepared.as_ref().unwrap().kernel.signs()),
                0
            );
        } else {
            assert!(result.is_err());
            assert!(source.is_some() && join.is_some());
            assert!(play.prepared.is_none());
            if mutation == 4 {
                let flat::IssuedCustody::First(first) = &play.flat_refusal else {
                    panic!("actual first bearer must survive Join issuance refusal");
                };
                let retained = first.inspection().clone();
                assert_eq!(
                    retained.scope.active_play_id,
                    play.active.as_ref().unwrap().active_play_id
                );
                let active = play.active.clone();
                let (retry, stats) = allocation_probe::observe(|| {
                    play.install_linked_held(
                        &image,
                        &pins,
                        c::OfferGeneration(u64::MAX),
                        &mut source,
                        &mut join,
                        &mut issuer,
                    )
                });
                assert!(retry.is_err());
                assert_eq!(stats.allocations, 0);
                assert_eq!(play.active, active);
                assert!(source.is_some() && join.is_some());
                let flat::IssuedCustody::First(first) = &play.flat_refusal else {
                    panic!("retry may not overwrite irreversible issuance");
                };
                assert_eq!(first.inspection(), &retained);
            } else {
                assert!(play.active.is_none());
                assert!(play.flat_refusal.is_empty());
            }
        }
    }
}
#[test]
fn actual_whole_constructor_counts_inline_custody_separately_from_heap() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (play, stats) = allocation_probe::observe(|| {
        let (mut source, mut join) = ready();
        let mut play = NativePlay::new();
        play.install_linked_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
        )
        .unwrap();
        assert!(source.is_none() && join.is_none());
        play
    });
    println!(
        "whole actual qualification constructor CPU{} heap {:?}; inline wrapper={} custody={}",
        usize::BITS,
        stats,
        core::mem::size_of::<NativePlay>(),
        core::mem::size_of::<flat::IssuedCustody>()
    );
    assert_eq!(play.phase, "installed-held");
    assert!(play.flat_refusal.is_empty());
    assert_eq!(
        k::SignSink::len(play.prepared.as_ref().unwrap().kernel.signs()),
        0
    );
}
