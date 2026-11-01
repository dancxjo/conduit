//! Membership custody recovery is distinct from a newly authorized live carrier.
use super::BrowserAdmittedSnapshot;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum BrowserAdmissionCompletion {
    Active {
        snapshot: Box<BrowserAdmittedSnapshot>,
    },
    CustodyOnly {
        receipt: Box<conduit_body::PortableMembershipAdmissionReceipt>,
    },
}
