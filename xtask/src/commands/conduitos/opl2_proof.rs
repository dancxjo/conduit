use std::fs;

use serde_json::Value;

use crate::cli::GlobalOpts;

use super::{profile::Paths, run, ConduitosArch, ConduitosError};

const PREFIX: &str = "CONDUIT_OPL2_SIGN ";

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let _run = run::execute(ConduitosArch::X86_64, opts)?;
    let serial = fs::read_to_string(paths.target.join("boot-serial.log")).map_err(|error| {
        ConduitosError::refusal(
            "opl2-proof-unavailable",
            format!("cannot read serial: {error}"),
        )
    })?;
    let signs = serial
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-opl2-sign",
            format!("expected one exact OPL2 Sign, found {}", signs.len()),
        ));
    }
    let sign: Value = serde_json::from_str(signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-opl2-sign", error.to_string()))?;
    validate(&sign)?;
    fs::write(
        &paths.opl2_proof,
        serde_json::to_vec_pretty(&sign)
            .map_err(|error| ConduitosError::refusal("malformed-opl2-sign", error.to_string()))?,
    )
    .map_err(|error| {
        ConduitosError::refusal(
            "opl2-proof-unavailable",
            format!("cannot write {}: {error}", paths.opl2_proof.display()),
        )
    })?;
    println!("ConduitOS OPL2 proof: {}", paths.opl2_proof.display());
    Ok(())
}

fn validate(sign: &Value) -> Result<(), ConduitosError> {
    let exact_strings = [
        ("schema", "conduit.conduitos.opl2-proof/v1"),
        ("status", "completed"),
        ("proof_class", "freestanding-emulator"),
        ("implementation", conduitos::opl2_offer::OPL2_IMPLEMENTATION),
        (
            "execution_profile",
            conduitos::opl2_offer::OPL2_EXECUTION_PROFILE,
        ),
        ("patch_profile", conduitos::opl2_offer::OPL2_PATCH_PROFILE),
        ("device", "qemu-adlib-ym3812"),
    ];
    let exact_numbers = [
        ("placements", 3),
        ("cords", 2),
        ("events", 24),
        ("peak_voices", 9),
        ("voice_capacity", 9),
        ("reset_writes", 245),
        ("patch_writes", 99),
        ("event_writes", 36),
        ("quiesce_writes", 9),
        ("register_write_capacity", 512),
        ("final_active_voices", 0),
        ("iobase", 904),
        ("normalized_events", 24),
    ];
    let strings_valid = exact_strings
        .iter()
        .all(|(key, expected)| sign[*key].as_str() == Some(*expected));
    let numbers_valid = exact_numbers
        .iter()
        .all(|(key, expected)| sign[*key].as_u64() == Some(*expected));
    let identities_valid = ["host_id", "boot_id", "base_id", "plan_id", "active_play_id"]
        .iter()
        .all(|key| sign[*key].as_str().is_some_and(|value| value.len() == 64));
    let truth_valid = sign["bounded"] == true
        && sign["completed"] == true
        && sign["pcm_claimed"] == false
        && sign["subtractive_controls_claimed"] == false
        && sign["physical_hardware_claimed"] == false
        && sign["kernel_decisions"]
            .as_u64()
            .is_some_and(|value| value > 0)
        && sign["kernel_signs"].as_u64().is_some_and(|value| value > 0);
    let oracle_valid = sign["normalized_terminal"] == "completed"
        && sign["normalized_plan_id"] == sign["plan_id"]
        && sign["normalized_implementation"] == sign["implementation"];
    if strings_valid && numbers_valid && identities_valid && truth_valid && oracle_valid {
        Ok(())
    } else {
        Err(ConduitosError::refusal(
            "invalid-opl2-proof",
            "OPL2 Sign did not preserve exact Plan/Base/voice/work/terminal proof",
        ))
    }
}
