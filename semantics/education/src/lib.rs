//! Portable education question and response meaning.
#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    EducationAnswer, EducationHint, EducationQuestion, EducationRefusedResponse, EducationResponse,
    EducationResponseAnswer, EducationResponseEvent, EducationResponseHintRequest,
    EducationResponseRefused, EducationResponseTimeout,
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
}
