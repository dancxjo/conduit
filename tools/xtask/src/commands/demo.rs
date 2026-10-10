//! Repository demonstration entrances that hide package and fixture details.

use crate::cli::{GlobalOpts, PatchbayDemoArgs, PatchbayHost};
use crate::process::{run_step, Step};
use crate::workspace::workspace_root;

pub(crate) const STD_STEP: Step = Step::new(
    "journey.std",
    "Launch the ordinary std Host with the canonical Hello Plot",
    "cargo",
    &[
        "run",
        "-p",
        "conduit",
        "--",
        "run",
        "plots/hello/main.conduit",
    ],
);

pub(crate) const TRIPLE_STEP: Step = Step::new(
    "journey.triple",
    "Run the three-sink Plot locally",
    "cargo",
    &[
        "run",
        "-p",
        "conduit",
        "--",
        "run",
        "proof/fixtures/plots/triple-signal.conduit",
        "--placements",
        "proof/fixtures/placements/triple-local.placements",
        "--await-terminal",
    ],
);

pub fn run_std(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    run_step(&STD_STEP, &workspace_root()?, opts)?;
    Ok(())
}

pub fn run_triple(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    run_step(&TRIPLE_STEP, &workspace_root()?, opts)?;
    Ok(())
}

pub fn run_patchbay(
    args: &PatchbayDemoArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    if !args.first_run_proof && args.on == PatchbayHost::Browser {
        run_step(
            &Step::new(
                "journey.patchbay.browser-host",
                "Build the real browser Host membership runtime",
                "cargo",
                &[
                    "build",
                    "-p",
                    "conduit-browser-runtime",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--release",
                    "--no-default-features",
                ],
            ),
            &root,
            opts,
        )?;
    }
    let command = if args.first_run_proof {
        &[
            "run",
            "-p",
            "patchbay-native",
            "--",
            "--plot",
            "plots/default-welcome/main.conduit",
            "--first-run-proof",
        ][..]
    } else if args.on == PatchbayHost::Native {
        &["run", "-p", "patchbay-native", "--", "--front-door"][..]
    } else {
        &[
            "run",
            "-p",
            "conduit-browser-patchbay-workbench",
            "--",
            "--seed",
            "Text Lab",
            "plots/text-lab/main.conduit",
            "--seed",
            "Hello",
            "plots/hello/main.conduit",
            "--seed",
            "Greet",
            "plots/greet/main.conduit",
            "--seed",
            "Clock",
            "plots/clock/main.conduit",
            "--seed",
            "Count",
            "plots/count/main.conduit",
        ][..]
    };
    let step = Step::new(
        "journey.patchbay",
        if args.first_run_proof {
            "Prove the bounded native Patchbay first-run journey"
        } else if args.on == PatchbayHost::Browser {
            "Build and serve the shared Patchbay entrance through a browser Host"
        } else {
            "Build and launch the shared Patchbay entrance through the native Host"
        },
        "cargo",
        command,
    );
    run_step(&step, &root, opts)?;
    Ok(())
}

pub fn run_body_membership(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    super::body_membership_demo::run(opts)
}

pub fn run_environment(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let step = Step::new(
        "journey.environment",
        "Open the bounded authored physical-environment workspace",
        "cargo",
        &[
            "run",
            "-p",
            "patchbay-native",
            "--",
            "--environment",
            "plots/patchbay/workbench/examples/maker-workbench.json",
        ],
    );
    run_step(&step, &root, opts)?;
    Ok(())
}

pub fn run_prewake(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let step = Step::new(
        "journey.prewake",
        "Rehearse the canonical Plot against authored simulation truth",
        "cargo",
        &[
            "run",
            "-p",
            "patchbay-native",
            "--",
            "--prewake",
            "--plot",
            "plots/hello/main.conduit",
            "--environment",
            "plots/patchbay/workbench/examples/maker-workbench.json",
        ],
    );
    run_step(&step, &root, opts)?;
    Ok(())
}

pub fn run_text_lab(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let step = Step::new(
        "journey.text-lab",
        "Open the ordinary native Text Lab through effect-free PREWAKE",
        "cargo",
        &[
            "run",
            "-p",
            "patchbay-native",
            "--",
            "--prewake",
            "--prewake-hold",
            "--plot",
            "plots/text-lab/main.conduit",
            "--environment",
            "plots/patchbay/workbench/examples/maker-workbench.json",
        ],
    );
    run_step(&step, &root, opts)?;
    Ok(())
}

/// The public repository entrance owns package invocation details.
pub fn run_thermostat(
    args: &crate::cli::ThermostatDemoArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    if args.verify {
        run_step(
            &Step::new(
                "thermostat.contracts",
                "Check thermostat meaning, Face availability and authored kernel execution",
                "cargo",
                &[
                    "test",
                    "--locked",
                    "-p",
                    "conduit-thermostat-plot",
                    "-p",
                    "conduit-thermostat-face",
                    "-p",
                    "conduit-thermostat-app",
                ],
            ),
            &root,
            opts,
        )?;
        run_step(
            &Step::new(
                "thermostat.browser",
                "Prove the semantic thermostat controls in pinned Chromium",
                "node",
                &[
                    "proof/browser/node_modules/@playwright/test/cli.js",
                    "test",
                    "--config",
                    "proof/browser/thermostat.config.mjs",
                    "--project",
                    "chromium",
                ],
            ),
            &root,
            opts,
        )?;
    } else {
        let step = Step::new(
            "journey.thermostat",
            "Open the semantic thermostat browser Mask over an authored kernel Plot",
            "cargo",
            &["run", "--locked", "-p", "conduit-thermostat-app"],
        );
        let mut arguments: Vec<String> = step.args.iter().map(|value| (*value).into()).collect();
        arguments.extend(["--".into(), "--port".into(), args.port.to_string()]);
        crate::process::run_step_with_arguments(&step, &arguments, &root, opts)?;
    }
    Ok(())
}
