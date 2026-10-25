//! Attended Linux Body owner for an independently initialized native MCU Host.
//! Current offers and signed Root custody remain separate from Body lifecycle.
extern crate alloc;
mod admission;
mod execution;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_owner_control.rs"]
#[allow(dead_code)]
mod native_owner_control;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_owner_receipt.rs"]
#[allow(dead_code)]
mod native_owner_receipt;
mod observation;
mod planning;
mod transport;
use std::{error::Error, fmt::Debug, path::PathBuf};
pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
pub fn checked<T, E: Debug>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|error| format!("{error:?}").into())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--provision" {
        return admission::provision(std::path::Path::new(&args[2]));
    }
    if !(3..=4).contains(&args.len()) {
        return Err(
            "expected SERIAL_PORT PRIVATE_STATE_DIRECTORY [PUBLICATIONS=1, maximum 960]".into(),
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
    let mut body = authority.birth(&source)?;
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
    let prepared = planning::prepare(&mut body, &authority, source, advertisement, policy)?;
    authority.save(&body, "body-prepared.json")?;
    execution::run(
        &mut line, &authority, &mut body, prepared, device_key, limit,
    )?;
    Ok(())
}
