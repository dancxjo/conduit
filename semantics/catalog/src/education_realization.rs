//! Deterministic arithmetic and rhythm-feedback realizations for education Info.

use alloc::{string::ToString, vec};
use conduit_core::{StructuredInfoRefusal, StructuredInfoValue};
use conduit_form::rust_binding::NativeRustBinding;

use crate::education_value::{
    count_value, leaf_count, leaf_text, ratio_value, record_field, record_value, text_value,
    unit_value,
};
use crate::{
    education_assessment_outcome_type, education_assessment_type, education_evidence_class_type,
    education_feedback_provenance_type, education_lesson_feedback_type,
    education_optional_hint_type, education_progress_state_type, education_progress_type,
    education_rhythm_feedback_type, timing_feedback_type, EducationHint, EducationHints,
    EducationQuestion, EducationResponse,
};

pub const ARITHMETIC_RESPONSE_PROFILE: &str = "education/response/integer-text@1";
pub const ARITHMETIC_EVALUATION_PROFILE: &str = "education/evaluate/exact-integer@1";

pub struct EducationFixture {
    pub question: StructuredInfoValue,
    pub response: StructuredInfoValue,
}

pub struct EducationEvaluation {
    pub assessment: StructuredInfoValue,
    pub feedback: StructuredInfoValue,
    pub progress: StructuredInfoValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EducationInfoRefusal {
    MalformedInfo,
    UnsupportedProfile,
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for EducationInfoRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

impl From<conduit_form::rust_binding::NativeBindingRefusal> for EducationInfoRefusal {
    fn from(_: conduit_form::rust_binding::NativeBindingRefusal) -> Self {
        Self::MalformedInfo
    }
}

pub fn deterministic_arithmetic_fixture() -> Result<EducationFixture, EducationInfoRefusal> {
    let question_identity = "question/arithmetic-7-plus-5";
    let hints = EducationHints::try_from_iter([
        EducationHint::new(
            "Count five steps forward from seven.".into(),
            "hint/arithmetic-7-plus-5/1".into(),
            question_identity.into(),
            1,
        )?,
        EducationHint::new(
            "Ten is three steps after seven; continue two more.".into(),
            "hint/arithmetic-7-plus-5/2".into(),
            question_identity.into(),
            2,
        )?,
        EducationHint::new(
            "The answer is twelve.".into(),
            "hint/arithmetic-7-plus-5/3".into(),
            question_identity.into(),
            3,
        )?,
    ])
    .map_err(|_| EducationInfoRefusal::MalformedInfo)?;
    let question = EducationQuestion::new(
        ARITHMETIC_EVALUATION_PROFILE.into(),
        hints,
        "What is 7 + 5?".into(),
        question_identity.into(),
        ARITHMETIC_RESPONSE_PROFILE.into(),
    )?
    .into_structured()?;
    let response = EducationResponse::answer(
        "12".into(),
        "event/arithmetic-answer/1".into(),
        question_identity.into(),
        "response/arithmetic/1".into(),
        "fixture-time/arithmetic/1".into(),
    )?
    .into_structured()?;
    Ok(EducationFixture { question, response })
}

pub fn deterministic_hint_request(
    question_identity: &str,
) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    response_event("hint_request", question_identity, "response/hint-request/1")
}

pub fn deterministic_timeout(
    question_identity: &str,
) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    response_event("timeout", question_identity, "response/timeout/1")
}

pub fn deterministic_refused_response(
    question_identity: &str,
    reason: &str,
) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    Ok(EducationResponse::refused(
        question_identity.into(),
        reason.into(),
        "response/refused/1".into(),
    )?
    .into_structured()?)
}

fn response_event(
    tag: &str,
    question_identity: &str,
    response_identity: &str,
) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    let response = match tag {
        "hint_request" => EducationResponse::hint_request(
            "event/education-fixture/1".into(),
            question_identity.into(),
            response_identity.into(),
            "fixture-time/education/1".into(),
        )?,
        "timeout" => EducationResponse::timeout(
            "event/education-fixture/1".into(),
            question_identity.into(),
            response_identity.into(),
            "fixture-time/education/1".into(),
        )?,
        _ => return Err(EducationInfoRefusal::MalformedInfo),
    };
    Ok(response.into_structured()?)
}

