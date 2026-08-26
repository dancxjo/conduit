//! Repository-development entrance and artifact retention for the capstone.

use std::{fs, path::Path};

use crate::cli::GlobalOpts;

pub(crate) fn run(
    output: &Path,
    source_identity: &str,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let receipt = super::prove(source_identity)?;
    if opts.dry_run {
        println!("would retain {} at {}", receipt.schema, output.display());
        return Ok(());
    }
    fs::create_dir_all(output)?;
    fs::write(
        output.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    for image in &receipt.images {
        let directory = output.join(&image.profile_name);
        fs::create_dir_all(&directory)?;
        fs::write(
            directory.join("build-manifest.json"),
            serde_json::to_vec_pretty(&image.manifest)?,
        )?;
        fs::write(
            directory.join("image.json"),
            serde_json::to_vec_pretty(&image.image)?,
        )?;
    }
    if opts.json {
        println!("{}", serde_json::to_string(&receipt)?);
    } else if !opts.quiet {
        println!(
            "proved {} with {} IMAGEs, {} Parts, and {} revised Manifestations",
            receipt.schema,
            receipt.images.len(),
            receipt.part_ids.len(),
            receipt.revised_manifestations.manifestations.len()
        );
        println!("receipt: {}", output.join("receipt.json").display());
    }
    Ok(())
}
