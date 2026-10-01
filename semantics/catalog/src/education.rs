//! Portable finite education, assessment, hint, feedback, and progress Info.
//!
//! Content profiles remain explicit identifiers. The lesson substrate does not
//! retain learner history or choose a Presenter, classroom model, or evaluator.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType, StructuredInfoType, StructuredVariantCase, QUANTITY_INFO_ID,
};
pub use conduit_education::{
    education_answer_type, education_hint_type, education_hints_type, education_question_type,
    education_refused_response_type, education_response_event_type, education_response_type,
    EducationAnswer, EducationHint, EducationHints, EducationQuestion, EducationRefusedResponse,
    EducationResponse, EducationResponseEvent, EducationResponseHintRequest,
    EducationResponseRefused, EducationResponseTimeout, MAXIMUM_EDUCATION_HINTS,
};

use crate::timing_feedback_type;

pub const EDUCATION_QUESTION_TYPE: &str = "EducationQuestion";
pub const EDUCATION_RESPONSE_TYPE: &str = "EducationResponse";
pub const EDUCATION_ASSESSMENT_TYPE: &str = "EducationAssessment";
pub const EDUCATION_HINT_TYPE: &str = "EducationHint";
pub const EDUCATION_LESSON_FEEDBACK_TYPE: &str = "EducationLessonFeedback";
pub const EDUCATION_PROGRESS_TYPE: &str = "EducationProgress";
pub const EDUCATION_RHYTHM_FEEDBACK_TYPE: &str = "EducationRhythmFeedback";

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed education leaf")
}

fn text_type() -> StructuredInfoType {
    leaf("value/text")
}

fn count_type() -> StructuredInfoType {
    leaf("value/count")
}

fn unit_type() -> StructuredInfoType {
    leaf("value/unit")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed education field")
}

fn case(name: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, payload_type).expect("reviewed education case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed education record")
}

pub fn education_assessment_outcome_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("education/assessment-outcome@2"),
        vec![
            case("correct", unit_type()),
            case("hint_requested", text_type()),
            case("incorrect", unit_type()),
            case("partial", leaf(QUANTITY_INFO_ID)),
            case("refused", text_type()),
            case("timeout", text_type()),
        ],
    )
    .expect("reviewed assessment outcomes")
}

pub fn education_assessment_type() -> StructuredInfoType {
    record(
        "education/assessment@2",
        vec![
            field("evaluation_profile", text_type()),
            field("outcome", education_assessment_outcome_type()),
            field("question_identity", text_type()),
            field("response_identity", text_type()),
            field("score", leaf(QUANTITY_INFO_ID)),
        ],
    )
}

pub fn education_optional_hint_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("education/optional-hint@1"),
        vec![
            case("absent", unit_type()),
            case("provided", education_hint_type()),
        ],
    )
    .expect("reviewed optional hint")
}

pub fn education_evidence_class_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("education/evidence-class@1"),
        vec![
            case("deterministic", unit_type()),
            case("model_derived", unit_type()),
        ],
    )
    .expect("reviewed feedback evidence class")
}

pub fn education_feedback_provenance_type() -> StructuredInfoType {
    record(
        "education/feedback-provenance@1",
        vec![
            field("evidence_class", education_evidence_class_type()),
            field("profile", text_type()),
            field("revision", text_type()),
            field("source", text_type()),
        ],
    )
}

pub fn education_lesson_feedback_type() -> StructuredInfoType {
    record(
        "education/lesson-feedback@1",
        vec![
            field("assessment", education_assessment_type()),
            field("hint", education_optional_hint_type()),
            field("message", text_type()),
            field("provenance", education_feedback_provenance_type()),
        ],
    )
}

pub fn education_progress_state_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("education/progress-state@1"),
        vec![
            case("awaiting_response", unit_type()),
            case("completed", unit_type()),
            case("hinting", unit_type()),
            case("refused", unit_type()),
            case("timed_out", unit_type()),
        ],
    )
    .expect("reviewed lesson states")
}

pub fn education_progress_type() -> StructuredInfoType {
    record(
        "education/progress@1",
        vec![
            field("attempt_count", count_type()),
            field("question_identity", text_type()),
            field("state", education_progress_state_type()),
        ],
    )
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
