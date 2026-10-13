//! Decode a browser-captured owner Face without losing its exact u64 revision.
//! The separately captured CLI decimal is checked against the numeric carrier
//! and the original Face content identity; it cannot authorize a rewritten Face.
use conduit_presentation::Presentation;
use serde_json::Value;

pub(super) fn decode(owner: &Value) -> Result<Presentation, String> {
    let decimal = owner["presentation_revision_decimal"]
        .as_str()
        .ok_or("missing exact owner revision")?;
    let exact: u64 = decimal
        .parse()
        .map_err(|_| "invalid exact owner revision")?;
    if decimal != exact.to_string() {
        return Err("noncanonical exact owner revision".into());
    }
    let numeric = owner["presentation"]["revision"]
        .as_u64()
        .ok_or("missing numeric owner revision")?;
    if numeric != exact && numeric as f64 != exact as f64 {
        return Err("owner numeric revision differs from exact decimal".into());
    }
    let mut face: Presentation =
        serde_json::from_value(owner["presentation"].clone()).map_err(|error| error.to_string())?;
    face.revision = exact;
    face.validate().map_err(|error| error.to_string())?;
    Ok(face)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::{PresentationBasis, PresentationRole, PresentationSubject};
    fn fixture() -> (Presentation, Value) {
        let face = Presentation::new_with_semantics(
            (1u64 << 60) + 1,
            PresentationBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            vec![PresentationSubject {
                identity: "proof/region".into(),
                role: PresentationRole::Region,
                name: "Proof".into(),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )
        .unwrap();
        let mut owner = serde_json::json!({"presentation": face,
            "presentation_revision_decimal": face.revision.to_string()});
        owner["presentation"]["revision"] = serde_json::json!(1u64 << 60);
        (face, owner)
    }
    #[test]
    fn javascript_rounded_revision_recovers_only_the_captured_exact_face() {
        let (face, owner) = fixture();
        assert!(
            serde_json::from_value::<Presentation>(owner["presentation"].clone())
                .unwrap()
                .validate()
                .is_err()
        );
        assert_eq!(decode(&owner).unwrap(), face);
    }
    #[test]
    fn missing_substituted_and_inconsistent_exact_revisions_refuse() {
        let (_, owner) = fixture();
        for decimal in [
            Value::Null,
            serde_json::json!(((1u64 << 60) + 2).to_string()),
            serde_json::json!("01"),
            serde_json::json!("18446744073709551616"),
        ] {
            let mut changed = owner.clone();
            changed["presentation_revision_decimal"] = decimal;
            assert!(decode(&changed).is_err());
        }
        let mut changed = owner;
        changed["presentation"]["revision"] = serde_json::json!((1u64 << 60) + 1024);
        assert!(decode(&changed).is_err());
    }
}
