//! Std binding of the existing shared finite sampler to exact selected contracts.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::CURRENT_SAMPLE_FINITE_IMPLEMENTATION,
    budget,
    prepare,
};

fn validate(gear: &PlannedGear) -> Result<u32, String> {
    let (value, trigger) = super::current_sample_back::exact_contracts(gear)?;
    let expected =
        conduit_std_offers::current_sample_finite_offer(value, trigger).map_err(str::to_owned)?;
    if gear.kind_id != expected.kind_id
        || gear.kind_contract_revision != expected.kind_contract_revision
        || gear.capability_id != expected.capability_id
        || gear.execution_profile_id != expected.implementation.execution_profile_id
        || gear.implementation_id != expected.implementation.implementation_id
        || gear.artifact_id != expected.implementation.artifact_id
        || gear.inputs != expected.inputs
        || gear.outputs != expected.outputs
        || gear.semantic_contract != expected.semantic_contract
        || gear.limits != expected.limits
        || !gear.configuration.is_empty()
        || !gear.host_calls.is_empty()
        || gear.base.is_some()
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
        || !gear.pool_references.is_empty()
        || !gear.realization_properties.is_empty()
        || !gear.realization_characteristics.is_empty()
    {
        return Err("finite sampler differs from its exact selected realization".into());
    }
    Ok(value.maximum_bytes)
}

fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    let maximum = validate(gear)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: maximum,
        maximum_value_bytes: maximum,
        host_requests: 0,
        sign_items: 32,
    })
}

fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let maximum = validate(gear)?;
    Ok(InstalledBack::CurrentSample(
        conduit_composite::CurrentSampleBack::prepare_finite(maximum),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, BaseImplementationId, ConnectionTrack, PortDirection};
    use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Samples(Vec<Vec<u8>>);
    impl crate::ExternalForeOutputAdapter for Samples {
        fn deliver(&mut self, output: crate::ExternalForeDelivery) -> Result<(), String> {
            if output.track == ConnectionTrack::Payload {
                self.0.push(output.bytes.to_vec());
            }
            Ok(())
        }
    }

    #[test]
    fn finite_sampler_executes_exact_large_values_and_closes_without_an_extra_sample() {
        let maximum = conduit_std_offers::CURRENT_SAMPLE_FINITE_MAXIMUM_BYTES;
        let value =
            conduit_core::CheckedValueContract::new(kind_id("value/bytes"), maximum, vec![])
                .unwrap();
        let trigger =
            conduit_core::CheckedValueContract::new(kind_id("value/u64"), 8, vec![]).unwrap();
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profile = conduit_plot::ProfileCatalog::new();
        startup
            .insert_value_kind_alias("SampleValue", value.value_kind.clone())
            .unwrap();
        startup
            .insert_value_kind_alias("SampleTick", trigger.value_kind.clone())
            .unwrap();
        conduit_semantic_catalog::install_current_sample_finite_kind(
            &value,
            &trigger,
            &mut startup,
            &mut profile,
        )
        .unwrap();
        conduit_semantic_catalog::install_flow_coalesce_finite_kind(
            &value,
            &mut startup,
            &mut profile,
        )
        .unwrap();
        let checked = conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(
            "plot finite-sample (\n >> data: SampleValue...| <= 128KiB\n >> ticks: SampleTick...| <= 8B\n samples: SampleValue...| <= 128KiB >>\n) {\n latest: flow/coalesce-latest-finite\n sample: current/sample/finite\n data >> latest.in\n latest.out >> sample.current\n ticks >> sample.trigger\n sample.value >> samples\n}\n"
        ), &startup).unwrap();
        let expanded =
            conduit_plot::expand_canonical_plot_for_authoring(&checked, "finite-sample", &profile)
                .unwrap();
        let mut host = crate::StdHost::new_with_config(crate::StdHostConfig {
            host_id: "host/finite-sampler-proof".into(),
            boot_id: "boot/finite-sampler-proof".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        });
        host.install_finite_current_sampling(&value, &trigger)
            .unwrap();
        let hosts = [host.advertisement().clone()];
        let placements =
            conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
        let boundaries = [
            ("data", PortDirection::Input),
            ("ticks", PortDirection::Input),
            ("samples", PortDirection::Output),
        ]
        .into_iter()
        .map(|(name, direction)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: name.into(),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: maximum,
                },
            )
        })
        .collect();
        let mut plan = conduit_planner::plan_expanded_authoring_with_options(
            &expanded,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: maximum,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .unwrap();
        let fragment = plan.fragments.pop().unwrap();
        let gear = fragment
            .placements
            .iter()
            .find(|gear| {
                gear.implementation_id.as_str()
                    == conduit_std_offers::CURRENT_SAMPLE_FINITE_IMPLEMENTATION
            })
            .unwrap();
        assert_eq!(budget(gear).unwrap().maximum_value_bytes, maximum);
        let mut forged = gear.clone();
        forged.capability_id = "unselected/sampler".into();
        assert!(budget(&forged).is_err());
        let payload = vec![0x5a; 65536];
        let inputs = [
            crate::ExternalForeInput {
                front_port_id: "data".into(),
                track: ConnectionTrack::Payload,
                bytes: payload.clone(),
            },
            crate::ExternalForeInput {
                front_port_id: "ticks".into(),
                track: ConnectionTrack::Payload,
                bytes: 1u64.to_le_bytes().to_vec(),
            },
            crate::ExternalForeInput {
                front_port_id: "ticks".into(),
                track: ConnectionTrack::Payload,
                bytes: 2u64.to_le_bytes().to_vec(),
            },
        ];
        let mut samples = Samples::default();
        let report = host
            .run_external_plot_sequence_to(
                fragment,
                &inputs,
                &mut samples,
                &mut Vec::new(),
                &mut crate::ThreadTimer,
            )
            .unwrap();
        assert_eq!(samples.0, vec![payload.clone(), payload]);
        assert!(matches!(
            &report.observations.last().unwrap().kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Completed
            }
        ));
    }
}
