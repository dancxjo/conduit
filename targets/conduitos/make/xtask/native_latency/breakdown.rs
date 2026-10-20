//! Counter breakdowns derived from the same guest's identity-bearing receipts.
use super::{refusal, ConduitosError};
use serde_json::{json, Value};

fn records(serial: &str, prefix: &str) -> Result<Vec<Value>, ConduitosError> {
    serial
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|line| {
            serde_json::from_str(line).map_err(|_| refusal("invalid timing custody record"))
        })
        .collect()
}

pub(super) fn derive(
    serial: &str,
    guest: &Value,
    host: &str,
    boot: &str,
) -> Result<Value, ConduitosError> {
    let intervals = guest["records"]
        .as_array()
        .ok_or_else(|| refusal("missing counter family"))?;
    let point = |stage| {
        intervals
            .iter()
            .find(|r| r["stage"] == stage)
            .and_then(|r| r["end_ticks"].as_u64())
            .ok_or_else(|| refusal("missing native startup point"))
    };
    let entry = point("boot-entry")?;
    let paint = point("first-framebuffer-commit")?;
    if paint < entry {
        return Err(refusal("startup counter reversed"));
    }
    let faces = records(serial, "CONDUIT_NATIVE_FACE ")?;
    let focus = records(serial, "CONDUIT_CRECHE_FOCUS ")?
        .into_iter()
        .find(|r| {
            r["native_observer"]["clock"] == "x86-tsc-lfence"
                && r["native_observer"]["ticks"]
                    .as_u64()
                    .is_some_and(|ticks| ticks > paint)
        })
        .ok_or_else(|| refusal("missing actual post-paint usable input acknowledgement"))?;
    let owner = faces
        .iter()
        .find(|face| {
            face["show"]["show_id"] == focus["show_id"]
                && face["show"]["face_id"] == focus["face_id"]
                && face["show"]["face_revision"] == focus["face_revision"]
        })
        .ok_or_else(|| refusal("usable input lacks its exact acknowledged Face/Show"))?;
    if owner["show"]["host_id"] != host || owner["show"]["boot_id"] != boot {
        return Err(refusal("usable input belongs to another Host/Boot"));
    }
    let input = focus["native_observer"]["ticks"].as_u64().unwrap();
    let mut stages = serde_json::Map::new();
    for stage in [
        "plot-build",
        "plan-seal",
        "workset-prepare",
        "kernel-prepare",
        "play-start",
        "play-input",
        "pointer-feedback",
    ] {
        let rows = intervals.iter().filter(|r| r["stage"] == stage).map(|r| {
            json!({"sequence":r["sequence"],"start_ticks":r["start_ticks"],"end_ticks":r["end_ticks"],
                "elapsed_ticks":r["end_ticks"].as_u64().unwrap() - r["start_ticks"].as_u64().unwrap()})
        }).collect::<Vec<_>>();
        if rows.is_empty() {
            return Err(refusal("missing actual measured execution stage"));
        }
        stages.insert(stage.into(), rows.into());
    }
    let presentations = records(serial, "CONDUIT_NATIVE_JOURNEY_TIMING ")?;
    if presentations.is_empty() {
        return Err(refusal("missing drawn journey timing custody"));
    }
    for record in &presentations {
        if record["host_id"] != host
            || record["boot_id"] != boot
            || record["native_observer"]["clock"] != "x86-tsc-lfence"
            || record["native_observer"]["ticks"].as_u64().is_none()
        {
            return Err(refusal(
                "presentation timing crossed identity or clock families",
            ));
        }
    }
    Ok(
        json!({"clock":"x86-tsc-lfence","units":"ticks","frequency_hz":null,
            "boot_entry_ticks":entry,"first_framebuffer_commit_ticks":paint,
            "boot_to_first_commit_ticks":paint-entry,"first_usable_input_ack_ticks":input,
            "boot_to_first_usable_input_ack_ticks":input-entry,
            "first_usable_input_custody":focus,"stages":stages,
            "drawn_journey_receipts":presentations,
            "reading_receipts":records(serial,"CONDUIT_NATIVE_READING ")?,
            "interpretation":"First usable input is an actual acknowledged current-Show focus input after initial framebuffer commit. Its elapsed Boot interval includes harness arrival/capture and input dispatch; it is not time-to-input-readiness. Stage intervals may overlap and include nested observation publication; do not sum them or infer a counter frequency. Host typing and page-navigation intervals are retained separately."
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn family() -> (String, Value) {
        let stages = [
            "boot-entry",
            "first-framebuffer-commit",
            "plot-build",
            "plan-seal",
            "workset-prepare",
            "kernel-prepare",
            "play-start",
            "play-input",
            "pointer-feedback",
        ];
        let guest = json!({"records":stages.iter().enumerate().map(|(sequence,stage)|
            json!({"sequence":sequence,"stage":stage,"start_ticks":sequence as u64*10,
                "end_ticks":sequence as u64*10})).collect::<Vec<_>>()});
        let face = json!({"show":{"show_id":"show","face_id":"face","face_revision":2,
            "host_id":"host","boot_id":"boot"}});
        let focus = json!({"show_id":"show","face_id":"face","face_revision":2,
            "native_observer":{"clock":"x86-tsc-lfence","ticks":15}});
        let drawn = json!({"host_id":"host","boot_id":"boot",
            "native_observer":{"clock":"x86-tsc-lfence","ticks":90}});
        (
            format!("CONDUIT_NATIVE_FACE {face}\nCONDUIT_CRECHE_FOCUS {focus}\nCONDUIT_NATIVE_JOURNEY_TIMING {drawn}\n"),
            guest,
        )
    }
    #[test]
    fn startup_acknowledgement_keeps_unknown_frequency_and_exact_show_custody() {
        let (serial, guest) = family();
        let result = derive(&serial, &guest, "host", "boot").unwrap();
        assert_eq!(result["boot_to_first_commit_ticks"], 10);
        assert_eq!(result["boot_to_first_usable_input_ack_ticks"], 15);
        assert!(result["frequency_hz"].is_null());
        assert_eq!(result["first_usable_input_custody"]["show_id"], "show");
    }
    #[test]
    fn wrong_boot_stale_show_and_prepaint_focus_cannot_stand_in_for_usable_input() {
        let (serial, guest) = family();
        assert!(derive(&serial, &guest, "host", "other-boot").is_err());
        assert!(derive(
            &serial.replace(
                "\"face_revision\":2,\"native_observer\"",
                "\"face_revision\":3,\"native_observer\""
            ),
            &guest,
            "host",
            "boot"
        )
        .is_err());
        assert!(derive(
            &serial.replace("\"ticks\":15", "\"ticks\":5"),
            &guest,
            "host",
            "boot"
        )
        .is_err());
    }
}
