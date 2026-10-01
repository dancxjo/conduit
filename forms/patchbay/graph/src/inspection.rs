//! Exact subject references and inspection facts.
use crate::{prelude::*, *};
use conduit_core::{CheckedValueContract, PortDirection, ValueConstraint};

impl PatchbayGraph {
    pub fn subject_identities(&self) -> impl Iterator<Item = &str> {
        self.front_inputs
            .iter()
            .chain(&self.front_outputs)
            .map(|port| port.identity.as_str())
            .chain(self.compositions.iter().flat_map(|composition| {
                core::iter::once(composition.identity.as_str())
                    .chain(composition.inputs.iter().map(|port| port.identity.as_str()))
                    .chain(
                        composition
                            .outputs
                            .iter()
                            .map(|port| port.identity.as_str()),
                    )
            }))
            .chain(self.gears.iter().flat_map(|gear| {
                core::iter::once(gear.identity.as_str())
                    .chain(gear.inputs.iter().map(|port| port.identity.as_str()))
                    .chain(gear.outputs.iter().map(|port| port.identity.as_str()))
            }))
            .chain(self.cords.iter().map(|cord| cord.identity.as_str()))
    }

    pub fn subject_ref(&self, identity: &str) -> Result<PatchbaySubjectRef, PatchbayGraphError> {
        self.subject_index(identity)?;
        Ok(PatchbaySubjectRef {
            expanded_form_id: self.expanded_form_id.clone(),
            subject_identity: identity.into(),
        })
    }

    pub fn resolve_subject_ref(
        &self,
        subject: &PatchbaySubjectRef,
    ) -> Result<usize, PatchbayGraphError> {
        if subject.expanded_form_id != self.expanded_form_id {
            return Err(PatchbayGraphError::StaleGraphBasis);
        }
        self.subject_index(&subject.subject_identity)
    }

