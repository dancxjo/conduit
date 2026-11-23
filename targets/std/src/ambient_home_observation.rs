//! Compose one bounded Home observation in the admitted native source worker.
use crate::{
    ambient_clock::PreparedLiveClock,
    ambient_wallpaper::{
        acquisition::PreparedDeck, selection_wire::PreparedWallpaperSelectionWire,
    },
    NativeReadContext, NativeSourceWorkerProvider, WorkerFailure,
};
use conduit_core::{
    validate_canonical_structured_value as canonical, PreparedStructuredComposer as Composer,
    StructuredInfoTypeShape,
};
use std::net::SocketAddr;

pub const HOME_OBSERVATION_BYTES: usize = 8192;
const CLOCK_OBSERVATION_BYTES: usize = 4096;

pub struct PreparedAmbientHomeObservation {
    clock: PreparedLiveClock,
    clock_bytes: Box<[u8]>,
    deck: PreparedDeck,
    wallpaper: PreparedWallpaperSelectionWire,
    unavailable_wallpaper: Box<[u8]>,
    home: Composer,
}

impl PreparedAmbientHomeObservation {
    /// The caller admits the peer, the worker, and the original photo custody.
    /// The deck remains available for separately admitted graphical realization.
    pub fn prepare(
        timezone: &str,
        endpoint: SocketAddr,
        deck: PreparedDeck,
    ) -> Result<Self, String> {
        let types = conduit_semantic_catalog::prepare_ambient_display_types()?;
        let ty = &types
            .native_types
            .iter()
            .find(|ty| ty.name == "AmbientHomeObservation")
            .ok_or("missing AmbientHomeObservation")?
            .value_type;
        let StructuredInfoTypeShape::Record {
            fields: [clock_field, wallpaper_field],
            ..
        } = ty.shape()
        else {
            return Err("unexpected Home observation fields".into());
        };
        if clock_field.name() != "clock_weather" || wallpaper_field.name() != "wallpaper" {
            return Err("unexpected Home observation field order".into());
        }
        let wallpaper_type = wallpaper_field.value_type();
        let StructuredInfoTypeShape::Variant { cases, .. } = wallpaper_type.shape() else {
            return Err("expected wallpaper observation cases".into());
        };
        let unavailable = cases
            .iter()
            .find(|case| case.tag() == "unavailable")
            .ok_or("missing wallpaper absence case")?;
        let text_type = unavailable.payload_type();
        let mut reason = Composer::new(text_type, 256).map_err(debug_error)?;
        let reason = canonical(
            reason
                .leaf(b"Wallpaper selection time unavailable")
                .map_err(debug_error)?,
        )
        .map_err(debug_error)?;
        let mut absence =
            Composer::new(wallpaper_type, HOME_OBSERVATION_BYTES).map_err(debug_error)?;
        let unavailable_wallpaper = absence
            .variant("unavailable", reason)
            .map_err(debug_error)?
            .into();
        Ok(Self {
            clock: PreparedLiveClock::prepare(timezone, endpoint)?,
            clock_bytes: vec![0; CLOCK_OBSERVATION_BYTES].into_boxed_slice(),
            wallpaper: PreparedWallpaperSelectionWire::prepare(&deck).map_err(debug_error)?,
            deck,
            unavailable_wallpaper,
            home: Composer::new(ty, HOME_OBSERVATION_BYTES).map_err(debug_error)?,
        })
    }

    pub fn photos(&self) -> &PreparedDeck {
        &self.deck
    }

