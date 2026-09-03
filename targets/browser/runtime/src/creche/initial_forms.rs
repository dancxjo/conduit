//! Check and admit the bounded initial Form selection as workload revision zero.

use super::protocol::InitialFormReceipt;

pub(super) fn checked_workset(
    source: &str,
    initial_forms_json: &str,
) -> Result<(conduit_body::BodyWorkset, Vec<InitialFormReceipt>), String> {
    let selected_names: Vec<String> = serde_json::from_str(initial_forms_json)
        .map_err(|_| "initial Form selection is not a JSON string list".to_string())?;
    if selected_names.len() > conduit_body::MAX_BODY_FORMS {
        return Err("initial Form selection exceeds Body capacity".into());
    }

    let (startup, _) = crate::installed_browser::catalogs()?;
    let syntax = conduit_form::parse_syntax_document(source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(format!("parse Body Form source: {}", diagnostic.message));
    }
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check Body Form source: {error:?}"))?;
    let mut receipts = Vec::with_capacity(selected_names.len());
    let mut workset = conduit_body::BodyWorkset::default();
    for name in selected_names {
        let checked_form = checked
            .forms
            .iter()
            .find(|form| form.name == name)
            .ok_or_else(|| {
                format!("selected initial Form {name:?} is absent from checked source")
            })?;
        workset
            .add(conduit_body::ResidentForm::new(
                checked.source_document_id.clone(),
                checked_form.checked_form_id.clone(),
            ))
            .map_err(|error| format!("admit initial Form: {error:?}"))?;
        receipts.push(InitialFormReceipt {
            name,
            source_document_id: checked.source_document_id.as_str().into(),
            checked_form_id: checked_form.checked_form_id.as_str().into(),
        });
    }
    Ok((workset, receipts))
}
