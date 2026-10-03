//! Bounded serial receipt for one owner-validated native Mask submission.
//! This observes the typed action; the owner remains the only state authority.

use conduit_presentation::{FaceInteraction, MaskShow, UTF8_TEXT_VALUE_KIND};

use crate::{arch, native_owner_return::NativeReturnOutcome};

pub(super) fn emit(show: &MaskShow, interaction: &FaceInteraction, outcome: &NativeReturnOutcome) {
    let requested_interval_ms = requested_clock_interval(interaction);
    let evidence = serde_json::json!({
        "schema":"conduit.conduitos/native-owner-action@1",
        "status":if outcome.accepted { "accepted" } else if outcome.code == "control-outcome-unknown" { "unknown" } else { "refused" },
        "code":&outcome.code,
        "prior_show_id":show.show_id.as_str(),
        "interaction_id":interaction.identity.as_str(),
        "face_id":&interaction.face_id,
        "face_revision":interaction.face_revision,
        "action_id":&interaction.action_id,
        "requested_interval_ms":requested_interval_ms,
        "face_refreshed":outcome.face.is_some(),
    });
    if let Ok(bytes) = serde_json::to_vec(&evidence)
        && bytes.len() <= 1_024
    {
        arch::early_write(b"CONDUIT_NATIVE_OWNER_ACTION ");
        arch::early_write(&bytes);
        arch::early_write(b"\n");
    }
}

fn requested_clock_interval(interaction: &FaceInteraction) -> Option<u64> {
    if !interaction
        .action_id
        .starts_with("body/action/change-clock-interval/")
    {
        return None;
    }
    let [argument] = interaction.arguments.as_slice() else {
        return None;
    };
    if argument.name != "clock/interval-ms" || argument.value_kind != UTF8_TEXT_VALUE_KIND {
        return None;
    }
    let value = core::str::from_utf8(&argument.value).ok()?.parse().ok()?;
    [250, 500, 1000, 2000].contains(&value).then_some(value)
}
