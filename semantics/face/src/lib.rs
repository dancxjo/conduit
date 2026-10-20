#![no_std]

//! The bounded universal grammar spoken by a Body's Face.
//!
//! [`Face`] carries semantic subjects, relationships, facts, wording,
//! controls, disclosure, time, and exact provenance without choosing a widget,
//! scene, spoken script, or renderer mechanism. Ordinary Mask Plots interpret
//! this grammar into Shows. [`SemanticApplicationView`] and [`ApplicationView`]
//! remain finite downstream composition and compatibility vocabularies; they
//! do not define the Face boundary.

extern crate alloc;

pub mod application_canvas;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));

    pub(crate) fn generated_action_affordance_checked_type(
    ) -> Result<conduit_plot::CheckedNativeType, conduit_plot::rust_binding::NativeBindingRefusal>
    {
        let value_type = GeneratedActionAffordance::semantic_type()?;
        let conduit_core::StructuredInfoTypeShape::Record { schema, .. } = value_type.shape()
        else {
            return Err(
                conduit_plot::rust_binding::NativeBindingRefusal::InvalidValue(
                    conduit_core::StructuredInfoRefusal::WrongType,
                ),
            );
        };
        Ok(conduit_plot::CheckedNativeType {
            name: "GeneratedActionAffordance".into(),
            identity: schema.clone(),
            value_type,
            value_contracts: GeneratedActionAffordance::value_contracts(),
            // build.rs fences any future nested owner where law.
            invariants: alloc::vec::Vec::new(),
        })
    }

    pub(crate) fn action_input_binding_checked_type(
    ) -> Result<conduit_plot::CheckedNativeType, conduit_plot::rust_binding::NativeBindingRefusal>
    {
        let value_type = ActionInputBindingProposalInfo::semantic_type()?;
        let conduit_core::StructuredInfoTypeShape::Record { schema, .. } = value_type.shape()
        else {
            return Err(
                conduit_plot::rust_binding::NativeBindingRefusal::InvalidValue(
                    conduit_core::StructuredInfoRefusal::WrongType,
                ),
            );
        };
        Ok(conduit_plot::CheckedNativeType {
            name: "ActionInputBindingProposalInfo".into(),
            identity: schema.clone(),
            value_type,
            value_contracts: ActionInputBindingProposalInfo::value_contracts(),
            // build.rs refuses added owner/nested where laws until this import retains them.
            invariants: alloc::vec::Vec::new(),
        })
    }

    impl ThemeColor {
        pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
            Self { red, green, blue }
        }

        pub const fn packed_rgb(self) -> u32 {
            ((self.red as u32) << 16) | ((self.green as u32) << 8) | self.blue as u32
        }
    }
}
pub use generated::{
    LookFacetAvailability, LookFacetRequest, LookGenerativeFacetInfo, LookGenerativeFacetOption,
    LookGenerativeRegister, LookInfo, LookSpokenDelivery, LookSpokenFacetInfo,
    LookSpokenFacetOption, LookVerbosity, LookVisualFacetInfo, LookVisualFacetOption,
    LookVisualTokensInfo,
};

