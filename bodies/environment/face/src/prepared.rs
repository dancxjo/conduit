//! Pre-admitted storage for successive immutable environmental Face revisions.
//! Mutation is private; readers can only borrow a validated, sealed revision.
use crate::{property, SensorStatus, SUBJECT};
use alloc::{string::String, vec};
use conduit_core::{
    kind_id, CheckedValueContract, Quantity, Unit, QUANTITY_ENCODED_LEN, RATIO_INFO_ID,
    TEMPERATURE_INFO_ID,
};
use conduit_face::{Face, FaceBasis, FaceProperty, FacePropertyValue, FaceRole, FaceSubject};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationRefusal {
    InvalidBasis,
    InvalidTemperature,
    InvalidHumidity,
    InvalidStatus,
    Exhausted,
    ClockReversed,
    InvalidFace,
}

pub struct PreparedObservationFace {
    face: Face,
    measurement: Option<[FaceProperty; 3]>,
    failure: Option<FaceProperty>,
    maximum_samples: u32,
    last_publication_ms: u64,
    stale_after_ms: u64,
}
impl PreparedObservationFace {
    /// All quantity leaves, wording, properties and identity storage are
    /// reserved here. No measurement is present in the warming Face.
    pub fn prepare(
        basis: FaceBasis,
        maximum_samples: u32,
        stale_after_ms: u64,
    ) -> Result<Self, PublicationRefusal> {
        if maximum_samples == 0 {
            return Err(PublicationRefusal::Exhausted);
        }
        let mut status = String::with_capacity(16);
        status.push_str("warming");
        let mut reason = String::with_capacity(32);
        reason.push_str("none");
        let mut face = Face::new(
            0,
            basis,
            vec![FaceSubject {
                identity: SUBJECT.into(),
                role: FaceRole::Info,
                name: "Room conditions".into(),
            }],
            vec![],
            vec![
                property("fresh", FacePropertyValue::Flag(false)),
                property("sensor-status", FacePropertyValue::Text(status)),
                property("sequence", FacePropertyValue::Count(0)),
            ],
            vec![],
        )
        .map_err(|_| PublicationRefusal::InvalidBasis)?;
        face.properties.reserve_exact(4);
        let quantity = |name, kind, value: Quantity| {
            property(
                name,
                FacePropertyValue::TypedValue {
                    contract: CheckedValueContract::new(
                        kind_id(kind),
                        QUANTITY_ENCODED_LEN as u32,
                        vec![],
                    )
                    .unwrap(),
                    bytes: value.encode().to_vec(),
                },
            )
        };
        Ok(Self {
            face,
            measurement: Some([
                quantity(
                    "temperature",
                    TEMPERATURE_INFO_ID,
                    Quantity::new(0, Unit::Celsius),
                ),
                quantity(
                    "relative-humidity",
                    RATIO_INFO_ID,
                    Quantity::new(0, Unit::Percent),
                ),
                property("sampled-at-boot-ms", FacePropertyValue::Count(0)),
            ]),
            failure: Some(property("sensor-failure", FacePropertyValue::Text(reason))),
            maximum_samples,
            last_publication_ms: 0,
            stale_after_ms,
        })
    }

