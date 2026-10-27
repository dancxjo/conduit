//! Optional barometer page over the common typed environmental Face.
use super::*;
use conduit_core::{Quantity, QuantityRole, Unit};
use conduit_face::{Face, FacePropertyValue};

/// Additive view: the existing temperature/RH projection keeps its layout and
/// legacy signed records and Source identities are not rewritten.
pub struct PreparedEnvironmentalFace<'a> {
    environment: PreparedEnvironmentFace<'a>,
    pressure: Option<Quantity>,
}
impl<'a> PreparedEnvironmentalFace<'a> {
    pub fn prepare(face: &'a Face, subject: &str) -> Result<Self, FaceProjectionRefusal> {
        let environment = PreparedEnvironmentFace::prepare(face, subject)?;
        let mut properties = face.properties.iter().filter(|property| {
            property.subject == subject && property.name == "atmospheric-pressure"
        });
        let pressure = match properties.next().map(|property| &property.value) {
            None => None,
            Some(FacePropertyValue::TypedValue { contract, bytes }) => {
                contract
                    .validate(bytes)
                    .map_err(|_| FaceProjectionRefusal::InvalidQuantity)?;
                let value =
                    Quantity::decode(bytes).map_err(|_| FaceProjectionRefusal::InvalidQuantity)?;
                if value.family() != Unit::Pascal.family()
                    || value.role() != QuantityRole::Linear
                    || !value
                        .compare(Quantity::new(0, Unit::Pascal))
                        .map_err(|_| FaceProjectionRefusal::InvalidQuantity)?
                        .is_gt()
                {
                    return Err(FaceProjectionRefusal::InvalidQuantity);
                }
                Some(value)
            }
            _ => return Err(FaceProjectionRefusal::WrongPropertyType),
        };
        if properties.next().is_some() {
            return Err(FaceProjectionRefusal::DuplicateProperty);
        }
        Ok(Self {
            environment,
            pressure,
        })
    }
    pub fn environment(&self) -> &PreparedEnvironmentFace<'a> {
        &self.environment
    }
    pub fn has_pressure(&self) -> bool {
        self.pressure.is_some()
    }
}

