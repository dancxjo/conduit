//! Explicit authored/planned lossy adaptation conformance.

use conduit_audio::{
    Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId, ToneIntent, MUSIC_NOTE_INFO_ID,
    SOUND_TONE_INFO_ID,
};
use conduit_core::{
    kind_id, port_id, ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ConfigurationValue, ExecutionProfileId, HostAdvertisement, HostId,
    HostProfileId, ImplementationId, ImplementationOffer, KindContractRevision, OfferGeneration,
    PortDescriptor, PortDirection, PortTemporal, PROTOCOL_VERSION,
};
use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};
use serde::Serialize;

use super::super::CatalogError;

const SOURCE_KIND: &str = "conduit-conformance/note-source";
const ADAPTER_KIND: &str = "music/to-monophonic-tone";
const SINK_KIND: &str = "sound/tone-play";
const ADAPTER_IMPLEMENTATION: &str = "conduit.reference/newest-note-to-tone@1";
const POLYPHONY_KEY: &str = "polyphony-policy";
const VELOCITY_KEY: &str = "velocity-policy";
const PITCH_KEY: &str = "pitch-policy";
const POLYPHONY_POLICY: &str = "newest-note-priority";
const VELOCITY_POLICY: &str = "discard-explicitly";
const PITCH_POLICY: &str = "preserve-exact";
const MAXIMUM_ACTIVE_NOTES: usize = 8;

const ADAPTED_FORM: &str = "form explicit-loss {\n source: conduit-conformance/note-source\n adapt: music/to-monophonic-tone(polyphony-policy = \"newest-note-priority\", velocity-policy = \"discard-explicitly\", pitch-policy = \"preserve-exact\")\n output: sound/tone-play\n source.notes > adapt.notes\n adapt.tone > output.tone\n}\n";
const UNADAPTED_FORM: &str = "form implicit-loss {\n source: conduit-conformance/note-source\n output: sound/tone-play\n source.notes > output.tone\n}\n";