    /// Retain last valid quantities on failure. A refused publication leaves
    /// the prior Face unchanged, including its exact content identity.
    pub fn publish(
        &mut self,
        observation: Option<(Quantity, Quantity)>,
        status: SensorStatus,
        now: u64,
    ) -> Result<(), PublicationRefusal> {
        use PublicationRefusal as R;
        if now < self.last_publication_ms {
            return Err(R::ClockReversed);
        }
        let sequence = self.count("sequence");
        if sequence >= u64::from(self.maximum_samples) {
            return Err(R::Exhausted);
        }
        if (status == SensorStatus::Available) != observation.is_some()
            || status == SensorStatus::Warming
        {
            return Err(R::InvalidStatus);
        }
        if let Some((temperature, humidity)) = observation {
            for (name, value, refusal) in [
                ("temperature", temperature, R::InvalidTemperature),
                ("relative-humidity", humidity, R::InvalidHumidity),
            ] {
                let p = self
                    .face
                    .properties
                    .iter()
                    .chain(self.measurement.iter().flatten())
                    .find(|p| p.name == name)
                    .unwrap();
                let FacePropertyValue::TypedValue { contract, .. } = &p.value else {
                    unreachable!()
                };
                contract.validate(&value.encode()).map_err(|_| refusal)?;
            }
            if humidity
                .compare(Quantity::new(0, Unit::Percent))
                .map_err(|_| R::InvalidHumidity)?
                .is_lt()
                || humidity
                    .compare(Quantity::new(100, Unit::Percent))
                    .map_err(|_| R::InvalidHumidity)?
                    .is_gt()
            {
                return Err(R::InvalidHumidity);
            }
            if let Some(properties) = self.measurement.take() {
                self.face.properties.extend(properties);
            }
            for (name, value) in [
                ("temperature", temperature),
                ("relative-humidity", humidity),
            ] {
                let FacePropertyValue::TypedValue { bytes, .. } = self.value_mut(name) else {
                    unreachable!()
                };
                bytes.copy_from_slice(&value.encode());
            }
            *self.value_mut("sampled-at-boot-ms") = FacePropertyValue::Count(now);
        }
        let label = match status {
            SensorStatus::Available => "available",
            SensorStatus::Unavailable(_) => "unavailable",
            SensorStatus::Warming => unreachable!(),
        };
        self.set_text("sensor-status", label);
        if let SensorStatus::Unavailable(reason) = status {
            if let Some(property) = self.failure.take() {
                self.face.properties.push(property);
            }
            self.set_text("sensor-failure", reason.label());
        } else if let Some(index) = self
            .face
            .properties
            .iter()
            .position(|p| p.name == "sensor-failure")
        {
            self.failure = Some(self.face.properties.swap_remove(index));
        }
        let fresh = self.measurement.is_none()
            && now - self.count("sampled-at-boot-ms") <= self.stale_after_ms;
        *self.value_mut("fresh") = FacePropertyValue::Flag(fresh);
        *self.value_mut("sequence") = FacePropertyValue::Count(sequence + 1);
        self.last_publication_ms = now;
        self.seal()
    }

    /// Freshness is explicit Face truth; a Mask never infers it from cached
    /// display text. Repeated inspection of the same state retains its identity.
    pub fn at(&mut self, now: u64) -> Result<&Face, PublicationRefusal> {
        if now < self.last_publication_ms {
            return Err(PublicationRefusal::ClockReversed);
        }
        let expired = self.measurement.is_none()
            && now - self.count("sampled-at-boot-ms") > self.stale_after_ms;
        if expired && matches!(self.value_mut("fresh"), FacePropertyValue::Flag(true)) {
            *self.value_mut("fresh") = FacePropertyValue::Flag(false);
            self.seal()?;
        }
        Ok(&self.face)
    }
    pub fn current(&self) -> &Face {
        &self.face
    }
    fn count(&self, name: &str) -> u64 {
        match &self
            .face
            .properties
            .iter()
            .find(|p| p.name == name)
            .unwrap()
            .value
        {
            FacePropertyValue::Count(value) => *value,
            _ => unreachable!(),
        }
    }
    fn value_mut(&mut self, name: &str) -> &mut FacePropertyValue {
        &mut self
            .face
            .properties
            .iter_mut()
            .find(|p| p.name == name)
            .unwrap()
            .value
    }
    fn set_text(&mut self, name: &str, value: &str) {
        let FacePropertyValue::Text(text) = self.value_mut(name) else {
            unreachable!()
        };
        assert!(value.len() <= text.capacity());
        text.clear();
        text.push_str(value);
    }
    fn seal(&mut self) -> Result<(), PublicationRefusal> {
        // The admitted sample bound permits at most two revisions per sample.
        self.face.revision += 1;
        self.face
            .reseal_prepared()
            .map_err(|_| PublicationRefusal::InvalidFace)
    }
}
