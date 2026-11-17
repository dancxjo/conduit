//! Actual wall-clock sampling through the selected CPU Host Call, one kernel.
use super::{
    InstalledScheduler,
    back::{BackBudget, BackFactory, InstalledBack},
};
use conduit_core::*;
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallOutcome, PortId,
    scheduler::{HostCallBack, HostCallRequest, StepBack, StepInputBytes, StepIo, StepOutcome},
};
use conduit_plan_lowering::lowering::LoweredHostCall;
use conduit_std_offers::{
    AMBIENT_OBSERVE_CALL as CALL, AMBIENT_OBSERVE_IMPLEMENTATION as IMPLEMENTATION,
};
use std::cell::RefCell;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) static RESIDENT_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::AMBIENT_RESIDENT_OBSERVE_IMPLEMENTATION,
    budget,
    prepare,
};

struct ClockHome {
    clock: crate::ambient_clock::PreparedAmbientClockWire,
    home: PreparedStructuredComposer,
    wallpaper: Box<[u8]>,
    clock_first: bool,
}
impl ClockHome {
    fn prepare(zone: &str) -> Result<Self, String> {
        let types = conduit_semantic_catalog::prepare_ambient_display_types()?;
        let ty = |name| {
            types
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .map(|ty| &ty.value_type)
                .ok_or("missing ambient observation Type")
        };
        let wallpaper_type = ty("AmbientWallpaperObservation")?;
        let StructuredInfoTypeShape::Variant { cases, .. } = wallpaper_type.shape() else {
            return Err("wallpaper Type shape".into());
        };
        let absent = cases
            .iter()
            .find(|case| case.tag() == "unavailable")
            .ok_or("wallpaper unavailable Type")?;
        let reason = conduit_plot::rust_binding::primitive_into_structured(
            absent.payload_type().clone(),
            &"Wallpaper provider unavailable".to_owned(),
        )
        .map_err(|why| format!("wallpaper absence: {why:?}"))?;
        let wallpaper = StructuredInfoValue::variant(wallpaper_type.clone(), "unavailable", reason)
            .map_err(|why| format!("wallpaper absence: {why:?}"))?
            .canonical_bytes()
            .map_err(|why| format!("wallpaper encoding: {why:?}"))?
            .into_boxed_slice();
        let home_type = ty("AmbientHomeObservation")?;
        let StructuredInfoTypeShape::Record { fields, .. } = home_type.shape() else {
            return Err("Home Type shape".into());
        };
        let clock_first = match fields {
            [a, b] if a.name() == "clock_weather" && b.name() == "wallpaper" => true,
            [a, b] if a.name() == "wallpaper" && b.name() == "clock_weather" => false,
            _ => return Err("Home field identity differs".into()),
        };
        Ok(Self {
            clock: crate::ambient_clock::PreparedAmbientClockWire::prepare(
                zone,
                "Weather provider unavailable",
            )
            .map_err(|why| format!("clock preparation: {why:?}"))?,
            home: PreparedStructuredComposer::new(home_type, 8192)
                .map_err(|why| format!("Home encoder: {why:?}"))?,
            wallpaper,
            clock_first,
        })
    }
    fn heap_bytes(&self) -> usize {
        self.clock.prepared_heap_bytes() + self.home.prepared_heap_bytes() + self.wallpaper.len()
    }
    fn sample(&mut self, input: &[u8]) -> Result<&[u8], conduit_kernel::Failure> {
        conduit_time::decode_tick(input).map_err(|_| conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::InvalidInput,
            detail: 82,
        })?;
        let clock = self.clock.sample().map_err(|_| conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::HostCallFailed,
            detail: 83,
        })?;
        let clock =
            validate_canonical_structured_value(clock).map_err(|_| conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidLifecycle,
                detail: 84,
            })?;
        let wallpaper = validate_canonical_structured_value(&self.wallpaper).map_err(|_| {
            conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidLifecycle,
                detail: 85,
            }
        })?;
        let values = if self.clock_first {
            [clock, wallpaper]
        } else {
            [wallpaper, clock]
        };
        self.home
            .record(&values)
            .map_err(|_| conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidLifecycle,
                detail: 86,
            })
    }
}

