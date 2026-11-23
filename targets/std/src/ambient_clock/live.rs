//! A prepared clock worker with bounded, independently admitted weather IO.
use super::PreparedAmbientClockWire;
use crate::{
    ambient_weather::{
        OpenMeteoHttp, OpenMeteoHttpRefusal, PreparedCurrentWeatherWire, MAXIMUM_CURRENT_BODY_BYTES,
    },
    NativeReadContext, NativeSourceWorkerProvider, WorkerFailure,
};
use std::{
    net::SocketAddr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const WEATHER_INTERVAL: Duration = Duration::from_secs(300);
const CLOCK_RESERVE: Duration = Duration::from_millis(50);

pub struct PreparedLiveClock {
    clock: PreparedAmbientClockWire,
    weather: PreparedCurrentWeatherWire,
    transport: OpenMeteoHttp,
    response: Box<[u8]>,
    last_attempt: Option<Instant>,
}

impl PreparedLiveClock {
    /// The caller must admit the exact peer, worker deadline and all backing
    /// before starting the route. Preparation performs no network effects.
    pub fn prepare(timezone: &str, endpoint: SocketAddr) -> Result<Self, String> {
        Ok(Self {
            clock: PreparedAmbientClockWire::prepare_live_weather(timezone)
                .map_err(|error| format!("prepare civil clock: {error:?}"))?,
            weather: PreparedCurrentWeatherWire::prepare("Seattle", 900_000)
                .map_err(|error| format!("prepare weather cache: {error:?}"))?,
            transport: OpenMeteoHttp::prepare(endpoint)
                .map_err(|error| format!("prepare weather endpoint: {error:?}"))?,
            response: vec![0; MAXIMUM_CURRENT_BODY_BYTES].into_boxed_slice(),
            last_attempt: None,
        })
    }

    pub fn prepared_heap_bytes(&self) -> usize {
        self.clock.prepared_heap_bytes() + self.weather.prepared_heap_bytes() + self.response.len()
    }

    fn observe(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        deadline: Instant,
        cancelled: impl Fn() -> bool,
    ) -> Result<usize, WorkerFailure> {
        check(deadline, &cancelled)?;
        conduit_time::decode_tick(input).map_err(|_| WorkerFailure::Provider)?;
        let now = Instant::now();
        if self
            .last_attempt
            .is_none_or(|last| now.duration_since(last) >= WEATHER_INTERVAL)
        {
            if let Some(weather_deadline) =
                deadline.checked_sub(CLOCK_RESERVE).filter(|end| *end > now)
            {
                self.last_attempt = Some(now);
                match self.transport.read_into(
                    input,
                    &mut self.response,
                    weather_deadline,
                    &cancelled,
                ) {
                    Ok(length) => {
                        // A rejected response preserves the last checked reading.
                        let _ = self.weather.refresh(&self.response[..length], unix_ms()?);
                    }
                    Err(OpenMeteoHttpRefusal::Cancelled) => return Err(WorkerFailure::Cancelled),
                    Err(_) => {}
                }
            }
        }
        check(deadline, &cancelled)?;
        let bytes = self
            .clock
            .encode_with_current_weather(unix_ms()?, &mut self.weather)
            .map_err(|_| WorkerFailure::Provider)?;
        check(deadline, &cancelled)?;
        let destination = output
            .get_mut(..bytes.len())
            .ok_or(WorkerFailure::InvalidOutput)?;
        destination.copy_from_slice(bytes);
        Ok(bytes.len())
    }
}

fn unix_ms() -> Result<u64, WorkerFailure> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| WorkerFailure::Provider)?
        .as_millis()
        .try_into()
        .map_err(|_| WorkerFailure::Provider)
}

fn check(deadline: Instant, cancelled: &impl Fn() -> bool) -> Result<(), WorkerFailure> {
    if cancelled() {
        Err(WorkerFailure::Cancelled)
    } else if Instant::now() >= deadline {
        Err(WorkerFailure::TimedOut)
    } else {
        Ok(())
    }
}

// SAFETY: prepared backing is fixed and no borrowed buffers escape. The
// transport polls the supplied cancellation/deadline for the exact admitted
// peer. The clock and weather projections allocate only during preparation.
unsafe impl NativeSourceWorkerProvider for PreparedLiveClock {
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
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn weather_refusal_keeps_clock_current_without_growing_storage() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut worker =
            PreparedLiveClock::prepare("America/Los_Angeles", listener.local_addr().unwrap())
                .unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 1024);
            }
            stream
                .write_all(b"HTTP/1.0 503 Unavailable\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
        });
        let tick = conduit_time::encode_tick(0);
        let mut output = [0; 4096];
        let probe = crate::allocation_probe::begin();
        let length = worker
            .observe(
                &tick,
                &mut output,
                Instant::now() + Duration::from_secs(3),
                || false,
            )
            .unwrap();
        assert_eq!(probe.finish_bytes(), 0);
        server.join().unwrap();
        let value = conduit_core::validate_canonical_structured_value(&output[..length]).unwrap();
        assert_eq!(
            value
                .record_field("weather")
                .unwrap()
                .unwrap()
                .variant_tag()
                .unwrap(),
            "unavailable"
        );
        assert!(value.record_field("local_time").unwrap().is_some());
        output.fill(0xA5);
        assert_eq!(
            worker.observe(
                &tick,
                &mut output,
                Instant::now() + Duration::from_secs(3),
                || true
            ),
            Err(WorkerFailure::Cancelled)
        );
        assert!(output.iter().all(|byte| *byte == 0xA5));
    }
}
