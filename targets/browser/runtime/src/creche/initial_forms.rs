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
    title: Option<String>,
    #[serde(default)]
    entry: Option<String>,
    #[serde(default)]
    presentation_profile: u8,
    source: String,
}

pub(super) struct CheckedInventoryEntry {
    pub(super) source: String,
    pub(super) checked: conduit_form::CheckedSyntaxDocument,
    pub(super) entry_name: Option<String>,
    pub(super) entry_title: Option<String>,
    pub(super) presentation: crate::installed_browser::PresentationProfile,
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
        for form in entry.checked.forms.iter().filter(|form| {
            entry
                .entry_name
                .as_ref()
                .is_none_or(|name| name == &form.name)
        }) {
            let mut required_kinds: Vec<String> =
                form.gears.iter().map(|gear| gear.kind.clone()).collect();
            required_kinds.sort();
            required_kinds.dedup();
            forms.push(ReviewedForm {
                name: form.name.clone(),
                title: entry
                    .entry_title
                    .clone()
                    .unwrap_or_else(|| title(&form.name)),
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
                    .find(|form| {
                        form.name == selection.name
                            && entry
                                .entry_name
                                .as_ref()
                                .is_none_or(|name| name == &form.name)
                    })
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
                entry_name: None,
                entry_title: None,
                presentation: crate::installed_browser::PresentationProfile::Annotation,
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
        if entry.slug.is_empty()
            || entry.source.is_empty()
            || entry
                .title
                .as_ref()
                .is_some_and(|title| title.is_empty() || title.len() > 256)
        {
            return Err("reviewed Form bundle entry is malformed".into());
        }
        let presentation = match entry.presentation_profile {
            0 => crate::installed_browser::PresentationProfile::Annotation,
            1 => crate::installed_browser::PresentationProfile::Quantity,
            2 => crate::installed_browser::PresentationProfile::NormalizedDurations,
            3 => crate::installed_browser::PresentationProfile::PatternComparison,
            _ => return Err("reviewed Form has an unsupported presentation profile".into()),
        };
        let document = check_source_for_presentation(&entry.source, presentation)?;
        let expected_entry = entry.entry.unwrap_or_else(|| entry.slug.replace('-', "_"));
        let form = document
            .forms
            .iter()
            .find(|form| form.name == expected_entry)
            .ok_or_else(|| {
                format!(
                    "reviewed Form bundle entry {:?} has mismatched declared entry {:?}",
                    entry.slug, expected_entry
                )
            })?;
        if !names.insert(form.name.clone())
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
            entry_name: Some(expected_entry),
            entry_title: entry.title,
            presentation,
        });
    }
    Ok(checked)
}

pub(super) fn check_source(source: &str) -> Result<conduit_form::CheckedSyntaxDocument, String> {
    check_source_for_presentation(
        source,
        crate::installed_browser::PresentationProfile::Annotation,
    )
}

fn check_source_for_presentation(
    source: &str,
    presentation: crate::installed_browser::PresentationProfile,
) -> Result<conduit_form::CheckedSyntaxDocument, String> {
    let (startup, _) = crate::installed_browser::catalogs_for_presentation(presentation)?;
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

/// The installed generic selector realizes only exact checked selector contracts.
/// This prepares the local browser offer; it does not add offers to other Hosts.
pub(super) fn reviewed_browser_host(
    source: &str,
    host: conduit_core::HostId,
    boot: conduit_core::BootId,
) -> Result<conduit_core::HostAdvertisement, String> {
    let mut host = crate::installed_browser::advertisement(host, boot);
    for entry in check_inventory(source)? {
        let (_, mut profile) =
            crate::installed_browser::catalogs_for_presentation(entry.presentation)?;
        let offers = crate::installed_browser::catalogs::install_checked_structured_selectors(
            &entry.checked,
            &mut profile,
        )?;
        for offer in offers {
            if !host
                .capabilities
                .iter()
                .any(|current| current.capability_id == offer.capability_id)
            {
                host.capabilities.push(offer);
            }
        }
    }
    host.capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    Ok(host)
}