pub fn evaluate_arithmetic_response(
    question: &StructuredInfoValue,
    response: &StructuredInfoValue,
) -> Result<EducationEvaluation, EducationInfoRefusal> {
    let question = EducationQuestion::from_structured(question.clone())?;
    let response = EducationResponse::from_structured(response.clone())?;
    if question.response_profile() != ARITHMETIC_RESPONSE_PROFILE
        || question.evaluation_profile() != ARITHMETIC_EVALUATION_PROFILE
    {
        return Err(EducationInfoRefusal::UnsupportedProfile);
    }
    let question_identity = question.question_identity().as_str();
    let (response_identity, response_question) = match &response {
        EducationResponse::Answer(value) => (value.response_identity(), value.question_identity()),
        EducationResponse::HintRequest(value) => {
            (value.response_identity(), value.question_identity())
        }
        EducationResponse::Timeout(value) => (value.response_identity(), value.question_identity()),
        EducationResponse::Refused(value) => (value.response_identity(), value.question_identity()),
    };

    let (outcome_tag, outcome_payload, score, message, hint, progress_state) =
        if response_question != question_identity {
            (
                "refused",
                text_value("response-question-mismatch"),
                0,
                "Response belongs to a different question.",
                None,
                "refused",
            )
        } else {
            match &response {
                EducationResponse::Answer(answer) => {
                    let content = answer.content();
                    if content == "12" {
                        (
                            "correct",
                            unit_value()?,
                            1_000_000,
                            "Correct: 7 + 5 is 12.",
                            None,
                            "completed",
                        )
                    } else {
                        (
                            "incorrect",
                            unit_value()?,
                            0,
                            "That answer is not 12.",
                            None,
                            "awaiting_response",
                        )
                    }
                }
                EducationResponse::HintRequest(_) => (
                    "hint_requested",
                    text_value("learner-requested"),
                    0,
                    "Here is one bounded hint.",
                    first_hint(&question)?,
                    "hinting",
                ),
                EducationResponse::Timeout(_) => (
                    "timeout",
                    text_value("response-window-ended"),
                    0,
                    "The response window ended.",
                    None,
                    "timed_out",
                ),
                EducationResponse::Refused(refused) => (
                    "refused",
                    text_value(refused.reason()),
                    0,
                    "The response was refused.",
                    None,
                    "refused",
                ),
            }
        };

    let outcome = StructuredInfoValue::variant(
        education_assessment_outcome_type(),
        outcome_tag,
        outcome_payload,
    )?;
    let assessment = record_value(
        education_assessment_type(),
        vec![
            (
                "evaluation_profile",
                text_value(ARITHMETIC_EVALUATION_PROFILE),
            ),
            ("outcome", outcome),
            ("question_identity", text_value(question_identity)),
            ("response_identity", text_value(response_identity)),
            ("score", ratio_value(score)?),
        ],
    )?;
    let feedback = feedback_value(
        assessment.clone(),
        hint,
        message,
        "education/deterministic-arithmetic@1",
        "fixture/arithmetic-evaluator",
    )?;
    let progress = progress_value(question_identity, progress_state)?;
    Ok(EducationEvaluation {
        assessment,
        feedback,
        progress,
    })
}

pub fn adapt_rhythm_feedback(
    timing: &StructuredInfoValue,
) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    if timing.value_type() != &timing_feedback_type() {
        return Err(EducationInfoRefusal::MalformedInfo);
    }
    let classification = leaf_text(record_field(timing, "classification")?)?;
    let (outcome_tag, outcome_payload, score, message, progress_state) = match classification {
        "on-time" => (
            "correct",
            unit_value()?,
            1_000_000,
            "Timing is within the exact lesson tolerance.",
            "completed",
        ),
        "early" | "late" => (
            "partial",
            ratio_value(500_000)?,
            500_000,
            "Timing is outside tolerance; exact musical timing is attached.",
            "awaiting_response",
        ),
        "missed" => (
            "timeout",
            text_value("beat-not-observed"),
            0,
            "No performance event was observed for this beat.",
            "timed_out",
        ),
        _ => (
            "refused",
            text_value("unknown-timing-classification"),
            0,
            "The timing classification is unsupported.",
            "refused",
        ),
    };
    let beat = leaf_count(record_field(timing, "beat")?)?;
    let beat = beat.to_string();
    let question_identity = ["question/rhythm-beat/", beat.as_str()].concat();
    let response_identity = ["response/rhythm-beat/", beat.as_str()].concat();
    let outcome = StructuredInfoValue::variant(
        education_assessment_outcome_type(),
        outcome_tag,
        outcome_payload,
    )?;
    let assessment = record_value(
        education_assessment_type(),
        vec![
            (
                "evaluation_profile",
                text_value("education/evaluate/rhythm-timing@1"),
            ),
            ("outcome", outcome),
            ("question_identity", text_value(&question_identity)),
            ("response_identity", text_value(&response_identity)),
            ("score", ratio_value(score)?),
        ],
    )?;
    let feedback = feedback_value(
        assessment,
        None,
        message,
        "education/rhythm-feedback-adapter@1",
        "adapter/music-timing",
    )?;
    let progress = progress_value(&question_identity, progress_state)?;
    record_value(
        education_rhythm_feedback_type(),
        vec![
            ("feedback", feedback),
            ("progress", progress),
            ("timing", timing.clone()),
        ],
    )
}

fn first_hint(
    question: &EducationQuestion,
) -> Result<Option<StructuredInfoValue>, EducationInfoRefusal> {
    question
        .hints()
        .iter()
        .next()
        .cloned()
        .map(NativeRustBinding::into_structured)
        .transpose()
        .map_err(Into::into)
}

fn feedback_value(
    assessment: StructuredInfoValue,
    hint: Option<StructuredInfoValue>,
    message: &str,
    profile: &str,
    source: &str,
) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    let optional_hint = match hint {
        Some(value) => {
            StructuredInfoValue::variant(education_optional_hint_type(), "provided", value)?
        }
        None => {
            StructuredInfoValue::variant(education_optional_hint_type(), "absent", unit_value()?)?
        }
    };
    let evidence = StructuredInfoValue::variant(
        education_evidence_class_type(),
        "deterministic",
        unit_value()?,
    )?;
    let provenance = record_value(
        education_feedback_provenance_type(),
        vec![
            ("evidence_class", evidence),
            ("profile", text_value(profile)),
            ("revision", text_value("fixture-1")),
            ("source", text_value(source)),
        ],
    )?;
    record_value(
        education_lesson_feedback_type(),
        vec![
            ("assessment", assessment),
            ("hint", optional_hint),
            ("message", text_value(message)),
            ("provenance", provenance),
        ],
    )
}

fn progress_value(
    question_identity: &str,
    state: &str,
) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    record_value(
        education_progress_type(),
        vec![
            ("attempt_count", count_value(1)),
            ("question_identity", text_value(question_identity)),
            (
                "state",
                StructuredInfoValue::variant(
                    education_progress_state_type(),
                    state,
                    unit_value()?,
                )?,
            ),
        ],
    )
}
