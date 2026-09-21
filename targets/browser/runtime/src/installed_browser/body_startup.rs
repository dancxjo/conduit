//! A finite source initialized from exact retained body startup evidence.
use super::{
    factory::{validate_placement, BrowserInstallation},
    BrowserOperation,
};
use conduit_body::{BodyStartup, StartupScope};
use conduit_core::{CapabilityOffer, InfoBool, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    HostedValueStore, PortId, ValueRef, ValueStorage,
};

const WAKE_IMPLEMENTATION: &str = "browser/body-wake@1";
const FIRST_IMPLEMENTATION: &str = "browser/body-first-wake@1";
pub(super) static WAKE: BrowserInstallation = BrowserInstallation {
    implementation_id: WAKE_IMPLEMENTATION,
    offer: wake_offer,
    prepare: without_body,
    perform: None,
};
pub(super) static FIRST_WAKE: BrowserInstallation = BrowserInstallation {
    implementation_id: FIRST_IMPLEMENTATION,
    offer: first_offer,
    prepare: without_body,
    perform: None,
};
fn wake_offer() -> CapabilityOffer {
    offer(false)
}
fn first_offer() -> CapabilityOffer {
    offer(true)
}
fn offer(first: bool) -> CapabilityOffer {
    let implementation = if first {
        FIRST_IMPLEMENTATION
    } else {
        WAKE_IMPLEMENTATION
    };
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::body_wake_contract(first),
        if first {
            conduit_semantic_catalog::BODY_FIRST_WAKE_REVISION
        } else {
            conduit_semantic_catalog::BODY_WAKE_REVISION
        },
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: implementation,
            execution_profile: implementation,
            implementation,
            artifact: "conduit-browser-runtime/body-startup@1",
        },
        vec![],
        vec![],
        vec![],
    )
}
fn without_body(_: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    Err("Body startup sources require an admitted body lifecycle".into())
}

pub(crate) fn prepare_for_body(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
    startup: Option<&BodyStartup>,
) -> Result<Option<BrowserOperation>, String> {
    let first = match placement.implementation_id.as_str() {
        WAKE_IMPLEMENTATION => false,
        FIRST_IMPLEMENTATION => true,
        _ => return Ok(None),
    };
    validate_placement(placement, &offer(first))?;
    let startup = startup.ok_or("Body startup source has no admitted lifecycle evidence")?;
    let value = if startup.eligible(if first {
        StartupScope::Body
    } else {
        StartupScope::Wake
    }) {
        Some(
            values
                .store(&InfoBool::new(true).encode())
                .map_err(|error| format!("startup pulse storage: {error:?}"))?,
        )
    } else {
        None
    };
    Ok(Some(BrowserOperation::installed_step(StartupSource {
        value,
    })))
}

struct StartupSource {
    value: Option<ValueRef>,
}
impl<const PORTS: usize> StepBack<PORTS> for StartupSource {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let Some(value) = self.value else {
            return StepOutcome::Complete;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), value)
            .expect("ready body-startup output");
        self.value = None;
        StepOutcome::Complete
    }
    fn cancel(&mut self) {
        self.value = None;
    }
}
