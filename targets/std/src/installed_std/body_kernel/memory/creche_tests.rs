//! Kernel-owned backing only; adapter/context/worker admission is independent.
use super::*;
use crate::hosted_creche::{test_support, CrecheHost, HostedCrecheAdapter};

#[test]
fn exact_undispatched_creche_kernel_has_finite_receipt_without_starting_owner() {
    let mut adapter = test_support::Adapter::new();
    let (host, plan, play, _) = test_support::planned(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    let inputs = [ExternalForeInput {
        front_port_id: "request".into(),
        track: conduit_core::ConnectionTrack::Payload,
        bytes: owner.request().0.to_vec(),
    }];
    let plans = plan
        .plots
        .iter()
        .map(|partition| &partition.plan)
        .collect::<Vec<_>>();
    let mut kernel = BodyKernel::prepare_on_host(
        &plans,
        false,
        &play.active_play_id,
        &inputs,
        false,
        true,
        &host,
        None,
        #[cfg(target_os = "linux")]
        None,
    )
    .unwrap();
    assert!(kernel
        .prepare_memory(&mut crate::ThreadTimer, &host.host_id, &host.boot_id, 0)
        .is_err());
    kernel.attach_creche(&owner, &plan, &play, &host).unwrap();
    kernel
        .prepare_memory(&mut crate::ThreadTimer, &host.host_id, &host.boot_id, 0)
        .unwrap();
    owner.retire().unwrap();
    drop(owner);
    assert_eq!((adapter.begins, adapter.polls), (0, 0));
    assert_eq!(adapter.limits().maximum_polls, 2);
}
