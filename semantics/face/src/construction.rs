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

/// A constructor-checked Face whose contents remain immutable until consumed.
/// This is validation provenance, not permission to perform an interaction.
///
/// ```compile_fail
/// fn mutate(checked: &mut conduit_face::CheckedFace) {
///     checked.as_face().revision += 1;
/// }
/// ```
/// ```compile_fail
/// fn forge(face: conduit_face::Face) -> conduit_face::CheckedFace {
///     conduit_face::CheckedFace { face }
/// }
/// ```
#[derive(Debug)]
pub struct CheckedFace {
    face: Face,
}

impl CheckedFace {
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
        Face::new_with_semantics(
            revision,
            basis,
            subjects,
            relationships,
            properties,
            text,
            actions,
            disclosures,
        )
        .map(|face| Self { face })
    }

    pub fn as_face(&self) -> &Face {
        &self.face
    }

    pub fn into_face(self) -> Face {
        self.face
    }

    pub fn validate_interaction(
        &self,
        input: &crate::FaceInteraction,
        show: &crate::MaskShow,
    ) -> Result<(), crate::FaceInteractionRefusal> {
        input.validate_against_validated(
            &crate::mask_show::ValidatedFace::from_constructed(self),
            show,
        )
    }
}
