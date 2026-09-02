//! Repository demonstration entrances that hide package and fixture details.

use crate::cli::{GlobalOpts, PatchbayDemoArgs, PatchbayHost};
use crate::process::{run_step, Step};
use crate::workspace::workspace_root;

pub fn run_book(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let product = root.join("target/book-product");
    if product.exists() {
        std::fs::remove_dir_all(&product)?;
    }
    run_step(
        &Step::new(
            "demo.book.runtime",
            "Build the ordinary bounded browser Host runtime",
            "cargo",
            &[
                "build",
                "-p",
                "conduit-browser-runtime",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
            ],
        ),
        &root,
        opts,
    )?;
    run_step(
        &Step::new(
            "demo.book.package",
            "Stage the exact admitted Book application",
            "scripts/ci/stage-book-product.sh",
            &[
                "target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
                "target/book-product",
            ],
        ),
        &root,
        opts,
    )?;
    run_step(
        &Step::new(
            "demo.book.host",
            "Open the inline executable Conduit book",
            "cargo",
            &[
                "run",
                "-p",
                "conduit-browser-host",
                "--",
                "--application",
                "target/book-product",
                "--mount",
                "/book/",
            ],
        ),
        &root,
        opts,
    )?;
    Ok(())
}

pub fn run_std(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    run(
        "demo.std",
        "Launch the ordinary std Host with the canonical Hello Form",
        &[
            "run",
            "-p",
            "conduit",
            "--",
            "run",
            "examples/hello.conduit",
        ],
        opts,
    )
}

pub fn run_triple(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    run(
        "demo.triple",
        "Run the three-sink Form locally",
        &[
            "run",
            "-p",
            "conduit",
            "--",
            "run",
            "proof/fixtures/forms/triple-signal.conduit",
            "--placements",
            "examples/triple-local.placements",
        ],
        opts,
    )
}

fn run(
    id: &'static str,
    description: &'static str,
    args: &'static [&'static str],
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let step = Step::new(id, description, "cargo", args);
    run_step(&step, &root, opts)?;
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
                "demo.patchbay.browser-host",
                "Build the real browser Host membership runtime",
                "cargo",
                &[
                    "build",
                    "-p",
                    "conduit-browser-runtime",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--release",
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
            "examples/default-welcome.conduit",
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
            "examples/text-lab.conduit",
            "--seed",
            "Hello",
            "examples/hello.conduit",
            "--seed",
            "Greet",
            "examples/greet.conduit",
            "--seed",
            "Clock",
            "examples/clock.conduit",
            "--seed",
            "Count",
            "examples/count.conduit",
        ][..]
    };
    let step = Step::new(
        "demo.patchbay",
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
        "demo.environment",
        "Open the bounded authored physical-environment workspace",
        "cargo",
        &[
            "run",
            "-p",
            "patchbay-native",
            "--",
            "--environment",
            "examples/maker-workbench.json",
        ],
    );
    run_step(&step, &root, opts)?;
    Ok(())
}

pub fn run_prewake(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let step = Step::new(
        "demo.prewake",
        "Rehearse the canonical Form against authored simulation truth",
        "cargo",
        &[
            "run",
            "-p",
            "patchbay-native",
            "--",
            "--prewake",
            "--form",
            "examples/hello.conduit",
            "--environment",
            "examples/maker-workbench.json",
        ],
    );
    run_step(&step, &root, opts)?;
    Ok(())
}

pub fn run_text_lab(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let step = Step::new(
        "demo.text-lab",
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
            "examples/text-lab.conduit",
            "--environment",
            "examples/maker-workbench.json",
        ],
    );
    run_step(&step, &root, opts)?;
    Ok(())
}
