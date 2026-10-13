//! Original caller-admitted model provenance, separate from current samples.
//! The proof owner must own the producing pipe and pin this Host snapshot.
//! Deserialization checks attribution; it never authenticates a producer.
use conduit_core::*;
use conduit_planner::resource_cost::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    host: HostAdvertisement,
    source: Attribution,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attribution {
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    host_profile: HostProfileId,
    capability_id: CapabilityId,
    kind_id: KindId,
    kind_contract_revision: KindIdentity,
    execution_profile_id: ExecutionProfileId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
    resource: Option<(ResourcePoolId, ResourceClassId)>,
    sign_id: SignId,
    calibration_identity: ArtifactId,
    calibration_revision: u64,
    observed: MonotonicInstant,
    expires: MonotonicInstant,
}
impl Request {
    pub(super) fn admit<'a>(
        &'a self,
        current: &CostEvidenceSource<'_>,
        now: &MonotonicInstant,
    ) -> Result<CostEvidenceSource<'a>, String> {
        let value = &self.source;
        if value.calibration_identity.as_str() != "conduit.proof/adapter-service-calibration@1"
            || value.calibration_revision != 1
        {
            return Err("model metric definition changed".into());
        }
        let offer = self
            .host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == value.capability_id)
            .ok_or("model offer absent from original Host")?;
        let resource = value
            .resource
            .as_ref()
            .map(|(pool, class)| {
                self.host
                    .resources
                    .iter()
                    .find(|resource| &resource.pool_id == pool && &resource.class_id == class)
                    .ok_or("model resource absent from original Host")
            })
            .transpose()?;
        let window = CostObservationWindow::admit(value.observed.clone(), value.expires.clone())
            .map_err(|e| format!("model window: {e:?}"))?;
        if value
            .expires
            .elapsed_since(&value.observed)
            .map_err(|e| format!("model lifetime: {e:?}"))?
            .ticks()
            > 1_000_000
            || value.observed.clock().scale() != TemporalScale::Microseconds
        {
            return Err("model lifetime bound changed".into());
        }
        let revision =
            CostCalibrationRevision::new(&value.calibration_identity, value.calibration_revision)
                .map_err(|e| format!("model revision: {e:?}"))?;
        let source = CostEvidenceSource::bind(
            &self.host,
            offer,
            resource,
            &value.sign_id,
            revision,
            window,
        )
        .map_err(|e| format!("model source: {e:?}"))?;
        let admitted = CostSourceAttribution::from_source(&source);
        if admitted.host_id != value.host_id
            || admitted.boot_id != value.boot_id
            || admitted.offer_generation != value.offer_generation
            || admitted.host_profile != value.host_profile
            || admitted.kind_id != value.kind_id
            || admitted.kind_contract_revision != value.kind_contract_revision
            || admitted.execution_profile_id != value.execution_profile_id
            || admitted.implementation_id != value.implementation_id
            || admitted.implementation_artifact != value.artifact_id
        {
            return Err("model attribution changed original subject".into());
        }
        if source.offer() != current.offer() {
            return Err("model source changed the sampled realization".into());
        }
        source
            .validate_at(current.host(), revision, now)
            .map_err(|e| format!("original model current validation: {e:?}"))?;
        Ok(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Request {
        let host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("host/model"),
            boot_id: BootId::from("boot/model"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("profile/model"),
            bases: vec![],
            resources: vec![],
            planner_capabilities: vec![],
            capabilities: vec![capability_offer_from_parts! {
                semantic_contract: Default::default(), startup_parameters: vec![], shorthand: None,
                capability_id: CapabilityId::from("cap/model"), kind_id: KindId::from("llm/generate"),
                kind_contract_revision: KindIdentity::from("llm/generate@1"),
                implementation: ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from("profile/adapter"),
                    implementation_id: ImplementationId::from("impl/model"), artifact_id: ArtifactId::from("artifact/model"),
                },
                inputs: vec![], outputs: vec![], host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![],
                limits: CapabilityLimits { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: 1 },
            }],
        };
        let clock = MonotonicClockIdentity::new(
            host.host_id.clone(),
            host.boot_id.clone(),
            "clock/model".into(),
            TemporalScale::Microseconds,
            1,
            1,
        )
        .unwrap();
        let offer = &host.capabilities[0];
        let source = Attribution {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            host_profile: host.profile.clone(),
            capability_id: offer.capability_id.clone(),
            kind_id: offer.kind_id.clone(),
            kind_contract_revision: offer.kind_contract_revision.clone(),
            execution_profile_id: offer.implementation.execution_profile_id.clone(),
            implementation_id: offer.implementation.implementation_id.clone(),
            artifact_id: offer.implementation.artifact_id.clone(),
            resource: None,
            sign_id: SignId::from("completion/original"),
            calibration_identity: ArtifactId::from("conduit.proof/adapter-service-calibration@1"),
            calibration_revision: 1,
            observed: MonotonicInstant::new(10, clock.clone()).unwrap(),
            expires: MonotonicInstant::new(1000, clock).unwrap(),
        };
        Request { host, source }
    }
    #[test]
    fn fresh_sampler_cannot_renew_original_model_window() {
        let request = fixture();
        let sign = SignId::from("sample/current");
        let metric = ArtifactId::from("conduit.proof/model-runtime-sampling@1");
        let clock = request.source.observed.clock().clone();
        let current = CostEvidenceSource::bind(
            &request.host,
            &request.host.capabilities[0],
            None,
            &sign,
            CostCalibrationRevision::new(&metric, 1).unwrap(),
            CostObservationWindow::admit(
                MonotonicInstant::new(100, clock.clone()).unwrap(),
                MonotonicInstant::new(2000, clock.clone()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let admitted = request
            .admit(
                &current,
                &MonotonicInstant::new(500, clock.clone()).unwrap(),
            )
            .unwrap();
        assert_eq!(admitted.sign(), &request.source.sign_id);
        assert_eq!(admitted.window().observed(), &request.source.observed);
        assert_eq!(admitted.window().expires(), &request.source.expires);
        assert_ne!(admitted.calibration(), current.calibration());
        let expired = MonotonicInstant::new(1000, clock).unwrap();
        assert!(current
            .validate_at(&request.host, current.calibration(), &expired)
            .is_ok());
        assert!(request.admit(&current, &expired).is_err());
    }
    #[test]
    fn current_generation_change_refuses_original_model() {
        let request = fixture();
        let mut changed = request.host.clone();
        changed.offer_generation = OfferGeneration(2);
        let sign = SignId::from("sample/changed");
        let revision =
            CostCalibrationRevision::new(&request.source.calibration_identity, 1).unwrap();
        let window = CostObservationWindow::admit(
            request.source.observed.clone(),
            request.source.expires.clone(),
        )
        .unwrap();
        let current = CostEvidenceSource::bind(
            &changed,
            &changed.capabilities[0],
            None,
            &sign,
            revision,
            window,
        )
        .unwrap();
        assert!(request.admit(&current, &request.source.observed).is_err());
    }
    #[test]
    fn another_capability_on_same_host_is_not_the_sampled_realization() {
        let mut request = fixture();
        let mut alternate = request.host.capabilities[0].clone();
        alternate.capability_id = CapabilityId::from("cap/other");
        request.host.capabilities.push(alternate);
        let owner = request.host.clone();
        let sign = SignId::from("sample/other-capability");
        let revision =
            CostCalibrationRevision::new(&request.source.calibration_identity, 1).unwrap();
        let window = CostObservationWindow::admit(
            request.source.observed.clone(),
            request.source.expires.clone(),
        )
        .unwrap();
        let current = CostEvidenceSource::bind(
            &owner,
            &owner.capabilities[1],
            None,
            &sign,
            revision,
            window,
        )
        .unwrap();
        assert_eq!(
            request
                .admit(&current, &request.source.observed)
                .unwrap_err(),
            "model source changed the sampled realization"
        );
    }
    #[test]
    fn changed_attribution_or_metric_is_not_an_original_model() {
        let mut request = fixture();
        let owner = request.host.clone();
        let metric = request.source.calibration_identity.clone();
        let sign = SignId::from("sample/current");
        let window = CostObservationWindow::admit(
            request.source.observed.clone(),
            request.source.expires.clone(),
        )
        .unwrap();
        let current = CostEvidenceSource::bind(
            &owner,
            &owner.capabilities[0],
            None,
            &sign,
            CostCalibrationRevision::new(&metric, 1).unwrap(),
            window,
        )
        .unwrap();
        request.source.artifact_id = ArtifactId::from("artifact/forged");
        assert!(request.admit(&current, &request.source.observed).is_err());
        request.source.artifact_id = owner.capabilities[0].implementation.artifact_id.clone();
        request.source.calibration_revision = 2;
        assert!(request.admit(&current, &request.source.observed).is_err());
    }
}
