//! Portable finite education, assessment, hint, feedback, and progress Info.
//!
//! Content profiles remain explicit identifiers. The lesson substrate does not
//! retain learner history or choose a Presenter, classroom model, or evaluator.

use alloc::{vec, vec::Vec};
use conduit_core::{kind_id, StructuredFieldType, StructuredInfoType};
pub use conduit_education::{
    education_answer_type, education_assessment_outcome_type, education_assessment_type,
    education_evidence_class_type, education_feedback_provenance_type, education_hint_type,
    education_hints_type, education_lesson_feedback_type, education_optional_hint_type,
    education_progress_state_type, education_progress_type, education_question_type,
    education_refused_response_type, education_response_event_type, education_response_type,
    EducationAnswer, EducationAssessment, EducationAssessmentOutcome, EducationEvidenceClass,
    EducationFeedbackProvenance, EducationHint, EducationHints, EducationLessonFeedback,
    EducationOptionalHint, EducationOptionalHintProvided, EducationProgress,
    EducationProgressState, EducationQuestion, EducationRefusedResponse, EducationResponse,
    EducationResponseEvent, EducationResponseHintRequest, EducationResponseRefused,
    EducationResponseTimeout, MAXIMUM_EDUCATION_HINTS,
};

use crate::timing_feedback_type;

pub const EDUCATION_QUESTION_TYPE: &str = "EducationQuestion";
pub const EDUCATION_RESPONSE_TYPE: &str = "EducationResponse";
pub const EDUCATION_ASSESSMENT_TYPE: &str = "EducationAssessment";
pub const EDUCATION_HINT_TYPE: &str = "EducationHint";
pub const EDUCATION_LESSON_FEEDBACK_TYPE: &str = "EducationLessonFeedback";
pub const EDUCATION_PROGRESS_TYPE: &str = "EducationProgress";
pub const EDUCATION_RHYTHM_FEEDBACK_TYPE: &str = "EducationRhythmFeedback";

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed education field")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed education record")
}

pub fn education_rhythm_feedback_type() -> StructuredInfoType {
    record(
        "education/rhythm-feedback@1",
        vec![
            field("feedback", education_lesson_feedback_type()),
            field("progress", education_progress_type()),
            field("timing", timing_feedback_type()),
        ],
    )
}

pub fn education_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (EDUCATION_QUESTION_TYPE, education_question_type()),
        (EDUCATION_RESPONSE_TYPE, education_response_type()),
        (EDUCATION_ASSESSMENT_TYPE, education_assessment_type()),
        (EDUCATION_HINT_TYPE, education_hint_type()),
        (
            EDUCATION_LESSON_FEEDBACK_TYPE,
            education_lesson_feedback_type(),
        ),
        (EDUCATION_PROGRESS_TYPE, education_progress_type()),
        (
            EDUCATION_RHYTHM_FEEDBACK_TYPE,
            education_rhythm_feedback_type(),
        ),
    ]
}
