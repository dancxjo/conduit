//! Construction entry points for immutable portable Presentation revisions.

use alloc::vec::Vec;

use crate::{
    Presentation, PresentationAction, PresentationBasis, PresentationContentId,
    PresentationDisclosure, PresentationError, PresentationProperty, PresentationRelationship,
    PresentationSubject, PresentationText,
};

impl Presentation {
    /// Replace provenance with an exact producing Body/Plot/Plan basis and
    /// derive a new immutable Face identity.
    pub fn with_basis(mut self, basis: PresentationBasis) -> Result<Self, PresentationError> {
        self.basis = basis;
        self.validate_content()?;
        self.identity = PresentationContentId(self.content_digest());
        Ok(self)
    }

    pub fn with_composition(
        mut self,
        composition: Vec<crate::PresentationCompositionRelation>,
    ) -> Result<Self, PresentationError> {
        self.composition = composition;
        self.validate_content()?;
        self.identity = PresentationContentId(self.content_digest());
        Ok(self)
    }

    /// Bind this immutable semantic content to one exact interaction context.
    ///
    /// Context changes produce a new Presentation identity, so navigation,
    /// actions, arguments, Shows, and Manifestations correlated to the old
    /// identity become stale without a separate audience-state channel.
    pub fn with_interaction_context(
        mut self,
        context: crate::PresentationInteractionContext,
    ) -> Result<Self, PresentationError> {
        self.interaction_context = context;
        self.validate_content()?;
        self.identity = PresentationContentId(self.content_digest());
        Ok(self)
    }

    pub fn new(
        revision: u64,
        basis: PresentationBasis,
        subjects: Vec<PresentationSubject>,
        relationships: Vec<PresentationRelationship>,
        properties: Vec<PresentationProperty>,
        text: Vec<PresentationText>,
    ) -> Result<Self, PresentationError> {
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
        basis: PresentationBasis,
        subjects: Vec<PresentationSubject>,
        relationships: Vec<PresentationRelationship>,
        properties: Vec<PresentationProperty>,
        text: Vec<PresentationText>,
        actions: Vec<PresentationAction>,
        disclosures: Vec<PresentationDisclosure>,
    ) -> Result<Self, PresentationError> {
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
