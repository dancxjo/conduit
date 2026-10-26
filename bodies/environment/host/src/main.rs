//! Attended Linux Body owner for an independently initialized native MCU Host.
//! Current offers and signed Root custody remain separate from Body lifecycle.
extern crate alloc;
mod admission;
mod display_check;
mod display_evidence;
mod execution;
mod local_recorder;
mod mask;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_display_evidence.rs"]
#[allow(dead_code)]
mod native_display_evidence;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_owner_control.rs"]
#[allow(dead_code)]
mod native_owner_control;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_owner_receipt.rs"]
#[allow(dead_code)]
mod native_owner_receipt;
mod observation;
mod observed_history;
mod owner_exchange;
mod owner_reconnect;
mod planning;
mod transport;
use std::{error::Error, fmt::Debug, path::PathBuf};
pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
pub fn checked<T, E: Debug>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|error| format!("{error:?}").into())
}
fn main() -> Result<()> {
    let mut args: Vec<_> = std::env::args().collect();
    if args
        .get(1)
        .is_some_and(|argument| argument == "--record-joined")
    {
        let once = args.last().is_some_and(|argument| argument == "--once");
        if once {
            args.pop();
        }
        if args.len() != 5 {
            return Err(
                "expected --record-joined PRIVATE_BINDING PRIVATE_STATE HOST:PORT [--once]".into(),
            );
        }
        return local_recorder::run_joined(
            std::path::Path::new(&args[2]),
            &args[4],
            std::path::Path::new(&args[3]),
            once,
        );
    }
    let resume = args
        .iter()
        .position(|arg| arg.starts_with("--resume="))
        .map(|index| args.remove(index)["--resume=".len()..].to_owned());
    let history_peer = args
        .iter()
        .position(|arg| arg.starts_with("--history-peer="))
        .map(|index| args.remove(index)["--history-peer=".len()..].parse::<std::net::SocketAddr>())
        .transpose()?;
    let display_check = args.last().is_some_and(|a| a == "--display-check");
    let mask_mode = args.last().is_some_and(|a| a == "--mask-prepare");
    if display_check || mask_mode {
        args.pop();
    }
    if args.len() == 3 && args[1] == "--provision" {
        return admission::provision(std::path::Path::new(&args[2]));
    }
    if !(3..=4).contains(&args.len()) {
        return Err(
            "expected SERIAL_PORT PRIVATE_STATE_DIRECTORY [PUBLICATIONS=1, maximum 960] [--display-check|--mask-prepare] [--history-peer=IP:PORT]".into(),
        );
    }
    let limit: u32 = args.get(3).map_or(Ok(1), |s| s.parse())?;
    if !(1..=960).contains(&limit) {
        return Err("publication bound must be 1..=960".into());
    }
    let state = PathBuf::from(&args[2]);
    let mut line = transport::Line::open(&args[1])?;
    let root = line.command("offer", "native-root-offer")?;
    if root["owner_control_configured"] != true {
        return Err("MCU owner public key is not provisioned".into());
    }
    if root["owner_control_sequence"] != 0 {
        return Err(
            "this attended profile requires a fresh MCU Boot; prior ownership remains retained"
                .into(),
        );
    }
    let authority = admission::Authority::new(&state)?;
    let source = checked(conduit_environment_runtime::construction::compile_default())?;
    let display_source = if display_check {
        Some(checked(
            conduit_environment_make::DisplayOutputSource::compile(),
        )?)
    } else {
        None
    };
    let mut body = match resume {
        Some(path) => authority.resume(std::path::Path::new(&path))?,
        None => authority.birth(&source, display_source.as_ref())?,
    };
    let (mut advertisement, device_key) = authority.admit(&mut body, &mut line, &args[1])?;
    let software = line.command("software-offer", "native-software-offer")?;
    if software["host_id"] != serde_json::to_value(&advertisement.host_id)?
        || software["boot_id"] != serde_json::to_value(&advertisement.boot_id)?
        || software["offer_generation"] != advertisement.offer_generation.0
    {
        return Err("software offer has a different current Host scope".into());
    }
    advertisement.capabilities.extend(
        serde_json::from_value::<Vec<conduit_core::CapabilityOffer>>(
            software["capabilities"].clone(),
        )?,
    );
    let policy = line.command("appliance-policy", "native-appliance-policy")?;
    let display_policy = if display_check {
        let value = line.command("display-software-offer", "native-display-software-offer")?;
        advertisement.capabilities.extend(serde_json::from_value::<
            Vec<conduit_core::CapabilityOffer>,
        >(value["capabilities"].clone())?);
        Some(value)
    } else {
        None
    };
    let display = display_source
        .as_ref()
        .zip(display_policy.as_ref())
        .map(|(source, policy)| crate::display_check::prepare(source, &advertisement, policy))
        .transpose()?;
    let mask = if mask_mode {
        let value = line.command("mask-policy", "native-mask-policy")?;
        advertisement.capabilities.extend(serde_json::from_value::<
            Vec<conduit_core::CapabilityOffer>,
        >(value["capabilities"].clone())?);
        Some(mask::prepare(&advertisement, &value)?)
    } else {
        None
    };
    let prepared = planning::prepare(
        &mut body,
        &authority,
        source,
        advertisement,
        policy,
        display,
        mask,
    )?;
    authority.save(&body, "body-prepared.json")?;
    execution::run(
        &mut line,
        &authority,
        &mut body,
        prepared,
        device_key,
        limit,
        history_peer,
    )?;
    Ok(())
}

#[cfg(test)]
mod evidence_compact_tests;
#[cfg(test)]
mod evidence_journal_tests;
#[cfg(test)]
mod observed_record_tests;