pub use generated::{
    ActionInputBindingProposalInfo, AdmittedNavigationDestination, ApplicationAction,
    ApplicationComponent, ApplicationComponentForm, ApplicationEventKind, ApplicationEventKindForm,
    ApplicationNodeState, ApplicationNodeStateForm, ApplicationViewRefusal, ChoiceMultiplicity,
    ComponentActionBinding, ComponentArgumentBinding, ComponentBindingRole,
    ComponentPropertyBinding, ComponentRecipeInfo, ComponentRegionInfo, ComponentSubjectBinding,
    ComponentSubjectSelector, ComponentSubjectSelectorBound, CompositionItemKind,
    CompositionItemKindForm, CompositionRole, CompositionRoleForm, EvidenceDisposition, Extent2,
    FaceAspect, FaceAspectForm, FaceCompositionKind, FaceCompositionKindSemantic,
    FaceCompositionRelation, FaceContributionRole, FaceContributionRoleForm, FaceDepth,
    FaceDepthForm, FaceDisclosureLevel, FaceMechanismKind, FacePlace, FacePlaceForm,
    FaceTemporalRole, FaceUtteranceClauseKind, FaceUtteranceProvenance,
    FaceUtteranceProvenanceAction, FaceUtteranceProvenanceActionArgument,
    FaceUtteranceProvenanceComposition, FaceUtteranceProvenanceDisclosure,
    FaceUtteranceProvenanceProperty, FaceUtteranceProvenanceRelationship,
    FaceUtteranceProvenanceSubject, FaceUtteranceProvenanceTemporalFact,
    FaceUtteranceProvenanceTemporalReference, FaceUtteranceProvenanceText,
    GeneratedActionAffordance, GeneratedContentRole, GeneratedManifestationDisposition,
    GenerativeNarratorRole, GenerativeNarratorRoleForm, GraphicsClipClass, GraphicsCommandKind,
    GraphicsCommandKindForm, GraphicsPaintRole, GraphicsPaintRoleForm, GraphicsPoint,
    GraphicsShapeStyle, GraphicsShapeStyleForm, GraphicsTextRole, GraphicsTextRoleForm,
    ImageColorProfile, ImageFormat, ImagePixelExtent, ImageRegion2, ImageResource, LayoutAlignment,
    LayoutAxis, LayoutError, LayoutFrame, LayoutRect, MaskPlanningDisposition,
    MaskPlanningDispositionForm, MaskWardrobeError, MaskWardrobeErrorForm, MaskWardrobeLifetime,
    MaskWardrobeLifetimeForm, NavigationRefusal, NavigationRefusalForm, Path2Four, Point2, Point3,
    Rect2, RoboticsPose2, StatusKind, ThemeColor, Transform2, Vector2, Vector3, VisionColorSample,
    VisionDetection, VisionDetectionSlot, VisionDetectionsFour, VisionEvidenceClass,
    VisionKeypoint, VisionLandmarkSlot, VisionLandmarks, VisionMotionObservation,
    VisionMotionsFour, VisionObjectObservation, VisionObjectObservations,
    VisionObservationEvidenceClass, VisionObservationProvenance, VisionObservationSigns,
    VisionOptionalPixelRegion, VisionPixelRegion, VisionProvenance, VisionRgbSample,
    VisionTextsEight, VisionTrackObservation, VisionTracksFour, VisionVisibleTextObservation,
    VisualDensityPreference, VisualExperience, VisualExperienceObservation,
    VisualExperienceObservationImpression, VisualExperienceObservationMotion,
    VisualExperienceObservationObject, VisualExperienceObservationTrack,
    VisualExperienceObservationVisibleText, VisualExperienceObservations, VisualExperienceRelation,
    VisualExperienceRelations, VisualExpressionInfo, VisualImpression, VisualImpressionDisposition,
    VisualMotionPreference, VisualPalettePreference, VisualSelectedObservationSigns,
};

mod application_event;
mod application_view;
mod aural;
mod bitmap;
mod bitmap_catalog;
mod body_face;
mod calendar_time;
mod committed_state_basis;
mod composition;
mod construction;
mod contract;
mod face;
mod face_fragment;
mod face_reading;
mod generated_correlation;
mod generated_validation;
mod generative_interaction;
mod generative_manifestation;
#[cfg(test)]
mod generative_manifestation_tests;
mod generative_presenter;
mod generative_presenter_policy;
mod geometry;
#[cfg(feature = "plot-catalog")]
mod geometry_catalog;
mod graphics;
mod identity;
mod interaction;
mod interaction_ledger;
mod layout;
mod linear;
mod linear_navigation;
mod look;
mod look_selection;
mod manifestation;
mod manifestation_set;
#[cfg(feature = "plot-catalog")]
mod mask_catalog;
mod mask_journey;
mod mask_plot;
mod mask_routes;
mod mask_show;
mod mask_topology;
mod mask_wardrobe;
mod mask_wardrobe_control;
mod navigation;
mod navigation_journey;
mod navigation_observation;
mod orifina_face;
mod owner_face_plan;
mod owner_face_route;
mod owner_mask_route;
mod projection;
mod readable_choices;
mod remote_owner_mask_route;
mod renderer_execution;
mod renderer_inspection;
mod rhetorical_composition;
mod semantic_ui;
mod semantics;
mod spoken_mask;
mod stroke_capture;
mod structured_info;
mod temporal;
mod temporal_model;
mod temporal_wording;
mod visual_expression;

