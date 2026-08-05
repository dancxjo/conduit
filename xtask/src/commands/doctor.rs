use std::process::Command;

use crate::{
    cli::{DoctorArgs, DoctorTarget, GlobalOpts},
    process::StepError,
    workspace::workspace_root,
};

pub fn run(args: DoctorArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root()
        .map_err(|e| StepError::prereq("workspace-root", e))?;

    match args.target {
        DoctorTarget::All => {
            general(opts);
            browser_doctor(opts);
        }
        DoctorTarget::Browser => browser_doctor(opts),
        DoctorTarget::Pico => pico_doctor(opts),
    }
    let _ = root;
    Ok(())
}

fn tool_version(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "(not found)".into())
}

fn general(opts: &GlobalOpts) {
    if opts.quiet {
        return;
    }
    println!("── General prerequisites ────────────────────────────────");

    let rustc = tool_version("rustc", &["--version"]);
    let cargo = tool_version("cargo", &["--version"]);
    println!("  rustc   : {rustc}");
    println!("  cargo   : {cargo}");

    let node = tool_version("node", &["--version"]);
    let npm = tool_version("npm", &["--version"]);
    let npx = tool_version("npx", &["--version"]);
    println!("  node    : {node}");
    println!("  npm     : {npm}");
    println!("  npx     : {npx}");

    let targets = tool_version("rustup", &["target", "list", "--installed"]);
    println!("  rustup targets installed:\n{}", indent(&targets, 4));

    // Git commit / dirty state.
    let commit = tool_version("git", &["rev-parse", "--short", "HEAD"]);
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    println!("  git commit : {commit}{}", if dirty { " (dirty)" } else { "" });
}

fn browser_doctor(opts: &GlobalOpts) {
    if opts.quiet {
        return;
    }
    println!("── Browser prerequisites ─────────────────────────────────");

    let pw = tool_version("npx", &["playwright", "--version"]);
    println!("  playwright : {pw}");

    // Check for chromium install by querying playwright.
    let chromium = Command::new("npx")
        .args(["playwright", "install", "--dry-run", "chromium"])
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout).to_string()
                + &String::from_utf8_lossy(&o.stderr);
            if s.contains("chromium") { "present (or installable)" } else { "unknown" }
        })
        .unwrap_or("unknown");
    println!("  chromium   : {chromium}");
    println!("  Repair hint: npx playwright install chromium");
}

fn pico_doctor(opts: &GlobalOpts) {
    if opts.quiet {
        return;
    }
    println!("── Pico prerequisites ────────────────────────────────────");

    let targets = tool_version("rustup", &["target", "list", "--installed"]);
    let thumb = if targets.contains("thumbv6m-none-eabi") { "installed" } else { "MISSING" };
    println!("  thumbv6m-none-eabi target : {thumb}");
    if thumb == "MISSING" {
        println!("  Repair hint: rustup target add thumbv6m-none-eabi");
    }

    let elf2uf2 = tool_version("elf2uf2-rs", &["--version"]);
    println!("  elf2uf2-rs : {elf2uf2}");
    if elf2uf2 == "(not found)" {
        println!("  Repair hint: cargo install elf2uf2-rs");
    }
}

fn indent(s: &str, n: usize) -> String {
    let pad = " ".repeat(n);
    s.lines().map(|l| format!("{pad}{l}")).collect::<Vec<_>>().join("\n")
}
