//! Repository demonstration entrances that hide package and fixture details.

use crate::cli::{GlobalOpts, PatchbayDemoArgs, PatchbayHost};
use crate::process::{run_step, Step};
use crate::workspace::workspace_root;

pub(crate) const STD_STEP: Step = Step::new(
    "journey.std",
    "Launch the ordinary std Host with the canonical Hello Form",
    "cargo",
    &[
        "run",
        "-p",
        "conduit",
        "--",
        "run",
        "forms/hello/main.conduit",
    ],
);

pub(crate) const TRIPLE_STEP: Step = Step::new(
    "journey.triple",
    "Run the three-sink Form locally",
    "cargo",
    &[
        "run",
        "-p",
        "conduit",
        "--",
        "run",
        "proof/fixtures/forms/triple-signal.conduit",
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
            "--form",
            "forms/default-welcome/main.conduit",
            "--first-run-proof",
        ][..]
    } else if args.on == PatchbayHost::Native {
        &["run", "-p", "patchbay-native", "--", "--front-door"][..]
    } else {
        &[
            "run",
            "-p",
            "patchbay-html",
            "--",
            "--seed",
            "Text Lab",
            "forms/text-lab/main.conduit",
            "--seed",
            "Hello",
            "forms/hello/main.conduit",
            "--seed",
            "Greet",
            "forms/greet/main.conduit",
            "--seed",
            "Clock",
            "forms/clock/main.conduit",
            "--seed",
            "Count",
            "forms/count/main.conduit",
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
            "forms/patchbay/workbench/examples/maker-workbench.json",
        ],
    );
    run_step(&step, &root, opts)?;
    Ok(())
}

pub fn run_prewake(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let step = Step::new(
        "journey.prewake",
        "Rehearse the canonical Form against authored simulation truth",
        "cargo",
        &[
            "run",
            "-p",
            "patchbay-native",
            "--",
            "--prewake",
            "--form",
            "forms/hello/main.conduit",
            "--environment",
            "forms/patchbay/workbench/examples/maker-workbench.json",
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
            "--form",
            "forms/text-lab/main.conduit",
            "--environment",
            "forms/patchbay/workbench/examples/maker-workbench.json",
        ],
    );
    run_step(&step, &root, opts)?;
    Ok(())
}
