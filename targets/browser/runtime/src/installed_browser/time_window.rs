//! Dynamic browser realization of exact bounded processing-time windows.

use super::factory::BrowserInstallation;
use super::{BrowserBack, BROWSER_TIMER_MAXIMUM_MILLIS};
use conduit_core::{
    encode_monotonic_duration, resource_requirement, ArtifactId, Back, BackOfferBuilder,
    CapabilityId, CapabilityOffer, CheckedValueContract, ConfigurationValue, ExecutionProfileId,
    FrontValueLocation, ImplementationId, KindSemanticLaw, KindTerminalBehavior, PlannedGear,
    QuantityUnit,
};
use conduit_kernel::ValueStorage;

pub(crate) const IMPLEMENTATION: &str = "browser/kernel-time-window@1";
const PROFILE: &str = "browser/time-window-kernel-hosted@1";
const ARTIFACT: &str = "conduit-browser-runtime/time-window@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer: unreachable_offer,
    prepare,
    perform: None,
};

pub(crate) fn offer_for_expanded(
    gear: &conduit_plot::CheckedGear,
) -> Result<CapabilityOffer, String> {
    let (value, _, maximum_items) = contracts(&gear.semantic_contract)?;
    offer(value, maximum_items)
}

fn offer(value: &CheckedValueContract, maximum_items: u16) -> Result<CapabilityOffer, String> {
    let contract = conduit_semantic_catalog::time_window_semantic_contract(value, maximum_items)
        .map_err(str::to_string)?;
    let target = contract.kind_id.clone();
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("browser/{}", target.as_str())),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![conduit_core::wait_host_call_requirement()],
            resource_requirements: vec![resource_requirement(
                conduit_core::TIMER_RESOURCE_CLASS,
                1,
            )],
            authority_requirements: Vec::new(),
        },
    )
    .build())
}

fn contracts(
    semantic: &conduit_core::KindSemanticContract,
) -> Result<(&CheckedValueContract, &CheckedValueContract, u16), String> {
    let input = semantic
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("value")))
        .ok_or("time/window browser placement has no exact input contract")?;
    let output = semantic
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Output(conduit_core::port_id("window")))
        .ok_or("time/window browser placement has no exact output contract")?;
    let maximum_items = semantic
        .laws
        .iter()
        .find_map(|law| match law {
            KindSemanticLaw::Terminal(KindTerminalBehavior::TumblingProcessingTimeWindow {
                maximum_items,
            }) => Some(maximum_items),
            _ => None,
        })
        .ok_or("time/window browser placement has no exact window law")?;
    Ok((&input.contract, &output.contract, *maximum_items))
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    let (value, window, maximum_items) = contracts(&placement.semantic_contract)?;
    let exact = offer(value, maximum_items)?;
    if placement.kind_id != exact.kind_id
        || placement.kind_contract_revision != exact.kind_contract_revision
        || placement.capability_id != exact.capability_id
        || placement.execution_profile_id != exact.implementation.execution_profile_id
        || placement.implementation_id != exact.implementation.implementation_id
        || placement.artifact_id != exact.implementation.artifact_id
        || placement.inputs != exact.inputs
        || placement.outputs != exact.outputs
        || placement.semantic_contract != exact.semantic_contract
        || placement.limits != exact.limits
        || placement.host_calls != exact.host_calls
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str() != conduit_core::TIMER_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || placement.resources[0].protected.is_some()
        || placement.resources[0].compute.is_some()
        || placement.resources[0].content.is_some()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned time/window differs from its exact browser realization".into());
    }
    let duration = configured_duration(placement)?;
    if duration > BROWSER_TIMER_MAXIMUM_MILLIS {
        return Err("time/window duration-ms exceeds the browser timer bound".into());
    }
    let duration = values
        .store(&encode_monotonic_duration(duration))
        .map_err(|error| format!("store browser time/window duration: {error:?}"))?;
    let operation = conduit_time::ProcessingTimeWindowBack::prepare(
        duration,
        value,
        window.maximum_bytes,
        maximum_items,
    )?;
    Ok(BrowserBack::installed_step(operation))
}

fn configured_duration(placement: &PlannedGear) -> Result<u64, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("time/window requires one exact duration-ms configuration".into());
    };
    let ("duration-ms", ConfigurationValue::Quantity(value)) = (&*entry.key, &entry.value) else {
        return Err("time/window duration-ms configuration is malformed".into());
    };
    value
        .convert(QuantityUnit::Millisecond)
        .map_err(|_| "time/window duration must be an exact time quantity".to_string())?
        .value()
        .try_into()
        .map_err(|_| "time/window duration must be nonnegative".to_string())
}