/// Explicit pressure opt-in. The ordinary two-page Mask remains unchanged.
/// Capability differences change the finite page selection, never manufacture
/// an unsupported reading: BME T/RH/P, BMP T/P, DHT T/RH.
pub struct TimedEnvironmentalMask {
    look: SevenSegmentLook,
    timing: Alternation,
    period_ms: u64,
}
impl TimedEnvironmentalMask {
    pub fn prepare(
        look: SevenSegmentLook,
        period_ms: u64,
        lifetime_ms: u64,
    ) -> Result<Self, TimingRefusal> {
        Ok(Self {
            look,
            timing: Alternation::new(period_ms, lifetime_ms)?,
            period_ms,
        })
    }
    pub fn render<'a>(
        &self,
        face: &PreparedEnvironmentalFace<'a>,
        elapsed_ms: u64,
        profile: &Tm1637Profile,
    ) -> Result<RenderedShow<'a>, WiringRefusal> {
        profile.validate()?;
        if self.timing.select(elapsed_ms).is_err() {
            return Ok(RenderedShow {
                face_content_id: face.environment.content_id,
                face_revision: face.environment.revision,
                show: unavailable(),
                disposition: MaskDisposition::Unavailable(MaskUnavailable::OutsideLifetime),
            });
        }
        let humidity = face.environment.humidity.is_some();
        let pages = 1 + u64::from(humidity) + u64::from(face.pressure.is_some());
        match (elapsed_ms / self.period_ms) % pages {
            0 => render_for_profile(
                &face.environment,
                SevenSegmentSelection::Temperature,
                &self.look,
                profile,
            ),
            1 if humidity => render_for_profile(
                &face.environment,
                SevenSegmentSelection::Humidity,
                &self.look,
                profile,
            ),
            _ => render_pressure(face, profile),
        }
    }
    pub fn prepare_show(
        &self,
        face: &PreparedEnvironmentalFace<'_>,
        profile: &Tm1637Profile,
    ) -> Result<PreparedEnvironmentalShow, ShowPreparationRefusal> {
        let identity = face
            .environment
            .content_id
            .as_bytes()
            .try_into()
            .map_err(|_| ShowPreparationRefusal::IdentityExtent)?;
        let temperature = self
            .render(face, 0, profile)
            .map_err(ShowPreparationRefusal::Wiring)?;
        let humidity = face.environment.humidity.is_some();
        let pressure = face.pressure.is_some();
        let count = 1 + u8::from(humidity) + u8::from(pressure);
        // Resolve directly, without inventing elapsed times outside the admitted
        // lifetime merely to prepare a page that is never selected in that Play.
        let second = if humidity {
            render_for_profile(
                &face.environment,
                SevenSegmentSelection::Humidity,
                &self.look,
                profile,
            )
        } else if pressure {
            render_pressure(face, profile)
        } else {
            render_for_profile(
                &face.environment,
                SevenSegmentSelection::Temperature,
                &self.look,
                profile,
            )
        }
        .map_err(ShowPreparationRefusal::Wiring)?;
        let third = if pressure {
            render_pressure(face, profile)
        } else {
            render_for_profile(
                &face.environment,
                SevenSegmentSelection::Temperature,
                &self.look,
                profile,
            )
        }
        .map_err(ShowPreparationRefusal::Wiring)?;
        Ok(PreparedEnvironmentalShow {
            identity,
            revision: face.environment.revision,
            pages: [
                crate::prepared_show::Page::prepare(temperature),
                crate::prepared_show::Page::prepare(second),
                crate::prepared_show::Page::prepare(third),
            ],
            count,
            timing: self.timing,
            period_ms: self.period_ms,
        })
    }
}
fn render_pressure<'a>(
    face: &PreparedEnvironmentalFace<'a>,
    profile: &Tm1637Profile,
) -> Result<RenderedShow<'a>, WiringRefusal> {
    profile.validate()?;
    let value = if !face.environment.available {
        Err(MaskUnavailable::Sensor)
    } else if !face.environment.fresh {
        Err(MaskUnavailable::Stale)
    } else {
        face.pressure
            .ok_or(MaskUnavailable::MissingValue)
            .and_then(|value| {
                let unit = Unit::resolve("hPa").map_err(|_| MaskUnavailable::NumericExtent)?;
                let rounded = crate::rounding::rounded(value, unit, 0)?;
                if !(0..=9999).contains(&rounded) {
                    return Err(MaskUnavailable::Overflow);
                }
                let mut digits = [pattern(0); 4];
                let mut remainder = rounded as u16;
                for position in (0..4).rev() {
                    digits[position] = pattern(NUMERALS[usize::from(remainder % 10)]);
                    remainder /= 10;
                    if remainder == 0 {
                        break;
                    }
                }
                Ok(SevenSegmentShow::from_parts(digits, [false; 4], false))
            })
    };
    let (show, disposition) = match value {
        Ok(show) => (show, MaskDisposition::PressureHectopascal),
        Err(why) => (unavailable(), MaskDisposition::Unavailable(why)),
    };
    Ok(RenderedShow {
        face_content_id: face.environment.content_id,
        face_revision: face.environment.revision,
        show,
        disposition,
    })
}

/// Three fixed resolved pages; no Face, Quantity, timer or effect owner retained.
/// Separate from the legacy two-page value so ESP DHT storage does not grow.
pub struct PreparedEnvironmentalShow {
    identity: [u8; 64],
    revision: u64,
    pages: [crate::prepared_show::Page; 3],
    count: u8,
    timing: Alternation,
    period_ms: u64,
}
impl PreparedEnvironmentalShow {
    pub fn face_content_id(&self) -> &str {
        core::str::from_utf8(&self.identity).expect("validated Face identity")
    }
    pub fn face_revision(&self) -> u64 {
        self.revision
    }
    pub fn at(&self, elapsed_ms: u64) -> RenderedShow<'_> {
        let (show, disposition) = if self.timing.select(elapsed_ms).is_ok() {
            self.pages[((elapsed_ms / self.period_ms) % u64::from(self.count)) as usize].show()
        } else {
            (
                unavailable(),
                MaskDisposition::Unavailable(MaskUnavailable::OutsideLifetime),
            )
        };
        RenderedShow {
            face_content_id: self.face_content_id(),
            face_revision: self.revision,
            show,
            disposition,
        }
    }
}
