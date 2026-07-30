use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(
    name = "xtask",
    about = "Conduit repository automation and verification tasks"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate or check the exact library catalog and Tour index
    CatalogIndex {
        #[arg(long)]
        check: bool,
    },
    /// Generate or check the static tour browser plan artifact
    GenerateBrowserPlan {
        #[arg(long)]
        check: bool,
    },
    /// Verify C1 canonical descriptor vectors
    VerifyCanonical {
        vectors: Option<PathBuf>,
        #[arg(long)]
        show: bool,
    },
    /// Run or update the performance and artifact byte size gate
    PerformanceGate {
        #[arg(long)]
        update: bool,
    },
    /// Inspect and verify RP2040 reference firmware budget limits
    EmbeddedGate,
    /// Run RP2040 hardware-in-the-loop verification or probe
    Rp2040Hil {
        #[arg(long)]
        port: Option<String>,
        #[arg(long)]
        expected_plan_hash: Option<String>,
        #[arg(long)]
        expected_firmware_identity: Option<String>,
        #[arg(long, default_value = "64")]
        maximum_decisions: u32,
        #[arg(long, default_value = "10.0")]
        timeout_seconds: f64,
        #[arg(long)]
        probe: bool,
        #[arg(long)]
        require_hardware: bool,
    },
    /// Report adversarial containment support for a target profile
    AdversarialProfile {
        #[arg(long)]
        profile: String,
    },
    /// Serve static HTTP files from a directory
    Serve {
        #[arg(long, default_value = ".")]
        directory: PathBuf,
        #[arg(long, default_value = "0")]
        port: u16,
    },
    /// Verify that zero repository-owned Python scripts or dependencies exist
    CheckPythonBoundary,
    /// Verify release claims and emit commit-bound release evidence
    ReleaseGate {
        #[arg(long)]
        check: bool,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Run complete workspace check suite (formatting, clippy, tests, gates, boundaries)
    CheckAll,
}

fn locate_workspace_root() -> PathBuf {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let path = Path::new(&manifest_dir);
        if let Some(parent) = path.parent() {
            if parent.join("Cargo.toml").exists() {
                return parent.to_path_buf();
            }
        }
        return path.to_path_buf();
    }

    let current = std::env::current_dir().expect("failed to get current dir");
    let mut ancestor = current.as_path();
    loop {
        if ancestor.join("Cargo.toml").exists() && ancestor.join("crates").exists() {
            return ancestor.to_path_buf();
        }
        match ancestor.parent() {
            Some(parent) => ancestor = parent,
            None => break,
        }
    }
    current
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let root = locate_workspace_root();

    match cli.command {
        Commands::CatalogIndex { check } => commands::catalog_index::run(&root, check),
        Commands::GenerateBrowserPlan { check } => {
            commands::generate_browser_plan::run(&root, check)
        }
        Commands::VerifyCanonical { vectors, show } => {
            commands::verify_canonical::run(&root, vectors, show)
        }
        Commands::PerformanceGate { update } => commands::performance_gate::run(&root, update),
        Commands::EmbeddedGate => commands::embedded_gate::run(&root),
        Commands::Rp2040Hil {
            port,
            expected_plan_hash,
            expected_firmware_identity,
            maximum_decisions,
            timeout_seconds,
            probe,
            require_hardware,
        } => commands::rp2040_hil::run(
            &root,
            commands::rp2040_hil::Rp2040HilOptions {
                port,
                expected_plan_hash,
                expected_firmware_identity,
                maximum_decisions,
                timeout_seconds,
                probe,
                require_hardware,
            },
        ),
        Commands::AdversarialProfile { profile } => {
            commands::adversarial_profile::run(&root, &profile)
        }
        Commands::Serve { directory, port } => commands::serve::run(directory, port),
        Commands::CheckPythonBoundary => commands::check_python_boundary::run(&root),
        Commands::ReleaseGate { check, output } => {
            commands::release_gate::run(&root, check, output.as_deref())
        }
        Commands::CheckAll => commands::check_all::run(&root),
    }
}