fn unreachable_offer() -> CapabilityOffer {
    panic!("dynamic time/window offers must be derived from checked source")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        kind_id, port_id, BaseImplementationId, CapabilityLimits, FrontValueContract,
        HostAdvertisement, HostId, Kind, OfferGeneration, PortDescriptor, PortDirection,
        PortTemporal, PreparedLeafSequenceEncoder, ResourceBinding, ResourceOffer,
        PROTOCOL_VERSION,
    };
    use conduit_kernel::HostedValueStore;

    fn specialized() -> (conduit_core::Kind, CapabilityOffer) {
        let value =
            CheckedValueContract::new(conduit_core::kind_id("value/text"), 73, vec![]).unwrap();
        let kind = conduit_semantic_catalog::time_window_semantic_contract(&value, 7).unwrap();
        let offer = offer(&value, 7).unwrap();
        (kind, offer)
    }

    fn placement() -> PlannedGear {
        let (kind, offered) = specialized();
        conduit_core::planned_gear_from_parts! {
            semantic_contract: kind.semantic_contract(),
            placement_id: "browser-window-placement".into(),
            gear_id: "window".into(),
            kind_id: offered.kind_id,
            kind_contract_revision: offered.kind_contract_revision,
            execution_profile_id: offered.implementation.execution_profile_id,
            configuration: vec![conduit_core::ConfigurationEntry {
                key: "duration-ms".into(),
                value: ConfigurationValue::Quantity(conduit_core::Quantity::new(
                    5,
                    QuantityUnit::Millisecond,
                )),
            }],
            host_id: "browser/window".into(),
            boot_id: "browser-window-boot".into(),
            offer_generation: OfferGeneration(1),
            capability_id: offered.capability_id,
            implementation_id: offered.implementation.implementation_id,
            artifact_id: offered.implementation.artifact_id,
            base: None,
            realization_characteristics: vec![],
            limits: offered.limits,
            inputs: offered.inputs,
            outputs: offered.outputs,
            terminal_transductions: Vec::new(),
            host_calls: offered.host_calls,
            resources: vec![ResourceBinding {
                pool_id: "browser/timer".into(),
                class_id: conduit_core::TIMER_RESOURCE_CLASS.into(),
                units: 1,
                protected: None,
                compute: None,
                content: None,
            }],
            authority: vec![],
            pool_references: vec![],
        }
    }

    fn endpoint(identity: &str, direction: PortDirection, value: &CheckedValueContract) -> Kind {
        let port = PortDescriptor {
            port_id: port_id(if direction == PortDirection::Input {
                "in"
            } else {
                "out"
            }),
            value_kind: value.value_kind.clone(),
            direction,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        };
        Kind {
            startup_parameters: Vec::new(),
            shorthand: None,
            kind_id: kind_id(identity),
            kind_contract_revision: format!("{identity}@1").into(),
            inputs: (direction == PortDirection::Input)
                .then(|| port.clone())
                .into_iter()
                .collect(),
            outputs: (direction == PortDirection::Output)
                .then(|| port.clone())
                .into_iter()
                .collect(),
            configuration: Vec::new(),
            semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: if direction == PortDirection::Input {
                    FrontValueLocation::Input(port.port_id)
                } else {
                    FrontValueLocation::Output(port.port_id)
                },
                contract: value.clone(),
            }])],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: value.maximum_bytes,
            },
        }
    }

    fn endpoint_offer(kind: Kind) -> CapabilityOffer {
        let identity = kind.kind_id.as_str().to_string();
        BackOfferBuilder::new(
            kind,
            Back {
                capability_id: format!("browser/{identity}").into(),
                execution_profile_id: "browser/time-window-proof@1".into(),
                implementation_id: format!("browser/{identity}-back@1").into(),
                artifact_id: "conduit-browser-runtime/time-window-proof@1".into(),
                host_calls: Vec::new(),
                resource_requirements: Vec::new(),
                authority_requirements: Vec::new(),
            },
        )
        .build()
    }

    #[test]
    fn checked_specialization_produces_one_exact_browser_offer() {
        let value =
            CheckedValueContract::new(conduit_core::kind_id("value/text"), 73, vec![]).unwrap();
        let kind = conduit_semantic_catalog::time_window_semantic_contract(&value, 7).unwrap();
        let gear = conduit_plot::checked_gear_from_parts! {
            gear_id: conduit_core::GearId::from("window"),
            kind_id: kind.kind_id.clone(),
            kind_contract_revision: kind.kind_contract_revision.clone(),
            startup_parameters: kind.startup_parameters.clone(),
            shorthand: kind.shorthand.clone(),
            inputs: kind.inputs.clone(),
            outputs: kind.outputs.clone(),
            semantic_contract: kind.semantic_contract(),
            terminal_transductions: Vec::new(),
            resource_ports: Vec::new(),
            configuration: vec![conduit_core::ConfigurationEntry {
                key: "duration-ms".into(),
                value: ConfigurationValue::Quantity(conduit_core::Quantity::new(
                    5,
                    QuantityUnit::Millisecond,
                )),
            }],
            pool_references: Vec::new(),
        };

        let offer = offer_for_expanded(&gear).unwrap();
        assert_eq!(offer.kind_id, kind.kind_id);
        assert_eq!(
            offer.implementation.implementation_id.as_str(),
            IMPLEMENTATION
        );
        assert_eq!(offer.implementation.execution_profile_id.as_str(), PROFILE);
        assert_eq!(offer.inputs, kind.inputs);
        assert_eq!(offer.outputs, kind.outputs);
        assert_eq!(offer.host_calls.len(), 1);
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(
            offer.resource_requirements[0].class_id.as_str(),
            conduit_core::TIMER_RESOURCE_CLASS
        );
    }

    #[test]
    fn preparation_accepts_only_the_exact_admitted_specialization() {
        let mut values = HostedValueStore::new(8, 128, 1_024).unwrap();
        let capacities = values.allocation_capacities();
        let _back = prepare(&placement(), &mut values).unwrap();
        assert_eq!(values.allocation_capacities(), capacities);

        let mut wrong_semantics = placement();
        wrong_semantics.semantic_contract = Default::default();
        assert!(prepare(&wrong_semantics, &mut values).is_err());

        let mut wrong_resource = placement();
        wrong_resource.resources[0].units = 2;
        assert!(prepare(&wrong_resource, &mut values).is_err());

        let mut wrong_artifact = placement();
        wrong_artifact.artifact_id = "foreign".into();
        assert!(prepare(&wrong_artifact, &mut values).is_err());
    }

    #[test]
    fn unchanged_checked_plot_plans_the_exact_browser_specialization() {
        let value = CheckedValueContract::new(kind_id("value/text"), 73, vec![]).unwrap();
        let encoder = PreparedLeafSequenceEncoder::new(
            value.value_kind.clone(),
            value.maximum_bytes,
            conduit_semantic_catalog::TIME_WINDOW_MAXIMUM_ITEMS,
        )
        .unwrap();
        let window = CheckedValueContract::new(
            encoder
                .value_type()
                .unwrap()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            encoder.maximum_bytes(),
            vec![],
        )
        .unwrap();
        let source = endpoint("test/window-text-source", PortDirection::Output, &value);
        let sink = endpoint("test/window-text-sink", PortDirection::Input, &window);
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profile = conduit_plot::ProfileCatalog::new();
        for kind in [&source, &sink] {
            startup
                .insert(conduit_plot::KindSignature {
                    kind: kind.kind_id.as_str().into(),
                    startup_parameters: Vec::new(),
                })
                .unwrap();
            profile.insert_kind((*kind).clone()).unwrap();
        }
        conduit_semantic_catalog::install_time_window_kind(
            &value,
            conduit_semantic_catalog::TIME_WINDOW_MAXIMUM_ITEMS,
            &mut startup,
            &mut profile,
        )
        .unwrap();
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(
                "plot windowed-text {\n source: test/window-text-source\n window: time/window(duration-ms = 5ms)\n sink: test/window-text-sink\n source.out >> window.value\n window.window >> sink.in\n}.\n",
            ),
            &startup,
        )
        .unwrap();
        let expanded =
            conduit_plot::expand_canonical_plot(&checked, "windowed-text", &profile).unwrap();
        let window_offer = super::super::catalogs::offers_for_expanded_time_windows(&expanded)
            .unwrap()
            .pop()
            .unwrap();
        let host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("browser/window-proof"),
            boot_id: "browser/window-proof-boot".into(),
            offer_generation: OfferGeneration(1),
            profile: "browser/window-proof@1".into(),
            bases: Vec::new(),
            resources: vec![ResourceOffer {
                pool_id: "browser/window-proof/timer".into(),
                class_id: conduit_core::TIMER_RESOURCE_CLASS.into(),
                capacity_units: 1,
                compute: None,
                content: None,
            }],
            capabilities: vec![endpoint_offer(source), window_offer, endpoint_offer(sink)],
            planner_capabilities: Vec::new(),
        };
        let placements =
            conduit_planner::default_expanded_placements(&expanded, core::slice::from_ref(&host))
                .unwrap();
        let plan = conduit_planner::plan_expanded_canonical(
            &expanded,
            core::slice::from_ref(&host),
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
        )
        .unwrap();
        let planned = plan.fragments[0]
            .placements
            .iter()
            .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::TIME_WINDOW_KIND)
            .unwrap();
        let mut values = HostedValueStore::new(8, 128, 1_024).unwrap();
        prepare(planned, &mut values).unwrap();
    }
}
