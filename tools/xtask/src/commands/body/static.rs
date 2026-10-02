//! Typed composition of reviewed BrowserBundle and existing application packaging.
use crate::cli::GlobalOpts;
use clap::Args;
use std::{fs, path::PathBuf, process::Command};

#[derive(Args, Debug)]
pub(super) struct StaticArgs {
    /// Existing application-package-template@1 JSON; paths are relative to its directory.
    #[arg(long)]
    application: PathBuf,
    /// New directory to copy verbatim to an ordinary static host.
    #[arg(long)]
    output: PathBuf,
    /// Reuse this exact reviewed BrowserBundle instead of building another runtime.
    #[arg(long)]
    release: Option<PathBuf>,
    /// Render repository handbook prose before overlaying the declared application shell.
    #[arg(long)]
    handbook: bool,
}

pub(super) fn run(args: StaticArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if args.output.exists() {
        return Err(format!(
            "static application output already exists: {}",
            args.output.display()
        )
        .into());
    }
    if !args.application.is_file() {
        return Err("static application requires an application package template file".into());
    }
    if opts.dry_run {
        println!(
            "would package {} at {} using {}",
            args.application.display(),
            args.output.display(),
            args.release
                .as_deref()
                .map_or("a freshly reviewed BrowserBundle".into(), |path| path
                    .display()
                    .to_string())
        );
        return Ok(());
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temporary = std::env::temp_dir().join(format!(
        "conduit-static-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&temporary)?;
    let result = (|| {
        let release = match args.release {
            Some(path) => path.canonicalize()?,
            None => {
                let path = temporary.join("release");
                let status = Command::new(std::env::current_exe()?)
                    .args([
                        "make",
                        "host",
                        "release",
                        "--platform",
                        "browser",
                        "--output",
                    ])
                    .arg(&path)
                    .current_dir(&repository)
                    .status()?;
                if !status.success() {
                    return Err("reviewed browser release build failed".into());
                }
                path
            }
        };
        let handbook = if args.handbook {
            let output = temporary.join("handbook");
            crate::commands::handbook::run_handbook(crate::commands::handbook::HandbookArgs {
                output: output.clone(),
            })?;
            crate::commands::handbook::static_body::attach(&output)?;
            Some(output)
        } else {
            None
        };
        let mut command = Command::new("node");
        command
            .arg(repository.join("targets/browser/tools/package-static-application.mjs"))
            .arg(args.application.canonicalize()?)
            .arg(&release)
            .arg(&args.output);
        if let Some(handbook) = handbook {
            command.arg(handbook);
        }
        let status = command.status()?;
        if !status.success() {
            return Err("static application packaging failed".into());
        }
        if !opts.quiet {
            println!("Static browser application: {}", args.output.display());
        }
        Ok(())
    })();
    fs::remove_dir_all(&temporary)?;
    result
}
