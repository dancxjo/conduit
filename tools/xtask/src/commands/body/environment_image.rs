//! A real construction Host supplies birth attribution; Make owns Crèche birth.
use crate::cli::GlobalOpts;
use clap::Args as ClapArgs;
use conduit_core::bind_sign;
use conduit_std_host::StdHost;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, io::Write, path::PathBuf, process::Command};

#[derive(ClapArgs, Debug)]
pub(super) struct Args {
    /// Parent directory; each construction gets a distinct birth-Sign directory.
    #[arg(long, default_value = "target/environment-body")]
    output: PathBuf,
    /// Retain a fresh construction birth Sign for a separately selected image profile.
    #[arg(long)]
    prepare_only: bool,
}

pub(super) fn run(args: Args, opts: &GlobalOpts) -> Result<(), Box<dyn Error>> {
    let root = crate::workspace::workspace_root()?;
    let host = StdHost::new();
    let observation = host.advertisement();
    let sign = bind_sign(&observation.host_id, &observation.boot_id, None, 1);
    let base = if args.output.is_absolute() {
        args.output
    } else {
        root.join(args.output)
    };
    let output = base.join(sign.sign_id.as_str());
    fs::create_dir_all(&output)?;
    let output = fs::canonicalize(output)?;
    let birth = output.join("birth-sign.json");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&birth)?
        .write_all(&serde_json::to_vec_pretty(&sign)?)?;
    // The scalar Sign and construction directory survive the initialized Host;
    // its offers are never forwarded as availability on the future MCU.
    drop(host);
    if args.prepare_only {
        println!("{}", birth.display());
        return Ok(());
    }
    let status = Command::new("cargo")
        .current_dir(&root)
        .args(["xtask", "prove", "environment", "native-body-esp32"])
        .env("CONDUIT_ENVIRONMENT_BODY_BIRTH_SIGN", &birth)
        .status()?;
    if !status.success() {
        return Err("environmental body image construction failed".into());
    }
    let biography = output.join("body-biography.json");
    let biography_bytes = fs::read(&biography)?;
    let retained: conduit_body::BodyBiographyEvidence = serde_json::from_slice(&biography_bytes)?;
    retained
        .validate()
        .map_err(|why| format!("constructed biography: {why:?}"))?;
    let artifact = root
        .join("target/environment-esp32/xtensa-esp32-none-elf/release/conduit-esp32-tm1637-probe");
    let bytes = fs::read(artifact)?;
    if !bytes
        .windows(biography_bytes.len())
        .any(|window| window == biography_bytes)
    {
        return Err("constructed image does not contain its retained biography".into());
    }
    let image = output.join("firmware.elf");
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&image)?
        .write_all(&bytes)?;
    let receipt = serde_json::json!({
        "schema":"conduit.environment/body-image-build@1",
        "body_id":retained.body_id,
        "friendly_name":retained.friendly_name,
        "image":image,
        "image_sha256":format!("{:x}", Sha256::digest(&bytes)),
        "biography":biography,
        "birth_sign":birth,
        "flashed":false,
        "member_admitted":false,
        "play_started":false,
    });
    let receipt_text = serde_json::to_string_pretty(&receipt)?;
    fs::write(output.join("build.json"), &receipt_text)?;
    if opts.json {
        println!("{receipt_text}");
    } else {
        println!(
            "Built body {} ({})",
            retained.friendly_name,
            retained.body_id.as_str()
        );
        println!("Image: {}", image.display());
        println!(
            "Body startup is still in development; this command has not flashed or started it."
        );
    }
    Ok(())
}
