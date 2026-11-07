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

/// Inert, exclusively owned storage. No Face revision can be read or published
/// before binding a real execution basis; absence here is not a fabricated Play.
pub struct PreparedObservationBacking {
    prepared: PreparedObservationFace,
}
impl PreparedObservationBacking {
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.prepared.retained_backing_bytes()
    }
    pub fn prepare(maximum_samples: u32, stale_after_ms: u64) -> Result<Self, PublicationRefusal> {
        let unbound = FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: alloc::vec::Vec::new(),
        };
        Ok(Self {
            prepared: PreparedObservationFace::prepare_observed(
                unbound,
                maximum_samples,
                stale_after_ms,
            )?,
        })
    }
    /// Consume actual pre-Start scope ownership. No allocation or sample is
    /// introduced here; original Root admission and provenance remain mandatory.
    pub fn bind(self, basis: FaceBasis) -> Result<PreparedObservationFace, PublicationRefusal> {
        self.try_bind(basis).map_err(|(_, error)| error)
    }
    /// Invalid scope returns the original inert allocation for another attempt.
    pub fn try_bind(
        mut self,
        basis: FaceBasis,
    ) -> Result<PreparedObservationFace, (Self, PublicationRefusal)> {
        if basis.source_document_id.is_none()
            || basis.checked_plot_id.is_none()
            || basis.expanded_plot_id.is_none()
            || basis.plan_id.is_none()
            || basis.active_play_id.is_none()
            || basis.body_id.is_some() != basis.wake_id.is_some()
        {
            return Err((self, PublicationRefusal::InvalidBasis));
        }
        let previous = core::mem::replace(&mut self.prepared.face.basis, basis);
        if self.prepared.face.reseal_prepared().is_err() {
            self.prepared.face.basis = previous;
            // This restores the previously valid inert revision, using the same
            // bounded identity/hash storage. No publication has occurred.
            let _ = self.prepared.face.reseal_prepared();
            return Err((self, PublicationRefusal::InvalidBasis));
        }
        Ok(self.prepared)
    }
}

