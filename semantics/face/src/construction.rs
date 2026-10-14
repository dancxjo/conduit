//! Construction entry points for immutable portable Face revisions.

use alloc::vec::Vec;

use crate::{
    Face, FaceAction, FaceBasis, FaceContentId, FaceDisclosure, FaceError, FaceProperty,
    FaceRelationship, FaceSubject, FaceText,
};

impl Face {
    /// Replace provenance with an exact producing Body/Plot/Plan basis and
    /// derive a new immutable Face identity.
    pub fn with_basis(mut self, basis: FaceBasis) -> Result<Self, FaceError> {
        self.basis = basis;
        self.validate_content()?;
        self.identity = FaceContentId(self.content_digest());
        Ok(self)
    }

    pub fn with_composition(
        mut self,
        composition: Vec<crate::FaceCompositionRelation>,
    ) -> Result<Self, FaceError> {
        self.composition = composition;
        self.validate_content()?;
        self.identity = FaceContentId(self.content_digest());
        Ok(self)
    }

    /// Bind this immutable semantic content to one exact interaction context.
    ///
    /// Context changes produce a new Face identity, so navigation,
    /// actions, arguments, Shows, and Manifestations correlated to the old
    /// identity become stale without a separate audience-state channel.
    pub fn with_interaction_context(
        mut self,
        context: crate::FaceInteractionContext,
    ) -> Result<Self, FaceError> {
        self.interaction_context = context;
        self.validate_content()?;
        self.identity = FaceContentId(self.content_digest());
        Ok(self)
    }

    pub fn new(
        revision: u64,
        basis: FaceBasis,
        subjects: Vec<FaceSubject>,
        relationships: Vec<FaceRelationship>,
        properties: Vec<FaceProperty>,
        text: Vec<FaceText>,
    ) -> Result<Self, FaceError> {
        Self::new_with_semantics(
            revision,
            basis,
            subjects,
            relationships,
            properties,
            text,
            Vec::new(),
            Vec::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_semantics(
        revision: u64,
        basis: FaceBasis,
        subjects: Vec<FaceSubject>,
        relationships: Vec<FaceRelationship>,
        properties: Vec<FaceProperty>,
        text: Vec<FaceText>,
        actions: Vec<FaceAction>,
        disclosures: Vec<FaceDisclosure>,
    ) -> Result<Self, FaceError> {
        Self::new_with_semantics_and_temporal(
            revision,
            basis,
            subjects,
            relationships,
            properties,
            text,
            actions,
            disclosures,
            Vec::new(),
            Vec::new(),
        )
    }
}
