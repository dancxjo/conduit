//! Parallel generated Pico sink images for the three-peer R1 control Plan.

use std::fs;
use std::path::{Path, PathBuf};

use conduit_core::BootId;
use conduit_embedded_build::generate_embedded_plan;
use conduit_runtime::lowering::lower_plan_fragment;

use super::{
    pico_signal_bounds, render_firmware_module, render_identity_sidecar,
    GeneratedFirmwareIdentity, R1_CONTROL_FORM,
};

pub(super) fn generate(out: &Path, activate: bool) {
    let form = conduit_form::parse(
        R1_CONTROL_FORM,
        &conduit_signal::signal_profile_catalog(),
    )
    .expect("R1 three-peer control Form must check");
    for (stem, routes) in [
        (
            "r1_control_plan_a_signal",
            conduit_system_continuity::R1SignalRouteSet::WebSocketOnly,
        ),
        (
            "r1_control_plan_b_signal",
            conduit_system_continuity::R1SignalRouteSet::UsbOnly,
        ),
        (
            "r1_control_plan_c_signal",
            conduit_system_continuity::R1SignalRouteSet::WebSocketThenUsb,
        ),
    ] {
        let exact = conduit_system_continuity::exact_r1_control_plan(
            BootId::from(conduit_net::R1_PICO_BOOT_ID),
            routes,
        )
        .expect("exact R1 three-peer control Plan must resolve");
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id.as_str() == conduit_net::R1_PICO_HOST_ID)
            .expect("R1 control Plan must contain the Pico fragment");
        let lowered = lower_plan_fragment(fragment).expect("R1 control Pico fragment must lower");
        let generated = generate_embedded_plan(fragment, &lowered, pico_signal_bounds())
            .expect("R1 control Pico fragment must fit reviewed fixed-image bounds");
        let identity = GeneratedFirmwareIdentity::new(&form, &generated);
        let rendered = render_firmware_module(&generated, &identity);
        let sidecar = render_identity_sidecar(&generated, &identity);
        fs::write(out.join(format!("{stem}_image.rs")), &rendered)
        .expect("generated R1 control Pico image should be writable");
        fs::write(out.join(format!("{stem}_identity.json")), &sidecar)
        .expect("generated R1 control Pico identity sidecar should be writable");
        if activate {
            let active_stem = match routes {
                conduit_system_continuity::R1SignalRouteSet::WebSocketOnly => {
                    "pico_signal_image.rs"
                }
                conduit_system_continuity::R1SignalRouteSet::UsbOnly => {
                    "r1_plan_b_signal_image.rs"
                }
                conduit_system_continuity::R1SignalRouteSet::WebSocketThenUsb => {
                    "r1_plan_c_signal_image.rs"
                }
            };
            fs::write(out.join(active_stem), &rendered)
                .expect("active R1 control Pico image should be writable");
            if matches!(
                routes,
                conduit_system_continuity::R1SignalRouteSet::WebSocketOnly
            ) {
                write_explicit_identity(&sidecar);
            }
        }
    }
}

fn write_explicit_identity(sidecar: &str) {
    let Ok(path) = std::env::var(super::IDENTITY_SIDECAR_ENV) else {
        return;
    };
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .expect("explicit R1 control identity directory should be writable");
    }
    fs::write(path, sidecar).expect("explicit R1 control identity sidecar should be writable");
}
