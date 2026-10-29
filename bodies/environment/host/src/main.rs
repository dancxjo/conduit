//! Attended Linux Body owner for an independently initialized native MCU Host.
//! Current offers and signed Root custody remain separate from Body lifecycle.
extern crate alloc;
mod admission;
#[allow(dead_code, clippy::duplicate_mod)]
// Shared membership/producer proof APIs accompany the typed owner route.
mod biography_custody_remote;
mod biography_owner_client;
mod body_join;
mod body_join_proxy;
mod native_body_join {
    pub(crate) use crate::body_join::owner;
}
mod display_check;
mod display_evidence;
mod execution;
mod face_export_client;
#[cfg(test)]
mod history_relay;
#[cfg(test)]
mod history_relay_kernel;
#[cfg(test)]
mod history_relay_plan;
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
mod network_custody_client;
mod observation;
#[allow(clippy::duplicate_mod)] // Prototype tests separately include this exact portable client.
#[allow(dead_code)] // Typed custody remains dormant until authenticated producer offers exist.
mod observed_custody_client;
mod observed_custody_live;
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
    if args.get(1).is_some_and(|a| a == "--inspect-retained-history") {
        if args.len() != 6 {
            return Err("expected --inspect-retained-history PRIVATE_BINDING EPOCH_DIRECTORY PRODUCER_BOOT PRIVATE_SERIES_FILE".into());
        }
        return observed_history::retained::run(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
            &args[4],
            std::path::Path::new(&args[5]),
        );
    }
    if args.get(1).is_some_and(|a| a == "--publish-face" || a == "--publish-face-once") {
        if args.len()!=5 { return Err("expected --publish-face PRIVATE_BINDING HOST:PORT PRIVATE_FACE_FILE".into()); }
        return face_export_client::run(std::path::Path::new(&args[2]),&args[3],std::path::Path::new(&args[4]),args[1]=="--publish-face-once");
    }
    if args.get(1).is_some_and(|a| a == "--retain-network-snapshot") {
        if args.len() != 6 {
            return Err("expected --retain-network-snapshot BINDING PEER PRIVATE_RUN PRODUCER_BOOT".into());
        }
        return network_custody_client::resume::run(
            std::path::Path::new(&args[2]), &args[3], std::path::Path::new(&args[4]), &args[5]);
    }
    if args.get(1).is_some_and(|a| a == "--ack-network-snapshot") {
        if args.len() != 9 {
            return Err("expected --ack-network-snapshot BINDING CONTINUITY_PRIVATE PEER PRIVATE_STORE OFFER_JSON SNAPSHOT_BIN PRODUCER_BOOT".into());
        }
        return network_custody_client::live::run(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
            &args[4],
            std::path::Path::new(&args[5]),
            std::path::Path::new(&args[6]),
            std::path::Path::new(&args[7]),
            &args[8],
        );
    }
    if args.get(1).is_some_and(|a| a == "--ack-observed-live") {
        if args.len() != 7 {
            return Err("expected --ack-observed-live PRIVATE_BINDING CONTINUITY_PRIVATE PEER HISTORY_DIRECTORY SLOT".into());
        }
        return observed_custody_live::run(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
            &args[4],
            std::path::Path::new(&args[5]),
            args[6].parse()?,
        );
    }
    if args.get(1).is_some_and(|a| a == "--ack-observed-epoch") {
        if args.len() != 7 {
            return Err("expected --ack-observed-epoch PRIVATE_BINDING CONTINUITY_PRIVATE SERIAL HISTORY_DIRECTORY SLOT".into());
        }
        return observed_custody_client::run(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
            &args[4],
            std::path::Path::new(&args[5]),
            args[6].parse()?,
        );
    }
    if args.get(1).is_some_and(|a| a == "--offload-biography") {
        if args.len() != 6 {
            return Err(
                "expected --offload-biography OWNER_STATE SERIAL PRIVATE_STORE PRIVATE_PIN_JSON"
                    .into(),
            );
        }
        return biography_owner_client::run(
            std::path::Path::new(&args[2]),
            &args[3],
            std::path::Path::new(&args[4]),
            std::path::Path::new(&args[5]),
        );
    }
    if args.get(1).is_some_and(|a| a == "--return-observer") {
        if args.len() != 6 {
            return Err("expected --return-observer PRIVATE_BINDING CONTINUITY_PRIVATE NEW_PRIVATE_STATE HOST:PORT".into());
        }
        return body_join::return_observer(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
            std::path::Path::new(&args[4]),
            &args[5],
        );
    }
    if args.get(1).is_some_and(|a| a == "--browser-join-proxy") {
        if !(3..=4).contains(&args.len()) {
            return Err(
                "expected --browser-join-proxy DEVICE_HOST:PORT [LOOPBACK_PORT=8787]".into(),
            );
        }
        return body_join_proxy::run(&args[2], args.get(3).map_or(Ok(8787), |port| port.parse())?);
    }

    if args.get(1).is_some_and(|a| a == "--browser-invitation") {
        if args.len() != 4 {
            return Err("expected --browser-invitation PRIVATE_BOOTSTRAP LOCAL_HELPER.html".into());
        }
        return body_join::browser(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        );
    }

    if args
        .get(1)
        .is_some_and(|a| a == "--issue-private-invitation" || a == "--join-observer")
    {
        if args.len() != 5 {
            return Err("expected --issue-private-invitation OWNER_STATE SERIAL PRIVATE_OUTPUT or --join-observer PRIVATE_BOOTSTRAP NEW_PRIVATE_STATE HOST:PORT".into());
        }
        return if args[1] == "--issue-private-invitation" {
            body_join::issue(
                std::path::Path::new(&args[2]),
                &args[3],
                std::path::Path::new(&args[4]),
            )
        } else {
            body_join::join(
                std::path::Path::new(&args[2]),
                std::path::Path::new(&args[3]),
                &args[4],
            )
        };
    }
    let projection_args = if matches!(
        args.get(1).map(String::as_str),
        Some("--record-joined" | "--record-joined-continuing")
    ) {
        if let Some(index) = args.iter().position(|arg| arg == "--retained-projection") {
            if index < 2 || index + 2 >= args.len() {
                return Err("expected --retained-projection PRODUCER_BOOT PRIVATE_OUTPUT".into());
            }
            let values: Vec<_> = args.drain(index..index + 3).collect();
            Some((values[1].clone(), values[2].clone()))
        } else {
            None
        }
    } else {
        None
    };
    let projection =
        projection_args
            .as_ref()
            .map(|(boot, output)| observed_history::retained::Projection {
                producer_boot: boot,
                output: std::path::Path::new(output),
            });
    if args
        .get(1)
        .is_some_and(|argument| argument == "--record-joined-continuing")
    {
        if !matches!(args.len(), 6 | 8) {
            return Err("expected --record-joined-continuing PRIVATE_BINDING CONTINUITY_PRIVATE HISTORY_DIRECTORY HOST:PORT[,HOST:PORT...] (at most four explicit peers) [PRODUCER_BOOT PRIVATE_NETWORK_RUN] [--retained-projection PRODUCER_BOOT PRIVATE_OUTPUT]".into());
        }
        return local_recorder::run_joined_continuing(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
            &args[5],
            std::path::Path::new(&args[4]),
            (args.len() == 8).then(|| local_recorder::network::Options {
                producer_boot: &args[6],
                private_run: std::path::Path::new(&args[7]),
            }),
            projection,
        );
    }
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
                "expected --record-joined PRIVATE_BINDING PRIVATE_STATE HOST:PORT [--once] [--retained-projection PRODUCER_BOOT PRIVATE_OUTPUT]".into(),
            );
        }
        return local_recorder::run_joined(
            std::path::Path::new(&args[2]),
            &args[4],
            std::path::Path::new(&args[3]),
            once,
            projection,
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

#[cfg(all(feature = "carrier-source", target_os = "linux"))]
mod device_carrier_source;
