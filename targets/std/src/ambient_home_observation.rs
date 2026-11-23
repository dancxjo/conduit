//! Compose one bounded Home observation in the admitted native source worker.
mod configuration;
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
pub use configuration::{ambient_home_connection_limits, CheckedAmbientHomeSource};
use std::net::SocketAddr;

pub const HOME_OBSERVATION_BYTES: usize =
    conduit_semantic_catalog::AMBIENT_HOME_OBSERVATION_BYTES as usize;
const CLOCK_OBSERVATION_BYTES: usize = 4096;

pub struct PreparedAmbientHomeObservation {
    clock: PreparedLiveClock,
    clock_bytes: Box<[u8]>,
    deck: Option<PreparedDeck>,
    wallpaper: Option<PreparedWallpaperSelectionWire>,
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
        Self::prepare_with_optional_deck(timezone, endpoint, Some(deck))
    }

    pub fn prepare_with_optional_deck(
        timezone: &str,
        endpoint: SocketAddr,
        deck: Option<PreparedDeck>,
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
        let reason = canonical(reason.leaf(b"Wallpaper unavailable").map_err(debug_error)?)
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
            wallpaper: deck
                .as_ref()
                .map(PreparedWallpaperSelectionWire::prepare)
                .transpose()
                .map_err(debug_error)?,
            deck,
            unavailable_wallpaper,
            home: Composer::new(ty, HOME_OBSERVATION_BYTES).map_err(debug_error)?,
        })
    }

    pub fn photos(&self) -> Option<&PreparedDeck> {
        self.deck.as_ref()
    }

    pub fn prepared_heap_bytes(&self) -> usize {
        self.deck.as_ref().map_or(0, PreparedDeck::retained_bytes)
            + self.clock.prepared_heap_bytes()
            + self.clock_bytes.len()
            + self
                .wallpaper
                .as_ref()
                .map_or(0, PreparedWallpaperSelectionWire::prepared_heap_bytes)
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
            .as_mut()
            .and_then(|wallpaper| wallpaper.encode(instant).ok())
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
#[path = "ambient_home_observation/tests.rs"]
mod tests;
