//! Explicit fictional offers for planner tests; no physical Ready owner exists.
use conduit_core::*;
use conduit_plot::ProfileCatalog;
pub const CALLER: &str = "plot main {\n source: fixture/bme-input\n compensate: device/bme280-compensation\n sink: fixture/bme-result\n source.output >> compensate.input\n compensate.result >> sink.input\n}\n";
pub const RESOURCE: &str = "fixture/compensation-workspace";
pub fn definitions(native: &Kind) -> [Kind; 3] {
    let mut source = native.clone();
    source.kind_id = "fixture/bme-input".into();
    source.kind_contract_revision = "fixture/bme-input@1".into();
    source.shorthand = None;
    source.inputs.clear();
    source.outputs = vec![native.inputs[0].clone()];
    source.outputs[0].direction = PortDirection::Output;
    source.outputs[0].port_id = "output".into();
    let mut sink = native.clone();
    sink.kind_id = "fixture/bme-result".into();
    sink.kind_contract_revision = "fixture/bme-result@1".into();
    sink.shorthand = None;
    sink.outputs.clear();
    sink.inputs = vec![native.outputs[0].clone()];
    sink.inputs[0].direction = PortDirection::Input;
    sink.inputs[0].port_id = "input".into();
    [source, native.clone(), sink]
}
pub fn profiles(definitions: &[Kind]) -> ProfileCatalog {
    let mut result = ProfileCatalog::new();
    for definition in definitions {
        result.insert_kind(definition.clone()).unwrap();
    }
    result
}
fn offer(kind: &Kind) -> CapabilityOffer {
    let slug = kind.kind_id.as_str().replace('/', "-");
    conduit_core::capability_offer_from_parts! {
        semantic_contract: KindSemanticContract {
            configuration: kind.configuration.clone(), laws: kind.semantic_laws.clone(),
        },
        startup_parameters: kind.startup_parameters.clone(),
        shorthand: kind.shorthand.clone(),
        capability_id: CapabilityId::from(format!("fixture/{slug}")),
        kind_id: kind.kind_id.clone(), kind_contract_revision: kind.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: "fixture/bounded-pure@1".into(),
            implementation_id: ImplementationId::from(format!("fixture/{slug}@1")),
            artifact_id: ArtifactId::from(format!("fixture/{slug}-artifact@1")),
        },
        inputs: kind.inputs.clone(), outputs: kind.outputs.clone(),
        host_calls: vec![],
        // Deliberately fictional units exercise ordinary resource admission;
        // they are NOT a RAM measurement or a hardware-readiness claim.
        resource_requirements: if kind.kind_id.as_str() == "device/bme280-compensation" {
            vec![resource_requirement(RESOURCE, 2)]
        } else { vec![] },
        authority_requirements: vec![], limits: kind.limits.clone(),
    }
}
pub fn host(definitions: &[Kind]) -> HostAdvertisement {
    HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/host@1".into(),
        bases: vec![],
        resources: vec![resource_offer("fixture/workspace", RESOURCE, 2)],
        capabilities: definitions.iter().map(offer).collect(),
        planner_capabilities: vec![],
    }
}
