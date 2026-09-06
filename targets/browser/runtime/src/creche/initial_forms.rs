//! Check and admit the bounded initial Form selection as workload revision zero.

use super::protocol::InitialFormReceipt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Deserialize)]
struct ReviewedFormBundle {
    schema: String,
    forms: Vec<BundledForm>,
}

#[derive(Deserialize)]
struct BundledForm {
    slug: String,
    #[serde(default)]
    entry: Option<String>,
    source: String,
}

pub(super) struct CheckedInventoryEntry {
    pub(super) source: String,
    pub(super) checked: conduit_form::CheckedSyntaxDocument,
}

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
    pub(super) source: String,
    pub(super) source_document_id: String,
    pub(super) checked_form_id: String,
    pub(super) required_kinds: Vec<String>,
}

pub(super) fn reviewed_inventory(source: &str) -> Result<ReviewedFormInventory, String> {
    let checked_documents = check_inventory(source)?;
    let source_document_id = conduit_form::source_document_identity(source)
        .as_str()
        .to_string();
    let mut forms = Vec::new();
    for entry in &checked_documents {
        for form in &entry.checked.forms {
            let mut required_kinds: Vec<String> =
                form.gears.iter().map(|gear| gear.kind.clone()).collect();
            required_kinds.sort();
            required_kinds.dedup();
            forms.push(ReviewedForm {
                name: form.name.clone(),
                title: title(&form.name),
                source: entry.source.clone(),
                source_document_id: entry.checked.source_document_id.as_str().to_string(),
                checked_form_id: form.checked_form_id.as_str().to_string(),
                required_kinds,
            });
        }
    }
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

    let checked_documents = check_inventory(source)?;
    let mut receipts = Vec::with_capacity(selected.len());
    let mut workset = conduit_body::BodyWorkset::default();
    for selection in selected {
        let (checked, checked_form) = checked_documents
            .iter()
            .find_map(|entry| {
                entry
                    .checked
                    .forms
                    .iter()
                    .find(|form| form.name == selection.name)
                    .map(|form| (&entry.checked, form))
            })
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

pub(super) fn check_inventory(source: &str) -> Result<Vec<CheckedInventoryEntry>, String> {
    let Ok(bundle) = serde_json::from_str::<ReviewedFormBundle>(source) else {
        return check_source(source).map(|checked| {
            vec![CheckedInventoryEntry {
                source: source.to_owned(),
                checked,
            }]
        });
    };
    if bundle.schema != "conduit.creche/reviewed-form-bundle@1"
        || bundle.forms.is_empty()
        || bundle.forms.len() > conduit_body::MAX_BODY_FORMS
    {
        return Err("reviewed Form bundle is malformed or over capacity".into());
    }
    let mut checked = Vec::with_capacity(bundle.forms.len());
    let mut names = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for entry in bundle.forms {
        if entry.slug.is_empty() || entry.source.is_empty() {
            return Err("reviewed Form bundle entry is malformed".into());
        }
        let document = check_source(&entry.source)?;
        if document.forms.len() != 1 {
            return Err(format!(
                "reviewed Form bundle entry {:?} must own exactly one Form",
                entry.slug
            ));
        }
        let form = &document.forms[0];
        let expected_entry = entry.entry.unwrap_or_else(|| entry.slug.replace('-', "_"));
        if expected_entry != form.name
            || !names.insert(form.name.clone())
            || !identities.insert(form.checked_form_id.as_str().to_string())
        {
            return Err(format!(
                "reviewed Form bundle entry {:?} has mismatched or duplicate provenance",
                entry.slug
            ));
        }
        checked.push(CheckedInventoryEntry {
            source: entry.source,
            checked: document,
        });
    }
    Ok(checked)
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
