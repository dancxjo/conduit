//! Bounded PREWAKE-only robotics sources and differential-drive projection.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use super::robotics_effect::SimulatedDriveEffect;
use conduit_core::{
    BatteryObservation, ConfigurationEntry, ConfigurationValue, InfoBool, OdometryObservation,
    OrientationObservation, PlannedGear, RangeObservation, Scalar, BOOL_ENCODED_LEN,
    ROBOTICS_BATTERY_ENCODED_LEN, ROBOTICS_ODOMETRY_ENCODED_LEN, ROBOTICS_ORIENTATION_ENCODED_LEN,
    ROBOTICS_RANGE_ENCODED_LEN, SCALAR_ENCODED_LEN,
};
use conduit_kernel::{
    Failure, FailureCode, HostedValueStore, OperationAction, OperationInput, PortId, ValueRef,
    ValueStorage,
};

pub(super) static ROBOTICS_OBSERVE_BUMP_FACTORY: InstalledFactory =
    factory(conduit_std_catalog::ROBOTICS_OBSERVE_BUMP_IMPLEMENTATION);
pub(super) static ROBOTICS_OBSERVE_IMU_FACTORY: InstalledFactory =
    factory(conduit_std_catalog::ROBOTICS_OBSERVE_IMU_IMPLEMENTATION);
pub(super) static ROBOTICS_OBSERVE_RANGE_FACTORY: InstalledFactory =
    factory(conduit_std_catalog::ROBOTICS_OBSERVE_RANGE_IMPLEMENTATION);
pub(super) static ROBOTICS_OBSERVE_ODOMETRY_FACTORY: InstalledFactory =
    factory(conduit_std_catalog::ROBOTICS_OBSERVE_ODOMETRY_IMPLEMENTATION);
pub(super) static ROBOTICS_OBSERVE_BATTERY_FACTORY: InstalledFactory =
    factory(conduit_std_catalog::ROBOTICS_OBSERVE_BATTERY_IMPLEMENTATION);
pub(super) static ROBOTICS_VELOCITY_INTENT_FACTORY: InstalledFactory =
    factory(conduit_std_catalog::ROBOTICS_VELOCITY_INTENT_IMPLEMENTATION);
pub(super) static ROBOTICS_DRIVE_DIFFERENTIAL_FACTORY: InstalledFactory =
    factory(conduit_std_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_IMPLEMENTATION);

const fn factory(implementation_id: &'static str) -> InstalledFactory {
    InstalledFactory {
        implementation_id,
        budget: robotics_budget,
        prepare: prepare_robotics,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimulatedAvailability {
    Fresh,
    Missing,
    Stale,
}

pub(super) struct RoboticsSourceOperation {
    availability: SimulatedAvailability,
    values: [Option<ValueRef>; 2],
    next: usize,
    cancelled: bool,
}

impl RoboticsSourceOperation {
    pub(super) fn allocation_capacity(&self) -> usize {
        0
    }

    pub(super) fn start(&mut self) -> OperationAction {
        if self.cancelled {
            return cancelled();
        }
        match self.availability {
            SimulatedAvailability::Fresh => self.emit_or_complete(),
            SimulatedAvailability::Missing => fail(FailureCode::InvalidInput, 40),
            SimulatedAvailability::Stale => fail(FailureCode::InvalidInput, 41),
        }
    }

    pub(super) fn resume(&mut self, _input: OperationInput) -> OperationAction {
        fail(FailureCode::InvalidLifecycle, 42)
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.next = self.next.saturating_add(1);
        self.emit_or_complete()
    }

    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
    }

    fn emit_or_complete(&self) -> OperationAction {
        self.values
            .get(self.next)
            .copied()
            .flatten()
            .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                port: PortId(u16::try_from(self.next).expect("robotics has at most two outputs")),
                value,
            })
    }
}

pub(super) struct RoboticsDriveOperation {
    linear: Option<Scalar>,
    angular: Option<Scalar>,
    bumper_pressed: Option<bool>,
    forward_range: Option<RangeObservation>,
    closed: [bool; 4],
    minimum_clearance_mm: u32,
    maximum_range_age_ms: u32,
    effect: Option<SimulatedDriveEffect>,
}

