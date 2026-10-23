//! Current Face journey over the retained product IMAGE; never rebuilds it.
use super::{
    journey_proof, profile::Paths, report::git_head, target_build::verify_artifact_digest,
    ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use conduit_host_make::BuildManifest;
use serde_json::Value;
use std::{fs, path::Path};

pub(crate) fn prove(
    image: &Path,
    expected: &BuildManifest,
    digest: &str,
    image_binding: &str,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    let backend = super::target_backend::select(&expected.target)?;
    if backend.arch != ConduitosArch::X86_64 {
        return Err(refusal(
            "the graphical user journey requires an x86_64 product IMAGE",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    if git_head(&paths.root)? != expected.source_identity {
        return Err(refusal(
            "product and journey driver must have the same source revision",
        ));
    }
    verify_artifact_digest(image, digest)?;
    journey_proof::execute_supplied(opts, image, digest.to_owned())?;
    verify_artifact_digest(image, digest)?;
    let proof = fs::read(paths.target.join("journey-proof.json"))
        .map_err(|error| refusal(error.to_string()))?;
    let mut receipt: Value =
        serde_json::from_slice(&proof).map_err(|error| refusal(error.to_string()))?;
    validate_binding(
        &receipt,
        &expected.source_identity,
        digest,
        &expected.profile_id,
        &expected.build_id,
        image_binding,
    )?;
    // Reuse the verified normal-product journey and its retained artifacts.
    // Keeping the complete receipt here lets the existing CI product-evidence
    // copy retain the strict lifecycle proof without another build or boot.
    receipt["compositor_lifecycle"] = super::compositor_lifecycle_collection::collect(
        &paths,
        image,
        &expected.source_identity,
        digest,
    )?;
    if git_head(&paths.root)? != expected.source_identity {
        return Err(refusal("source changed during the product journey"));
    }
    fs::write(
        paths.target.join("x86_64-product-journey-proof.json"),
        serde_json::to_vec_pretty(&receipt).map_err(|error| refusal(error.to_string()))?,
    )
    .map_err(|error| refusal(error.to_string()))?;
    Ok(())
}

fn validate_binding(
    receipt: &Value,
    source: &str,
    digest: &str,
    profile: &str,
    build: &str,
    image_binding: &str,
) -> Result<(), ConduitosError> {
    for (field, expected) in [
        ("schema", "conduit.conduitos/face-journey-proof@1"),
        ("source_commit", source),
        ("image_sha256", digest),
        ("profile_id", profile),
        ("build_id", build),
        ("image_id", image_binding),
    ] {
        if receipt[field].as_str() != Some(expected) {
            return Err(refusal(format!(
                "journey {field} differs from the verified product"
            )));
        }
    }
    Ok(())
}

fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("product-journey-artifact-mismatch", detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_successful_journey_cannot_substitute_another_product_or_driver() {
        let receipt = serde_json::json!({
            "schema":"conduit.conduitos/face-journey-proof@1", "source_commit":"source",
            "image_sha256":"digest", "profile_id":"profile", "build_id":"build", "image_id":"binding"
        });
        let check = |value: &Value| {
            validate_binding(value, "source", "digest", "profile", "build", "binding")
        };
        assert!(check(&receipt).is_ok());
        for field in [
            "schema",
            "source_commit",
            "image_sha256",
            "profile_id",
            "build_id",
            "image_id",
        ] {
            let mut changed = receipt.clone();
            changed[field] = "different".into();
            assert!(check(&changed).is_err(), "{field}");
            changed.as_object_mut().unwrap().remove(field);
            assert!(check(&changed).is_err(), "missing {field}");
        }
    }
}
