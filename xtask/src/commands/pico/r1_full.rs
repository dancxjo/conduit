//! One-boot physical R1 orchestration across both recovery branches.

use conduit_std_host::usb_cdc::{NativePathCdcLine, NativePathCdcLineReader};

use super::firmware::FirmwareIdentity;
use super::transcript::RuntimeTranscriptIdentity;
use super::PicoResult;

pub fn run(
    line: &mut NativePathCdcLine,
    sign: &mut NativePathCdcLineReader,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
    interactive: bool,
    membership_receipt_path: Option<&std::path::Path>,
) -> PicoResult<()> {
    let membership = membership_receipt_path
        .map(|path| verify_membership_receipt(path, runtime))
        .transpose()?;
    let lifecycle = super::prove_websocket::verify_new_plan_recovery(
        line,
        sign,
        identity,
        runtime,
        interactive,
    )?;
    let body_id = lifecycle.body.body_id.clone();
    println!("==> Restore real Wi-Fi/network availability for Plan C, then press Enter");
    let mut confirmation = String::new();
    std::io::stdin().read_line(&mut confirmation)?;
    let final_lifecycle = super::prove_websocket::verify_plan_c_continuation(
        line,
        sign,
        identity,
        runtime,
        interactive,
        Some(lifecycle),
    )?;
    if final_lifecycle.body.body_id != body_id {
        return Err("combined R1 proof changed Body identity between branches".into());
    }
    if membership
        .as_ref()
        .is_some_and(|receipt| receipt.body_id != body_id.as_str())
    {
        return Err("browser membership receipt and physical Play changed Body identity".into());
    }
    println!(
        "{}",
        serde_json::json!({
            "schema": "conduit.r1/complete-hil@1",
            "proof_class": "physical-cross-host",
            "body_id": body_id.as_str(),
            "pico_host_id": conduit_net::R1_PICO_HOST_ID,
            "pico_boot_id": runtime.boot_id.as_str(),
            "new_plan_recovery_completed": true,
            "same_plan_continuation_completed": true,
            "body_lulled_after_each_branch": true,
            "combined_physical_acceptance": true,
            "same_membership_body": membership.is_some(),
            "membership_receipt": membership.as_ref(),
        })
    );
    println!("==> Combined physical R1 new-Plan and same-Plan recovery completed");
    Ok(())
}

#[derive(serde::Serialize)]
struct VerifiedMembershipReceipt {
    schema: &'static str,
    body_id: String,
    pico_boot_id: String,
    active_plan_id: String,
    local_std_parts: usize,
    browser_parts: usize,
    pico_parts: usize,
    exact_identity_match: bool,
}