impl RoboticsDriveOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port } => {
                let index = usize::from(port.0);
                if index >= self.closed.len() || self.closed[index] {
                    return fail(FailureCode::InvalidPort, 43);
                }
                self.closed[index] = true;
                if self.closed.iter().all(|closed| *closed) {
                    self.effect = Some(SimulatedDriveEffect::Suppressed);
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 44),
        }
    }

    pub(super) fn resume_value(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
        let index = usize::from(port.0);
        if index >= self.closed.len() || self.closed[index] {
            return fail(FailureCode::InvalidInput, 45);
        }
        let decoded = match index {
            0 if self.linear.is_none() && value.byte_len == SCALAR_ENCODED_LEN as u32 => {
                Scalar::decode(canonical).map(|value| self.linear = Some(value))
            }
            1 if self.angular.is_none() && value.byte_len == SCALAR_ENCODED_LEN as u32 => {
                Scalar::decode(canonical).map(|value| self.angular = Some(value))
            }
            2 if self.bumper_pressed.is_none() && value.byte_len == BOOL_ENCODED_LEN as u32 => {
                InfoBool::decode(canonical).map(|value| self.bumper_pressed = Some(value.get()))
            }
            3 if self.forward_range.is_none()
                && value.byte_len == ROBOTICS_RANGE_ENCODED_LEN as u32 =>
            {
                RangeObservation::decode(canonical).map(|value| self.forward_range = Some(value))
            }
            _ => return fail(FailureCode::InvalidInput, 46),
        };
        if decoded.is_err() {
            return fail(FailureCode::InvalidInput, 46);
        }
        match (
            self.linear,
            self.angular,
            self.bumper_pressed,
            self.forward_range,
        ) {
            (Some(linear), Some(angular), Some(pressed), Some(range)) => {
                self.effect = if pressed
                    || range.distance_mm() < self.minimum_clearance_mm
                    || range.age_ms() > self.maximum_range_age_ms
                {
                    Some(SimulatedDriveEffect::Suppressed)
                } else {
                    Some(SimulatedDriveEffect::Projected { linear, angular })
                };
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.effect = Some(SimulatedDriveEffect::Cancelled);
    }

    pub(super) fn effect(&self) -> Option<SimulatedDriveEffect> {
        self.effect
    }
}

fn robotics_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let offer = expected_offer(placement)?;
    validate_exact_placement(placement, &offer)?;
    if placement.kind_id.as_str() == conduit_std_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_KIND {
        return Ok(OperationBudget {
            value_items: 0,
            value_bytes: 0,
            host_requests: 0,
            sign_items: 64,
            maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
        });
    }
    let (items, bytes, maximum) = source_shape(placement)?;
    Ok(OperationBudget {
        value_items: items,
        value_bytes: bytes,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: maximum,
    })
}

fn prepare_robotics(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    let offer = expected_offer(placement)?;
    validate_exact_placement(placement, &offer)?;
    if placement.kind_id.as_str() == conduit_std_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_KIND {
        return Ok(InstalledOperation::RoboticsDrive(RoboticsDriveOperation {
            linear: None,
            angular: None,
            bumper_pressed: None,
            forward_range: None,
            closed: [false; 4],
            minimum_clearance_mm: u32_value(&placement.configuration, "minimum-clearance-mm", 250)?,
            maximum_range_age_ms: u32_value(
                &placement.configuration,
                "maximum-range-age-ms",
                1_000,
            )?,
            effect: None,
        }));
    }
    let availability = availability(&placement.configuration)?;
    let mut prepared = [None; 2];
    let encoded = source_values(placement)?;
    for (slot, canonical) in prepared.iter_mut().zip(encoded.iter()) {
        if let Some(canonical) = canonical {
            *slot = Some(
                values
                    .store(canonical)
                    .map_err(|error| format!("store simulated robotics value: {error:?}"))?,
            );
        }
    }
    Ok(InstalledOperation::RoboticsSource(
        RoboticsSourceOperation {
            availability,
            values: prepared,
            next: 0,
            cancelled: false,
        },
    ))
}

