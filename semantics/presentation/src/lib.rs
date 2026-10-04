#![no_std]

//! The bounded universal grammar spoken by a Body's Face.
//!
//! [`Presentation`] carries semantic subjects, relationships, facts, wording,
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
    AdmittedNavigationDestination, ApplicationAction, ApplicationComponent,
    ApplicationComponentForm, ApplicationEventKind, ApplicationEventKindForm, ApplicationNodeState,
    ApplicationNodeStateForm, ApplicationViewRefusal, ChoiceMultiplicity, CompositionItemKind,
    CompositionItemKindForm, CompositionRole, CompositionRoleForm, EvidenceDisposition, Extent2,
    FaceContributionRole, FaceContributionRoleForm, FaceUtteranceClauseKind,
    FaceUtteranceProvenance, FaceUtteranceProvenanceAction, FaceUtteranceProvenanceActionArgument,
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
    PresentationAspect, PresentationAspectForm, PresentationCompositionKind,
    PresentationCompositionKindSemantic, PresentationCompositionRelation, PresentationDepth,
    PresentationDepthForm, PresentationDisclosureLevel, PresentationMechanismKind,
    PresentationPlace, PresentationPlaceForm, PresentationTemporalRole, Rect2, RoboticsPose2,
    StatusKind, ThemeColor, Transform2, Vector2, Vector3, VisionColorSample, VisionDetection,
    VisionDetectionSlot, VisionDetectionsFour, VisionEvidenceClass, VisionKeypoint,
    VisionLandmarkSlot, VisionLandmarks, VisionMotionObservation, VisionMotionsFour,
    VisionObjectObservation, VisionObjectObservations, VisionObservationEvidenceClass,
    VisionObservationProvenance, VisionObservationSigns, VisionOptionalPixelRegion,
    VisionPixelRegion, VisionProvenance, VisionRgbSample, VisionTextsEight, VisionTrackObservation,
    VisionTracksFour, VisionVisibleTextObservation, VisualExperience, VisualExperienceObservation,
    VisualExperienceObservationImpression, VisualExperienceObservationMotion,
    VisualExperienceObservationObject, VisualExperienceObservationTrack,
    VisualExperienceObservationVisibleText, VisualExperienceObservations, VisualExperienceRelation,
    VisualExperienceRelations, VisualImpression, VisualImpressionDisposition,
    VisualSelectedObservationSigns,
};

mod application_event;
mod application_theme;
mod application_view;
mod aural;
mod bitmap;
mod bitmap_catalog;
mod calendar_time;
mod composition;
mod construction;
mod contract;
mod face;
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
mod orifina_presentation;
mod owner_face_route;
mod owner_mask_route;
mod owner_presentation_plan;
mod presentation;
mod presentation_fragment;
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

pub use application_event::*;
pub use application_theme::*;
pub use application_view::*;
pub use aural::*;
pub use bitmap::*;
pub use bitmap_catalog::*;
pub use calendar_time::*;
pub use composition::*;
pub use contract::*;
pub use face::*;
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
pub use orifina_presentation::*;
pub use owner_face_route::*;
pub use owner_mask_route::*;
pub use owner_presentation_plan::*;
pub use presentation::*;
pub use presentation_fragment::*;
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
