//! Capacity-based ownership accounting; never encoded-length approximation.
use crate::*;
use alloc::vec::Vec;
use core::mem::size_of;

impl FaceContentId {
    /// Owned String payload only; excludes the inline identity wrapper.
    pub fn retained_backing_bytes(&self) -> usize {
        self.0.capacity()
    }
}

#[derive(Default)]
struct Bytes(usize);
impl Bytes {
    fn add(&mut self, bytes: usize) -> Option<()> {
        self.0 = self.0.checked_add(bytes)?;
        Some(())
    }
    fn vector<T>(&mut self, values: &Vec<T>) -> Option<()> {
        self.add(values.capacity().checked_mul(size_of::<T>())?)
    }
    fn relationship(&mut self, kind: &FaceRelationshipKind) -> Option<()> {
        if let FaceRelationshipKind::Semantic(kind) = kind {
            self.add(kind.0.capacity())?;
        }
        Some(())
    }
}
impl Face {
    /// Actual owned heap payload, including spare capacities and nested value
    /// contracts. Excludes this inline Face, allocator overhead, serialization,
    /// scenes and transient preparation workspace. Those owners charge separately.
    /// No validation, allocation, effects or authority follow from inspection.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut n = Bytes::default();
        n.add(self.identity.0.capacity())?;
        if let Some(id) = &self.basis.body_id {
            n.add(id.retained_backing_bytes())?;
        }
        if let Some(id) = &self.basis.wake_id {
            n.add(id.retained_backing_bytes())?;
        }
        for capacity in [
            self.basis
                .source_document_id
                .as_ref()
                .map(|id| id.0.capacity()),
            self.basis
                .checked_plot_id
                .as_ref()
                .map(|id| id.0.capacity()),
            self.basis
                .expanded_plot_id
                .as_ref()
                .map(|id| id.0.capacity()),
            self.basis.plan_id.as_ref().map(|id| id.0.capacity()),
            self.basis.active_play_id.as_ref().map(|id| id.0.capacity()),
        ]
        .into_iter()
        .flatten()
        {
            n.add(capacity)?;
        }
        n.vector(&self.basis.sign_ids)?;
        for id in &self.basis.sign_ids {
            n.add(id.0.capacity())?;
        }
        n.add(self.interaction_context.identity.capacity())?;
        n.vector(&self.interaction_context.basis)?;
        for basis in &self.interaction_context.basis {
            n.add(basis.source.capacity())?;
            n.add(basis.target.capacity())?;
            n.relationship(&basis.relationship)?;
        }
        n.vector(&self.subjects)?;
        for subject in &self.subjects {
            n.add(subject.identity.capacity())?;
            n.add(subject.name.capacity())?;
            if let FaceRole::Semantic(kind) = &subject.role {
                n.add(kind.0.capacity())?;
            }
        }
        n.vector(&self.relationships)?;
        for relation in &self.relationships {
            n.add(relation.source.capacity())?;
            n.add(relation.target.capacity())?;
            n.relationship(&relation.kind)?;
        }
        n.vector(&self.composition)?;
        for relation in &self.composition {
            n.add(relation.identity.capacity())?;
            n.add(relation.source.capacity())?;
            n.add(relation.target.capacity())?;
            if let FaceCompositionKind::Semantic(kind) = &relation.kind {
                n.add(kind.identity().capacity())?;
            }
        }
        n.vector(&self.properties)?;
        for property in &self.properties {
            n.add(property.retained_backing_bytes()?)?;
        }
        n.vector(&self.text)?;
        for text in &self.text {
            n.add(text.subject.capacity())?;
            n.add(text.text.capacity())?;
        }
        n.vector(&self.actions)?;
        for action in &self.actions {
            n.add(action.identity.capacity())?;
            n.add(action.intent.capacity())?;
            n.add(action.target.capacity())?;
            n.add(action.name.capacity())?;
            n.vector(&action.arguments)?;
            for argument in &action.arguments {
                n.add(argument.name.capacity())?;
                n.add(argument.value_name.capacity())?;
                n.add(argument.contract.retained_backing_bytes()?)?;
            }
            match &action.availability {
                FaceActionAvailability::Available => (),
                FaceActionAvailability::Unavailable {
                    reason_code,
                    explanation,
                }
                | FaceActionAvailability::Refused {
                    reason_code,
                    explanation,
                } => {
                    n.add(reason_code.capacity())?;
                    n.add(explanation.capacity())?;
                }
            }
        }
        n.vector(&self.disclosures)?;
        for disclosure in &self.disclosures {
            n.add(disclosure.subject.capacity())?;
        }
        n.vector(&self.temporal_references)?;
        for reference in &self.temporal_references {
            n.add(reference.identity.capacity())?;
            n.add(reference.instant.clock_basis.capacity())?;
        }
        n.vector(&self.temporal_facts)?;
        for fact in &self.temporal_facts {
            n.add(fact.subject.capacity())?;
            n.add(fact.reference.capacity())?;
            n.add(fact.source.clock_basis.capacity())?;
            if let Some(sign) = &fact.sign_id {
                n.add(sign.0.capacity())?;
            }
        }
        Some(n.0)
    }
}

#[cfg(test)]
#[path = "retained_backing/tests.rs"]
mod tests;

impl FaceProperty {
    /// Owned payload only; the containing array/vector charges inline storage.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut n = Bytes::default();
        n.add(self.subject.capacity())?;
        n.add(self.name.capacity())?;
        match &self.value {
            FacePropertyValue::Identity(s) | FacePropertyValue::Text(s) => n.add(s.capacity())?,
            FacePropertyValue::BaseImplementationId(id) => n.add(id.0.capacity())?,
            FacePropertyValue::Count(_)
            | FacePropertyValue::Signed(_)
            | FacePropertyValue::Flag(_) => (),
            FacePropertyValue::ValueContract(contract) => {
                n.add(contract.retained_backing_bytes()?)?
            }
            FacePropertyValue::Content(bytes) => n.add(bytes.capacity())?,
            FacePropertyValue::TypedValue { contract, bytes } => {
                n.add(contract.retained_backing_bytes()?)?;
                n.add(bytes.capacity())?;
            }
        }
        Some(n.0)
    }
}