#[derive(Debug, Serialize)]
pub(super) struct LossyAdapterProof {
    kind_id: &'static str,
    implementation_id: &'static str,
    policies: [&'static str; 3],
    unadapted_result: &'static str,
    adapted_result: &'static str,
    plan_id: String,
    source_document_id: String,
    adapter_configuration: Vec<(String, ConfigurationValue)>,
    maximum_active_notes: usize,
    proof_class: &'static str,
}

pub(super) fn build() -> Result<LossyAdapterProof, CatalogError> {
    let catalog = catalog()?;
    if conduit_form::parse(UNADAPTED_FORM, &catalog).is_ok() {
        return Err(CatalogError::new(
            "implicit-sound-loss-accepted",
            "note Info connected directly to tone Info without an authored adapter",
        ));
    }
    let form = conduit_form::parse(ADAPTED_FORM, &catalog).map_err(|error| {
        CatalogError::new(
            "explicit-sound-adapter-invalid",
            format!("explicit adapter Form did not check: {error}"),
        )
    })?;
    let host = host(&catalog)?;
    let placements = conduit_planner::default_placements(&form, core::slice::from_ref(&host))
        .map_err(|error| CatalogError::new("sound-adapter-placement-failed", error.to_string()))?;
    let plan = conduit_planner::plan(
        &form,
        core::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .map_err(|error| CatalogError::new("sound-adapter-plan-failed", error.to_string()))?;
    let adapter = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.kind_id.as_str() == ADAPTER_KIND)
        .ok_or_else(|| CatalogError::new("sound-adapter-plan-missing", ADAPTER_KIND))?;
    if adapter.implementation_id.as_str() != ADAPTER_IMPLEMENTATION
        || adapter.configuration != exact_configuration()
    {
        return Err(CatalogError::new(
            "sound-adapter-plan-not-exact",
            format!(
                "adapter implementation or policy configuration was not sealed into the Plan: implementation={} configuration={:?}",
                adapter.implementation_id.as_str(),
                adapter.configuration
            ),
        ));
    }
    validate_reference_policy()?;
    Ok(LossyAdapterProof {
        kind_id: ADAPTER_KIND,
        implementation_id: ADAPTER_IMPLEMENTATION,
        policies: [POLYPHONY_POLICY, VELOCITY_POLICY, PITCH_POLICY],
        unadapted_result: "refused-info-kind-mismatch",
        adapted_result: "planned-explicit-policy",
        plan_id: plan.plan_id.as_str().to_owned(),
        source_document_id: form.source_document_id.as_str().to_owned(),
        adapter_configuration: adapter
            .configuration
            .iter()
            .map(|entry| (entry.key.clone(), entry.value.clone()))
            .collect(),
        maximum_active_notes: MAXIMUM_ACTIVE_NOTES,
        proof_class: "deterministic-reference",
    })
}

fn catalog() -> Result<ProfileCatalog, CatalogError> {
    let mut catalog = ProfileCatalog::new();
    for definition in [source_definition(), adapter_definition(), sink_definition()] {
        catalog.insert(definition).map_err(|error| {
            CatalogError::new("sound-adapter-catalog-invalid", error.to_string())
        })?;
    }
    Ok(catalog)
}

fn source_definition() -> KindDefinition {
    definition(
        SOURCE_KIND,
        Vec::new(),
        vec![port("notes", MUSIC_NOTE_INFO_ID, PortDirection::Output)],
        Vec::new(),
    )
}

fn adapter_definition() -> KindDefinition {
    definition(
        ADAPTER_KIND,
        vec![port("notes", MUSIC_NOTE_INFO_ID, PortDirection::Input)],
        vec![port("tone", SOUND_TONE_INFO_ID, PortDirection::Output)],
        vec![
            text_policy(POLYPHONY_KEY, POLYPHONY_POLICY),
            text_policy(VELOCITY_KEY, VELOCITY_POLICY),
            text_policy(PITCH_KEY, PITCH_POLICY),
        ],
    )
}

fn sink_definition() -> KindDefinition {
    definition(
        SINK_KIND,
        vec![port("tone", SOUND_TONE_INFO_ID, PortDirection::Input)],
        Vec::new(),
        Vec::new(),
    )
}

fn definition(
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    configuration: Vec<ConfigurationField>,
) -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("{kind}@1")),
        inputs,
        outputs,
        configuration,
    }
}

fn port(name: &str, info: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn text_policy(key: &str, value: &str) -> ConfigurationField {
    ConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text(value.into()),
        validation: ConfigurationRule::TextOneOf {
            values: vec![value.into()],
        },
    }
}

fn exact_configuration() -> Vec<conduit_core::ConfigurationEntry> {
    vec![
        conduit_core::ConfigurationEntry {
            key: POLYPHONY_KEY.into(),
            value: ConfigurationValue::Text(POLYPHONY_POLICY.into()),
        },
        conduit_core::ConfigurationEntry {
            key: VELOCITY_KEY.into(),
            value: ConfigurationValue::Text(VELOCITY_POLICY.into()),
        },
        conduit_core::ConfigurationEntry {
            key: PITCH_KEY.into(),
            value: ConfigurationValue::Text(PITCH_POLICY.into()),
        },
    ]
}

fn host(catalog: &ProfileCatalog) -> Result<HostAdvertisement, CatalogError> {
    let capabilities = [SOURCE_KIND, ADAPTER_KIND, SINK_KIND]
        .into_iter()
        .map(|kind| capability(catalog, kind))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("sound-adapter-reference-host"),
        boot_id: BootId::from("sound-adapter-reference-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduit-conformance/sound-adapter@1"),
        resources: Vec::new(),
        capabilities,
        planner_capabilities: Vec::new(),
    })
}

