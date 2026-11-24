//! Prepare one bounded wallpaper observation from an explicitly acquired deck.
use super::{acquisition::PreparedDeck, selection_wire::PreparedWallpaperSelectionWire};
use crate::{NativeReadContext, NativeSourceWorkerProvider, WorkerFailure};
use conduit_core::{
    validate_canonical_structured_value as canonical, PreparedStructuredComposer as Composer,
    StructuredInfoType, StructuredInfoTypeShape,
};
use std::time::{SystemTime, UNIX_EPOCH};

pub const WALLPAPER_OBSERVATION_BYTES: usize =
    conduit_semantic_catalog::RESIDENT_WALLPAPER_BYTES as usize;

/// The finite outcome of the Host's admitted photograph preparation.
pub enum WallpaperPreparation {
    Ready(Box<PreparedDeck>),
    NotPrepared,
    ProviderRateLimited,
    ProviderUnavailable,
}

impl From<Option<PreparedDeck>> for WallpaperPreparation {
    fn from(deck: Option<PreparedDeck>) -> Self {
        deck.map_or(Self::NotPrepared, |deck| Self::Ready(Box::new(deck)))
    }
}

pub struct PreparedWallpaperObservation {
    deck: Option<PreparedDeck>,
    selection: Option<PreparedWallpaperSelectionWire>,
    unavailable: Box<[u8]>,
}

impl PreparedWallpaperObservation {
    pub fn prepare(deck: Option<PreparedDeck>) -> Result<Self, String> {
        Self::prepare_from_outcome(deck.into())
    }

    pub(crate) fn prepare_from_outcome(outcome: WallpaperPreparation) -> Result<Self, String> {
        let types = conduit_semantic_catalog::prepare_ambient_display_types()?;
        let ty = types
            .native_types
            .iter()
            .find(|ty| ty.name == "AmbientWallpaperObservation")
            .ok_or("missing AmbientWallpaperObservation")?;
        Self::prepare_with_type(outcome, &ty.value_type)
    }

    pub(crate) fn prepare_with_type(
        outcome: WallpaperPreparation,
        observation_type: &StructuredInfoType,
    ) -> Result<Self, String> {
        let StructuredInfoTypeShape::Variant { cases, .. } = observation_type.shape() else {
            return Err("expected wallpaper observation cases".into());
        };
        let unavailable = cases
            .iter()
            .find(|case| case.tag() == "unavailable")
            .ok_or("missing wallpaper absence case")?;
        let (deck, explanation) = match outcome {
            WallpaperPreparation::Ready(deck) => (Some(*deck), "Photograph selection unavailable"),
            WallpaperPreparation::NotPrepared => (None, "No photograph deck was prepared"),
            WallpaperPreparation::ProviderRateLimited => {
                (None, "The photograph provider rate-limited acquisition")
            }
            WallpaperPreparation::ProviderUnavailable => {
                (None, "The photograph provider did not supply a usable deck")
            }
        };
        let mut reason = Composer::new(unavailable.payload_type(), 256).map_err(debug_error)?;
        let reason = canonical(reason.leaf(explanation.as_bytes()).map_err(debug_error)?)
            .map_err(debug_error)?;
        let mut absence =
            Composer::new(observation_type, WALLPAPER_OBSERVATION_BYTES).map_err(debug_error)?;
        let unavailable = absence
            .variant("unavailable", reason)
            .map_err(debug_error)?
            .into();
        let selection = deck
            .as_ref()
            .map(PreparedWallpaperSelectionWire::prepare)
            .transpose()
            .map_err(debug_error)?;
        Ok(Self {
            deck,
            selection,
            unavailable,
        })
    }

    pub fn photos(&self) -> Option<&PreparedDeck> {
        self.deck.as_ref()
    }

    pub fn prepared_heap_bytes(&self) -> usize {
        self.deck.as_ref().map_or(0, PreparedDeck::retained_bytes)
            + self
                .selection
                .as_ref()
                .map_or(0, PreparedWallpaperSelectionWire::prepared_heap_bytes)
            + self.unavailable.len()
    }

    pub fn observe_at(&mut self, unix_millis: u64) -> &[u8] {
        self.selection
            .as_mut()
            .and_then(|selection| selection.encode(unix_millis).ok())
            .unwrap_or(&self.unavailable)
    }
}

// SAFETY: retained deck and canonical buffers are finite, read never acquires
// a photograph, and no borrow escapes the native worker.
unsafe impl NativeSourceWorkerProvider for PreparedWallpaperObservation {
    fn read(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        context: &NativeReadContext<'_>,
    ) -> Result<usize, WorkerFailure> {
        check(context)?;
        conduit_time::decode_tick(input).map_err(|_| WorkerFailure::Provider)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| WorkerFailure::Provider)?
            .as_millis()
            .try_into()
            .map_err(|_| WorkerFailure::Provider)?;
        let bytes = self.observe_at(timestamp);
        check(context)?;
        output
            .get_mut(..bytes.len())
            .ok_or(WorkerFailure::InvalidOutput)?
            .copy_from_slice(bytes);
        Ok(bytes.len())
    }
}

fn check(context: &NativeReadContext<'_>) -> Result<(), WorkerFailure> {
    if context.cancelled() {
        Err(WorkerFailure::Cancelled)
    } else if context.remaining().is_zero() {
        Err(WorkerFailure::TimedOut)
    } else {
        Ok(())
    }
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_rate_limit_reaches_the_typed_wallpaper_observation() {
        let mut wallpaper = PreparedWallpaperObservation::prepare_from_outcome(
            WallpaperPreparation::ProviderRateLimited,
        )
        .unwrap();
        let value = canonical(wallpaper.observe_at(300_000)).unwrap();
        assert_eq!(value.variant_tag().unwrap(), "unavailable");
        let reason = value.variant_payload("unavailable").unwrap().unwrap();
        assert_eq!(
            reason.primitive_bytes(conduit_core::TEXT_INFO_ID).unwrap(),
            b"The photograph provider rate-limited acquisition"
        );
    }

    #[test]
    fn an_unavailable_deck_keeps_its_typed_status_across_clock_samples() {
        let mut wallpaper = PreparedWallpaperObservation::prepare(None).unwrap();
        assert!(wallpaper.photos().is_none());
        for instant in [0, 300_000, 600_000] {
            let value = canonical(wallpaper.observe_at(instant)).unwrap();
            assert_eq!(value.variant_tag().unwrap(), "unavailable");
            let reason = value.variant_payload("unavailable").unwrap().unwrap();
            assert_eq!(
                reason.primitive_bytes(conduit_core::TEXT_INFO_ID).unwrap(),
                b"No photograph deck was prepared"
            );
        }
    }
}