    pub fn inspect(&self, identity: &str) -> Result<PatchbayInspection, PatchbayGraphError> {
        if let Some(composition) = self
            .compositions
            .iter()
            .find(|composition| composition.identity == identity)
        {
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind: PatchbaySubjectKind::Composition,
                exact_facts: vec![
                    format!("Gear {}", composition.gear_name),
                    format!("Back {}", composition.back_name),
                    format!("checked {}", composition.checked_form_id.as_str()),
                    format!(
                        "inputs={} outputs={}",
                        composition.inputs.len(),
                        composition.outputs.len()
                    ),
                ],
            });
        }
        if let Some((composition, port)) = self.compositions.iter().find_map(|composition| {
            composition
                .inputs
                .iter()
                .chain(&composition.outputs)
                .find(|port| port.identity == identity)
                .map(|port| (composition, port))
        }) {
            let subject_kind = match port.descriptor.direction {
                PortDirection::Input => PatchbaySubjectKind::PortInput,
                PortDirection::Output => PatchbaySubjectKind::PortOutput,
            };
            let mut exact_facts = vec![
                format!("Composition {}", composition.gear_name),
                format!("Back {}", composition.back_name),
                format!("Port {}", port.descriptor.port_id.as_str()),
                format!("Info {}", port.descriptor.value_kind.as_str()),
                format!("temporal={:?}", port.descriptor.temporal),
            ];
            append_contract_facts(&mut exact_facts, port.value_contract.as_ref());
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind,
                exact_facts,
            });
        }
        if let Some(port) = self
            .front_inputs
            .iter()
            .chain(&self.front_outputs)
            .find(|port| port.identity == identity)
        {
            let subject_kind = match port.descriptor.direction {
                PortDirection::Input => PatchbaySubjectKind::FaceInput,
                PortDirection::Output => PatchbaySubjectKind::FaceOutput,
            };
            let mut exact_facts = vec![
                format!("Front Port {}", port.descriptor.port_id.as_str()),
                format!("direction={:?}", port.descriptor.direction),
                format!("Info {}", port.descriptor.value_kind.as_str()),
                format!("temporal={:?}", port.descriptor.temporal),
                "authoring boundary; runnable root requires an exact binding".into(),
            ];
            append_contract_facts(&mut exact_facts, port.value_contract.as_ref());
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind,
                exact_facts,
            });
        }
        if let Some(gear) = self.gears.iter().find(|gear| gear.identity == identity) {
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind: PatchbaySubjectKind::Gear,
                exact_facts: vec![
                    format!("Gear {}", gear.gear_id.as_str()),
                    format!("Kind {}", gear.kind_id.as_str()),
                    format!(
                        "inputs={} outputs={}",
                        gear.inputs.len(),
                        gear.outputs.len()
                    ),
                ],
            });
        }
        if let Some(port) = self
            .gears
            .iter()
            .flat_map(|gear| gear.inputs.iter().chain(&gear.outputs))
            .find(|port| port.identity == identity)
        {
            let subject_kind = match port.descriptor.direction {
                PortDirection::Input => PatchbaySubjectKind::PortInput,
                PortDirection::Output => PatchbaySubjectKind::PortOutput,
            };
            let mut exact_facts = vec![
                format!("Gear {}", port.gear_id.as_str()),
                format!("Port {}", port.descriptor.port_id.as_str()),
                format!("Info {}", port.descriptor.value_kind.as_str()),
                format!("temporal={:?}", port.descriptor.temporal),
            ];
            append_contract_facts(&mut exact_facts, port.value_contract.as_ref());
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind,
                exact_facts,
            });
        }
        if let Some(cord) = self.cords.iter().find(|cord| cord.identity == identity) {
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind: PatchbaySubjectKind::Cord,
                exact_facts: vec![
                    format!("from {}", cord.source_port),
                    format!("to {}", cord.sink_port),
                    format!("Info {}", cord.value_kind.as_str()),
                    format!("temporal={:?}", cord.temporal),
                    "semantic parameters: none exposed by this Cord contract".into(),
                    "Line / transport choices belong to realization".into(),
                ],
            });
        }
        Err(PatchbayGraphError::UnknownSubject)
    }

    pub fn preflight_value(
        &self,
        identity: &str,
        canonical: &[u8],
    ) -> Result<PatchbayValuePreflight, PatchbayGraphError> {
        let contract = self.value_contract(identity).ok_or_else(|| {
            if self.subject_identities().any(|subject| subject == identity) {
                PatchbayGraphError::NoValueContract
            } else {
                PatchbayGraphError::UnknownSubject
            }
        })?;
        let disposition = match contract.validate(canonical) {
            Ok(()) => PatchbayValueDisposition::Accepted,
            Err(refusal) => PatchbayValueDisposition::Refused(refusal),
        };
        Ok(PatchbayValuePreflight {
            subject_identity: identity.into(),
            contract: contract.clone(),
            disposition,
        })
    }

    fn value_contract(&self, identity: &str) -> Option<&CheckedValueContract> {
        self.front_inputs
            .iter()
            .chain(&self.front_outputs)
            .find(|port| port.identity == identity)
            .and_then(|port| port.value_contract.as_ref())
            .or_else(|| {
                self.compositions
                    .iter()
                    .flat_map(|composition| composition.inputs.iter().chain(&composition.outputs))
                    .find(|port| port.identity == identity)
                    .and_then(|port| port.value_contract.as_ref())
            })
            .or_else(|| {
                self.gears
                    .iter()
                    .flat_map(|gear| gear.inputs.iter().chain(&gear.outputs))
                    .find(|port| port.identity == identity)
                    .and_then(|port| port.value_contract.as_ref())
            })
    }

    fn subject_index(&self, identity: &str) -> Result<usize, PatchbayGraphError> {
        self.subject_identities()
            .position(|candidate| candidate == identity)
            .ok_or(PatchbayGraphError::UnknownSubject)
    }
}

fn append_contract_facts(facts: &mut Vec<String>, contract: Option<&CheckedValueContract>) {
    let Some(contract) = contract else {
        facts.push("value contract: no additional checked refinement".into());
        return;
    };
    facts.push(format!("maximum-bytes={}", contract.maximum_bytes));
    for constraint in &contract.constraints {
        facts.push(constraint_fact(constraint));
    }
}

fn constraint_fact(constraint: &ValueConstraint) -> String {
    match constraint {
        ValueConstraint::ByteLength { minimum, maximum } => {
            format!("constraint byte-length minimum={minimum} maximum={maximum}")
        }
        ValueConstraint::UnsignedRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => open_range_fact("unsigned-range", minimum, maximum, *minimum_endpoint, *maximum_endpoint),
        ValueConstraint::SignedRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => open_range_fact("signed-range", minimum, maximum, *minimum_endpoint, *maximum_endpoint),
        ValueConstraint::FixedIntegerRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => format!(
            "constraint fixed-integer-range minimum={}({minimum_endpoint:?}) maximum={}({maximum_endpoint:?})",
            minimum.as_ref().map(|value| format!("0x{}", hex_bytes(value))).unwrap_or_else(|| "open".into()),
            maximum.as_ref().map(|value| format!("0x{}", hex_bytes(value))).unwrap_or_else(|| "open".into()),
        ),
        ValueConstraint::QuantityRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => open_range_fact("quantity-range", minimum, maximum, *minimum_endpoint, *maximum_endpoint),
        ValueConstraint::FloatFinite => "constraint ieee-float finite=true".into(),
        ValueConstraint::FloatRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => format!(
            "constraint ieee-float-range minimum={}({minimum_endpoint:?}) maximum={}({maximum_endpoint:?})",
            minimum.as_ref().map(|value| format!("0x{}", hex_bytes(value))).unwrap_or_else(|| "open".into()),
            maximum.as_ref().map(|value| format!("0x{}", hex_bytes(value))).unwrap_or_else(|| "open".into()),
        ),
        ValueConstraint::CanonicalMembership { members, negated } => format!(
            "constraint canonical-membership negated={negated} values={} bytes={}",
            members.len(),
            members.iter().map(Vec::len).sum::<usize>()
        ),
        ValueConstraint::TextPattern {
            pattern,
            anchored_start,
            anchored_end,
            negated,
        } => format!(
            "constraint text-pattern negated={negated} anchored-start={anchored_start} anchored-end={anchored_end} states={} start={} maximum-characters={} maximum-steps={}",
            pattern.states.len(),
            pattern.start_state,
            pattern.maximum_input_characters,
            pattern.maximum_match_steps,
        ),
    }
}