pub struct PreparedObservationFace {
    face: Face,
    measurement: Option<[FaceProperty; 3]>,
    failure: Option<FaceProperty>,
    maximum_samples: u32,
    last_observed_ms: u64,
    stale_after_ms: u64,
    time_property: &'static str,
}
impl PreparedObservationFace {
    /// Include the spare property payloads held outside the current revision.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut bytes = self.face.retained_backing_bytes()?;
        if let Some(measurement) = &self.measurement {
            for property in measurement {
                bytes = bytes.checked_add(property.retained_backing_bytes()?)?;
            }
        }
        if let Some(failure) = &self.failure {
            bytes = bytes.checked_add(failure.retained_backing_bytes()?)?;
        }
        Some(bytes)
    }

    /// All quantity leaves, wording, properties and identity storage are
    /// reserved here. No measurement is present in the warming Face.
    pub fn prepare(
        basis: FaceBasis,
        maximum_samples: u32,
        stale_after_ms: u64,
    ) -> Result<Self, PublicationRefusal> {
        Self::prepare_with_time_property(
            basis,
            maximum_samples,
            stale_after_ms,
            "sampled-at-boot-ms",
            None,
        )
    }
    /// Prepare Host-observed timing when acquisition time is unavailable. A
    /// later failed capture retains the earlier quantities and observation time.
    pub fn prepare_observed(
        basis: FaceBasis,
        maximum_samples: u32,
        stale_after_ms: u64,
    ) -> Result<Self, PublicationRefusal> {
        Self::prepare_with_time_property(
            basis,
            maximum_samples,
            stale_after_ms,
            "observed-at-boot-ms",
            None,
        )
    }
    /// Prepare owner-supplied Body wording under the exact Face Body basis.
    /// The caller supplies its admitted name; wording grants no authority.
    pub fn prepare_observed_named(
        basis: FaceBasis,
        maximum_samples: u32,
        stale_after_ms: u64,
        body_name: &str,
    ) -> Result<Self, PublicationRefusal> {
        Self::prepare_with_time_property(
            basis,
            maximum_samples,
            stale_after_ms,
            "observed-at-boot-ms",
            Some(body_name),
        )
    }
    fn prepare_with_time_property(
        basis: FaceBasis,
        maximum_samples: u32,
        stale_after_ms: u64,
        time_property: &'static str,
        body_name: Option<&str>,
    ) -> Result<Self, PublicationRefusal> {
        if maximum_samples == 0 {
            return Err(PublicationRefusal::Exhausted);
        }
        let mut subjects = alloc::vec::Vec::with_capacity(1 + usize::from(body_name.is_some()));
        if let Some(name) = body_name {
            let body = basis
                .body_id
                .as_ref()
                .ok_or(PublicationRefusal::InvalidBasis)?;
            if name.trim().is_empty() || name.len() > conduit_face::MAX_FACE_TEXT_BYTES {
                return Err(PublicationRefusal::InvalidFace);
            }
            let mut wording = String::new();
            wording
                .try_reserve_exact(conduit_body::MAX_BODY_FRIENDLY_NAME_BYTES)
                .map_err(|_| PublicationRefusal::Exhausted)?;
            if name.len() > conduit_body::MAX_BODY_FRIENDLY_NAME_BYTES {
                return Err(PublicationRefusal::InvalidFace);
            }
            wording.push_str(name);
            subjects.push(FaceSubject {
                identity: alloc::format!("body/{}", body.as_str()),
                role: FaceRole::Body,
                name: wording,
            });
        }
        subjects.push(FaceSubject {
            identity: SUBJECT.into(),
            role: FaceRole::Info,
            name: "Room conditions".into(),
        });
        let mut status = String::with_capacity(16);
        status.push_str("warming");
        let mut reason = String::with_capacity(32);
        reason.push_str("none");
        let mut face = Face::new(
            0,
            basis,
            subjects,
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
                property(time_property, FacePropertyValue::Count(0)),
            ]),
            failure: Some(property("sensor-failure", FacePropertyValue::Text(reason))),
            maximum_samples,
            last_observed_ms: 0,
            stale_after_ms,
            time_property,
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
        if now < self.last_observed_ms {
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
            *self.value_mut(self.time_property) = FacePropertyValue::Count(now);
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
            && now - self.count(self.time_property) <= self.stale_after_ms;
        *self.value_mut("fresh") = FacePropertyValue::Flag(fresh);
        *self.value_mut("sequence") = FacePropertyValue::Count(sequence + 1);
        self.last_observed_ms = now;
        self.seal()
    }

    /// Freshness is explicit Face truth; a Mask never infers it from cached
    /// display text. Repeated inspection of the same state retains its identity.
    pub fn at(&mut self, now: u64) -> Result<&Face, PublicationRefusal> {
        if now < self.last_observed_ms {
            return Err(PublicationRefusal::ClockReversed);
        }
        let expired = self.measurement.is_none()
            && now - self.count(self.time_property) > self.stale_after_ms;
        if expired && matches!(self.value_mut("fresh"), FacePropertyValue::Flag(true)) {
            *self.value_mut("fresh") = FacePropertyValue::Flag(false);
            self.seal()?;
        }
        self.last_observed_ms = now;
        Ok(&self.face)
    }
    /// Project committed owner metadata into preparation-reserved wording.
    /// This changes no observation or execution basis and grants no authority.
    pub fn project_body_name(
        &mut self,
        body_id: &str,
        name: &str,
    ) -> Result<(), PublicationRefusal> {
        if self.face.basis.body_id.as_ref().map(|id| id.as_str()) != Some(body_id)
            || name.trim().is_empty()
            || name.len() > conduit_body::MAX_BODY_FRIENDLY_NAME_BYTES
            || name.chars().any(char::is_control)
        {
            return Err(PublicationRefusal::InvalidBasis);
        }
        let subject = self
            .face
            .subjects
            .iter_mut()
            .find(|subject| subject.role == FaceRole::Body)
            .ok_or(PublicationRefusal::InvalidBasis)?;
        if subject.name == name {
            return Ok(());
        }
        if name.len() > subject.name.capacity() || self.face.revision == u64::MAX {
            return Err(PublicationRefusal::Exhausted);
        }
        subject.name.clear();
        subject.name.push_str(name);
        self.seal()
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
