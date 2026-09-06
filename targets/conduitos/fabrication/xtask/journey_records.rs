//! Complete guest records; a partial serial tail is not a checkpoint.
use super::ConduitosError;
use serde_json::Value;
const PREFIX: &str = "CONDUIT_PRODUCT_JOURNEY ";
pub(super) fn decode(serial: &str) -> Result<Vec<Value>, ConduitosError> {
    serial
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .filter_map(|line| line.strip_prefix(PREFIX))
        .map(|json| {
            serde_json::from_str(json).map_err(|error| {
                ConduitosError::refusal("product-journey-sign-invalid", error.to_string())
            })
        })
        .collect()
}

pub(super) fn boot(serial: &str) -> Result<Option<Value>, ConduitosError> {
    serial
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .filter_map(|line| line.strip_prefix("CONDUIT_BOOT_SIGN "))
        .next_back()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|error| {
            ConduitosError::refusal("product-journey-boot-sign-invalid", error.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_serial_tail_cannot_satisfy_a_checkpoint() {
        let complete = "CONDUIT_PRODUCT_JOURNEY {\"status\":\"awake\"}\n";
        let partial = "CONDUIT_PRODUCT_JOURNEY {\"status\":\"playing\",\"plan_id\":\"";
        let records = decode(&format!("{complete}{partial}")).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["status"], "awake");
        assert!(decode(&format!("{partial}\n")).is_err());
    }
}
