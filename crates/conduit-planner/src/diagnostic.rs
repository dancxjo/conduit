use crate::PlannerError;
use conduit_form::{
    CheckedForm, DiagnosticSeverity, RelatedDiagnosticSubject, StructuredDiagnosticV1,
};
use std::collections::BTreeMap;

pub fn structured_planner_diagnostic(
    form: &CheckedForm,
    error: &PlannerError,
) -> Option<StructuredDiagnosticV1> {
    let (code, summary) = classification(error)?;
    Some(
        StructuredDiagnosticV1::new(
            code,
            DiagnosticSeverity::Error,
            summary,
            form.source_document_id.as_str(),
            None,
            None,
            vec![RelatedDiagnosticSubject {
                relationship: "checked-form".into(),
                subject: form.checked_form_id.as_str().into(),
                span: None,
            }],
            BTreeMap::new(),
            vec!["planner-detail".into()],
            vec!["host-local planner detail is redacted from the public diagnostic".into()],
        )
        .expect("one planning diagnostic is within the fixed schema bounds"),
    )
}

fn classification(error: &PlannerError) -> Option<(&'static str, &'static str)> {
    match error {
        PlannerError::UnknownCapability(_) => {
            Some(("CND-PLN-006", "no face-compatible realization is available"))
        }
        PlannerError::IncompatibleCheckedFace(_) => Some((
            "CND-PLN-012",
            "selected realization has a different canonical checked face",
        )),
        PlannerError::IncompatiblePortContract(_) => Some((
            "CND-PLN-011",
            "selected realization has an incompatible port contract",
        )),
        PlannerError::InvalidFormIdentity(_) => {
            Some(("CND-PLN-001", "checked Form identity is invalid"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{CheckedFormId, ExpandedFormId, SourceDocumentId};

    fn checked_form() -> CheckedForm {
        CheckedForm {
            source_document_id: SourceDocumentId::from("source-1"),
            checked_form_id: CheckedFormId::from("checked-1"),
            expanded_form_id: ExpandedFormId::from("expanded-1"),
            name: "demo".into(),
            operations: Vec::new(),
            connections: Vec::new(),
            exports: Vec::new(),
            nested_forms: Vec::new(),
        }
    }

    #[test]
    fn face_mismatch_is_structured_without_nominal_or_private_detail() {
        let diagnostic = structured_planner_diagnostic(
            &checked_form(),
            &PlannerError::IncompatibleCheckedFace(
                "operation secret face differs from host-local secret".into(),
            ),
        )
        .unwrap();
        let json = serde_json::to_string(&diagnostic).unwrap();
        assert_eq!(diagnostic.code, "CND-PLN-012");
        assert!(diagnostic.summary.contains("canonical checked face"));
        assert!(!json.contains("host-local secret"));
        assert!(!json.contains("operation secret"));
        assert_eq!(diagnostic.source_document_id, "source-1");
        assert_eq!(diagnostic.related[0].subject, "checked-1");
    }

    #[test]
    fn missing_face_compatible_offer_has_a_stable_functional_code() {
        let diagnostic = structured_planner_diagnostic(
            &checked_form(),
            &PlannerError::UnknownCapability("nominal-name-is-not-the-gate".into()),
        )
        .unwrap();
        assert_eq!(diagnostic.code, "CND-PLN-006");
        assert_eq!(
            diagnostic.summary,
            "no face-compatible realization is available"
        );
    }

    #[test]
    fn unreviewed_failures_do_not_collapse_into_a_generic_public_code() {
        let diagnostic = structured_planner_diagnostic(
            &checked_form(),
            &PlannerError::UnknownHost("private-host".into()),
        );
        assert!(diagnostic.is_none());
    }
}