fn verify_membership_receipt(
    path: &std::path::Path,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<VerifiedMembershipReceipt> {
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let field = |name: &str| {
        value
            .get(name)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("membership receipt missing string field {name}"))
    };
    if field("schema")? != "conduit.body/mixed-membership-capstone@1"
        || value.get("physical_pico_admitted") != Some(&serde_json::Value::Bool(true))
        || value.get("active_plan_unchanged_by_join") != Some(&serde_json::Value::Bool(true))
        || value.get("replacement_plan_distinct") != Some(&serde_json::Value::Bool(true))
    {
        return Err("membership receipt lacks the required physical/immutable/replan proof".into());
    }
    let parts = value
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .ok_or("membership receipt has no Parts array")?;
    let browser_parts = parts
        .iter()
        .filter(|part| {
            part.get("host_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|host| host.starts_with("browser/"))
        })
        .count();
    let pico_parts = parts
        .iter()
        .filter(|part| {
            part.get("host_id").and_then(serde_json::Value::as_str)
                == Some(conduit_net::R1_PICO_HOST_ID)
        })
        .count();
    let local_std_parts = parts
        .iter()
        .filter(|part| {
            part.get("host_id").and_then(serde_json::Value::as_str)
                == Some(conduit_net::R1_STD_HOST_ID)
        })
        .count();
    let pico_boot_id = parts
        .iter()
        .find(|part| {
            part.get("host_id").and_then(serde_json::Value::as_str)
                == Some(conduit_net::R1_PICO_HOST_ID)
        })
        .and_then(|part| part.get("boot_id"))
        .and_then(serde_json::Value::as_str)
        .ok_or("membership receipt has no physical Pico Boot")?;
    // Production R1 seals the Plan against its planned Pico slot, then binds the
    // authenticated physical Boot at session start without mutating that Plan.
    let exact_plan = conduit_system_continuity::exact_r1_control_plan(
        conduit_core::BootId::from(conduit_net::R1_PICO_BOOT_ID),
        conduit_system_continuity::R1SignalRouteSet::WebSocketOnly,
    )?;
    let body_id = field("body_id")?;
    let active_plan_id = field("active_plan_id")?;
    let expected_body = conduit_body::Body::born(
        exact_plan.plan.source_document_id.clone(),
        exact_plan.plan.checked_form_id.clone(),
        1,
        conduit_core::SignId::from("r1/physical/body-born"),
    )
    .map_err(|error| format!("derive expected physical Body: {error:?}"))?;
    if local_std_parts != 1
        || browser_parts < 2
        || pico_parts != 1
        || body_id != expected_body.body_id.as_str()
        || pico_boot_id != runtime.boot_id
        || active_plan_id != exact_plan.plan.plan_id.as_str()
    {
        return Err(
            "membership and physical Play receipts do not share exact std/Pico/Boot/Plan truth"
                .into(),
        );
    }
    Ok(VerifiedMembershipReceipt {
        schema: "conduit.body/verified-membership-link@1",
        body_id: body_id.into(),
        pico_boot_id: pico_boot_id.into(),
        active_plan_id: active_plan_id.into(),
        local_std_parts,
        browser_parts,
        pico_parts,
        exact_identity_match: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(runtime_boot: &str) -> serde_json::Value {
        let plan = conduit_system_continuity::exact_r1_control_plan(
            conduit_core::BootId::from(conduit_net::R1_PICO_BOOT_ID),
            conduit_system_continuity::R1SignalRouteSet::WebSocketOnly,
        )
        .unwrap()
        .plan;
        let body = conduit_body::Body::born(
            plan.source_document_id.clone(),
            plan.checked_form_id.clone(),
            1,
            conduit_core::SignId::from("r1/physical/body-born"),
        )
        .unwrap();
        serde_json::json!({
            "schema": "conduit.body/mixed-membership-capstone@1",
            "body_id": body.body_id.as_str(),
            "active_plan_id": plan.plan_id.as_str(),
            "physical_pico_admitted": true,
            "active_plan_unchanged_by_join": true,
            "replacement_plan_distinct": true,
            "parts": [
                { "host_id": conduit_net::R1_STD_HOST_ID, "boot_id": conduit_net::R1_STD_BOOT_ID },
                { "host_id": conduit_net::R1_PICO_HOST_ID, "boot_id": runtime_boot },
                { "host_id": "browser/a", "boot_id": "browser-boot/a" },
                { "host_id": "browser/b", "boot_id": "browser-boot/b" },
            ],
        })
    }

    fn verify(
        value: &serde_json::Value,
        runtime_boot: &str,
    ) -> PicoResult<VerifiedMembershipReceipt> {
        let path = std::env::temp_dir().join(format!(
            "conduit-membership-link-{}-{}.json",
            std::process::id(),
            value["body_id"].as_str().unwrap_or("invalid")
        ));
        std::fs::write(&path, serde_json::to_vec(value)?)?;
        let result = verify_membership_receipt(
            &path,
            &RuntimeTranscriptIdentity {
                boot_id: runtime_boot.into(),
                active_play_id: "runtime-play/test".into(),
            },
        );
        let _ = std::fs::remove_file(path);
        result
    }

    #[test]
    fn membership_link_requires_exact_body_parts_boot_and_plan() {
        let runtime_boot = "runtime-boot/test";
        let valid = receipt(runtime_boot);
        let linked = verify(&valid, runtime_boot).unwrap();
        assert!(linked.exact_identity_match);
        assert_eq!(
            (
                linked.local_std_parts,
                linked.browser_parts,
                linked.pico_parts
            ),
            (1, 2, 1)
        );

        for mutate in ["body", "boot", "plan", "flag"] {
            let mut invalid = valid.clone();
            match mutate {
                "body" => invalid["body_id"] = serde_json::Value::String("wrong-body".into()),
                "boot" => {
                    invalid["parts"][1]["boot_id"] = serde_json::Value::String("stale-boot".into())
                }
                "plan" => {
                    invalid["active_plan_id"] = serde_json::Value::String("wrong-plan".into())
                }
                "flag" => invalid["active_plan_unchanged_by_join"] = serde_json::Value::Bool(false),
                _ => unreachable!(),
            }
            assert!(
                verify(&invalid, runtime_boot).is_err(),
                "{mutate} mismatch passed"
            );
        }
    }
}
