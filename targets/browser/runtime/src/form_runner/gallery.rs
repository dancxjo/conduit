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

pub(super) fn reviewed_gallery_view(
    query: &str,
    selected_checked_form_id: Option<&str>,
    creche_url: &str,
    revision: u32,
) -> Result<Vec<u8>, String> {
    let gallery = reviewed_gallery()?;
    let entries = gallery
        .forms
        .iter()
        .map(|form| {
            let realization = form
                .realizability
                .requirements
                .iter()
                .map(|requirement| {
                    let class = match requirement.realization_class {
                        Some("pure-kernel-or-local") => "local",
                        Some("bounded-browser-host-operation") => "browser Host",
                        _ => "unrealized",
                    };
                    format!(
                        "{}={}/{}",
                        requirement.kind_id,
                        if requirement.offer_state == "current-host-offer" {
                            "current"
                        } else {
                            "missing"
                        },
                        class
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            conduit_tour_model::TourGalleryEntry {
                title: form.title.into(),
                checked_form_id: form.checked_form_id.clone(),
                realization: format!(
                    "Kinds and realization: {realization}. Offers {}/{}; {}.",
                    form.realizability.current_offer_count,
                    form.realizability.required_kind_count,
                    if form.realizability.status == "runnable-on-current-browser-host" {
                        "runnable here"
                    } else {
                        "not runnable here"
                    }
                ),
                runnable: form.realizability.status == "runnable-on-current-browser-host",
                handoff: form_handoff(creche_url, form),
            }
        })
        .collect();
    conduit_tour_model::TourGalleryState {
        revision,
        query: query.into(),
        selected_checked_form_id: selected_checked_form_id.map(Into::into),
        entries,
    }
    .presentation()
    .map_err(|error| format!("describe reviewed Gallery: {error:?}"))?
    .lower()
    .map_err(|error| format!("lower reviewed Gallery: {error:?}"))?
    .encode()
    .map_err(|error| format!("encode reviewed Gallery: {error:?}"))
}

fn form_handoff(creche_url: &str, form: &GalleryForm) -> String {
    format!(
        "{}{}form={}&source_document_id={}&checked_form_id={}",
        creche_url,
        if creche_url.contains('?') { "&" } else { "?" },
        percent_encode(form.name),
        percent_encode(&form.source_document_id),
        percent_encode(&form.checked_form_id),
    )
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use core::fmt::Write;
            write!(&mut encoded, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    encoded
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

    #[test]
    fn gallery_view_uses_the_product_semantic_model_and_exact_handoffs() {
        let encoded = reviewed_gallery_view("", None, "/conduit/creche/", 4).unwrap();
        let view = conduit_presentation::ApplicationView::decode(&encoded).unwrap();
        assert_eq!(view.revision, 4);
        assert_eq!(
            view.nodes
                .iter()
                .filter(|node| node.component == conduit_presentation::ApplicationComponent::Panel)
                .count(),
            4
        );
        let handoff = view
            .nodes
            .iter()
            .filter(|node| node.component == conduit_presentation::ApplicationComponent::Link)
            .find(|node| node.value.contains("form=memory_lantern"))
            .unwrap();
        assert!(handoff
            .value
            .starts_with("/conduit/creche/?form=memory_lantern"));
        assert!(handoff.value.contains("checked_form_id="));
    }
}
