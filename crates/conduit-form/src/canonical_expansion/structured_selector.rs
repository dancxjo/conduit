use super::*;
use crate::{ConfigurationField, KindDefinition};
use conduit_core::{
    KindContractRevision, KindId, PortDescriptor, PortDirection, PortTemporal, StructuredSelector,
};

pub fn structured_selector_definition(
    selector: &StructuredSelector,
    temporal: PortTemporal,
) -> KindDefinition {
    let digest = selector
        .semantic_digest()
        .expect("checked selector has a finite semantic identity");
    let mut encoded = String::with_capacity(digest.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    let kind_id = KindId::from(format!(
        "structured-info/selector-{encoded}-{}@1",
        temporal.as_str()
    ));
    KindDefinition {
        kind_id,
        kind_contract_revision: KindContractRevision::from("structured-info/selector-operation@1"),
        inputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("input"),
            value_kind: selector
                .input_type()
                .profile()
                .expect("checked selector input has a finite profile")
                .value_kind()
                .clone(),
            direction: PortDirection::Input,
            temporal,
        }],
        outputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("output"),
            value_kind: selector
                .output_type()
                .profile()
                .expect("checked selector output has a finite profile")
                .value_kind()
                .clone(),
            direction: PortDirection::Output,
            temporal,
        }],
        configuration: Vec::<ConfigurationField>::new(),
    }
}

pub(super) enum PendingStage {
    Ready(Stage),
    Selector {
        selector: StructuredSelector,
        source_span: crate::Span,
    },
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_selectors(
    pending: Vec<PendingStage>,
    source_form: &CheckedCanonicalForm,
    forms: &BTreeMap<&str, &CheckedCanonicalForm>,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
    stack: &mut Vec<String>,
    realization_backs: &mut Vec<conduit_core::RealizationBack>,
    depth: usize,
    gears: &mut Vec<CheckedGear>,
    connections: &mut Vec<CheckedConnection>,
    shared_pools: &mut Vec<ExpandedSharedPool>,
    provenance: &mut Vec<ExpandedGearProvenance>,
    gear_ids: &mut BTreeSet<GearId>,
    anonymous_counts: &mut BTreeMap<String, usize>,
) -> Result<Vec<Stage>, CanonicalExpansionDiagnostic> {
    let mut stages = Vec::with_capacity(pending.len());
    for (index, stage) in pending.iter().enumerate() {
        let PendingStage::Selector {
            selector,
            source_span,
        } = stage
        else {
            let PendingStage::Ready(stage) = stage else {
                unreachable!()
            };
            stages.push(stage.clone());
            continue;
        };
        let left = pending[..index].iter().rev().find_map(|stage| match stage {
            PendingStage::Ready(stage) => output_temporal(stage),
            PendingStage::Selector { .. } => None,
        });
        let right = pending[index + 1..].iter().find_map(|stage| match stage {
            PendingStage::Ready(stage) => input_temporal(stage),
            PendingStage::Selector { .. } => None,
        });
        let temporal = match (left, right) {
            (Some(left), Some(right)) if left != right => {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-045",
                    "structured selector cannot change a Cord's temporal contract".into(),
                ));
            }
            (Some(temporal), _) | (_, Some(temporal)) => temporal,
            (None, None) => {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-036",
                    "structured selector requires an adjacent typed Cord stage".into(),
                ));
            }
        };
        let definition = structured_selector_definition(selector, temporal);
        let key = definition.kind_id.as_str().to_string();
        let count = anonymous_counts.entry(key.clone()).or_default();
        let name = format!("selector-{}-{count}", &hash_string(&key)[..12]);
        *count += 1;
        let gear = CheckedCanonicalGear {
            name: None,
            kind: key,
            startup_parameters: vec![],
            startup_bindings: vec![],
            source_span: *source_span,
        };
        let instance = instantiate_gear(
            &gear,
            &name,
            source_form,
            forms,
            catalog,
            backs,
            environment,
            path,
            stack,
            realization_backs,
            depth,
            gears,
            connections,
            shared_pools,
            provenance,
            gear_ids,
        )?;
        stages.push(stage_for_instance(&name, &instance, None)?);
    }
    Ok(stages)
}

fn output_temporal(stage: &Stage) -> Option<PortTemporal> {
    match &stage.output {
        Some(StageSource::Internal(endpoint)) => Some(endpoint.port.temporal),
        Some(StageSource::FaceInput(_, _, temporal)) => Some(*temporal),
        None => None,
    }
}

fn input_temporal(stage: &Stage) -> Option<PortTemporal> {
    let inputs = stage.input.as_ref()?;
    let first = inputs.first().map(|sink| match sink {
        StageSink::Internal(endpoint) => endpoint.port.temporal,
        StageSink::FaceOutput(_, _, temporal) => *temporal,
    })?;
    inputs
        .iter()
        .all(|sink| match sink {
            StageSink::Internal(endpoint) => endpoint.port.temporal == first,
            StageSink::FaceOutput(_, _, temporal) => *temporal == first,
        })
        .then_some(first)
}