fn open_range_fact<T: core::fmt::Debug>(
    name: &str,
    minimum: &Option<T>,
    maximum: &Option<T>,
    minimum_endpoint: conduit_core::IntervalEndpoint,
    maximum_endpoint: conduit_core::IntervalEndpoint,
) -> String {
    let minimum = minimum
        .as_ref()
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| "open".into());
    let maximum = maximum
        .as_ref()
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| "open".into());
    format!("constraint {name} minimum={minimum}({minimum_endpoint:?}) maximum={maximum}({maximum_endpoint:?})")
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        encode_count, kind_id, port_id, CheckedFormId, ExpandedFormId, IntervalEndpoint,
        PortDescriptor, PortTemporal, SourceDocumentId, ValueConstraintRefusal,
    };

    fn graph() -> PatchbayGraph {
        let contract = CheckedValueContract::new(
            kind_id(conduit_core::COUNT_INFO_ID),
            conduit_core::COUNT_ENCODED_LEN as u32,
            vec![ValueConstraint::UnsignedRange {
                minimum: Some(1),
                maximum: Some(4),
                minimum_endpoint: IntervalEndpoint::Exclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        )
        .unwrap();
        PatchbayGraph {
            source_document_id: SourceDocumentId::from("source/refined"),
            checked_form_id: CheckedFormId::from("checked/refined"),
            expanded_form_id: ExpandedFormId::from("expanded/refined"),
            form_name: "refined".into(),
            front_inputs: vec![PatchbayFrontPort {
                identity: "front/input/count".into(),
                descriptor: PortDescriptor {
                    port_id: port_id("count"),
                    value_kind: kind_id(conduit_core::COUNT_INFO_ID),
                    direction: PortDirection::Input,
                    temporal: PortTemporal::Value,
                    abnormal_kind: None,
                },
                value_contract: Some(contract),
            }],
            front_outputs: Vec::new(),
            compositions: Vec::new(),
            gears: Vec::new(),
            cords: Vec::new(),
        }
    }

    #[test]
    fn inspection_exposes_exact_constraint_not_only_kind_and_bound() {
        let inspection = graph().inspect("front/input/count").unwrap();
        assert!(inspection.exact_facts.contains(&"maximum-bytes=8".into()));
        assert!(inspection.exact_facts.iter().any(|fact| {
            fact == "constraint unsigned-range minimum=1(Exclusive) maximum=4(Inclusive)"
        }));
    }

    #[test]
    fn fixed_integer_range_inspection_keeps_exact_canonical_bounds() {
        assert_eq!(
            constraint_fact(&ValueConstraint::FixedIntegerRange {
                minimum: Some(vec![0x00]),
                maximum: Some(vec![0x7f]),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }),
            "constraint fixed-integer-range minimum=0x00(Inclusive) maximum=0x7f(Inclusive)"
        );
    }

    #[test]
    fn preflight_preserves_exact_refusal_without_claiming_observed_evidence() {
        let graph = graph();
        assert_eq!(
            graph
                .preflight_value("front/input/count", &encode_count(2))
                .unwrap()
                .disposition,
            PatchbayValueDisposition::Accepted
        );
        assert_eq!(
            graph
                .preflight_value("front/input/count", &encode_count(1))
                .unwrap()
                .disposition,
            PatchbayValueDisposition::Refused(ValueConstraintRefusal::UnsignedRange)
        );
        assert_eq!(
            graph.preflight_value("renderer/invented", &encode_count(2)),
            Err(PatchbayGraphError::UnknownSubject)
        );
    }

    #[test]
    fn unrefined_subject_is_distinct_from_unknown_subject() {
        let mut graph = graph();
        graph.front_inputs[0].value_contract = None;
        assert_eq!(
            graph.preflight_value("front/input/count", &encode_count(2)),
            Err(PatchbayGraphError::NoValueContract)
        );
    }
}
