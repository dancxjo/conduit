//! CPU ownership/execution tests. This mock is never installed by USB acquisition.
use super::*;
use conduit_environment_runtime::environmental_reading::source_observation::{
    SourceObservation, SourceOutcome, SourceSample,
};
use conduit_environment_runtime::environmental_reading::EnvironmentalReading;
struct CpuSource {
    counter: u64,
    fail: bool,
}
impl EnvironmentalSourceProvider for CpuSource {
    async fn observe(
        &mut self,
    ) -> Result<SourceObservation, conduit_environment_runtime::environmental_source::Refusal> {
        if self.fail {
            return Err(conduit_environment_runtime::environmental_source::Refusal::Provider);
        }
        self.counter += 1;
        Ok(SourceObservation {
            observed_at_host_boot_ms: 100 + self.counter,
            outcome: SourceOutcome::Available(SourceSample {
                reading: EnvironmentalReading::centi_celsius_centi_pascal(2100, 10000000, None)
                    .unwrap(),
                source_counter: self.counter,
                reported_source_ms: Some(50 + self.counter),
            }),
        })
    }
}
fn fixture() -> UsbSourceRoot<CpuSource> {
    let host = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/native-source-host".into(),
        boot_id: "fixture/native-source-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: PROFILE.into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let identity = Identity {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        base_id: "fixture/base".into(),
        provider_instance_id: "fixture/source-slot".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/pool".into(),
        resource_generation_id: ResourceGenerationId("fixture/generation".into()),
        envelope_id: "fixture/envelope".into(),
        artifact_id: "fixture/native-source".into(),
        implementation_id: "fixture/source-implementation".into(),
        base_implementation_id: "fixture/source-base".into(),
    };
    let mut root = UsbSourceRoot {
        host: host.clone(),
        installed: None,
        identity,
        provider: Some(CpuSource {
            counter: 0,
            fail: false,
        }),
        key: [7; 32],
        next_play: 0,
    };
    let mut advertisement = host;
    root.append_to_advertisement(&mut advertisement).unwrap();
    root.host = advertisement;
    root
}
fn ready<F: std::future::Future>(future: F) -> F::Output {
    use std::{
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };
    struct Noop;
    impl Wake for Noop {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(Noop));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("CPU fixture unexpectedly suspended"),
    }
}
#[test]
fn checked_plot_runs_source_through_kernel_commit_and_retains_provider_across_real_plays() {
    let mut root = fixture();
    let mut installed =
        conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
            host_id: root.host.host_id.clone(),
            boot_id: root.host.boot_id.clone(),
            offer_generation: root.host.offer_generation,
        });
    installed.set_initial_kernel_play_sequence(9).unwrap();
    root.installed = Some(installed);
    let first = ready(root.sample()).unwrap();
    let second = ready(root.sample()).unwrap();
    assert_eq!(
        first.scope.play(),
        &bind_active_play(
            first.scope.plan(),
            &root.host.host_id,
            &root.host.boot_id,
            9
        )
        .active_play_id
    );
    assert_eq!(
        second.scope.play(),
        &bind_active_play(
            second.scope.plan(),
            &root.host.host_id,
            &root.host.boot_id,
            10
        )
        .active_play_id
    );
    assert_eq!(first.scope.host(), &root.host.host_id);
    assert_eq!(first.scope.boot(), &root.host.boot_id);
    assert_eq!(first.scope.plan(), second.scope.plan());
    assert_ne!(first.scope.play(), second.scope.play());
    assert_eq!(first.session(), second.session());
    assert_eq!(first.sequence, 1);
    assert_eq!(second.sequence, 2);
    let contract = ObservationContract::prepare().unwrap();
    let a = contract.decode(&first.canonical).unwrap();
    let b = contract.decode(&second.canonical).unwrap();
    assert_eq!(a.observed_at_host_boot_ms, 101);
    assert_eq!(b.observed_at_host_boot_ms, 102);
    assert_eq!(root.provider.as_ref().unwrap().counter, 2);
}
#[test]
fn provider_failure_cannot_publish_or_reacquire_possession() {
    let mut root = fixture();
    root.provider.as_mut().unwrap().fail = true;
    assert!(matches!(ready(root.sample()), Err(Refusal::Acquisition)));
    assert!(root.provider.is_none());
    assert!(matches!(ready(root.sample()), Err(Refusal::Acquisition)));
}
#[test]
fn foreign_boot_cannot_receive_actual_native_offer() {
    let root = fixture();
    let mut other = root.host.clone();
    other.boot_id = "fixture/foreign-boot".into();
    assert_eq!(
        root.append_to_advertisement(&mut other),
        Err(Refusal::WrongBoot)
    );
}

#[test]
fn received_fragment_cannot_issue_foreign_source_authority() {
    let root = fixture();
    let plan = root.plan().unwrap();
    let mut fragment = plan.fragments[0].clone();
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    fragment.boot_id = "fixture/foreign-boot".into();
    assert!(matches!(
        root.issue(&fragment, &active),
        Err(Refusal::Authority)
    ));
    assert_eq!(root.provider.as_ref().unwrap().counter, 0);
}
