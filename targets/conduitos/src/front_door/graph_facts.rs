//! Merge only a checked readonly contribution bound to the observed owner revision.
use super::*;
use alloc::vec::Vec;
use conduit_face::{FaceDisclosure, FaceProperty, FaceText};
impl FrontDoor {
    pub(super) fn append_graph_facts(
        &self,
        graph: &patchbay_graph::PatchbayGraph,
        body: &conduit_body::BodyId,
        subjects: &mut Vec<FaceSubject>,
        relationships: &mut Vec<FaceRelationship>,
        properties: &mut Vec<FaceProperty>,
        text: &mut Vec<FaceText>,
        disclosures: &mut Vec<FaceDisclosure>,
    ) -> Result<(), Error> {
        let contribution = self.graph_facts.as_ref().ok_or(Error::Face)?;
        let journey = self.journey.as_ref().ok_or(Error::Face)?;
        let workspace = self.workspace.as_ref().ok_or(Error::Face)?;
        let scope = &contribution.scope;
        let plot = format!("plot/{}", graph.checked_plot_id.as_str());
        if scope.host != self.host_id
            || scope.boot != self.boot_id
            || &scope.body != body
            || scope.owner_revision != journey.revision
            || scope.owner_revision != workspace.revision
            || scope.plot_present != subjects.iter().any(|subject| subject.identity == plot)
        {
            return Err(Error::Face);
        }
        // Only observation changes this graph; every such boundary clears this
        // cache before requesting fresh facts. Recheck its digest for exact custody.
        scope.validate(graph).map_err(|_| Error::Face)?;
        subjects.extend(contribution.subjects.iter().cloned());
        relationships.extend(contribution.relationships.iter().cloned());
        properties.extend(contribution.properties.iter().cloned());
        text.extend(contribution.text.iter().cloned());
        disclosures.extend(contribution.disclosures.iter().cloned());
        Ok(())
    }
}
