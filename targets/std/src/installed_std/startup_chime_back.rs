//! Installation boundary for the shared canonical optional sound sink.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION,
    budget,
    prepare,
};
fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::startup_chime_offer();
    let exact = placement.kind_id == offer.kind_id
        && placement.kind_contract_revision == offer.kind_contract_revision
        && placement.capability_id == offer.capability_id
        && placement.execution_profile_id == offer.implementation.execution_profile_id
        && placement.implementation_id == offer.implementation.implementation_id
        && placement.artifact_id == offer.implementation.artifact_id
        && placement.semantic_contract == offer.semantic_contract
        && placement.realization_properties == offer.realization_properties
        && placement.pool_references.is_empty()
        && placement.inputs == offer.inputs
        && placement.outputs == offer.outputs
        && placement.host_calls == offer.host_calls
        && placement.limits == offer.limits
        && placement.configuration.is_empty()
        && placement.terminal_transductions.is_empty()
        && placement.resources.len() == 1
        && placement.authority.len() == 1;
    let resource = placement.resources.first();
    let authority = placement.authority.first();
    if !exact
        || resource.is_none_or(|r| {
            r.class_id.as_str() != conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS
                || r.units != 1
                || r.content.is_some()
                || r.compute.is_some()
                || r.protected.is_some()
        })
        || authority.is_none_or(|a| {
            a.contract_id.as_str() != conduit_std_offers::AUDIO_PLAYBACK_AUTHORITY_CONTRACT
                || a.host_call_contract_id.as_str() != conduit_std_offers::STARTUP_CHIME_OPERATION
                || a.subject_kind.as_str() != conduit_core::BOOL_INFO_ID
                || a.host_id != placement.host_id
                || a.boot_id != placement.boot_id
                || a.capability_id != placement.capability_id
        })
    {
        return Err(
            "startup chime differs from exact selected sound realization/resource/authority".into(),
        );
    }
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::StartupChime(
        conduit_semantic_catalog::StartupChimeBack::new(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::*;
    fn fixture() -> PlannedGear {
        let offer = conduit_std_offers::startup_chime_offer();
        let resources = vec![ResourceBinding {
            pool_id: "selected/speaker".into(),
            class_id: conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS.into(),
            units: 1,
            protected: None,
            compute: None,
            content: None,
        }];
        let authority = vec![AuthorityBinding {
            grant_id: "selected/chime-grant".into(),
            contract_id: conduit_std_offers::AUDIO_PLAYBACK_AUTHORITY_CONTRACT.into(),
            host_call_contract_id: conduit_std_offers::STARTUP_CHIME_OPERATION.into(),
            subject_kind: BOOL_INFO_ID.into(),
            host_id: "host/fixture".into(),
            boot_id: "boot/fixture".into(),
            capability_id: offer.capability_id.clone(),
        }];
        planned_gear_from_parts! {
            semantic_contract: offer.semantic_contract, realization_properties: offer.realization_properties,
            placement_id: "placement/chime".into(), gear_id: "chime".into(),
            kind_id: offer.kind_id, kind_contract_revision: offer.kind_contract_revision,
            execution_profile_id: offer.implementation.execution_profile_id,
            configuration: vec![], host_id: "host/fixture".into(), boot_id: "boot/fixture".into(), offer_generation: OfferGeneration(1),
            capability_id: offer.capability_id, implementation_id: offer.implementation.implementation_id,
            artifact_id: offer.implementation.artifact_id, base: None, realization_characteristics: vec![],
            limits: offer.limits, inputs: offer.inputs, outputs: offer.outputs, terminal_transductions: vec![], host_calls: offer.host_calls,
            resources: resources, authority: authority, pool_references: vec![],
        }
    }
    #[test]
    fn exact_sound_sink_requires_its_realization_and_scoped_authority() {
        let valid = fixture();
        assert_eq!(budget(&valid).unwrap().host_requests, 1);
        for case in 0..7 {
            let mut invalid = valid.clone();
            match case {
                0 => invalid.authority.clear(),
                1 => invalid.authority[0].boot_id = "boot/stale".into(),
                2 => invalid.authority[0].host_call_contract_id = "foreign/operation".into(),
                3 => invalid.resources[0].units = 2,
                4 => invalid.execution_profile_id = "foreign/profile".into(),
                5 => invalid.host_calls[0].maximum_output_bytes = 1,
                _ => invalid.semantic_contract.outputs.push(PortDescriptor {
                    port_id: "invented".into(),
                    direction: PortDirection::Output,
                    value_kind: BOOL_INFO_ID.into(),
                    temporal: PortTemporal::Flow { closes: true },
                    abnormal_kind: None,
                }),
            }
            assert!(budget(&invalid).is_err(), "case {case}");
        }
    }
}
