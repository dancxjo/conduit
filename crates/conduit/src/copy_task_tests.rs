use crate::copy_task::result_message;
use conduit_std_host::CopyResult;

#[test]
fn every_copy_terminal_disposition_keeps_its_golden_product_message() {
    let cases = [
        (
            CopyResult::Success { bytes_copied: 7 },
            "Copied 7 bytes successfully.",
        ),
        (
            CopyResult::DestinationExists,
            "Not copied: destination already exists.",
        ),
        (CopyResult::Denied, "Not copied: access was denied."),
        (
            CopyResult::StaleHandle,
            "Not copied: a selected resource is stale.",
        ),
        (
            CopyResult::Oversized {
                source_bytes: 9,
                maximum_bytes: 8,
            },
            "Not copied: source is 9 bytes, above the 8-byte limit.",
        ),
        (
            CopyResult::Partial { bytes_copied: 3 },
            "Copy failed after 3 temporary bytes; destination was not committed.",
        ),
        (
            CopyResult::Cancelled { bytes_copied: 4 },
            "Stopped after 4 temporary bytes; destination was not committed.",
        ),
        (
            CopyResult::CleanupFailed { bytes_copied: 5 },
            "Copy stopped after 5 temporary bytes, but temporary cleanup failed.",
        ),
    ];
    for (result, expected) in cases {
        assert_eq!(result_message(&result), expected);
    }
}
