//! R1 Body identity shared by the physical membership and production-Play proofs.

use conduit_body::Body;
use conduit_core::{BootId, HostId, SignId};
use conduit_r1_network_conformance::{ExactR1ControlPlan, R1SignalRouteSet};

use super::physical_pico::{self, PendingPhysicalPico};

pub(super) struct PhysicalBody {
    pending: Option<PendingPhysicalPico>,
    exact: Option<ExactR1ControlPlan>,
}

impl PhysicalBody {
    pub(super) fn prepare() -> Result<Self, String> {
        let pending = std::env::var("CONDUIT_B9_PICO_LINK_PORT")
            .ok()
            .map(|path| physical_pico::observe(&path))
            .transpose()?;
        // The reviewed R1 Plan is sealed against the planned Pico slot. Physical
        // attachment binds that slot to the freshly authenticated runtime Boot
        // without changing Plan identity; keep the actual Boot on the Part row.
        let exact = pending
            .as_ref()
            .map(|_| {
                conduit_r1_network_conformance::exact_r1_control_plan(
                    BootId::from(conduit_r1_network_conformance::R1_PICO_BOOT_ID),
                    R1SignalRouteSet::WebSocketOnly,
                )
            })
            .transpose()?;
        Ok(Self { pending, exact })
    }

    pub(super) fn birth(
        &self,
        browser_basis: Option<(conduit_core::SourceDocumentId, conduit_core::CheckedFormId)>,
    ) -> Result<Body, String> {
        let (source, checked, sign) = match &self.exact {
            Some(exact) => (
                exact.plan.source_document_id.clone(),
                exact.plan.checked_form_id.clone(),
                SignId::from("r1/physical/body-born"),
            ),
            None => {
                let (source, checked) = browser_basis
                    .ok_or("canonical browser Form basis is required without a physical Plan")?;
                (
                    source,
                    checked,
                    SignId::from("browser-parts-capstone/body-born"),
                )
            }
        };
        Body::born(source, checked, 1, sign).map_err(|error| format!("Body birth: {error:?}"))
    }

    pub(super) fn here_identity(&self) -> (HostId, BootId) {
        self.exact.as_ref().map_or_else(
            || {
                (
                    HostId::from("body/std-here"),
                    BootId::from("body/std-here-boot"),
                )
            },
            |exact| {
                (
                    exact.source_advertisement.host_id.clone(),
                    exact.source_advertisement.boot_id.clone(),
                )
            },
        )
    }

    pub(super) const fn plan(&self) -> Option<&ExactR1ControlPlan> {
        self.exact.as_ref()
    }

    pub(super) fn take_pending(&mut self) -> Option<PendingPhysicalPico> {
        self.pending.take()
    }
}