pub(super) struct AmbientObserveBack {
    kernel: HostCallBack,
    provider: RefCell<ClockHome>,
    maximum_updates: Option<u32>,
    updates: u32,
    candidate: bool,
}
impl AmbientObserveBack {
    pub(super) fn retained_backing_bytes(&self) -> usize {
        core::mem::size_of::<Self>() + self.provider.borrow().heap_bytes()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for Box<AmbientObserveBack> {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.candidate = false;
        let requesting = io.host_completion().is_none() && io.input(PortId(0)).is_some();
        if requesting
            && (self
                .maximum_updates
                .is_some_and(|maximum| self.updates >= maximum)
                || self.updates == u32::MAX)
        {
            return StepOutcome::Fail(conduit_kernel::Failure {
                code: if self.maximum_updates.is_none() {
                    conduit_kernel::FailureCode::IdentityCapacityExhausted
                } else {
                    conduit_kernel::FailureCode::WorkBudgetExhausted
                },
                detail: 81,
            });
        }
        let result = self.kernel.step(io, bytes);
        self.candidate = requesting && result == StepOutcome::Progress;
        result
    }
    fn step_committed(&mut self) {
        if self.candidate {
            self.updates += 1;
            self.candidate = false;
        }
    }
    fn cancel(&mut self) {
        self.candidate = false;
        <HostCallBack as StepBack<PORTS>>::cancel(&mut self.kernel);
    }
}

fn configuration(placement: &PlannedGear) -> Result<(&str, Option<u32>), String> {
    let resident =
        placement.kind_id.as_str() == conduit_semantic_catalog::AMBIENT_RESIDENT_OBSERVE_KIND;
    let expected = if resident {
        conduit_std_offers::ambient_resident_observe_cpu_offer()?
    } else {
        conduit_std_offers::ambient_observe_cpu_offer()?
    };
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract
        || placement.capability_id != expected.capability_id
        || placement.limits != expected.limits
        || placement.host_calls != expected.host_calls
        || placement.configuration.len() != if resident { 1 } else { 2 }
    {
        return Err("Home clock placement differs from exact offer".into());
    }
    let zone = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("timezone", ConfigurationValue::Text(value))
                if !value.is_empty() && value.len() <= 64 =>
            {
                Some(value.as_str())
            }
            _ => None,
        })
        .ok_or("clock timezone missing")?;
    if resident {
        return Ok((zone, None));
    }
    let updates = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-updates", ConfigurationValue::U64(value)) if (1..=4096).contains(value) => {
                Some(*value as u32)
            }
            _ => None,
        })
        .ok_or("clock finite update bound missing")?;
    Ok((zone, Some(updates)))
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let (_, updates) = configuration(placement)?;
    Ok(BackBudget {
        // One output may remain on the Home Cord while the next Host Call
        // completes. Both prepared output slots fund the full declared bound;
        // the Tick input retains its existing upstream owner's backing.
        value_items: 2,
        value_bytes: 2 * conduit_semantic_catalog::AMBIENT_HOME_MAXIMUM_BYTES,
        host_requests: 1,
        sign_items: match updates {
            Some(updates) => (updates * 8 + 16)
                .try_into()
                .map_err(|_| "clock Sign bound overflow")?,
            None => 256,
        },
        maximum_value_bytes: 8192,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(placement)?;
    let (zone, maximum_updates) = configuration(placement)?;
    Ok(InstalledBack::AmbientObserve(Box::new(
        AmbientObserveBack {
            kernel: HostCallBack::new(8),
            provider: RefCell::new(ClockHome::prepare(zone)?),
            maximum_updates,
            updates: 0,
            candidate: false,
        },
    )))
}
pub(super) fn supports(operation: &LoweredHostCall) -> bool {
    ((operation.contract_id.as_str() == CALL
        && operation
            .target_kind
            .as_ref()
            .is_some_and(|k| k.as_str() == conduit_semantic_catalog::AMBIENT_OBSERVE_KIND))
        || (operation.contract_id.as_str() == conduit_std_offers::AMBIENT_RESIDENT_OBSERVE_CALL
            && operation.target_kind.as_ref().is_some_and(|k| {
                k.as_str() == conduit_semantic_catalog::AMBIENT_RESIDENT_OBSERVE_KIND
            })))
        && operation.maximum_in_flight == 1
        && operation.binding.maximum_input_bytes == 8
        && operation.binding.maximum_output_bytes == 8192
}
pub(super) fn complete(
    operation: &LoweredHostCall,
    request: HostCallRequest,
    scheduler: &mut InstalledScheduler,
) -> Result<bool, String> {
    if !supports(operation) {
        return Ok(false);
    }
    if operation.node != request.node
        || operation.call != request.call
        || request.input.admitted_bytes != 8
    {
        return Err("clock request differs from exact operation".into());
    }
    let mut result = [0u8; 8192];
    let length = {
        let input = scheduler
            .host_value(request.input.value)
            .map_err(|why| format!("clock input: {why:?}"))?;
        let Some(InstalledBack::AmbientObserve(back)) =
            scheduler.drivers().get(usize::from(request.node.0))
        else {
            return Err("clock driver missing".into());
        };
        let mut provider = back.provider.borrow_mut();
        provider.sample(input).map(|bytes| {
            result[..bytes.len()].copy_from_slice(bytes);
            bytes.len()
        })
    };
    let outcome = match length {
        Ok(length) => {
            let value = scheduler
                .store_host_value(&result[..length])
                .map_err(|why| format!("clock output: {why:?}"))?;
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(value, 8192)
                        .map_err(|why| format!("clock output bound: {why:?}"))?,
                ),
                failure: None,
            }
        }
        Err(failure) => HostCallOutcome {
            disposition: HostCallDisposition::Failed,
            output: None,
            failure: Some(failure),
        },
    };
    scheduler
        .complete_host_call(request.node, request.request, outcome)
        .map_err(|why| format!("clock completion: {why:?}"))?;
    Ok(true)
}
