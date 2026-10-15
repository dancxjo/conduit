//! Exact Play-lived State installation over the shared kernel State Back.
use super::{
    factory::{validate_placement, BrowserInstallation},
    BrowserBack,
};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    state_delay::{back::StateBack, StateDelay},
    Failure, FailureCode, PortId,
};
const MAXIMUM: usize = super::MAXIMUM_BROWSER_VALUE_BYTES;
pub(crate) const IMPLEMENTATION: &str = "browser/kernel-typed-state@1";
const PROFILE: &str = "browser/typed-state-play-4096@1";
const ARTIFACT: &str = "conduit-kernel/state-delay@1";
pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer: unreachable_offer,
    prepare,
    perform: None,
};
fn initial(configuration: &[ConfigurationEntry]) -> Result<StructuredInfoValue, String> {
    let Some(ConfigurationValue::Structured(value)) = configuration
        .iter()
        .find(|entry| entry.key == "initial")
        .map(|entry| &entry.value)
    else {
        return Err("typed State requires exact initialization".into());
    };
    StructuredInfoValue::from_canonical_bytes(value.canonical_value())
        .map_err(|error| format!("typed State initialization: {error:?}"))
}
fn offer(value: &StructuredInfoValue) -> Result<CapabilityOffer, String> {
    let mut contract = conduit_semantic_catalog::state_value::state_value_semantic_contract(
        "",
        value.value_type(),
        value,
    )?;
    contract.limits.max_queue_bytes = MAXIMUM as u32;
    contract.shorthand = Some((port_id("next"), port_id("current")));
    let digest = value
        .semantic_digest()
        .map_err(|error| format!("typed State identity: {error:?}"))?;
    let identity = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: format!("browser/typed-state/{identity}").into(),
            execution_profile_id: PROFILE.into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: ARTIFACT.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
    .with_state_retention(StateRetentionSupport {
        maximum_lifetime: StateLifetime::Play,
    })
    .map_err(|error| format!("typed State retention: {error:?}"))
}
pub(crate) fn offer_for_expanded(
    gear: &conduit_plot::CheckedGear,
) -> Result<Option<CapabilityOffer>, String> {
    if gear.kind_id.as_str() != STATE_VALUE_KIND {
        return Ok(None);
    }
    offer(&initial(&gear.configuration)?).map(Some)
}
enum Validator {
    Primitive(KindId),
    Structured(PreparedStructuredValueValidator),
}
struct TypedState {
    back: StateBack<MAXIMUM>,
    validator: Validator,
}
impl Validator {
    fn validate(&self, value: &[u8]) -> bool {
        match self {
            Self::Primitive(kind) => validate_primitive_info(kind.as_str(), value).is_ok(),
            Self::Structured(validator) => validator.validate(value).is_ok(),
        }
    }
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    let value = initial(&placement.configuration)?;
    let exact = offer(&value)?;
    validate_placement(placement, &exact)?;
    if placement.semantic_contract != exact.semantic_contract
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("typed State differs from the installed Play contract".into());
    }
    let duration = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "retained-duration")
        .map(|entry| &entry.value);
    if duration.is_some_and(
        |duration| !matches!(duration, ConfigurationValue::Text(value) if value == "play"),
    ) {
        return Err("browser typed State supports at most this Play".into());
    }
    let maximum = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "maximum-bytes")
        .map(|entry| &entry.value);
    let maximum = match maximum {
        Some(ConfigurationValue::U64(value)) => {
            usize::try_from(*value).map_err(|_| "typed State bound overflows")?
        }
        None => MAXIMUM,
        _ => return Err("typed State bound is malformed".into()),
    };
    if maximum == 0 || maximum > MAXIMUM {
        return Err("typed State exceeds installed storage".into());
    }
    let (bytes, validator) = match (value.value_type().shape(), value.shape()) {
        (StructuredInfoTypeShape::Leaf(kind), StructuredInfoValueShape::Leaf(bytes)) => {
            (bytes.to_vec(), Validator::Primitive(kind.clone()))
        }
        _ => (
            value
                .canonical_bytes()
                .map_err(|error| format!("typed State bytes: {error:?}"))?,
            Validator::Structured(
                PreparedStructuredValueValidator::new(value.value_type(), maximum)
                    .map_err(|error| format!("typed State validator: {error:?}"))?,
            ),
        ),
    };
    if !validator.validate(&bytes) {
        return Err("typed State initial value is invalid".into());
    }
    // The slot is local to this independently installed State Back; planned Gear
    // identity remains owned by the ordinary scheduler and its State boundary.
    let state = StateDelay::externally_continued(0, maximum, &bytes)
        .map_err(|error| format!("typed State storage: {error:?}"))?;
    let back = StateBack::new_prepared(state, PortId(0), PortId(0))
        .map_err(|error| format!("typed State Back: {error:?}"))?;
    Ok(BrowserBack::installed_step(TypedState { back, validator }))
}
impl<const PORTS: usize> StepBack<PORTS> for TypedState {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if io.input(PortId(0)).is_some()
            && !bytes
                .input(PortId(0))
                .is_some_and(|value| self.validator.validate(value))
        {
            <StateBack<MAXIMUM> as StepBack<PORTS>>::cancel(&mut self.back);
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 9,
            });
        }
        self.back.step(io, bytes)
    }
    fn step_committed(&mut self) {
        <StateBack<MAXIMUM> as StepBack<PORTS>>::step_committed(&mut self.back);
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        <StateBack<MAXIMUM> as StepBack<PORTS>>::prepared_output(&self.back, port)
    }
    fn cancel(&mut self) {
        <StateBack<MAXIMUM> as StepBack<PORTS>>::cancel(&mut self.back);
    }
}
fn unreachable_offer() -> CapabilityOffer {
    panic!("typed State offers require checked source")
}
