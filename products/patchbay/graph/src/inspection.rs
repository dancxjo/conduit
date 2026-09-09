//! Exact subject references and inspection facts.
use crate::*;
use conduit_core::PortDirection;

impl PatchbayGraph {
    pub fn subject_identities(&self) -> impl Iterator<Item = &str> {
        self.face_inputs
            .iter()
            .chain(&self.face_outputs)
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
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind,
                exact_facts: vec![
                    format!("Composition {}", composition.gear_name),
                    format!("Back {}", composition.back_name),
                    format!("Port {}", port.descriptor.port_id.as_str()),
                    format!("Info {}", port.descriptor.value_kind.as_str()),
                    format!("temporal={:?}", port.descriptor.temporal),
                ],
            });
        }
        if let Some(port) = self
            .face_inputs
            .iter()
            .chain(&self.face_outputs)
            .find(|port| port.identity == identity)
        {
            let subject_kind = match port.descriptor.direction {
                PortDirection::Input => PatchbaySubjectKind::FaceInput,
                PortDirection::Output => PatchbaySubjectKind::FaceOutput,
            };
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind,
                exact_facts: vec![
                    format!("Face Port {}", port.descriptor.port_id.as_str()),
                    format!("direction={:?}", port.descriptor.direction),
                    format!("Info {}", port.descriptor.value_kind.as_str()),
                    format!("temporal={:?}", port.descriptor.temporal),
                    "authoring boundary; runnable root requires an exact binding".into(),
                ],
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
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind,
                exact_facts: vec![
                    format!("Gear {}", port.gear_id.as_str()),
                    format!("Port {}", port.descriptor.port_id.as_str()),
                    format!("Info {}", port.descriptor.value_kind.as_str()),
                    format!("temporal={:?}", port.descriptor.temporal),
                ],
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

    fn subject_index(&self, identity: &str) -> Result<usize, PatchbayGraphError> {
        self.subject_identities()
            .position(|candidate| candidate == identity)
            .ok_or(PatchbayGraphError::UnknownSubject)
    }
}
