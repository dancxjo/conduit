//! Controlled provider/clock fixtures exercise the real std owner publisher.
use super::*;
use conduit_core::{ArtifactId, MonotonicClockIdentity, MonotonicInstant, TemporalScale};
use conduit_planner::resource_cost::*;
struct Provider {
    offer: LocalModelOffer,
    state: LocalModelRuntimeState,
}
impl HostedLocalModelAdapter for Provider {
    fn offer(&self) -> &LocalModelOffer {
        &self.offer
    }
    fn current_runtime_state(&self) -> Result<LocalModelRuntimeState, String> {
        Ok(self.state.clone())
    }
    fn execute(&mut self, _: &PlannedGear, _: &[u8], _: &mut Vec<u8>) -> LocalModelAdapterTerminal {
        panic!("observation must never execute or warm a model")
    }
}
struct Clock {
    values: std::vec::IntoIter<MonotonicInstant>,
}
impl crate::TimerAdapter for Clock {
    fn wait(&mut self, _: std::time::Duration) {
        panic!("sampling must not wait to bypass frequency admission")
    }
    fn monotonic_observation(&mut self, _: &HostId, _: &BootId) -> Option<MonotonicInstant> {
        self.values.next()
    }
}
fn clock(host: &StdHost, ticks: &[u64]) -> Clock {
    let a = host.advertisement();
    let clock = MonotonicClockIdentity::new(
        a.host_id.clone(),
        a.boot_id.clone(),
        "sampling/clock".into(),
        TemporalScale::Microseconds,
        1,
        1,
    )
    .unwrap();
    Clock {
        values: ticks
            .iter()
            .map(|tick| MonotonicInstant::new(*tick, clock.clone()).unwrap())
            .collect::<Vec<_>>()
            .into_iter(),
    }
}
fn policy() -> ModelCostSamplingPolicy {
    ModelCostSamplingPolicy {
        validity_microseconds: 5000,
        maximum_sampling_microseconds: 4000,
        minimum_interval_microseconds: 1000,
        maximum_attempts: 2,
    }
}
fn host(state: LocalModelRuntimeState) -> StdHost {
    StdHost::new_with_local_model(
        config(),
        StdHostComposition::default(),
        Box::new(Provider {
            offer: offer(vec![LocalModelKindProfile::Generate]),
            state,
        }),
    )
    .unwrap()
}
fn capability(host: &StdHost) -> conduit_core::CapabilityId {
    host.advertisement()
        .capabilities
        .iter()
        .find(|offer| {
            offer.implementation.implementation_id.as_str()
                == conduit_ai::LOCAL_MODEL_IMPLEMENTATION
        })
        .unwrap()
        .capability_id
        .clone()
}
#[test]
fn actual_owner_source_binds_clock_sign_and_reported_memory_without_inventing_warm_epoch() {
    let host = host(LocalModelRuntimeState {
        residency: ModelResidencyState::ResidentWithoutEpoch,
        runtime_version: None,
        resident_bytes: Some(4096),
        vram_bytes: None,
        context_length: Some(2048),
        provider_expiry: Some("unprojected-provider-wall-time".into()),
    });
    let mut sampler =
        ModelCostSampler::admit(host.advertisement(), &capability(&host), policy()).unwrap();
    let sign = SignId::from("sign/sampled-runtime");
    let artifact = ArtifactId::from("metric/model-runtime@1");
    let revision = CostCalibrationRevision::new(&artifact, 1).unwrap();
    let sample = host
        .observe_local_model_execution(
            &mut sampler,
            &sign,
            revision,
            &mut clock(&host, &[100, 200]),
        )
        .unwrap();
    assert_eq!(sample.source().sign(), &sign);
    assert_eq!(sample.source().window().observed().ticks(), 100);
    assert_eq!(sample.source().window().expires().ticks(), 5100);
    assert_eq!(sample.sampling_duration().ticks(), 100);
    assert_eq!(
        sample.execution_observation().unwrap().condition(),
        CostExecutionCondition::Unknown
    );
    assert_eq!(
        sample.resident_size().unwrap().coordinate(),
        conduit_core::ExactRational::from_i128(4096)
    );
    assert_eq!(
        sample.resident_size().unwrap().unit(),
        conduit_core::Unit::resolve("B").unwrap()
    );
    assert_eq!(sample.vram_size(), None);
    assert_eq!(
        sample.state().provider_expiry.as_deref(),
        Some("unprojected-provider-wall-time")
    );
    assert_eq!(sampler.attempts(), 1);
    assert!(host
        .observe_local_model_execution(&mut sampler, &sign, revision, &mut clock(&host, &[300]))
        .is_err());
    assert_eq!(sampler.attempts(), 1);
}
#[test]
fn acquisition_age_frequency_and_attempts_refuse_without_sleep_or_model_execution() {
    let host = host(LocalModelRuntimeState {
        residency: ModelResidencyState::NotResident,
        ..LocalModelRuntimeState::unknown()
    });
    let mut sampler =
        ModelCostSampler::admit(host.advertisement(), &capability(&host), policy()).unwrap();
    let sign = SignId::from("sign/cold-runtime");
    let artifact = ArtifactId::from("metric/model-runtime@1");
    let revision = CostCalibrationRevision::new(&artifact, 1).unwrap();
    let sample = host
        .observe_local_model_execution(
            &mut sampler,
            &sign,
            revision,
            &mut clock(&host, &[100, 200]),
        )
        .unwrap();
    assert_eq!(
        sample.execution_observation().unwrap().condition(),
        CostExecutionCondition::Cold
    );
    assert_eq!(sample.resident_size(), None);
    assert!(host
        .observe_local_model_execution(
            &mut sampler,
            &sign,
            revision,
            &mut clock(&host, &[1100, 6100])
        )
        .is_err());
    assert_eq!(sampler.attempts(), 2);
    assert!(host
        .observe_local_model_execution(&mut sampler, &sign, revision, &mut clock(&host, &[7100]))
        .is_err());
}

