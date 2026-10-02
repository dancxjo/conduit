//! Exact setup-only routes; all provisioning remains in the owning target modules.
#[allow(dead_code)] // The full xtask uses the remaining AVR upload helpers.
#[path = "../../xtask/src/commands/avr/avr_toolchain.rs"]
mod avr_toolchain;
#[path = "../../xtask/src/commands/avr/rust_toolchain.rs"]
mod rust_toolchain;
#[allow(dead_code)] // The build path also uses the pinned HAL revision.
#[path = "../../xtask/src/commands/avr/setup.rs"]
mod setup;
#[path = "../../xtask/src/commands/avr/tool_support.rs"]
mod tool_support;
use tool_support::{require_success, sha256_file};

type PicoResult<T> = Result<T, Box<dyn std::error::Error>>;
#[allow(dead_code)] // The full Pico tooling also consumes the firmware commit identity.
#[path = "../../../targets/rp2040/firmware/pico-w-signal/make/xtask/doctor.rs"]
mod pico_doctor;

#[derive(Debug, PartialEq, Eq)]
pub enum Setup {
    Avr,
    Pico,
}

pub fn route(arguments: &[String]) -> Option<Setup> {
    let [make, target, action, options @ ..] = arguments else {
        return None;
    };
    // Unknown options, including help/dry-run, retain the full CLI's parsing.
    if make != "make" || !(options.is_empty() || options == ["--locked"]) {
        return None;
    }
    match (target.as_str(), action.as_str()) {
        ("avr", "check") => Some(Setup::Avr),
        ("pico", "doctor") => Some(Setup::Pico),
        _ => None,
    }
}

pub fn run(setup: Setup) -> PicoResult<()> {
    match setup {
        Setup::Avr => {
            setup::check(&crate::workspace::workspace_root()?)?;
            println!("AVR boundary ready");
            Ok(())
        }
        Setup::Pico => pico_doctor::run_doctor(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn exact_setup_and_ci_locked_invocations_take_shared_fast_path() {
        for (target, action, expected) in [
            ("avr", "check", Setup::Avr),
            ("pico", "doctor", Setup::Pico),
        ] {
            assert_eq!(route(&args(&["make", target, action])), Some(expected));
            assert!(route(&args(&["make", target, action, "--locked"])).is_some());
        }
    }

    #[test]
    fn other_commands_and_options_keep_full_cli_semantics() {
        for values in [
            vec!["make", "avr", "build"],
            vec!["make", "pico", "flash"],
            vec!["make", "avr", "check", "--dry-run"],
            vec!["make", "pico", "doctor", "--help"],
            vec!["make", "pico", "doctor", "--locked", "--json"],
            vec!["make", "avr", "check", "--unknown"],
        ] {
            assert_eq!(route(&args(&values)), None, "{values:?}");
        }
    }
}
