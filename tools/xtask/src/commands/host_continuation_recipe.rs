//! Publish only exact recipe bytes from this checked Cargo build-script event.
use super::host_workset_source::WorksetSourceInput;
use conduit_host_make::HostImage;
use std::{fs, io::Read, path::Path};
pub(super) const NAME: &str = "continuation-recipe.json";
pub(super) fn publish(
    out_dir: &Path,
    output: &Path,
    image: &HostImage,
    configuration: &str,
    workset: Option<&WorksetSourceInput>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let Some(workset) = workset else {
        return Ok(false);
    };
    workset.verify()?;
    let input = out_dir.join(NAME);
    if !fs::symlink_metadata(&input)?.is_file() || input.canonicalize()?.parent() != Some(out_dir) {
        return Err("calibrated recipe escaped actual Cargo output custody".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(input)?
        .take(conduit_body::continuation_recipe::MAXIMUM_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > conduit_body::continuation_recipe::MAXIMUM_DOCUMENT_BYTES {
        return Err("calibrated recipe exceeded finite publication bound".into());
    }
    let document: conduit_body::continuation_recipe::Document = serde_json::from_slice(&bytes)?;
    document
        .checked_digest()
        .map_err(|e| format!("calibrated recipe transcript: {e:?}"))?;
    validate(
        &document,
        &image.manifest.image_id,
        configuration,
        &workset.sha256,
        &workset.entries,
    )?;
    fs::write(output.join(NAME), bytes)?;
    Ok(true)
}
fn validate(
    document: &conduit_body::continuation_recipe::Document,
    image: &str,
    configuration: &str,
    source: &str,
    entries: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    document
        .checked_digest()
        .map_err(|e| format!("calibrated recipe transcript: {e:?}"))?;
    if format!("{:x}", DigestHex(document.source_digest)) != source
        || document.image_id != image
        || document.configuration_id != configuration
        || document
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .ne(entries.iter().map(String::as_str))
    {
        return Err("calibrated recipe differs from original Make IMAGE/configuration/Source/entry selection".into());
    }
    Ok(())
}
struct DigestHex([u8; 32]);
impl std::fmt::LowerHex for DigestHex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_image_source_and_entry_pins_are_required_before_publication() {
        use conduit_body::{
            continuation_recipe::{Document, Entry, SCHEMA},
            ContinuationBundleEntry,
        };
        let placements = ["placement"];
        let nodes = [0];
        let pin = ContinuationBundleEntry {
            name: "entry",
            source_document: "source",
            checked_plot: "checked",
            expanded_plot: "expanded",
            projection_digest: [2; 32],
            recipe_digest: [3; 32],
            placement_ids: &placements,
            placement_nodes: &nodes,
        };
        let digest = conduit_body::continuation_bundle_digest([1; 32], &[pin]).unwrap();
        let document = Document {
            schema: SCHEMA.into(),
            image_id: "image".into(),
            configuration_id: "configuration".into(),
            source_digest: [1; 32],
            workset_digest: digest,
            entries: vec![Entry {
                name: "entry".into(),
                source_document: "source".into(),
                checked_plot: "checked".into(),
                expanded_plot: "expanded".into(),
                projection_digest: [2; 32],
                recipe_digest: [3; 32],
                placement_ids: vec!["placement".into()],
                placement_nodes: vec![0],
            }],
        };
        let source = format!("{:x}", DigestHex([1; 32]));
        let entries = vec!["entry".into()];
        assert!(validate(&document, "image", "configuration", &source, &entries).is_ok());
        assert!(validate(&document, "wrong-image", "configuration", &source, &entries).is_err());
        assert!(validate(&document, "image", "wrong-configuration", &source, &entries).is_err());
        assert!(validate(
            &document,
            "image",
            "configuration",
            &format!("{:x}", DigestHex([9; 32])),
            &entries
        )
        .is_err());
        assert!(validate(
            &document,
            "image",
            "configuration",
            &source,
            &["other-entry".into()]
        )
        .is_err());
    }
}
