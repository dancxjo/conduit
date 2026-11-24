//! Compose one bounded Home observation in the admitted native source worker.
mod configuration;
use crate::{
    ambient_clock::PreparedLiveClock,
    ambient_wallpaper::{
        acquisition::PreparedDeck,
        observation::{PreparedWallpaperObservation, WallpaperPreparation},
    },
    NativeReadContext, NativeSourceWorkerProvider, WorkerFailure,
};
use conduit_core::{
    validate_canonical_structured_value as canonical, PreparedStructuredComposer as Composer,
    StructuredInfoTypeShape,
};
#[cfg(all(target_os = "linux", feature = "linux-display"))]
pub(crate) use configuration::stage_ambient_worker;
pub use configuration::{ambient_home_connection_limits, CheckedAmbientHomeSource};
use std::net::SocketAddr;

pub const HOME_OBSERVATION_BYTES: usize =
    conduit_semantic_catalog::AMBIENT_HOME_OBSERVATION_BYTES as usize;
const CLOCK_OBSERVATION_BYTES: usize = 4096;

pub struct PreparedAmbientHomeObservation {
    clock: PreparedLiveClock,
    clock_bytes: Box<[u8]>,
    wallpaper: PreparedWallpaperObservation,
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
        Self::prepare_with_wallpaper(timezone, endpoint, deck.into())
    }

    pub(crate) fn prepare_with_wallpaper(
        timezone: &str,
        endpoint: SocketAddr,
        wallpaper: WallpaperPreparation,
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
        Ok(Self {
            clock: PreparedLiveClock::prepare(timezone, endpoint)?,
            clock_bytes: vec![0; CLOCK_OBSERVATION_BYTES].into_boxed_slice(),
            wallpaper: PreparedWallpaperObservation::prepare_with_type(
                wallpaper,
                wallpaper_field.value_type(),
            )?,
            home: Composer::new(ty, HOME_OBSERVATION_BYTES).map_err(debug_error)?,
        })
    }

    pub fn photos(&self) -> Option<&PreparedDeck> {
        self.wallpaper.photos()
    }

    pub fn prepared_heap_bytes(&self) -> usize {
        self.wallpaper.prepared_heap_bytes()
            + self.clock.prepared_heap_bytes()
            + self.clock_bytes.len()
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
        let wallpaper =
            canonical(self.wallpaper.observe_at(instant)).map_err(|_| WorkerFailure::Provider)?;
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
