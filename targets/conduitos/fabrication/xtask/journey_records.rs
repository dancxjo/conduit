//! Complete guest records; a partial serial tail is not a checkpoint.
use super::ConduitosError;
use serde_json::Value;
const PREFIX: &str = "CONDUIT_PRODUCT_JOURNEY ";
const TOUR_PREFIX: &str = "CONDUIT_TOUR_SIGN ";
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

pub(super) fn tour(serial: &str) -> Result<Vec<Value>, ConduitosError> {
    decode_prefix(serial, TOUR_PREFIX, "conduitos-tour-sign-invalid")
}

pub(super) fn latest_checkpoint(serial: &str) -> Result<Option<Value>, ConduitosError> {
    serial
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .filter_map(|line| {
            line.strip_prefix(PREFIX)
                .map(|json| (json, "product-journey-sign-invalid"))
                .or_else(|| {
                    line.strip_prefix(TOUR_PREFIX)
                        .map(|json| (json, "conduitos-tour-sign-invalid"))
                })
        })
        .map(|(json, reason)| {
            serde_json::from_str(json)
                .map_err(|error| ConduitosError::refusal(reason, error.to_string()))
        })
        .next_back()
        .transpose()
}

fn decode_prefix(
    serial: &str,
    prefix: &str,
    reason: &'static str,
) -> Result<Vec<Value>, ConduitosError> {
    serial
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|json| {
            serde_json::from_str(json)
                .map_err(|error| ConduitosError::refusal(reason, error.to_string()))
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

    #[test]
    fn latest_checkpoint_distinguishes_complete_tour_records() {
        let serial = concat!(
            "CONDUIT_PRODUCT_JOURNEY {\"status\":\"planned\"}\n",
            "CONDUIT_TOUR_SIGN {\"status\":\"result-visible\"}\n",
            "CONDUIT_TOUR_SIGN {\"status\":\"partial"
        );
        assert_eq!(tour(serial).unwrap().len(), 1);
        assert_eq!(
            latest_checkpoint(serial).unwrap().unwrap()["status"],
            "result-visible"
        );
    }
}
