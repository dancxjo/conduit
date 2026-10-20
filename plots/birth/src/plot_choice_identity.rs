//! Private stable identity for one exact Source/check pair in the Birth inventory.
use alloc::{format, string::String};
use conduit_body::{MAX_BODY_PLOT_IDENTITY_BYTES, ResidentPlot};
use sha2::{Digest, Sha256};

pub(crate) fn plot_choice_identity(plot: &ResidentPlot) -> String {
    // BirthDraft admits every pair through BodyWorkset before projection.
    // Its 2048-byte combined identity bound makes all framed lengths fit u32.
    const _: () = assert!(MAX_BODY_PLOT_IDENTITY_BYTES <= u32::MAX as usize);
    let mut digest = Sha256::new();
    for value in [
        "conduit.birth/plot-choice@1",
        plot.source_document_id.as_str(),
        plot.checked_plot_id.as_str(),
    ] {
        let length = u32::try_from(value.len()).expect("admitted bounded Birth identity");
        digest.update(length.to_le_bytes());
        digest.update(value.as_bytes());
    }
    format!("{:x}", digest.finalize())
}
