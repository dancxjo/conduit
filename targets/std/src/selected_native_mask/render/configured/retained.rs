//! Actual owned payload after canonical/native preparation has succeeded.
//! This is NOT the missing preallocation/factory bound and cannot authorize
//! calling PreparedConfiguredScene::prepare. Incoming Face/config stay separate.
use super::*;
use conduit_face::*;
use core::mem::size_of;

#[derive(Default)]
struct Bytes(usize);
impl Bytes {
    fn add(&mut self, bytes: usize) -> Option<()> {
        self.0 = self.0.checked_add(bytes)?;
        Some(())
    }
    fn strings(&mut self, values: &[&String]) -> Option<()> {
        for text in values {
            self.add(text.capacity())?;
        }
        Some(())
    }
    fn sequence<T, const N: usize>(
        &mut self,
        values: &conduit_plot::rust_binding::BoundedSequence<T, N>,
    ) -> Option<()> {
        if size_of::<T>() != 0 {
            self.add(values.physical_capacity().checked_mul(size_of::<T>())?)?;
        }
        Some(())
    }
    fn look(&mut self, look: &LookInfo) -> Option<()> {
        self.add(look.identity().capacity())?;
        if let LookVisualFacetOption::Some(visual) = look.visual() {
            self.add(visual.expression().identity().capacity())?;
        }
        // Remaining Look fields are generated inline numeric/Copy enums and
        // ThemeColor values. Spoken/generative facet choices own no strings.
        Some(())
    }
    fn selector(&mut self, selector: &ComponentSubjectSelector) -> Option<()> {
        match selector {
            ComponentSubjectSelector::Subject(id)
            | ComponentSubjectSelector::ActionTarget(id)
            | ComponentSubjectSelector::Role(id) => self.add(id.capacity()),
            ComponentSubjectSelector::Collection(binding) => self.add(binding.mode().capacity()),
            ComponentSubjectSelector::Bound(binding) => {
                self.add(binding.subject().capacity())?;
                match binding.role() {
                    ComponentBindingRole::Role(role) | ComponentBindingRole::Semantic(role) => {
                        self.add(role.capacity())?
                    }
                }
                self.sequence(binding.properties())?;
                for property in binding.properties() {
                    self.strings(&[property.name(), property.kind(), property.value_kind()])?;
                }
                self.sequence(binding.actions())?;
                for action in binding.actions() {
                    self.strings(&[action.identity(), action.intent()])?;
                    self.sequence(action.arguments())?;
                    for argument in action.arguments() {
                        self.strings(&[
                            argument.name(),
                            argument.value_kind(),
                            argument.contract_digest(),
                        ])?;
                    }
                }
                Some(())
            }
        }
    }
    fn provenance(&mut self, provenance: &FaceUtteranceProvenance) -> Option<()> {
        match provenance {
            FaceUtteranceProvenance::Subject(value) => self.add(value.identity().capacity()),
            FaceUtteranceProvenance::Composition(value) => self.add(value.identity().capacity()),
            FaceUtteranceProvenance::Action(value) => self.add(value.identity().capacity()),
            FaceUtteranceProvenance::ActionArgument(value) => {
                self.strings(&[value.action_identity(), value.argument_name()])
            }
            FaceUtteranceProvenance::Relationship(_)
            | FaceUtteranceProvenance::Text(_)
            | FaceUtteranceProvenance::Property(_)
            | FaceUtteranceProvenance::Disclosure(_)
            | FaceUtteranceProvenance::TemporalReference(_)
            | FaceUtteranceProvenance::TemporalFact(_) => Some(()),
        }
    }
}
impl PreparedConfiguredScene<'_> {
    /// Observed payload capacities after successful preparation. Excludes
    /// inline Self, borrowed original Face/configuration and allocator metadata.
    /// No decoder reservation, independent caller loan or ownership retirement
    /// follows from this value; it is only a post-factory capacity visitor.
    pub(in crate::selected_native_mask::render) fn retained_backing_bytes(&self) -> Option<usize> {
        let mut n = Bytes::default();
        let structure = self.structure.structure();
        n.strings(&[structure.identity(), structure.recipe_identity()])?;
        n.sequence(structure.candidates())?;
        for candidate in structure.candidates() {
            n.sequence(candidate.nodes())?;
            for node in candidate.nodes() {
                match node {
                    ComponentLayoutNode::Region(_) | ComponentLayoutNode::Fallback => (),
                    ComponentLayoutNode::Row(group) => n.sequence(group.children())?,
                    ComponentLayoutNode::Column(group) => n.sequence(group.children())?,
                    ComponentLayoutNode::Grid(grid) => n.sequence(grid.group().children())?,
                }
            }
        }
        let prepared = self.structure.recipe();
        n.add(prepared.recipe.identity().capacity())?;
        n.sequence(prepared.recipe.regions())?;
        for region in prepared.recipe.regions() {
            n.add(region.identity().capacity())?;
            n.selector(region.selector())?;
        }
        n.add(
            prepared
                .subject_regions
                .capacity()
                .checked_mul(size_of::<Option<usize>>())?,
        )?;
        let reading = &prepared.reading;
        n.add(reading.source_face_identity.capacity())?;
        n.add(
            reading
                .clauses
                .capacity()
                .checked_mul(size_of::<FaceUtteranceClause>())?,
        )?;
        for clause in &reading.clauses {
            n.add(clause.text.capacity())?;
            n.provenance(&clause.provenance)?;
        }
        n.look(&self.look.requested)?;
        n.look(&self.look.selected)?;
        n.add(self.look.visual.expression.identity.capacity())?;
        Some(n.0)
    }
}
