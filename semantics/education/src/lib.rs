//! Portable education question, response, assessment, feedback, and progress meaning.
#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    EducationAnswer, EducationAssessment, EducationAssessmentOutcome, EducationEvidenceClass,
    EducationFeedbackProvenance, EducationHint, EducationLessonFeedback, EducationOptionalHint,
    EducationOptionalHintProvided, EducationProgress, EducationProgressState, EducationQuestion,
    EducationRefusedResponse, EducationResponse, EducationResponseAnswer, EducationResponseEvent,
    EducationResponseHintRequest, EducationResponseRefused, EducationResponseTimeout,
};

pub type EducationHints = conduit_form::rust_binding::BoundedSequence<EducationHint, 3>;

pub const MAXIMUM_EDUCATION_HINTS: u16 = 3;

pub fn education_hint_type() -> conduit_core::StructuredInfoType {
    EducationHint::semantic_type().expect("checked education hint Type")
}

pub fn education_question_type() -> conduit_core::StructuredInfoType {
    EducationQuestion::semantic_type().expect("checked education question Type")
}

pub fn education_hints_type() -> conduit_core::StructuredInfoType {
    let question = education_question_type();
    let conduit_core::StructuredInfoTypeShape::Record { fields, .. } = question.shape() else {
        unreachable!("checked education question is a record")
    };
    fields
        .iter()
        .find(|field| field.name() == "hints")
        .expect("checked education question has hints")
        .value_type()
        .clone()
}

pub fn education_answer_type() -> conduit_core::StructuredInfoType {
    EducationAnswer::semantic_type().expect("checked education answer Type")
}

pub fn education_response_event_type() -> conduit_core::StructuredInfoType {
    EducationResponseEvent::semantic_type().expect("checked education response-event Type")
}

pub fn education_refused_response_type() -> conduit_core::StructuredInfoType {
    EducationRefusedResponse::semantic_type().expect("checked refused-response Type")
}

pub fn education_response_type() -> conduit_core::StructuredInfoType {
    EducationResponse::semantic_type().expect("checked education response Type")
}

pub fn education_assessment_outcome_type() -> conduit_core::StructuredInfoType {
    EducationAssessmentOutcome::semantic_type().expect("checked education assessment outcome Type")
}

pub fn education_assessment_type() -> conduit_core::StructuredInfoType {
    EducationAssessment::semantic_type().expect("checked education assessment Type")
}

pub fn education_optional_hint_type() -> conduit_core::StructuredInfoType {
    EducationOptionalHint::semantic_type().expect("checked education optional-hint Type")
}

pub fn education_evidence_class_type() -> conduit_core::StructuredInfoType {
    EducationEvidenceClass::semantic_type().expect("checked education evidence-class Type")
}

pub fn education_feedback_provenance_type() -> conduit_core::StructuredInfoType {
    EducationFeedbackProvenance::semantic_type().expect("checked feedback-provenance Type")
}

pub fn education_lesson_feedback_type() -> conduit_core::StructuredInfoType {
    EducationLessonFeedback::semantic_type().expect("checked lesson-feedback Type")
}

pub fn education_progress_state_type() -> conduit_core::StructuredInfoType {
    EducationProgressState::semantic_type().expect("checked education progress-state Type")
}

