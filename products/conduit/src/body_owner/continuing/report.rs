//! Keep selected IMAGE backing alive through worker handoff and receipt publication.
use conduit_std_host::body_execution::{BodyRunReport, ImageBodyRunReport};

pub(crate) enum RunReport {
    Reference(BodyRunReport),
    Image(ImageBodyRunReport),
}
impl RunReport {
    pub(super) fn report(&self) -> &BodyRunReport {
        match self {
            Self::Reference(report) => report,
            Self::Image(report) => report.report(),
        }
    }
}
