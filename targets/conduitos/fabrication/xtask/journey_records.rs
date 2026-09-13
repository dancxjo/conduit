//! Complete guest records; a partial serial tail is not a checkpoint.
use super::ConduitosError;
use serde_json::Value;
const PREFIX: &str = "CONDUIT_PRODUCT_JOURNEY ";
const TOUR_PREFIX: &str = "CONDUIT_TOUR_SIGN ";
const POINTER_PREFIX: &str = "CONDUIT_POINTER_SIGN ";
const TRANSIENT_PREFIX: &str = "CONDUIT_TRANSIENT_SIGN ";
const RESIZE_PREFIX: &str = "CONDUIT_RESIZE_SIGN ";
const USB_LINE_PREFIX: &str = "CONDUIT_USB_LINE_SIGN ";
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

pub(super) fn pointer(serial: &str) -> Result<Vec<Value>, ConduitosError> {
    decode_prefix(serial, POINTER_PREFIX, "conduitos-pointer-sign-invalid")
}

pub(super) fn transient(serial: &str) -> Result<Vec<Value>, ConduitosError> {
    decode_prefix(serial, TRANSIENT_PREFIX, "conduitos-transient-sign-invalid")
}

pub(super) fn resize(serial: &str) -> Result<Vec<Value>, ConduitosError> {
    decode_prefix(serial, RESIZE_PREFIX, "conduitos-resize-sign-invalid")
}

pub(super) fn usb_line(serial: &str) -> Result<Vec<Value>, ConduitosError> {
    decode_prefix(serial, USB_LINE_PREFIX, "conduitos-usb-line-sign-invalid")
}

pub(super) fn latest_checkpoint(serial: &str) -> Result<Option<Value>, ConduitosError> {
    serial
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .filter_map(|line| {
            line.strip_prefix(PREFIX)
                .map(|json| (json, "product-journey-sign-invalid"))
                .or_else(|| {
                    line.strip_prefix(RESIZE_PREFIX)
                        .map(|json| (json, "conduitos-resize-sign-invalid"))
                })
                .or_else(|| {
                    line.strip_prefix(TRANSIENT_PREFIX)
                        .map(|json| (json, "conduitos-transient-sign-invalid"))
                })
                .or_else(|| {
                    line.strip_prefix(TOUR_PREFIX)
                        .map(|json| (json, "conduitos-tour-sign-invalid"))
                })
                .or_else(|| {
                    line.strip_prefix(POINTER_PREFIX)
                        .map(|json| (json, "conduitos-pointer-sign-invalid"))
                })
                .or_else(|| {
                    line.strip_prefix(USB_LINE_PREFIX)
                        .map(|json| (json, "conduitos-usb-line-sign-invalid"))
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
        let partial =
            "CONDUIT_PRODUCT_JOURNEY {\"status\":\"quiescent-awaiting-input\",\"plan_id\":\"";
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

    #[test]
    fn latest_checkpoint_includes_complete_pointer_records() {
        let serial = concat!(
            "CONDUIT_TOUR_SIGN {\"status\":\"patchbay-open\"}\n",
            "CONDUIT_POINTER_SIGN {\"status\":\"selected\",\"sequence\":2}\n",
        );
        assert_eq!(pointer(serial).unwrap().len(), 1);
        assert_eq!(
            latest_checkpoint(serial).unwrap().unwrap()["status"],
            "selected"
        );
    }

    #[test]
    fn latest_checkpoint_includes_complete_usb_line_records() {
        let serial = concat!(
            "CONDUIT_POINTER_SIGN {\"status\":\"selected\"}\n",
            "CONDUIT_USB_LINE_SIGN {\"status\":\"line-lost\",\"line_id\":\"line/1\"}\n",
        );
        assert_eq!(usb_line(serial).unwrap().len(), 1);
        assert_eq!(
            latest_checkpoint(serial).unwrap().unwrap()["status"],
            "line-lost"
        );
    }
}
