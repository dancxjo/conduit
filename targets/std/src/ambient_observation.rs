//! Prepared ambient observation provider for an independently admitted IO worker.
//! Acquisition and actual grant/IMAGE construction belong to its caller.
use crate::{
    ambient_clock::PreparedAmbientClockWire,
    ambient_wallpaper::{
        acquisition::PreparedDeck, selection_wire::PreparedWallpaperSelectionWire,
    },
    ambient_weather::{OpenMeteoHttp, OpenMeteoHttpRefusal, PreparedCurrentWeatherWire},
    NativeReadContext, NativeSourceWorkerProvider, WorkerFailure,
};
use conduit_core::{
    validate_canonical_structured_value as canonical, PreparedStructuredComposer,
    StructuredInfoTypeShape,
};
use std::{
    net::SocketAddr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub mod initialization;

const WEATHER_INTERVAL: Duration = Duration::from_secs(300);
const MAXIMUM_UPDATES: u32 = 4096;

pub struct PreparedAmbientObservation {
    // Preserve actual pixels, not just their descriptive references, throughout
    // the provider lifetime. Target residences are admitted separately.
    deck: PreparedDeck,
    http: OpenMeteoHttp,
    weather: PreparedCurrentWeatherWire,
    clock: PreparedAmbientClockWire,
    wallpaper: PreparedWallpaperSelectionWire,
    home: PreparedStructuredComposer,
    clock_first: bool,
    response: Box<[u8]>,
    wallpaper_unavailable: Box<[u8]>,
    last_weather_attempt: Option<Instant>,
    maximum_updates: Option<u32>,
    updates: u32,
    last_transport_failure: Option<OpenMeteoHttpRefusal>,
}
impl PreparedAmbientObservation {
    /// Allocate only inside the caller's admitted preparation scope. `endpoint`
    /// must be independently authorized and resolved before worker execution.
    pub fn prepare(
        timezone: &str,
        endpoint: SocketAddr,
        deck: PreparedDeck,
        maximum_updates: u32,
    ) -> Result<Self, String> {
        if !(1..=MAXIMUM_UPDATES).contains(&maximum_updates) {
            return Err("ambient update bound is invalid".into());
        }
        Self::prepare_limit(timezone, endpoint, deck, Some(maximum_updates))
    }
    pub fn prepare_resident(
        timezone: &str,
        endpoint: SocketAddr,
        deck: PreparedDeck,
    ) -> Result<Self, String> {
        Self::prepare_limit(timezone, endpoint, deck, None)
    }
    fn prepare_limit(
        timezone: &str,
        endpoint: SocketAddr,
        deck: PreparedDeck,
        maximum_updates: Option<u32>,
    ) -> Result<Self, String> {
        let types = conduit_semantic_catalog::prepare_ambient_display_types()?;
        let ty = &types
            .native_types
            .iter()
            .find(|t| t.name == "AmbientHomeObservation")
            .ok_or("ambient Home Type missing")?
            .value_type;
        let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            return Err("ambient Home Type shape differs".into());
        };
        let clock_first = match fields {
            [a, b] if a.name() == "clock_weather" && b.name() == "wallpaper" => true,
            [a, b] if a.name() == "wallpaper" && b.name() == "clock_weather" => false,
            _ => return Err("ambient Home fields differ".into()),
        };
        let wallpaper_type = &types
            .native_types
            .iter()
            .find(|t| t.name == "AmbientWallpaperObservation")
            .ok_or("wallpaper Type missing")?
            .value_type;
        let StructuredInfoTypeShape::Variant { cases, .. } = wallpaper_type.shape() else {
            return Err("wallpaper observation Type differs".into());
        };
        let absent = cases
            .iter()
            .find(|c| c.tag() == "unavailable")
            .ok_or("wallpaper absence missing")?;
        let reason = conduit_plot::rust_binding::primitive_into_structured(
            absent.payload_type().clone(),
            &"Wallpaper selection time unavailable".to_owned(),
        )
        .map_err(|e| format!("wallpaper absence: {e:?}"))?;
        let wallpaper_unavailable = conduit_core::StructuredInfoValue::variant(
            wallpaper_type.clone(),
            "unavailable",
            reason,
        )
        .and_then(|v| v.canonical_bytes())
        .map_err(|e| format!("wallpaper absence: {e:?}"))?
        .into_boxed_slice();
        Ok(Self {
            http: OpenMeteoHttp::prepare(endpoint)
                .map_err(|e| format!("ambient endpoint: {e:?}"))?,
            weather: PreparedCurrentWeatherWire::prepare("Seattle", 900_000)
                .map_err(|e| format!("ambient weather: {e:?}"))?,
            clock: PreparedAmbientClockWire::prepare_live_weather(timezone)
                .map_err(|e| format!("ambient clock: {e:?}"))?,
            wallpaper: PreparedWallpaperSelectionWire::prepare(&deck)
                .map_err(|e| format!("ambient wallpaper: {e:?}"))?,
            home: PreparedStructuredComposer::new(ty, 8192)
                .map_err(|e| format!("ambient Home buffer: {e:?}"))?,
            deck,
            clock_first,
            response: vec![0; 4096].into_boxed_slice(),
            wallpaper_unavailable,
            last_weather_attempt: None,
            maximum_updates,
            updates: 0,
            last_transport_failure: None,
        })
    }
    /// Owned heap only. The caller also charges `size_of::<Self>()`, the worker's
    /// stack/control/input/output and its original definition/metadata custody.
    pub fn prepared_heap_bytes(&self) -> usize {
        self.deck.retained_bytes()
            + self.weather.prepared_heap_bytes()
            + self.clock.prepared_heap_bytes()
            + self.wallpaper.prepared_heap_bytes()
            + self.home.prepared_heap_bytes()
            + self.response.len()
            + self.wallpaper_unavailable.len()
    }
    pub fn photos(&self) -> &PreparedDeck {
        &self.deck
    }
    pub fn last_transport_failure(&self) -> Option<OpenMeteoHttpRefusal> {
        self.last_transport_failure
    }
    fn unix_ms() -> Result<u64, WorkerFailure> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| WorkerFailure::Provider)?
            .as_millis()
            .try_into()
            .map_err(|_| WorkerFailure::Provider)
    }
    fn project(&mut self, now: u64) -> Result<&[u8], WorkerFailure> {
        let clock = self
            .clock
            .encode_with_current_weather(now, &mut self.weather)
            .map_err(|_| WorkerFailure::Provider)?;
        let wallpaper = match self.wallpaper.encode(now) {
            Ok(bytes) => bytes,
            Err(_) => &self.wallpaper_unavailable,
        };
        let clock = canonical(clock).map_err(|_| WorkerFailure::Provider)?;
        let wallpaper = canonical(wallpaper).map_err(|_| WorkerFailure::Provider)?;
        let fields = if self.clock_first {
            [clock, wallpaper]
        } else {
            [wallpaper, clock]
        };
        self.home
            .record(&fields)
            .map_err(|_| WorkerFailure::InvalidOutput)
    }
    fn observe(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        deadline: Instant,
        cancelled: impl Fn() -> bool,
    ) -> Result<usize, WorkerFailure> {
        if cancelled() {
            return Err(WorkerFailure::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(WorkerFailure::TimedOut);
        }
        conduit_time::decode_tick(input).map_err(|_| WorkerFailure::Provider)?;
        if self
            .maximum_updates
            .is_some_and(|maximum| self.updates >= maximum)
            || self.updates == u32::MAX
        {
            return Err(WorkerFailure::Sequence);
        }
        let refresh = self
            .last_weather_attempt
            .is_none_or(|at| at.elapsed() >= WEATHER_INTERVAL);
        if refresh {
            self.last_weather_attempt = Some(Instant::now());
            match self
                .http
                .read_into(input, &mut self.response, deadline, &cancelled)
            {
                Ok(length) => {
                    self.last_transport_failure = None;
                    // A rejected response keeps the prior checked reading. Its
                    // freshness is reclassified below against the current clock.
                    let now = Self::unix_ms()?;
                    let _ = self.weather.refresh(&self.response[..length], now);
                }
                Err(OpenMeteoHttpRefusal::Cancelled) => return Err(WorkerFailure::Cancelled),
                Err(error) => self.last_transport_failure = Some(error),
            }
        }
        if cancelled() {
            return Err(WorkerFailure::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(WorkerFailure::TimedOut);
        }
        let encoded = self.project(Self::unix_ms()?)?;
        let length = encoded.len();
        if cancelled() {
            return Err(WorkerFailure::Cancelled);
        }
        let destination = output
            .get_mut(..length)
            .ok_or(WorkerFailure::InvalidOutput)?;
        destination.copy_from_slice(encoded);
        self.updates += 1;
        Ok(length)
    }
}
// SAFETY: all schema, canonical output, response and retained reading buffers
// are fixed during preparation. The only runtime IO is the fixed-peer HTTP
// component, with its bounded deadline/polling/cancellation contract. JSON and
// image selection do not allocate, and no caller borrows escape. Installation
// still requires the independently authorized matching original IMAGE/worker
// definition, complete backing and a cancellation budget >=250ms. This is not
// hostile-code or OS scheduling confinement.
unsafe impl NativeSourceWorkerProvider for PreparedAmbientObservation {
    fn read(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        context: &NativeReadContext<'_>,
    ) -> Result<usize, WorkerFailure> {
        self.observe(input, output, context.deadline(), || context.cancelled())
    }
}

#[cfg(test)]
#[path = "ambient_observation/tests.rs"]
mod tests;