pub fn education_progress_type() -> conduit_core::StructuredInfoType {
    EducationProgress::semantic_type().expect("checked education progress Type")
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec};
    use conduit_core::StructuredInfoTypeShape;
    use conduit_form::rust_binding::NativeRustBinding;

    #[test]
    fn question_and_every_response_round_trip_through_generated_bindings() {
        let hint = EducationHint::new(
            "count onward".to_string(),
            "hint/1".to_string(),
            "question/1".to_string(),
            1,
        )
        .unwrap();
        let hints = EducationHints::try_from_iter(vec![hint]).unwrap();
        let question = EducationQuestion::new(
            "evaluation/exact".to_string(),
            hints,
            "What is 1 + 1?".to_string(),
            "question/1".to_string(),
            "response/integer".to_string(),
        )
        .unwrap();
        let structured = question.clone().into_structured().unwrap();
        assert_eq!(
            EducationQuestion::from_structured(structured).unwrap(),
            question
        );

        let responses = [
            EducationResponse::answer(
                "2".to_string(),
                "event/1".to_string(),
                "question/1".to_string(),
                "response/1".to_string(),
                "time/1".to_string(),
            )
            .unwrap(),
            EducationResponse::hint_request(
                "event/2".to_string(),
                "question/1".to_string(),
                "response/2".to_string(),
                "time/2".to_string(),
            )
            .unwrap(),
            EducationResponse::timeout(
                "event/3".to_string(),
                "question/1".to_string(),
                "response/3".to_string(),
                "time/3".to_string(),
            )
            .unwrap(),
            EducationResponse::refused(
                "question/1".to_string(),
                "unsupported".to_string(),
                "response/4".to_string(),
            )
            .unwrap(),
        ];
        for response in responses {
            let structured = response.clone().into_structured().unwrap();
            assert_eq!(
                EducationResponse::from_structured(structured).unwrap(),
                response
            );
        }
    }

    #[test]
    fn hints_are_bounded_to_three_in_the_authoritative_question_type() {
        let question_type = education_question_type();
        let StructuredInfoTypeShape::Record { fields, .. } = question_type.shape() else {
            panic!("question must be a record");
        };
        let hints = fields.iter().find(|field| field.name() == "hints").unwrap();
        assert!(matches!(
            hints.value_type().shape(),
            StructuredInfoTypeShape::Sequence {
                minimum_items: 0,
                maximum_items: 3,
                ..
            }
        ));
    }

    #[test]
    fn assessment_feedback_and_progress_round_trip_through_generated_bindings() {
        let assessment = EducationAssessment::new(
            "evaluation/exact".to_string(),
            EducationAssessmentOutcome::partial(conduit_core::Quantity::new(
                500_000,
                conduit_core::QuantityUnit::Millionth,
            ))
            .unwrap(),
            "question/1".to_string(),
            "response/1".to_string(),
            conduit_core::Quantity::new(500_000, conduit_core::QuantityUnit::Millionth),
        )
        .unwrap();
        let hint = EducationOptionalHint::provided(
            "count onward".to_string(),
            "hint/1".to_string(),
            "question/1".to_string(),
            1,
        )
        .unwrap();
        let provenance = EducationFeedbackProvenance::new(
            EducationEvidenceClass::Deterministic,
            "education/deterministic".to_string(),
            "revision/1".to_string(),
            "fixture/1".to_string(),
        )
        .unwrap();
        let feedback = EducationLessonFeedback::new(
            assessment.clone(),
            hint,
            "Try once more.".to_string(),
            provenance,
        )
        .unwrap();
        let progress = EducationProgress::new(
            1,
            "question/1".to_string(),
            EducationProgressState::AwaitingResponse,
        )
        .unwrap();

        let assessment_value = assessment.clone().into_structured().unwrap();
        assert_eq!(
            EducationAssessment::from_structured(assessment_value).unwrap(),
            assessment
        );
        let feedback_value = feedback.clone().into_structured().unwrap();
        assert_eq!(
            EducationLessonFeedback::from_structured(feedback_value).unwrap(),
            feedback
        );
        let progress_value = progress.clone().into_structured().unwrap();
        assert_eq!(
            EducationProgress::from_structured(progress_value).unwrap(),
            progress
        );

        for outcome in [
            EducationAssessmentOutcome::Correct,
            EducationAssessmentOutcome::hint_requested("requested".to_string()).unwrap(),
            EducationAssessmentOutcome::Incorrect,
            EducationAssessmentOutcome::refused("unsupported".to_string()).unwrap(),
            EducationAssessmentOutcome::timeout("ended".to_string()).unwrap(),
        ] {
            let value = outcome.clone().into_structured().unwrap();
            assert_eq!(
                EducationAssessmentOutcome::from_structured(value).unwrap(),
                outcome
            );
        }
    }
}
