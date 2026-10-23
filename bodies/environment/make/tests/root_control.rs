//! Production MCU composition logic with CPU-only entropy/Boot/radio providers.
//! No physical Root, network, image attestation or Body installation is claimed.
extern crate self as esp_hal;
use conduit_core as c;
use conduit_environment_http::network_setup::*;
use conduit_net as net;
use conduit_network_join_call::{
    self as n,
    credential_installation::ReadyCredentialBase,
    installation::{NetworkNativeIdentity, ReadyNetworkJoinBase},
};
use std::{cell::Cell, rc::Rc};
#[path = "root_control_support/fixtures.rs"]
mod fixtures;
pub mod rng {
    pub use super::fixtures::Trng;
}
mod boot {
    pub use super::fixtures::ObservedBoot;
}
mod native_network {
    pub(crate) use super::fixtures::Provider;
}
mod native_credentials {
    pub(crate) use super::fixtures::RootCredentialSource;
}
mod network_planning {
    pub use conduit_environment_make::PreparedNetworkSetup;
}
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_authority.rs"]
mod native_authority;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_play.rs"]
mod native_play;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn run<T>(future: impl core::future::Future<Output = T>) -> T {
    let mut future = core::pin::pin!(future);
    match future.as_mut().poll(&mut core::task::Context::from_waker(
        core::task::Waker::noop(),
    )) {
        core::task::Poll::Ready(v) => v,
        core::task::Poll::Pending => panic!("CPU fixture unexpectedly pending"),
    }
}
#[test]
fn production_root_control_dispatches_once_and_retains_failure_cleanup_without_allocating() {
    use net::StationFailure as F;
    for (ack, failure, cleanup_failure) in [
        (true, None, false),
        (false, None, false),
        (true, Some(F::Rejected), false),
        (true, Some(F::Pressure), false),
        (true, Some(F::Deadline), false),
        (true, Some(F::ProviderUnavailable), false),
        (true, Some(F::Rejected), true),
    ] {
        let state = Rc::new(fixtures::State::default());
        state.failure.set(failure);
        state.cleanup_failure.set(cleanup_failure);
        let (source, join) = fixtures::ready(state.clone(), ack);
        let mut host = fixtures::host();
        source.append_to_advertisement(&mut host).unwrap();
        join.append_to_advertisement(&mut host).unwrap();
        let mut play = native_play::NativePlay::new();
        play.append_offer(&mut host);
        let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
        let grants = [
            issuer
                .grant(
                    source.native_identity(),
                    native_authority::Operation::Credentials,
                )
                .unwrap(),
            issuer
                .grant(join.native_identity(), native_authority::Operation::Join)
                .unwrap(),
        ];
        let prepared = network_planning::PreparedNetworkSetup::prepare(
            &host,
            &["conduit.base/local@1".into()],
            &grants,
        )
        .unwrap();
        let mut source = Some(source);
        let mut join = Some(join);
        play.install(&prepared, &mut source, &mut join, &mut issuer)
            .unwrap();
        assert!(source.is_none() && join.is_none() && play.installed() && play.attempted());
        let active = play.active().unwrap();
        assert_eq!(active.plan_id, prepared.plan().fragments[0].plan_id);
        let ((), memory) = allocation_probe::observe(|| {
            run(play.run());
            run(play.run());
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(state.reads.get(), 1);
        assert_eq!(state.joins.get(), u32::from(ack));
        assert!(play.progress().0 <= 100);
        if !ack {
            assert_eq!(play.status(), "setup-denied");
            assert_eq!(play.progress().1, [1, 0, 0]);
        } else if failure.is_none() {
            assert_eq!(play.status(), "attached");
            assert!(play.receipt_received());
            assert_eq!(play.progress().1, [1, 1, 1]);
        } else {
            assert_eq!(state.cleanups.get(), 1);
            assert!(!play.receipt_received());
            assert_eq!(play.progress().1, [1, 1, 0]);
            assert_eq!(
                play.status(),
                if cleanup_failure {
                    "cleanup-failed"
                } else {
                    "access-point-restored"
                }
            );
            assert_eq!(
                play.failure_phase(),
                if cleanup_failure {
                    "restore-access-point"
                } else {
                    "associate"
                }
            );
            assert_ne!(play.failure_reason(), "none");
        }
        let counts = (state.reads.get(), state.joins.get(), state.cleanups.get());
        assert!(play
            .install(&prepared, &mut source, &mut join, &mut issuer)
            .is_err());
        run(play.run());
        assert_eq!(
            counts,
            (state.reads.get(), state.joins.get(), state.cleanups.get())
        );
    }
}
#[test]
fn foreign_boot_grants_and_installation_never_consume_ready_owners() {
    let state = Rc::new(fixtures::State::default());
    let (source, join) = fixtures::ready(state.clone(), true);
    let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
    let mut foreign = fixtures::identity(true);
    foreign.boot = "boot/stale".into();
    assert!(issuer
        .grant(&foreign, native_authority::Operation::Join)
        .is_err());
    let mut host = fixtures::host();
    source.append_to_advertisement(&mut host).unwrap();
    join.append_to_advertisement(&mut host).unwrap();
    let mut play = native_play::NativePlay::new();
    play.append_offer(&mut host);
    let mut grants = [
        issuer
            .grant(
                source.native_identity(),
                native_authority::Operation::Credentials,
            )
            .unwrap(),
        issuer
            .grant(join.native_identity(), native_authority::Operation::Join)
            .unwrap(),
    ];
    host.boot_id = "boot/stale".into();
    for g in &mut grants {
        g.boot_id = host.boot_id.clone();
    }
    let prepared = network_planning::PreparedNetworkSetup::prepare(
        &host,
        &["conduit.base/local@1".into()],
        &grants,
    )
    .unwrap();
    let mut source = Some(source);
    let mut join = Some(join);
    assert!(play
        .install(&prepared, &mut source, &mut join, &mut issuer)
        .is_err());
    assert!(source.is_some() && join.is_some() && !play.installed());
    run(play.run());
    assert_eq!(
        (state.reads.get(), state.joins.get(), state.cleanups.get()),
        (0, 0, 0)
    );
}

#[test]
fn issuer_requires_entropy_and_never_falls_back_to_a_fixture_key() {
    for mode in [1, 2] {
        fixtures::entropy_mode(mode);
        assert!(native_authority::RootIssuer::observe(&boot::ObservedBoot).is_err());
    }
    fixtures::entropy_mode(0);
}