fn expected_offer(placement: &PlannedGear) -> Result<conduit_core::CapabilityOffer, String> {
    match placement.kind_id.as_str() {
        conduit_std_catalog::ROBOTICS_OBSERVE_BUMP_KIND => {
            Ok(conduit_std_catalog::robotics_observe_bump_offer())
        }
        conduit_std_catalog::ROBOTICS_OBSERVE_IMU_KIND => {
            Ok(conduit_std_catalog::robotics_observe_imu_offer())
        }
        conduit_std_catalog::ROBOTICS_OBSERVE_RANGE_KIND => {
            Ok(conduit_std_catalog::robotics_observe_range_offer())
        }
        conduit_std_catalog::ROBOTICS_OBSERVE_ODOMETRY_KIND => {
            Ok(conduit_std_catalog::robotics_observe_odometry_offer())
        }
        conduit_std_catalog::ROBOTICS_OBSERVE_BATTERY_KIND => {
            Ok(conduit_std_catalog::robotics_observe_battery_offer())
        }
        conduit_std_catalog::ROBOTICS_VELOCITY_INTENT_KIND => {
            Ok(conduit_std_catalog::robotics_velocity_intent_offer())
        }
        conduit_std_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_KIND => {
            Ok(conduit_std_catalog::robotics_drive_differential_offer())
        }
        _ => Err("unsupported installed robotics Kind".to_string()),
    }
}

fn validate_exact_placement(
    placement: &PlannedGear,
    offer: &conduit_core::CapabilityOffer,
) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || placement.limits != offer.limits
    {
        return Err(
            "planned robotics executable identity does not match its PREWAKE installation"
                .to_string(),
        );
    }
    Ok(())
}

fn source_shape(placement: &PlannedGear) -> Result<(u16, u32, u32), String> {
    match placement.kind_id.as_str() {
        conduit_std_catalog::ROBOTICS_OBSERVE_BUMP_KIND => {
            Ok((1, BOOL_ENCODED_LEN as u32, BOOL_ENCODED_LEN as u32))
        }
        conduit_std_catalog::ROBOTICS_OBSERVE_IMU_KIND => Ok((
            1,
            ROBOTICS_ORIENTATION_ENCODED_LEN as u32,
            ROBOTICS_ORIENTATION_ENCODED_LEN as u32,
        )),
        conduit_std_catalog::ROBOTICS_OBSERVE_RANGE_KIND => Ok((
            1,
            ROBOTICS_RANGE_ENCODED_LEN as u32,
            ROBOTICS_RANGE_ENCODED_LEN as u32,
        )),
        conduit_std_catalog::ROBOTICS_OBSERVE_ODOMETRY_KIND => Ok((
            1,
            ROBOTICS_ODOMETRY_ENCODED_LEN as u32,
            ROBOTICS_ODOMETRY_ENCODED_LEN as u32,
        )),
        conduit_std_catalog::ROBOTICS_OBSERVE_BATTERY_KIND => Ok((
            1,
            ROBOTICS_BATTERY_ENCODED_LEN as u32,
            ROBOTICS_BATTERY_ENCODED_LEN as u32,
        )),
        conduit_std_catalog::ROBOTICS_VELOCITY_INTENT_KIND => Ok((
            2,
            (SCALAR_ENCODED_LEN * 2) as u32,
            SCALAR_ENCODED_LEN as u32,
        )),
        _ => Err("robotics source shape is unsupported".to_string()),
    }
}