fn capability(catalog: &ProfileCatalog, kind: &str) -> Result<CapabilityOffer, CatalogError> {
    let definition = catalog
        .get(&kind_id(kind))
        .ok_or_else(|| CatalogError::new("sound-adapter-kind-missing", kind))?;
    Ok(CapabilityOffer {
        startup_parameters: definition
            .configuration
            .iter()
            .map(|field| conduit_core::FaceStartupParameter {
                name: field.key.clone(),
                value_type: match field.default_value {
                    ConfigurationValue::Bool(_) => "Boolean",
                    ConfigurationValue::U64(_) => "Count",
                    ConfigurationValue::I64(_) => "Scalar",
                    ConfigurationValue::Text(_) => "Text",
                    ConfigurationValue::Structured(ref value) => value.profile().as_str(),
                }
                .into(),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        capability_id: CapabilityId::from(format!("sound-adapter/{kind}")),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduit.reference/sound-adapter@1"),
            implementation_id: ImplementationId::from(if kind == ADAPTER_KIND {
                ADAPTER_IMPLEMENTATION
            } else {
                kind
            }),
            artifact_id: ArtifactId::from("conduit-reference/sound-adapter@1"),
        },
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 16,
            max_queue_bytes: 1_024,
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdaptError {
    Capacity,
    DuplicateOccurrence,
    UnknownOccurrence,
    OrderOverflow,
    InvalidTone,
}

#[derive(Default)]
struct NewestNoteAdapter {
    active: [Option<MusicalNoteEvent>; MAXIMUM_ACTIVE_NOTES],
    sounding: Option<u64>,
}

impl NewestNoteAdapter {
    fn apply(&mut self, event: MusicalNoteEvent) -> Result<[Option<ToneIntent>; 2], AdaptError> {
        let mut output = [None, None];
        match event.gate {
            Gate::On => {
                if self
                    .active
                    .iter()
                    .flatten()
                    .any(|active| active.occurrence == event.occurrence)
                {
                    return Err(AdaptError::DuplicateOccurrence);
                }
                let slot = self
                    .active
                    .iter_mut()
                    .find(|slot| slot.is_none())
                    .ok_or(AdaptError::Capacity)?;
                *slot = Some(event);
                if let Some(previous) = self.sounding {
                    let prior = self
                        .active
                        .iter()
                        .flatten()
                        .find(|active| active.occurrence.0 == previous)
                        .ok_or(AdaptError::UnknownOccurrence)?;
                    output[0] = Some(tone(*prior, Gate::Off, doubled_order(event.order)?)?);
                }
                output[1] = Some(tone(
                    event,
                    Gate::On,
                    doubled_order(event.order)?
                        .checked_add(1)
                        .ok_or(AdaptError::OrderOverflow)?,
                )?);
                self.sounding = Some(event.occurrence.0);
            }
            Gate::Off => {
                let slot = self
                    .active
                    .iter_mut()
                    .find(|slot| slot.is_some_and(|active| active.occurrence == event.occurrence))
                    .ok_or(AdaptError::UnknownOccurrence)?;
                *slot = None;
                if self.sounding == Some(event.occurrence.0) {
                    output[0] = Some(tone(event, Gate::Off, doubled_order(event.order)?)?);
                    let resumed = self
                        .active
                        .iter()
                        .flatten()
                        .max_by_key(|active| active.order)
                        .copied();
                    if let Some(resumed) = resumed {
                        output[1] = Some(tone(
                            resumed,
                            Gate::On,
                            doubled_order(event.order)?
                                .checked_add(1)
                                .ok_or(AdaptError::OrderOverflow)?,
                        )?);
                    }
                    self.sounding = resumed.map(|active| active.occurrence.0);
                }
            }
        }
        Ok(output)
    }
}

fn doubled_order(order: u32) -> Result<u32, AdaptError> {
    order.checked_mul(2).ok_or(AdaptError::OrderOverflow)
}

fn tone(event: MusicalNoteEvent, gate: Gate, order: u32) -> Result<ToneIntent, AdaptError> {
    ToneIntent::new(
        event.occurrence.0,
        event.pitch,
        gate,
        event.event_time_micros,
        order,
    )
    .map_err(|_| AdaptError::InvalidTone)
}

fn validate_reference_policy() -> Result<(), CatalogError> {
    let first = reference_note(1, Gate::On, 7, 1)?;
    let second = reference_note(2, Gate::On, 60_000, 2)?;
    let mut adapter = NewestNoteAdapter::default();
    let first_output = adapter
        .apply(first)
        .map_err(|error| adapter_error("first note", error))?;
    let overlap = adapter
        .apply(second)
        .map_err(|error| adapter_error("overlap", error))?;
    let release = adapter
        .apply(reference_note(2, Gate::Off, 0, 3)?)
        .map_err(|error| adapter_error("release", error))?;
    if first_output[1].is_none_or(|tone| tone.gate != Gate::On)
        || overlap[0].is_none_or(|tone| tone.correlation != 1 || tone.gate != Gate::Off)
        || overlap[1].is_none_or(|tone| tone.correlation != 2 || tone.pitch != second.pitch)
        || release[0].is_none_or(|tone| tone.correlation != 2 || tone.gate != Gate::Off)
        || release[1].is_none_or(|tone| tone.correlation != 1 || tone.pitch != first.pitch)
    {
        return Err(CatalogError::new(
            "sound-adapter-reference-mismatch",
            "newest-note reference did not match its authored policy",
        ));
    }
    Ok(())
}

fn reference_note(
    occurrence: u64,
    gate: Gate,
    velocity: u16,
    order: u32,
) -> Result<MusicalNoteEvent, CatalogError> {
    MusicalNoteEvent::new(
        NoteOccurrenceId(occurrence),
        MusicalPitch::new(440_000 + occurrence * 1_000, 440_000, 0)
            .map_err(|error| adapter_error("pitch", error))?,
        gate,
        velocity,
        u64::from(order) * 1_000,
        order,
    )
    .map_err(|error| adapter_error("note", error))
}

fn adapter_error(context: &str, error: impl core::fmt::Debug) -> CatalogError {
    CatalogError::new(
        "sound-adapter-reference-invalid",
        format!("{context}: {error:?}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note(occurrence: u64, gate: Gate, velocity: u16, order: u32) -> MusicalNoteEvent {
        MusicalNoteEvent::new(
            NoteOccurrenceId(occurrence),
            MusicalPitch::new(440_000 + occurrence * 1_000, 440_000, 0).unwrap(),
            gate,
            velocity,
            u64::from(order) * 1_000,
            order,
        )
        .unwrap()
    }

    #[test]
    fn unadapted_refuses_and_explicit_policy_is_sealed_into_plan() {
        let proof = build().unwrap();
        assert_eq!(proof.unadapted_result, "refused-info-kind-mismatch");
        assert_eq!(proof.adapted_result, "planned-explicit-policy");
        assert_eq!(
            proof.adapter_configuration,
            exact_configuration()
                .into_iter()
                .map(|entry| (entry.key, entry.value))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn newest_note_policy_makes_every_loss_visible_and_deterministic() {
        let mut adapter = NewestNoteAdapter::default();
        let first = note(1, Gate::On, 7, 1);
        let second = note(2, Gate::On, 60_000, 2);
        let first_output = adapter.apply(first).unwrap();
        assert_eq!(first_output[1].unwrap().gate, Gate::On);
        let overlap = adapter.apply(second).unwrap();
        assert_eq!(overlap[0].unwrap().correlation, 1);
        assert_eq!(overlap[0].unwrap().gate, Gate::Off);
        assert_eq!(overlap[1].unwrap().correlation, 2);
        assert_eq!(overlap[1].unwrap().pitch, second.pitch);
        let release = adapter.apply(note(2, Gate::Off, 0, 3)).unwrap();
        assert_eq!(release[0].unwrap().correlation, 2);
        assert_eq!(release[1].unwrap().correlation, 1);
        assert_eq!(release[1].unwrap().pitch, first.pitch);
    }
}
