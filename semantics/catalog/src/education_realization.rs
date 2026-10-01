//! Deterministic arithmetic and rhythm-feedback realizations for education Info.

use alloc::string::ToString;
use conduit_audio::{TimingClassification, TimingFeedback};
use conduit_core::{StructuredInfoRefusal, StructuredInfoValue};
use conduit_form::rust_binding::NativeRustBinding;

use crate::{
    timing_feedback_type, EducationAssessment, EducationAssessmentOutcome, EducationEvidenceClass,
    EducationFeedbackProvenance, EducationHint, EducationHints, EducationLessonFeedback,
    EducationOptionalHint, EducationProgress, EducationProgressState, EducationQuestion,
    EducationResponse, EducationRhythmFeedback,
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

    let (outcome, score, message, hint, progress_state) = if response_question != question_identity
    {
        (
            EducationAssessmentOutcome::refused("response-question-mismatch".into())?,
            0,
            "Response belongs to a different question.",
            None,
            EducationProgressState::Refused,
        )
    } else {
        match &response {
            EducationResponse::Answer(answer) => {
                let content = answer.content();
                if content == "12" {
                    (
                        EducationAssessmentOutcome::Correct,
                        1_000_000,
                        "Correct: 7 + 5 is 12.",
                        None,
                        EducationProgressState::Completed,
                    )
                } else {
                    (
                        EducationAssessmentOutcome::Incorrect,
                        0,
                        "That answer is not 12.",
                        None,
                        EducationProgressState::AwaitingResponse,
                    )
                }
            }
            EducationResponse::HintRequest(_) => (
                EducationAssessmentOutcome::hint_requested("learner-requested".into())?,
                0,
                "Here is one bounded hint.",
                first_hint(&question)?,
                EducationProgressState::Hinting,
            ),
            EducationResponse::Timeout(_) => (
                EducationAssessmentOutcome::timeout("response-window-ended".into())?,
                0,
                "The response window ended.",
                None,
                EducationProgressState::TimedOut,
            ),
            EducationResponse::Refused(refused) => (
                EducationAssessmentOutcome::refused(refused.reason().clone())?,
                0,
                "The response was refused.",
                None,
                EducationProgressState::Refused,
            ),
        }
    };

    let assessment = EducationAssessment::new(
        ARITHMETIC_EVALUATION_PROFILE.into(),
        outcome,
        question_identity.into(),
        response_identity.clone(),
        ratio(score),
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
        assessment: assessment.into_structured()?,
        feedback: feedback.into_structured()?,
        progress: progress.into_structured()?,
    })
}

pub fn adapt_rhythm_feedback(
    timing: &StructuredInfoValue,
) -> Result<StructuredInfoValue, EducationInfoRefusal> {
    if timing.value_type() != &timing_feedback_type() {
        return Err(EducationInfoRefusal::MalformedInfo);
    }
    let timing_binding = TimingFeedback::from_structured(timing.clone())
        .map_err(|_| EducationInfoRefusal::MalformedInfo)?;
    let classification = timing_binding.classification();
    let (outcome, score, message, progress_state) = match classification {
        TimingClassification::OnTime => (
            EducationAssessmentOutcome::Correct,
            1_000_000,
            "Timing is within the exact lesson tolerance.",
            EducationProgressState::Completed,
        ),
        TimingClassification::Early | TimingClassification::Late => (
            EducationAssessmentOutcome::partial(ratio(500_000))?,
            500_000,
            "Timing is outside tolerance; exact musical timing is attached.",
            EducationProgressState::AwaitingResponse,
        ),
        TimingClassification::Missed => (
            EducationAssessmentOutcome::timeout("beat-not-observed".into())?,
            0,
            "No performance event was observed for this beat.",
            EducationProgressState::TimedOut,
        ),
    };
    let beat = timing_binding.beat();
    let beat = beat.to_string();
    let question_identity = ["question/rhythm-beat/", beat.as_str()].concat();
    let response_identity = ["response/rhythm-beat/", beat.as_str()].concat();
    let assessment = EducationAssessment::new(
        "education/evaluate/rhythm-timing@1".into(),
        outcome,
        question_identity.clone(),
        response_identity,
        ratio(score),
    )?;
    let feedback = feedback_value(
        assessment,
        None,
        message,
        "education/rhythm-feedback-adapter@1",
        "adapter/music-timing",
    )?;
    let progress = progress_value(&question_identity, progress_state)?;
    Ok(EducationRhythmFeedback::new(feedback, progress, timing_binding)?.into_structured()?)
}

fn first_hint(question: &EducationQuestion) -> Result<Option<EducationHint>, EducationInfoRefusal> {
    Ok(question.hints().iter().next().cloned())
}

fn feedback_value(
    assessment: EducationAssessment,
    hint: Option<EducationHint>,
    message: &str,
    profile: &str,
    source: &str,
) -> Result<EducationLessonFeedback, EducationInfoRefusal> {
    let optional_hint = match hint {
        Some(value) => EducationOptionalHint::provided(
            value.content().clone(),
            value.hint_identity().clone(),
            value.question_identity().clone(),
            *value.sequence(),
        )?,
        None => EducationOptionalHint::Absent,
    };
    let provenance = EducationFeedbackProvenance::new(
        EducationEvidenceClass::Deterministic,
        profile.into(),
        "fixture-1".into(),
        source.into(),
    )?;
    Ok(EducationLessonFeedback::new(
        assessment,
        optional_hint,
        message.into(),
        provenance,
    )?)
}

fn progress_value(
    question_identity: &str,
    state: EducationProgressState,
) -> Result<EducationProgress, EducationInfoRefusal> {
    Ok(EducationProgress::new(1, question_identity.into(), state)?)
}

fn ratio(value: i64) -> conduit_core::Quantity {
    conduit_core::Quantity::new(value, conduit_core::QuantityUnit::Millionth)
}
