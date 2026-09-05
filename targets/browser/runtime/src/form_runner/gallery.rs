use serde::Serialize;

const REVIEWED_SOURCES: [(&str, &str, &str); 4] = [
    (
        "morse_network",
        "Morse Network",
        include_str!("../../../../../forms/morse-network/main.conduit"),
    ),
    (
        "memory_lantern",
        "Memory Lantern",
        include_str!("../../../../../forms/memory-lantern/main.conduit"),
    ),
    (
        "desk_telegraph",
        "Desk Telegraph",
        include_str!("../../../../../forms/desk-telegraph/main.conduit"),
    ),
    (
        "button_across_room",
        "Button Across the Room",
        include_str!("../../../../../forms/button-across-room/main.conduit"),
    ),
];

#[derive(Serialize)]
pub(super) struct Gallery {
    schema: &'static str,
    maximum_forms: usize,
    forms: Vec<GalleryForm>,
}

#[derive(Serialize)]
struct GalleryForm {
    name: &'static str,
    title: &'static str,
    source: &'static str,
    source_document_id: String,
    checked_form_id: String,
    required_kinds: Vec<String>,
    realizability: GalleryRealizability,
}

#[derive(Serialize)]
struct GalleryRealizability {
    status: &'static str,
    current_offer_count: usize,
    required_kind_count: usize,
    requirements: Vec<GalleryRequirement>,
}

#[derive(Serialize)]
struct GalleryRequirement {
    kind_id: String,
    offer_state: &'static str,
    realization_class: Option<&'static str>,
}

pub(super) fn reviewed_gallery() -> Result<Gallery, String> {
    let (startup, _) = crate::installed_browser::catalogs()?;
    let host_inventory = crate::installed_browser::inventory();
    let mut forms = Vec::with_capacity(REVIEWED_SOURCES.len());
    for (name, title, source) in REVIEWED_SOURCES {
        let syntax = conduit_form::parse_syntax_document(source);
        if let Some(diagnostic) = syntax.diagnostics.first() {
            return Err(format!(
                "parse reviewed Gallery Form {name}: {}",
                diagnostic.message
            ));
        }
        let checked = conduit_form::check_syntax_document(&syntax, &startup)
            .map_err(|error| format!("check reviewed Gallery Form {name}: {error:?}"))?;
        let form = checked
            .forms
            .iter()
            .find(|form| form.name == name)
            .ok_or_else(|| format!("reviewed Gallery source does not define {name}"))?;
        let mut required_kinds = form
            .gears
            .iter()
            .map(|gear| gear.kind.clone())
            .collect::<Vec<_>>();
        required_kinds.sort();
        required_kinds.dedup();
        let requirements = required_kinds
            .iter()
            .map(|kind| {
                let offer = host_inventory
                    .entries
                    .iter()
                    .find(|entry| entry.kind_id == *kind && entry.implementation_id.is_some());
                GalleryRequirement {
                    kind_id: kind.clone(),
                    offer_state: if offer.is_some() {
                        "current-host-offer"
                    } else {
                        "not-currently-offered"
                    },
                    realization_class: offer.map(|entry| entry.classification),
                }
            })
            .collect::<Vec<_>>();
        let current_offer_count = requirements
            .iter()
            .filter(|requirement| requirement.offer_state == "current-host-offer")
            .count();
        forms.push(GalleryForm {
            name,
            title,
            source,
            source_document_id: checked.source_document_id.as_str().into(),
            checked_form_id: form.checked_form_id.as_str().into(),
            required_kinds,
            realizability: GalleryRealizability {
                status: if current_offer_count == requirements.len() {
                    "runnable-on-current-browser-host"
                } else {
                    "missing-current-host-offer"
                },
                current_offer_count,
                required_kind_count: requirements.len(),
                requirements,
            },
        });
    }
    Ok(Gallery {
        schema: "conduit.tour/reviewed-form-gallery@1",
        maximum_forms: REVIEWED_SOURCES.len(),
        forms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gallery_projects_exact_checked_canonical_sources() {
        let gallery = reviewed_gallery().unwrap();
        assert_eq!(gallery.maximum_forms, 4);
        assert_eq!(gallery.forms.len(), 4);
        for form in gallery.forms {
            assert!(!form.source_document_id.is_empty());
            assert!(!form.checked_form_id.is_empty());
            assert!(form.source.contains(&format!("form {}", form.name)));
            assert!(!form.required_kinds.is_empty());
            assert_eq!(
                form.realizability.status,
                "runnable-on-current-browser-host"
            );
            assert_eq!(
                form.realizability.current_offer_count,
                form.realizability.required_kind_count
            );
            assert!(form.realizability.requirements.iter().all(|requirement| {
                requirement.offer_state == "current-host-offer"
                    && requirement.realization_class.is_some()
            }));
        }
    }
}