    pub fn prepared_heap_bytes(&self) -> usize {
        self.deck.retained_bytes()
            + self.clock.prepared_heap_bytes()
            + self.clock_bytes.len()
            + self.wallpaper.prepared_heap_bytes()
            + self.unavailable_wallpaper.len()
            + self.home.prepared_heap_bytes()
    }
    fn compose(&mut self, len: usize) -> Result<&[u8], WorkerFailure> {
        let clock = canonical(&self.clock_bytes[..len]).map_err(|_| WorkerFailure::Provider)?;
        let instant = clock
            .record_field("observed_at_unix_ms")
            .map_err(|_| WorkerFailure::Provider)?
            .ok_or(WorkerFailure::Provider)?
            .primitive_bytes("value/u64")
            .map_err(|_| WorkerFailure::Provider)?;
        let instant = u64::from_le_bytes(instant.try_into().map_err(|_| WorkerFailure::Provider)?);
        let wallpaper = self
            .wallpaper
            .encode(instant)
            .unwrap_or(&self.unavailable_wallpaper);
        let wallpaper = canonical(wallpaper).map_err(|_| WorkerFailure::Provider)?;
        self.home
            .record(&[clock, wallpaper])
            .map_err(|_| WorkerFailure::InvalidOutput)
    }
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

// SAFETY: both contributors use fixed prepared backing and the nested clock
// uses the same worker deadline and cancellation context. No borrow escapes.
unsafe impl NativeSourceWorkerProvider for PreparedAmbientHomeObservation {
    fn read(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        context: &NativeReadContext<'_>,
    ) -> Result<usize, WorkerFailure> {
        let len = self.clock.read(input, &mut self.clock_bytes, context)?;
        let combined = self.compose(len)?;
        if context.cancelled() {
            return Err(WorkerFailure::Cancelled);
        }
        if context.remaining().is_zero() {
            return Err(WorkerFailure::TimedOut);
        }
        output
            .get_mut(..combined.len())
            .ok_or(WorkerFailure::InvalidOutput)?
            .copy_from_slice(combined);
        Ok(combined.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ambient_clock::PreparedAmbientClockWire,
        ambient_wallpaper::{
            acquisition::{
                prepare_deck, PreparationAdmission, MAXIMUM_RETAINED_BYTES, PREPARATION_BYTES,
            },
            MAXIMUM_APOD_RESPONSE_BYTES,
        },
    };
    use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

    #[test]
    fn prepared_home_rotates_from_the_clock_observation_without_allocations() {
        let admission = PreparationAdmission::admit(
            chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
            MAXIMUM_RETAINED_BYTES,
            PREPARATION_BYTES,
        )
        .unwrap();
        let mut number = 0;
        let deck = prepare_deck(admission, |requested, maximum| {
            if maximum == MAXIMUM_APOD_RESPONSE_BYTES {
                return Ok(serde_json::to_vec(&serde_json::json!({
                    "date": requested.rsplit('=').next().unwrap(),
                    "title": "Fixture photo",
                    "media_type": "image",
                    "hdurl": "https://apod.nasa.gov/fixture.png"
                }))
                .unwrap());
            }
            number += 1;
            let mut pixels = Vec::new();
            PngEncoder::new(&mut pixels)
                .write_image(&[number, 2, 3, 255], 1, 1, ExtendedColorType::Rgba8)
                .unwrap();
            Ok(pixels)
        })
        .unwrap();
        let mut home = PreparedAmbientHomeObservation::prepare(
            "America/Los_Angeles",
            "127.0.0.1:80".parse().unwrap(),
            deck,
        )
        .unwrap();
        let mut clock =
            PreparedAmbientClockWire::prepare("America/Los_Angeles", "Weather unavailable")
                .unwrap();
        for (now, selected) in [(100_000, 100_000), (399_999, 100_000), (400_000, 400_000)] {
            let sample = clock.encode(now).unwrap();
            home.clock_bytes[..sample.len()].copy_from_slice(sample);
            let probe = crate::allocation_probe::begin();
            let encoded = home.compose(sample.len()).unwrap();
            assert_eq!(probe.finish_bytes(), 0);
            let observation = canonical(encoded).unwrap();
            let selection = observation
                .record_field("wallpaper")
                .unwrap()
                .unwrap()
                .variant_payload("available")
                .unwrap()
                .unwrap();
            let observed = selection
                .record_field("selected_at_unix_ms")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap();
            assert_eq!(u64::from_le_bytes(observed.try_into().unwrap()), selected);
        }
    }
}
