//! Exact validation of native Text Lab loss before portable projection.

use conduit_std_catalog::TextLabLineLossReceipt;
use conduit_std_catalog::{
    exact_text_lab_line_loss_outcome, exact_text_lab_split_plan, TEXT_LAB_RETURN_LINE,
};

pub(super) fn validate_loss(
    base: &str,
    plan: &conduit_core::Plan,
    receipt: &TextLabLineLossReceipt,
) -> Result<(), String> {
    let browser_upper = conduit_browser_runtime::presentation_nucleus::browser_text_upper_offer();
    let expected = exact_text_lab_line_loss_outcome(base, &browser_upper, TEXT_LAB_RETURN_LINE)?;
    let exact = exact_text_lab_split_plan(base, &browser_upper)?;
    let active = conduit_core::bind_active_play(
        &plan.plan_id,
        &exact.native.host_id,
        &exact.native.boot_id,
        0,
    );
    let sign = conduit_core::bind_sign(
        &exact.native.host_id,
        &exact.native.boot_id,
        Some(&active.active_play_id),
        receipt.sequence,
    );
    let valid = receipt.schema == "conduit.text-lab/line-loss@1"
        && receipt.code == "CND-TEXT-LIVE-301"
        && receipt.line_id == TEXT_LAB_RETURN_LINE
        && receipt.plan_id == plan.plan_id.as_str()
        && receipt.plan_id == expected.immutable_plan_id.as_str()
        && receipt.source_document_id == plan.source_document_id.as_str()
        && receipt.source_document_id == expected.source_document_id.as_str()
        && receipt.checked_form_id == plan.checked_form_id.as_str()
        && receipt.checked_form_id == expected.checked_form_id.as_str()
        && receipt.old_plan_disposition == "immutable"
        && receipt.fresh_planning == "unrealizable"
        && receipt.form_unchanged
        && receipt.refusal == expected.refusal
        && receipt.active_play_id == active.active_play_id.as_str()
        && receipt.sign_id == sign.sign_id.as_str()
        && matches!(
            receipt.phase.as_str(),
            "return-offer" | "return-accepted" | "return-delivered"
        )
        && !receipt.transport_failure.is_empty();
    if !valid {
        return Err("Text Lab loss receipt does not match exact current planning truth".into());
    }
    Ok(())
}