pub use application_event::*;
pub use application_view::*;
pub use aural::*;
pub use bitmap::*;
pub use bitmap_catalog::*;
pub use body_face::*;
pub use calendar_time::*;
pub use committed_state_basis::*;
pub use composition::*;
pub use contract::*;
pub use face::*;
pub use face_fragment::*;
pub use face_reading::*;
pub use generated_validation::*;
pub use generative_interaction::*;
pub use generative_manifestation::*;
pub use generative_presenter::*;
pub use generative_presenter_policy::*;
pub use geometry::*;
#[cfg(feature = "plot-catalog")]
pub use geometry_catalog::*;
pub use graphics::*;
pub use interaction::*;
pub use interaction_ledger::*;
pub use layout::*;
pub use linear::*;
pub use linear_navigation::*;
pub use look::*;
pub use look_selection::*;
pub use manifestation::*;
pub use manifestation_set::*;
#[cfg(feature = "plot-catalog")]
pub use mask_catalog::*;
pub use mask_journey::*;
pub use mask_plot::*;
pub use mask_routes::*;
pub use mask_show::*;
pub use mask_topology::*;
pub use mask_wardrobe::*;
pub use mask_wardrobe_control::*;
pub use navigation::*;
pub use navigation_journey::*;
pub use navigation_observation::*;
pub use orifina_face::*;
pub use owner_face_plan::*;
pub use owner_face_route::*;
pub use owner_mask_route::*;
pub use projection::*;
pub use readable_choices::*;
pub use remote_owner_mask_route::*;
pub use renderer_execution::*;
pub use renderer_inspection::*;
pub use rhetorical_composition::*;
pub use semantic_ui::*;
pub use semantics::*;
pub use spoken_mask::*;
pub use stroke_capture::*;
pub use structured_info::*;
pub use temporal::*;
pub use temporal_model::*;
pub use temporal_wording::*;
pub use visual_expression::*;

mod typed_value;
pub use typed_value::{display_typed_value, spoken_typed_value, MAX_FACE_VALUE_BYTES};

#[cfg(feature = "plot-catalog")]
mod configured_mask;
#[cfg(feature = "plot-catalog")]
pub use configured_mask::*;
mod visual_expression_info;
pub use visual_expression_info::*;
mod boolean_replacement;
mod component_recipe;
pub use boolean_replacement::reversible_boolean_value;
mod collections;
pub use collections::*;
mod component_slots;
pub use component_recipe::*;

mod checked_source_plan;
pub use checked_source_plan::*;

mod input_binding_producer;
pub use input_binding_producer::*;

mod input_binding_correlation;
pub use input_binding_correlation::*;

mod parsec_map_transition;
pub use parsec_map_transition::*;

mod spoken_template_artifact;
pub use spoken_template_artifact::*;

mod spoken_template_looks;
pub use spoken_template_looks::*;

mod parsec_input_policy_producer;
pub use parsec_input_policy_producer::*;
mod parsec_instruction_frame;
pub use parsec_instruction_frame::{
    InspectedParsecInstructionFrame, PreparedParsecInstructionFrameInspection,
};
mod parsec_input_instruction_producer;
pub use parsec_input_instruction_producer::*;