#[test]
fn changed_owner_clock_and_expired_acquisition_cannot_publish_a_sample() {
    let owner = host(LocalModelRuntimeState::unknown());
    let mut other_config = config();
    other_config.boot_id = BootId::from("boot/replaced-runtime");
    let other = StdHost::new_with_local_model(
        other_config,
        StdHostComposition::default(),
        Box::new(Provider {
            offer: offer(vec![LocalModelKindProfile::Generate]),
            state: LocalModelRuntimeState::unknown(),
        }),
    )
    .unwrap();
    let sign = SignId::from("sign/scoped-runtime");
    let artifact = ArtifactId::from("metric/model-runtime@1");
    let revision = CostCalibrationRevision::new(&artifact, 1).unwrap();
    let mut sampler =
        ModelCostSampler::admit(owner.advertisement(), &capability(&owner), policy()).unwrap();
    assert!(other
        .observe_local_model_execution(
            &mut sampler,
            &sign,
            revision,
            &mut clock(&other, &[100, 200])
        )
        .is_err());
    assert_eq!(sampler.attempts(), 0);
    assert!(owner
        .observe_local_model_execution(
            &mut sampler,
            &sign,
            revision,
            &mut clock(&other, &[100, 200])
        )
        .is_err());
    assert_eq!(sampler.attempts(), 0);
    let mut expiry_policy = policy();
    expiry_policy.maximum_sampling_microseconds = 6000;
    let mut sampler =
        ModelCostSampler::admit(owner.advertisement(), &capability(&owner), expiry_policy).unwrap();
    assert!(owner
        .observe_local_model_execution(
            &mut sampler,
            &sign,
            revision,
            &mut clock(&owner, &[100, 5100])
        )
        .is_err());
    assert_eq!(sampler.attempts(), 1);
}
