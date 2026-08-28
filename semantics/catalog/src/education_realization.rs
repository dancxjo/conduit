//! Deterministic arithmetic and rhythm-feedback realizations for education Info.

use alloc::vec;
use conduit_core::{StructuredInfoRefusal, StructuredInfoValue, StructuredInfoValueShape};

use crate::education_value::{
    count_value, leaf_text, ratio_value, record_field, record_value, text_value, unit_value,
    variant_payload_type,
};
use crate::{
    education_answer_type, education_assessment_outcome_type, education_assessment_type,
    education_evidence_class_type, education_feedback_provenance_type, education_hint_type,
    education_hints_type, education_lesson_feedback_type, education_optional_hint_type,
    education_progress_state_type, education_progress_type, education_question_type,
    education_refused_response_type, education_response_type, education_rhythm_feedback_type,
    timing_feedback_type, MAXIMUM_EDUCATION_HINTS,
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

pub fn deterministic_arithmetic_fixture() -> Result<EducationFixture, EducationInfoRefusal> {
    let question_identity = "question/arithmetic-7-plus-5";
    let hints = vec![
        hint_value(
            "hint/arithmetic-7-plus-5/1",
            question_identity,
            1,
            "Count five steps forward from seven.",
        )?,
        hint_value(
            "hint/arithmetic-7-plus-5/2",
            question_identity,
            2,
            "Ten is three steps after seven; continue two more.",
        )?,
        hint_value(
            "hint/arithmetic-7-plus-5/3",
            question_identity,
            3,
            "The answer is twelve.",
        )?,
    ];
    let question = record_value(
        education_question_type(),
        vec![
            (
                "evaluation_profile",
                text_value(ARITHMETIC_EVALUATION_PROFILE),
            ),
            (
                "hints",
                StructuredInfoValue::collection(education_hints_type(), hints)?,
            ),
            ("prompt", text_value("What is 7 + 5?")),
            ("question_identity", text_value(question_identity)),
            ("response_profile", text_value(ARITHMETIC_RESPONSE_PROFILE)),
        ],
    )?;
    let answer = record_value(
        education_answer_type(),
        vec![
            ("content", text_value("12")),
            ("event_identity", text_value("event/arithmetic-answer/1")),
            ("question_identity", text_value(question_identity)),
            ("response_identity", text_value("response/arithmetic/1")),
            ("time_identity", text_value("fixture-time/arithmetic/1")),
        ],
    )?;
    let response = StructuredInfoValue::variant(education_response_type(), "answer", answer)?;
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
    let payload = record_value(
        education_refused_response_type(),
        vec![
            ("question_identity", text_value(question_identity)),
            ("reason", text_value(reason)),
            ("response_identity", text_value("response/refused/1")),
        ],
    )?;
    Ok(StructuredInfoValue::variant(
        education_response_type(),
        "refused",
        payload,
    )?)
}

fn response_event(
    tag: &str,
    question_identity: &str,
    response_identity: &str,
) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    let payload_type = variant_payload_type(&education_response_type(), tag)?;
    let payload = record_value(
        payload_type,
        vec![
            ("event_identity", text_value("event/education-fixture/1")),
            ("question_identity", text_value(question_identity)),
            ("response_identity", text_value(response_identity)),
            ("time_identity", text_value("fixture-time/education/1")),
        ],
    )?;
    Ok(StructuredInfoValue::variant(
        education_response_type(),
        tag,
        payload,
    )?)
}

pub fn evaluate_arithmetic_response(
    question: &StructuredInfoValue,
    response: &StructuredInfoValue,
) -> Result<EducationEvaluation, EducationInfoRefusal> {
    if question.value_type() != &education_question_type()
        || response.value_type() != &education_response_type()
    {
        return Err(EducationInfoRefusal::MalformedInfo);
    }
    if leaf_text(record_field(question, "response_profile")?)? != ARITHMETIC_RESPONSE_PROFILE
        || leaf_text(record_field(question, "evaluation_profile")?)?
            != ARITHMETIC_EVALUATION_PROFILE
    {
        return Err(EducationInfoRefusal::UnsupportedProfile);
    }
    let question_identity = leaf_text(record_field(question, "question_identity")?)?;
    let StructuredInfoValueShape::Variant { tag, payload } = response.shape() else {
        return Err(EducationInfoRefusal::MalformedInfo);
    };
    let response_identity = leaf_text(record_field(payload, "response_identity")?)?;
    let response_question = leaf_text(record_field(payload, "question_identity")?)?;

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
            match tag {
                "answer" => {
                    let content = leaf_text(record_field(payload, "content")?)?;
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
                "hint_request" => (
                    "hint_requested",
                    text_value("learner-requested"),
                    0,
                    "Here is one bounded hint.",
                    first_hint(question)?,
                    "hinting",
                ),
                "timeout" => (
                    "timeout",
                    text_value("response-window-ended"),
                    0,
                    "The response window ended.",
                    None,
                    "timed_out",
                ),
                "refused" => (
                    "refused",
                    record_field(payload, "reason")?.clone(),
                    0,
                    "The response was refused.",
                    None,
                    "refused",
                ),
                _ => return Err(EducationInfoRefusal::MalformedInfo),
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
    let beat = leaf_text(record_field(timing, "beat")?)?;
    let question_identity = ["question/rhythm-beat/", beat].concat();
    let response_identity = ["response/rhythm-beat/", beat].concat();
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
    question: &StructuredInfoValue,
) -> Result<Option<StructuredInfoValue>, EducationInfoRefusal> {
    let StructuredInfoValueShape::Collection(hints) = record_field(question, "hints")?.shape()
    else {
        return Err(EducationInfoRefusal::MalformedInfo);
    };
    if hints.len() > usize::from(MAXIMUM_EDUCATION_HINTS) {
        return Err(EducationInfoRefusal::MalformedInfo);
    }
    Ok(hints.first().cloned())
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

fn hint_value(
    hint_identity: &str,
    question_identity: &str,
    sequence: u64,
    content: &str,
) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    record_value(
        education_hint_type(),
        vec![
            ("content", text_value(content)),
            ("hint_identity", text_value(hint_identity)),
            ("question_identity", text_value(question_identity)),
            ("sequence", count_value(sequence)),
        ],
    )
}
