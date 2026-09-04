use serde::Serialize;

const REVIEWED_SOURCES: [(&str, &str, &str); 3] = [
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
}

pub(super) fn reviewed_gallery() -> Result<Gallery, String> {
    let (startup, _) = crate::installed_browser::catalogs()?;
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
        forms.push(GalleryForm {
            name,
            title,
            source,
            source_document_id: checked.source_document_id.as_str().into(),
            checked_form_id: form.checked_form_id.as_str().into(),
            required_kinds,
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
        assert_eq!(gallery.maximum_forms, 3);
        assert_eq!(gallery.forms.len(), 3);
        for form in gallery.forms {
            assert!(!form.source_document_id.is_empty());
            assert!(!form.checked_form_id.is_empty());
            assert!(form.source.contains(&format!("form {}", form.name)));
            assert!(!form.required_kinds.is_empty());
        }
    }
}
