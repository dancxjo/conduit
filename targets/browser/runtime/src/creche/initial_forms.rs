//! Check and admit the bounded initial Form selection as workload revision zero.

use super::protocol::InitialFormReceipt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct InitialFormSelection {
    pub(super) name: String,
    pub(super) source_document_id: String,
    pub(super) checked_form_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct ReviewedFormInventory {
    pub(super) schema: &'static str,
    pub(super) source_document_id: String,
    pub(super) maximum_selection: usize,
    pub(super) forms: Vec<ReviewedForm>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct ReviewedForm {
    pub(super) name: String,
    pub(super) title: String,
    pub(super) source_document_id: String,
    pub(super) checked_form_id: String,
    pub(super) required_kinds: Vec<String>,
}

pub(super) fn reviewed_inventory(source: &str) -> Result<ReviewedFormInventory, String> {
    let checked = check_source(source)?;
    let source_document_id = checked.source_document_id.as_str().to_string();
    let forms = checked
        .forms
        .iter()
        .map(|form| {
            let mut required_kinds: Vec<String> =
                form.gears.iter().map(|gear| gear.kind.clone()).collect();
            required_kinds.sort();
            required_kinds.dedup();
            ReviewedForm {
                name: form.name.clone(),
                title: title(&form.name),
                source_document_id: source_document_id.clone(),
                checked_form_id: form.checked_form_id.as_str().to_string(),
                required_kinds,
            }
        })
        .collect();
    Ok(ReviewedFormInventory {
        schema: "conduit.creche/reviewed-form-inventory@1",
        source_document_id,
        maximum_selection: conduit_body::MAX_BODY_FORMS,
        forms,
    })
}

pub(super) fn checked_workset(
    source: &str,
    initial_forms_json: &str,
) -> Result<(conduit_body::BodyWorkset, Vec<InitialFormReceipt>), String> {
    let selected: Vec<InitialFormSelection> = serde_json::from_str(initial_forms_json)
        .map_err(|_| "initial Form selection is not an exact identity list".to_string())?;
    if selected.len() > conduit_body::MAX_BODY_FORMS {
        return Err("initial Form selection exceeds Body capacity".into());
    }

    let checked = check_source(source)?;
    let mut receipts = Vec::with_capacity(selected.len());
    let mut workset = conduit_body::BodyWorkset::default();
    for selection in selected {
        let checked_form = checked
            .forms
            .iter()
            .find(|form| form.name == selection.name)
            .ok_or_else(|| {
                format!(
                    "selected initial Form {:?} is absent from checked inventory",
                    selection.name
                )
            })?;
        if selection.source_document_id != checked.source_document_id.as_str()
            || selection.checked_form_id != checked_form.checked_form_id.as_str()
        {
            return Err(format!(
                "selected initial Form {:?} has a stale or substituted exact identity",
                selection.name
            ));
        }
        workset
            .add(conduit_body::ResidentForm::new(
                checked.source_document_id.clone(),
                checked_form.checked_form_id.clone(),
            ))
            .map_err(|error| format!("admit initial Form: {error:?}"))?;
        receipts.push(InitialFormReceipt {
            name: selection.name,
            source_document_id: checked.source_document_id.as_str().into(),
            checked_form_id: checked_form.checked_form_id.as_str().into(),
        });
    }
    Ok((workset, receipts))
}

pub(super) fn check_source(source: &str) -> Result<conduit_form::CheckedSyntaxDocument, String> {
    let (startup, _) = crate::installed_browser::catalogs()?;
    let syntax = conduit_form::parse_syntax_document(source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(format!(
            "parse reviewed Form inventory: {}",
            diagnostic.message
        ));
    }
    conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check reviewed Form inventory: {error:?}"))
}

fn title(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}