fn source_values(placement: &PlannedGear) -> Result<[Option<Vec<u8>>; 2], String> {
    let one = |value: Vec<u8>| Ok([Some(value), None]);
    match placement.kind_id.as_str() {
        conduit_std_catalog::ROBOTICS_OBSERVE_BUMP_KIND => {
            let pressed = text_or(&placement.configuration, "state", "clear")? == "pressed";
            one(InfoBool::new(pressed).encode().to_vec())
        }
        conduit_std_catalog::ROBOTICS_OBSERVE_IMU_KIND => one(OrientationObservation::new(
            i32_value(&placement.configuration, "roll-microradians", 0)?,
            i32_value(&placement.configuration, "pitch-microradians", 0)?,
            i32_value(&placement.configuration, "yaw-microradians", 0)?,
        )
        .map_err(|error| format!("invalid orientation observation: {error:?}"))?
        .encode()
        .to_vec()),
        conduit_std_catalog::ROBOTICS_OBSERVE_RANGE_KIND => one(RangeObservation::new(
            u32_value(&placement.configuration, "distance-mm", 1_000)?,
            u32_value(&placement.configuration, "age-ms", 0)?,
        )
        .map_err(|error| format!("invalid range observation: {error:?}"))?
        .encode()
        .to_vec()),
        conduit_std_catalog::ROBOTICS_OBSERVE_ODOMETRY_KIND => one(OdometryObservation::new(
            i32_value(&placement.configuration, "forward-mm", 0)?,
            i32_value(&placement.configuration, "lateral-mm", 0)?,
            i32_value(&placement.configuration, "yaw-microradians", 0)?,
        )
        .map_err(|error| format!("invalid odometry observation: {error:?}"))?
        .encode()
        .to_vec()),
        conduit_std_catalog::ROBOTICS_OBSERVE_BATTERY_KIND => one(BatteryObservation::new(
            u16_value(&placement.configuration, "charge-permille", 1_000)?,
            u16_value(&placement.configuration, "millivolts", 12_000)?,
        )
        .map_err(|error| format!("invalid battery observation: {error:?}"))?
        .encode()
        .to_vec()),
        conduit_std_catalog::ROBOTICS_VELOCITY_INTENT_KIND => Ok([
            Some(
                Scalar::from_raw_microunits(i64_value(
                    &placement.configuration,
                    "linear-microunits",
                    0,
                )?)
                .encode()
                .to_vec(),
            ),
            Some(
                Scalar::from_raw_microunits(i64_value(
                    &placement.configuration,
                    "angular-microunits",
                    0,
                )?)
                .encode()
                .to_vec(),
            ),
        ]),
        _ => Err("robotics source value is unsupported".to_string()),
    }
}

fn availability(entries: &[ConfigurationEntry]) -> Result<SimulatedAvailability, String> {
    match text_or(
        entries,
        conduit_std_catalog::ROBOTICS_AVAILABILITY_KEY,
        conduit_std_catalog::ROBOTICS_AVAILABILITY_FRESH,
    )? {
        conduit_std_catalog::ROBOTICS_AVAILABILITY_FRESH => Ok(SimulatedAvailability::Fresh),
        conduit_std_catalog::ROBOTICS_AVAILABILITY_MISSING => Ok(SimulatedAvailability::Missing),
        conduit_std_catalog::ROBOTICS_AVAILABILITY_STALE => Ok(SimulatedAvailability::Stale),
        _ => Err("invalid simulated robotics availability".to_string()),
    }
}

fn text_or<'a>(
    entries: &'a [ConfigurationEntry],
    key: &str,
    default: &'a str,
) -> Result<&'a str, String> {
    Ok(entries
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::Text(value)) if found == key => Some(value.as_str()),
            _ => None,
        })
        .unwrap_or(default))
}

fn i64_value(entries: &[ConfigurationEntry], key: &str, default: i64) -> Result<i64, String> {
    Ok(entries
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::I64(value)) if found == key => Some(*value),
            _ => None,
        })
        .unwrap_or(default))
}

fn i32_value(entries: &[ConfigurationEntry], key: &str, default: i64) -> Result<i32, String> {
    i32::try_from(i64_value(entries, key, default)?)
        .map_err(|_| format!("robotics configuration '{key}' does not fit i32"))
}

fn u64_value(entries: &[ConfigurationEntry], key: &str, default: u64) -> Result<u64, String> {
    Ok(entries
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::U64(value)) if found == key => Some(*value),
            _ => None,
        })
        .unwrap_or(default))
}

fn u32_value(entries: &[ConfigurationEntry], key: &str, default: u64) -> Result<u32, String> {
    u32::try_from(u64_value(entries, key, default)?)
        .map_err(|_| format!("robotics configuration '{key}' does not fit u32"))
}

fn u16_value(entries: &[ConfigurationEntry], key: &str, default: u64) -> Result<u16, String> {
    u16::try_from(u64_value(entries, key, default)?)
        .map_err(|_| format!("robotics configuration '{key}' does not fit u16"))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

fn cancelled() -> OperationAction {
    fail(FailureCode::Cancelled, 47)
}
